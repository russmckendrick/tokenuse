//! Rule framework: curated Rust ports of the anti-pattern rules from
//! Microsoft's AI-Engineering-Coach (MIT, Copyright (c) Microsoft
//! Corporation). Each rule keeps the reference id, group, severity, and
//! thresholds; detection predicates are hand-ported over `Turn`/session rows.
//! All user-facing wording lives in copy.json under `coach.rules.<id>` - the
//! engine emits ids and numbers only.

pub mod code_review;
pub mod prompt_quality;
pub mod session_hygiene;
pub mod tool_mastery;

use super::CoachContext;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RuleGroup {
    PromptQuality,
    SessionHygiene,
    CodeReview,
    ToolMastery,
}

impl RuleGroup {
    pub const ALL: [RuleGroup; 4] = [
        RuleGroup::PromptQuality,
        RuleGroup::SessionHygiene,
        RuleGroup::CodeReview,
        RuleGroup::ToolMastery,
    ];

    pub fn id(self) -> &'static str {
        match self {
            RuleGroup::PromptQuality => "prompt_quality",
            RuleGroup::SessionHygiene => "session_hygiene",
            RuleGroup::CodeReview => "code_review",
            RuleGroup::ToolMastery => "tool_mastery",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    High,
    Medium,
    Low,
}

impl Severity {
    /// Score penalty per triggered rule; reference values 12/7/3.
    pub fn penalty(self) -> u32 {
        match self {
            Severity::High => 12,
            Severity::Medium => 7,
            Severity::Low => 3,
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Severity::High => "high",
            Severity::Medium => "medium",
            Severity::Low => "low",
        }
    }
}

/// The maximum per-rule penalty; a group's worst case is `rules × MAX`.
pub const MAX_PENALTY: u32 = 12;

pub struct RuleDef {
    pub id: &'static str,
    pub group: RuleGroup,
    pub severity: Severity,
    pub detect: fn(&CoachContext) -> Option<RuleHit>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct RuleHit {
    pub occurrences: u64,
    pub total: u64,
    /// Percentage for `{pct}` copy slots, already rounded.
    pub pct: Option<u64>,
    /// One rule-specific pre-formatted value for the `{stat}` copy slot
    /// (top model name, average seconds, ...).
    pub stat: Option<String>,
    /// Escalate to high severity (reference `severity:` expressions).
    pub escalate: bool,
    pub examples: Vec<RuleExample>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuleExample {
    /// Short snippet, capped at 120 chars.
    pub text: String,
    pub detail: String,
}

pub const MAX_EXAMPLES: usize = 3;

pub fn example(text: &str, detail: String) -> RuleExample {
    RuleExample {
        text: crate::tools::jsonl::truncate_chars(text.trim(), 120),
        detail,
    }
}

pub struct RuleOutcome {
    pub id: &'static str,
    pub group: RuleGroup,
    pub severity: Severity,
    pub hit: RuleHit,
}

impl RuleOutcome {
    pub fn effective_severity(&self) -> Severity {
        if self.hit.escalate {
            Severity::High
        } else {
            self.severity
        }
    }
}

pub fn all_rules() -> Vec<RuleDef> {
    let mut rules = Vec::new();
    rules.extend(prompt_quality::rules());
    rules.extend(session_hygiene::rules());
    rules.extend(code_review::rules());
    rules.extend(tool_mastery::rules());
    rules
}

/// Run every rule; only triggered rules produce an outcome.
pub fn run_rules(ctx: &CoachContext) -> Vec<RuleOutcome> {
    all_rules()
        .into_iter()
        .filter_map(|rule| {
            (rule.detect)(ctx).map(|hit| RuleOutcome {
                id: rule.id,
                group: rule.group,
                severity: rule.severity,
                hit,
            })
        })
        .collect()
}

pub fn rules_per_group(group: RuleGroup) -> u64 {
    all_rules().iter().filter(|r| r.group == group).count() as u64
}

pub(crate) fn ratio_pct(count: u64, total: u64) -> Option<u64> {
    if total == 0 {
        None
    } else {
        Some(((count as f64 / total as f64) * 100.0).round() as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_rule_has_copy_and_every_rule_copy_has_a_rule() {
        let deck = crate::copy::copy();
        let rule_ids: Vec<&str> = all_rules().iter().map(|r| r.id).collect();
        for id in &rule_ids {
            assert!(
                deck.coach.rules.contains_key(*id),
                "copy.json is missing coach.rules.{id}"
            );
        }
        for key in deck.coach.rules.keys() {
            assert!(
                rule_ids.contains(&key.as_str()),
                "coach.rules.{key} in copy.json has no matching rule"
            );
        }
    }

    #[test]
    fn registry_has_the_curated_rule_set() {
        let rules = all_rules();
        assert_eq!(rules.len(), 28);
        let mut ids: Vec<&str> = rules.iter().map(|r| r.id).collect();
        ids.sort_unstable();
        let mut deduped = ids.clone();
        deduped.dedup();
        assert_eq!(ids, deduped, "rule ids must be unique");
        assert_eq!(rules_per_group(RuleGroup::PromptQuality), 8);
        assert_eq!(rules_per_group(RuleGroup::SessionHygiene), 10);
        assert_eq!(rules_per_group(RuleGroup::CodeReview), 5);
        assert_eq!(rules_per_group(RuleGroup::ToolMastery), 5);
    }
}
