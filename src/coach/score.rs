//! Practice-group scoring.
//!
//! Ported from Microsoft's AI-Engineering-Coach `analyzer-patterns.ts` (MIT,
//! Copyright (c) Microsoft Corporation): a group's penalty is the sum of the
//! severity penalties of its *triggered* rules (occurrence counts do not
//! affect the score); `score = max(0, round(100·(1 − penalty/max)))` with
//! `max = rules-in-group × 12`. Trends compare ISO-week scores: WoW is the
//! last week vs the one before; MoM is the mean of the last 4 weeks vs the
//! mean of weeks 5-8 back.

use chrono::{Datelike, Local};

use super::rules::{self, RuleGroup, RuleOutcome};
use super::CoachContext;
use crate::tools::ParsedCall;

#[derive(Debug, Clone)]
pub struct GroupScore {
    pub group: RuleGroup,
    pub score: u64,
    pub wow_pct: Option<i64>,
    pub mom_pct: Option<i64>,
    pub triggered: u64,
    pub total_rules: u64,
    /// Highest-penalty triggered rule, for the "top issue" line.
    pub top_rule: Option<&'static str>,
}

pub fn score_for(outcomes: &[&RuleOutcome], group: RuleGroup) -> u64 {
    let total_rules = rules::rules_per_group(group);
    let max_penalty = total_rules as u32 * rules::MAX_PENALTY;
    if max_penalty == 0 {
        return 100;
    }
    let penalty: u32 = outcomes
        .iter()
        .filter(|o| o.group == group)
        .map(|o| o.effective_severity().penalty())
        .sum();
    let ratio = f64::from(penalty) / f64::from(max_penalty);
    ((100.0 * (1.0 - ratio)).round()).clamp(0.0, 100.0) as u64
}

/// Weekly per-group scores: bucket calls by local ISO week, run the full
/// rule set per bucket, score each group. Buckets are ordered oldest-first.
pub fn weekly_group_scores(calls: &[&ParsedCall]) -> Vec<[u64; 4]> {
    let mut buckets: std::collections::BTreeMap<i64, Vec<&ParsedCall>> =
        std::collections::BTreeMap::new();
    for call in calls {
        let Some(ts) = call.timestamp else { continue };
        let iso = ts.with_timezone(&Local).iso_week();
        let key = i64::from(iso.year()) * 53 + i64::from(iso.week());
        buckets.entry(key).or_default().push(call);
    }

    buckets
        .into_values()
        .map(|bucket| {
            let ctx = CoachContext::new(&bucket);
            let outcomes = rules::run_rules(&ctx);
            let refs: Vec<&RuleOutcome> = outcomes.iter().collect();
            [
                score_for(&refs, RuleGroup::PromptQuality),
                score_for(&refs, RuleGroup::SessionHygiene),
                score_for(&refs, RuleGroup::CodeReview),
                score_for(&refs, RuleGroup::ToolMastery),
            ]
        })
        .collect()
}

fn pct_change(current: f64, previous: f64) -> Option<i64> {
    if previous <= 0.0 {
        return None;
    }
    Some((((current - previous) / previous) * 100.0).round() as i64)
}

fn group_index(group: RuleGroup) -> usize {
    match group {
        RuleGroup::PromptQuality => 0,
        RuleGroup::SessionHygiene => 1,
        RuleGroup::CodeReview => 2,
        RuleGroup::ToolMastery => 3,
    }
}

pub fn group_scores(outcomes: &[RuleOutcome], weekly: &[[u64; 4]]) -> Vec<GroupScore> {
    RuleGroup::ALL
        .into_iter()
        .map(|group| {
            let refs: Vec<&RuleOutcome> = outcomes.iter().collect();
            let score = score_for(&refs, group);
            let triggered: Vec<&RuleOutcome> =
                outcomes.iter().filter(|o| o.group == group).collect();
            let top_rule = triggered
                .iter()
                .max_by_key(|o| o.effective_severity().penalty())
                .map(|o| o.id);

            let idx = group_index(group);
            let series: Vec<u64> = weekly.iter().map(|w| w[idx]).collect();
            let wow_pct = if series.len() >= 2 {
                pct_change(
                    series[series.len() - 1] as f64,
                    series[series.len() - 2] as f64,
                )
            } else {
                None
            };
            let mom_pct = if series.len() >= 8 {
                let recent: f64 = series[series.len() - 4..].iter().sum::<u64>() as f64 / 4.0;
                let prior: f64 = series[series.len() - 8..series.len() - 4]
                    .iter()
                    .sum::<u64>() as f64
                    / 4.0;
                pct_change(recent, prior)
            } else {
                None
            };

            GroupScore {
                group,
                score,
                wow_pct,
                mom_pct,
                triggered: triggered.len() as u64,
                total_rules: rules::rules_per_group(group),
                top_rule,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coach::rules::{RuleHit, Severity};

    fn outcome(id: &'static str, group: RuleGroup, severity: Severity) -> RuleOutcome {
        RuleOutcome {
            id,
            group,
            severity,
            hit: RuleHit::default(),
        }
    }

    #[test]
    fn score_arithmetic_matches_reference() {
        // Prompt-quality has 8 rules -> max penalty 96.
        let outcomes = [
            outcome("a", RuleGroup::PromptQuality, Severity::High), // 12
            outcome("b", RuleGroup::PromptQuality, Severity::Medium), // 7
            outcome("c", RuleGroup::PromptQuality, Severity::Low),  // 3
        ];
        let refs: Vec<&RuleOutcome> = outcomes.iter().collect();
        // 100 * (1 - 22/96) = 77.08 -> 77
        assert_eq!(score_for(&refs, RuleGroup::PromptQuality), 77);
        assert_eq!(score_for(&refs, RuleGroup::CodeReview), 100);
    }

    #[test]
    fn escalated_outcomes_penalize_as_high() {
        let mut escalated = outcome("a", RuleGroup::CodeReview, Severity::Low);
        escalated.hit.escalate = true;
        let refs = [&escalated];
        // Code-review has 5 rules -> max 60; penalty 12 -> 100*(1-0.2)=80.
        assert_eq!(score_for(&refs, RuleGroup::CodeReview), 80);
    }

    #[test]
    fn trends_need_enough_weeks() {
        let outcomes: Vec<RuleOutcome> = Vec::new();
        let weekly = vec![[80, 100, 100, 100], [90, 100, 100, 100]];
        let scores = group_scores(&outcomes, &weekly);
        let pq = &scores[0];
        assert_eq!(pq.wow_pct, Some(13), "90 vs 80 rounds to +13%");
        assert_eq!(pq.mom_pct, None, "needs 8 weekly buckets");
        assert_eq!(pq.score, 100);
        assert_eq!(pq.triggered, 0);
    }
}
