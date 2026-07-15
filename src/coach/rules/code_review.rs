//! Code-review rules.
//!
//! Ported from Microsoft's AI-Engineering-Coach rule files (MIT, Copyright
//! (c) Microsoft Corporation): copy-paste-blindness, speed-accept,
//! vibe-coding, tunnel-vision, no-language-exploration.

use chrono::{Datelike, Local};

use super::{example, ratio_pct, RuleDef, RuleGroup, RuleHit, Severity, MAX_EXAMPLES};
use crate::coach::sessions::think_gap_ms;
use crate::coach::{text, CoachContext};

pub fn rules() -> Vec<RuleDef> {
    vec![
        RuleDef {
            id: "copy-paste-blindness",
            group: RuleGroup::CodeReview,
            severity: Severity::High,
            detect: copy_paste_blindness,
        },
        RuleDef {
            id: "speed-accept",
            group: RuleGroup::CodeReview,
            severity: Severity::High,
            detect: speed_accept,
        },
        RuleDef {
            id: "vibe-coding",
            group: RuleGroup::CodeReview,
            severity: Severity::High,
            detect: vibe_coding,
        },
        RuleDef {
            id: "tunnel-vision",
            group: RuleGroup::CodeReview,
            severity: Severity::Low,
            detect: tunnel_vision,
        },
        RuleDef {
            id: "no-language-exploration",
            group: RuleGroup::CodeReview,
            severity: Severity::Low,
            detect: no_language_exploration,
        },
    ]
}

/// Sessions producing 50+ AI LoC with no follow-up refinement (no
/// refinement wording and no edits in later turns), three or more times.
fn copy_paste_blindness(ctx: &CoachContext) -> Option<RuleHit> {
    const MIN_AI_LOC: u64 = 50;
    const MIN_SESSIONS: u64 = 3;

    let mut count = 0u64;
    let mut total = 0u64;
    let mut examples = Vec::new();
    for session in ctx.conversational_sessions() {
        total += 1;
        if session.turns.len() < 2 || session.ai_loc() < MIN_AI_LOC {
            continue;
        }
        let refined = session.turns[1..].iter().any(|turn| {
            turn.edited_files > 0
                || (!turn.user_message.is_empty()
                    && text::has_refinement_language(turn.user_message))
        });
        if !refined {
            count += 1;
            if examples.len() < MAX_EXAMPLES {
                examples.push(example(
                    &session.project,
                    format!("{} AI LoC, no refinement", session.ai_loc()),
                ));
            }
        }
    }
    (count >= MIN_SESSIONS).then_some(RuleHit {
        occurrences: count,
        total,
        pct: None,
        stat: None,
        escalate: false,
        examples,
    })
}

/// Next message sent within 15s of receiving 20+ lines of AI code, five or
/// more times.
fn speed_accept(ctx: &CoachContext) -> Option<RuleHit> {
    const MIN_AI_LOC: u64 = 20;
    const MAX_GAP_MS: i64 = 15_000;
    const MIN_OCCURRENCES: u64 = 5;

    let mut count = 0u64;
    let mut gap_sum_ms = 0i64;
    let mut loc_sum = 0u64;
    let mut examples = Vec::new();
    for session in ctx.conversational_sessions() {
        for pair in session.turns.windows(2) {
            let (cur, next) = (&pair[0], &pair[1]);
            if cur.ai_loc < MIN_AI_LOC {
                continue;
            }
            let Some(gap) = think_gap_ms(cur, next) else {
                continue;
            };
            if (0..=MAX_GAP_MS).contains(&gap) {
                count += 1;
                gap_sum_ms += gap;
                loc_sum += cur.ai_loc;
                if examples.len() < MAX_EXAMPLES {
                    examples.push(example(
                        &session.project,
                        format!("{} LoC accepted in {}s", cur.ai_loc, gap / 1000),
                    ));
                }
            }
        }
    }
    (count >= MIN_OCCURRENCES).then(|| RuleHit {
        occurrences: count,
        total: count,
        pct: None,
        stat: Some(format!(
            "{} LoC / {}s",
            loc_sum / count.max(1),
            gap_sum_ms / count.max(1) as i64 / 1000
        )),
        escalate: false,
        examples,
    })
}

/// 100+ AI LoC from five or fewer unstructured prompts, three or more
/// sessions.
fn vibe_coding(ctx: &CoachContext) -> Option<RuleHit> {
    const MIN_AI_LOC: u64 = 100;
    const MAX_USER_PROMPTS: usize = 5;
    const MIN_SESSIONS: u64 = 3;

    let mut count = 0u64;
    let mut total = 0u64;
    let mut vibe_loc = 0u64;
    let mut examples = Vec::new();
    for session in ctx.conversational_sessions() {
        total += 1;
        if session.ai_loc() < MIN_AI_LOC || session.turns.len() > MAX_USER_PROMPTS {
            continue;
        }
        let first = session
            .turns
            .iter()
            .find(|t| !t.user_message.is_empty())
            .map(|t| t.user_message)
            .unwrap_or("");
        if first.is_empty() || text::looks_like_spec(first) {
            continue;
        }
        count += 1;
        vibe_loc += session.ai_loc();
        if examples.len() < MAX_EXAMPLES {
            examples.push(example(
                &session.project,
                format!(
                    "{} AI LoC in {} messages",
                    session.ai_loc(),
                    session.turns.len()
                ),
            ));
        }
    }
    (count >= MIN_SESSIONS).then(|| RuleHit {
        occurrences: count,
        total,
        pct: None,
        stat: Some(vibe_loc.to_string()),
        escalate: false,
        examples,
    })
}

/// More than 95% of turns in one project, with 3+ projects known and 50+ turns.
fn tunnel_vision(ctx: &CoachContext) -> Option<RuleHit> {
    const MAX_TOP_RATE: f64 = 0.95;
    const MIN_TURNS: u64 = 50;
    const MIN_PROJECTS: usize = 3;

    let mut by_project: std::collections::HashMap<&str, u64> = std::collections::HashMap::new();
    let mut total = 0u64;
    for session in &ctx.sessions {
        let turns = session.turns.len() as u64;
        total += turns;
        *by_project.entry(session.project.as_str()).or_insert(0) += turns;
    }
    let (top_project, top_turns) = by_project
        .iter()
        .max_by_key(|(_, count)| **count)
        .map(|(p, c)| (p.to_string(), *c))?;

    (by_project.len() >= MIN_PROJECTS
        && top_turns >= MIN_TURNS
        && total > 0
        && top_turns as f64 / total as f64 > MAX_TOP_RATE)
        .then(|| RuleHit {
            occurrences: top_turns,
            total,
            pct: ratio_pct(top_turns, total),
            stat: Some(crate::ingest::projects::raw_project_display(&top_project)),
            escalate: false,
            examples: Vec::new(),
        })
}

/// No new programming language first seen in the last 8 ISO weeks of the
/// range (min 4 active weeks); escalates past 12 weeks without novelty.
fn no_language_exploration(ctx: &CoachContext) -> Option<RuleHit> {
    const MIN_WEEKS: u64 = 4;
    const RECENT_WEEKS: i64 = 8;
    const HIGH_WEEKS: i64 = 12;

    // ISO week index -> first-seen week per language, over turn code blocks.
    let mut weeks: std::collections::BTreeSet<i64> = std::collections::BTreeSet::new();
    let mut first_seen: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    for turn in ctx.timestamped_turns() {
        let local = turn.timestamp.expect("timestamped").with_timezone(&Local);
        let iso = local.iso_week();
        let week_index = i64::from(iso.year()) * 53 + i64::from(iso.week());
        if turn.code_blocks.is_empty() {
            continue;
        }
        weeks.insert(week_index);
        for block in &turn.code_blocks {
            if block.language == "unknown" {
                continue;
            }
            first_seen
                .entry(block.language.clone())
                .and_modify(|w| *w = (*w).min(week_index))
                .or_insert(week_index);
        }
    }
    let total_weeks = weeks.len() as u64;
    if total_weeks < MIN_WEEKS || first_seen.is_empty() {
        return None;
    }
    let last_week = *weeks.iter().next_back().expect("non-empty");
    let newest_first_seen = *first_seen.values().max().expect("non-empty");
    let weeks_since_new = last_week - newest_first_seen;

    (weeks_since_new >= RECENT_WEEKS).then(|| RuleHit {
        occurrences: weeks_since_new.max(0) as u64,
        total: total_weeks,
        pct: None,
        stat: Some(first_seen.len().to_string()),
        escalate: weeks_since_new > HIGH_WEEKS,
        examples: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::super::run_rules;
    use crate::coach::testutil::{call, ctx_calls, with_code};
    use crate::coach::CoachContext;

    fn triggered_ids(calls: &[crate::tools::ParsedCall]) -> Vec<&'static str> {
        let refs = ctx_calls(calls);
        let ctx = CoachContext::new(&refs);
        run_rules(&ctx).into_iter().map(|o| o.id).collect()
    }

    #[test]
    fn vibe_coding_triggers_on_unstructured_high_loc_sessions() {
        let mut calls = Vec::new();
        for s in 0..3 {
            calls.push(with_code(
                call(&format!("vibe{s}"), s * 10, "make me a dashboard app"),
                "rust",
                150,
            ));
            calls.push(call(&format!("vibe{s}"), s * 10 + 1, "looks good thanks"));
        }
        assert!(triggered_ids(&calls).contains(&"vibe-coding"));
    }

    #[test]
    fn vibe_coding_stays_clean_with_specs_or_iteration() {
        let mut calls = Vec::new();
        for s in 0..3 {
            calls.push(with_code(
                call(
                    &format!("spec{s}"),
                    s * 10,
                    "Requirements:\n- must parse JSONL\n- must handle errors\n- must have tests",
                ),
                "rust",
                150,
            ));
            calls.push(call(&format!("spec{s}"), s * 10 + 1, "looks good thanks"));
        }
        assert!(!triggered_ids(&calls).contains(&"vibe-coding"));
    }

    #[test]
    fn speed_accept_counts_fast_followups_after_big_output() {
        let mut calls = Vec::new();
        for s in 0..5 {
            let mut first = with_code(
                call(&format!("fast{s}"), s * 10, "write the whole feature now"),
                "rust",
                80,
            );
            first.elapsed_ms = Some(30_000);
            calls.push(first);
            // Next turn lands 35s later with 30s of latency: 5s think time.
            let mut second = call(&format!("fast{s}"), s * 10, "and the next one right away");
            second.timestamp = first_ts_plus(&calls, 35);
            second.elapsed_ms = Some(30_000);
            calls.push(second);
        }
        assert!(triggered_ids(&calls).contains(&"speed-accept"));
    }

    fn first_ts_plus(
        calls: &[crate::tools::ParsedCall],
        secs: i64,
    ) -> Option<chrono::DateTime<chrono::Utc>> {
        calls
            .last()
            .and_then(|c| c.timestamp)
            .map(|t| t + chrono::Duration::seconds(secs))
    }

    #[test]
    fn copy_paste_blindness_spares_sessions_with_refinement() {
        let mut blind = Vec::new();
        for s in 0..3 {
            blind.push(with_code(
                call(&format!("blind{s}"), s * 10, "generate the parser module"),
                "rust",
                80,
            ));
            blind.push(call(
                &format!("blind{s}"),
                s * 10 + 1,
                "great, thanks a lot",
            ));
        }
        assert!(triggered_ids(&blind).contains(&"copy-paste-blindness"));

        let mut refined = Vec::new();
        for s in 0..3 {
            refined.push(with_code(
                call(&format!("ref{s}"), s * 10, "generate the parser module"),
                "rust",
                80,
            ));
            refined.push(call(
                &format!("ref{s}"),
                s * 10 + 1,
                "fix the error handling in that",
            ));
        }
        assert!(!triggered_ids(&refined).contains(&"copy-paste-blindness"));
    }
}
