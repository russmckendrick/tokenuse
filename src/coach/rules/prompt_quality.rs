//! Prompt-quality rules.
//!
//! Ported from Microsoft's AI-Engineering-Coach rule files (MIT, Copyright
//! (c) Microsoft Corporation): lazy-prompting, caps-lock, frustration-signals,
//! repeated-prompts, low-constraint-usage, verbose-output,
//! verbose-prompt-no-compression, excessive-file-context. Threshold values
//! are the reference defaults; deviations are noted per rule and in
//! docs/development/coach.md.

use std::collections::HashMap;

use super::{example, ratio_pct, RuleDef, RuleGroup, RuleHit, Severity, MAX_EXAMPLES};
use crate::coach::text;
use crate::coach::CoachContext;

pub fn rules() -> Vec<RuleDef> {
    vec![
        RuleDef {
            id: "lazy-prompting",
            group: RuleGroup::PromptQuality,
            severity: Severity::Medium,
            detect: lazy_prompting,
        },
        RuleDef {
            id: "caps-lock",
            group: RuleGroup::PromptQuality,
            severity: Severity::Medium,
            detect: caps_lock,
        },
        RuleDef {
            id: "frustration-signals",
            group: RuleGroup::PromptQuality,
            severity: Severity::Medium,
            detect: frustration_signals,
        },
        RuleDef {
            id: "repeated-prompts",
            group: RuleGroup::PromptQuality,
            severity: Severity::Medium,
            detect: repeated_prompts,
        },
        RuleDef {
            id: "low-constraint-usage",
            group: RuleGroup::PromptQuality,
            severity: Severity::Medium,
            detect: low_constraint_usage,
        },
        RuleDef {
            id: "verbose-output",
            group: RuleGroup::PromptQuality,
            severity: Severity::Medium,
            detect: verbose_output,
        },
        RuleDef {
            id: "verbose-prompt-no-compression",
            group: RuleGroup::PromptQuality,
            severity: Severity::Low,
            detect: verbose_prompt_no_compression,
        },
        RuleDef {
            id: "excessive-file-context",
            group: RuleGroup::PromptQuality,
            severity: Severity::Medium,
            detect: excessive_file_context,
        },
    ]
}

/// More than 30% of prompts under 30 chars (min 10 samples).
fn lazy_prompting(ctx: &CoachContext) -> Option<RuleHit> {
    const MIN_CHARS: u64 = 30;
    const MAX_RATIO: f64 = 0.3;
    const MIN_SAMPLE: u64 = 10;

    let mut count = 0u64;
    let mut total = 0u64;
    let mut examples = Vec::new();
    for turn in ctx.prompt_turns() {
        total += 1;
        let chars = turn.prompt_chars.unwrap_or(0);
        if chars < MIN_CHARS {
            count += 1;
            if examples.len() < MAX_EXAMPLES {
                examples.push(example(turn.user_message, format!("{chars} chars")));
            }
        }
    }
    (count > MIN_SAMPLE && count as f64 / total as f64 > MAX_RATIO).then(|| RuleHit {
        occurrences: count,
        total,
        pct: ratio_pct(count, total),
        stat: None,
        escalate: false,
        examples,
    })
}

/// Messages written >=90% in caps (min length 10).
fn caps_lock(ctx: &CoachContext) -> Option<RuleHit> {
    const MIN_LENGTH: u64 = 10;
    const CAPS_RATE: f64 = 0.9;
    const MIN_REQS: u64 = 1;

    let mut count = 0u64;
    let mut total = 0u64;
    let mut examples = Vec::new();
    for turn in ctx.prompt_turns() {
        total += 1;
        if turn.prompt_chars.unwrap_or(0) >= MIN_LENGTH
            && text::caps_letter_ratio(turn.user_message) >= CAPS_RATE
        {
            count += 1;
            if examples.len() < MAX_EXAMPLES {
                examples.push(example(turn.user_message, String::new()));
            }
        }
    }
    (count >= MIN_REQS).then_some(RuleHit {
        occurrences: count,
        total,
        pct: None,
        stat: None,
        escalate: false,
        examples,
    })
}

/// Excessive punctuation or ALL-CAPS word runs (min 2 occurrences).
fn frustration_signals(ctx: &CoachContext) -> Option<RuleHit> {
    const CAPS_RATE: f64 = 0.4;
    const MIN_WORDS: usize = 3;
    const MIN_REQS: u64 = 2;

    let mut count = 0u64;
    let mut total = 0u64;
    let mut examples = Vec::new();
    for turn in ctx.prompt_turns() {
        total += 1;
        if turn.prompt_chars.unwrap_or(0) >= 10
            && (text::has_frustration_signal(turn.user_message)
                || text::caps_word_ratio(turn.user_message, MIN_WORDS) >= CAPS_RATE)
        {
            count += 1;
            if examples.len() < MAX_EXAMPLES {
                examples.push(example(turn.user_message, String::new()));
            }
        }
    }
    (count >= MIN_REQS).then_some(RuleHit {
        occurrences: count,
        total,
        pct: None,
        stat: None,
        escalate: false,
        examples,
    })
}

/// Near-duplicate prompts: keyed on the first 100 chars, lowercased. Groups
/// of >=3 count; escalates past 20 duplicates.
fn repeated_prompts(ctx: &CoachContext) -> Option<RuleHit> {
    const MIN_KEY_LEN: usize = 10;
    const MIN_DUPLICATES: u64 = 3;
    const HIGH_THRESHOLD: u64 = 20;

    let mut groups: HashMap<String, (u64, String)> = HashMap::new();
    let mut total = 0u64;
    for turn in ctx.prompt_turns() {
        total += 1;
        let key: String = turn
            .user_message
            .chars()
            .take(100)
            .collect::<String>()
            .to_lowercase()
            .trim()
            .to_string();
        if key.chars().count() >= MIN_KEY_LEN {
            let entry = groups
                .entry(key)
                .or_insert_with(|| (0, turn.user_message.to_string()));
            entry.0 += 1;
        }
    }
    let mut dupes: Vec<(u64, String)> = groups
        .into_values()
        .filter(|(count, _)| *count >= MIN_DUPLICATES)
        .collect();
    dupes.sort_by_key(|d| std::cmp::Reverse(d.0));
    let total_dupes: u64 = dupes.iter().map(|(c, _)| c).sum();

    (total_dupes >= MIN_DUPLICATES).then(|| RuleHit {
        occurrences: total_dupes,
        total,
        pct: ratio_pct(total_dupes, total),
        stat: Some(dupes.len().to_string()),
        escalate: total_dupes > HIGH_THRESHOLD,
        examples: dupes
            .iter()
            .take(MAX_EXAMPLES)
            .map(|(count, msg)| example(msg, format!("repeated {count}x")))
            .collect(),
    })
}

/// <8% of substantial prompts contain constraint keywords (min 30 samples).
fn low_constraint_usage(ctx: &CoachContext) -> Option<RuleHit> {
    const MIN_REQS: u64 = 30;
    const MIN_MESSAGE_LENGTH: u64 = 40;
    const CONSTRAINT_RATE: f64 = 0.08;

    let mut without = 0u64;
    let mut substantial = 0u64;
    let mut examples = Vec::new();
    for turn in ctx.prompt_turns() {
        if turn.prompt_chars.unwrap_or(0) < MIN_MESSAGE_LENGTH {
            continue;
        }
        substantial += 1;
        if !text::has_constraint(turn.user_message) {
            without += 1;
            if examples.len() < MAX_EXAMPLES {
                examples.push(example(turn.user_message, "no constraints".into()));
            }
        }
    }
    (substantial >= MIN_REQS && without as f64 / substantial as f64 > 1.0 - CONSTRAINT_RATE).then(
        || RuleHit {
            occurrences: without,
            total: substantial,
            pct: ratio_pct(substantial - without, substantial),
            stat: Some(format!("{}", substantial - without)),
            escalate: false,
            examples,
        },
    )
}

/// More than 5K output tokens from prompts under 200 chars (>10% of prompts, min 10).
fn verbose_output(ctx: &CoachContext) -> Option<RuleHit> {
    const MIN_OUTPUT_TOKENS: u64 = 5_000;
    const MAX_MESSAGE_LENGTH: u64 = 200;
    const MIN_SAMPLE: u64 = 10;
    const MAX_RATIO: f64 = 0.1;

    let mut count = 0u64;
    let mut total = 0u64;
    let mut examples = Vec::new();
    for turn in ctx.prompt_turns() {
        total += 1;
        let chars = turn.prompt_chars.unwrap_or(0);
        if turn.output_tokens > MIN_OUTPUT_TOKENS && chars > 0 && chars < MAX_MESSAGE_LENGTH {
            count += 1;
            if examples.len() < MAX_EXAMPLES {
                examples.push(example(
                    turn.user_message,
                    format!("{} output tokens", turn.output_tokens),
                ));
            }
        }
    }
    (count > MIN_SAMPLE && count as f64 / total as f64 > MAX_RATIO).then(|| RuleHit {
        occurrences: count,
        total,
        pct: ratio_pct(count, total),
        stat: None,
        escalate: false,
        examples,
    })
}

/// Long prompts stuffed with filler words (>20% of prompts, min 15).
/// Deviation from the reference: the "compression skill installed" exemption
/// is dropped (tokens has no installed-skills signal), and filler matching
/// runs on the stored 500-char prompt prefix.
fn verbose_prompt_no_compression(ctx: &CoachContext) -> Option<RuleHit> {
    const MIN_MESSAGE_LENGTH: u64 = 800;
    const MIN_SAMPLE: u64 = 15;
    const MAX_RATIO: f64 = 0.2;

    let mut count = 0u64;
    let mut total = 0u64;
    let mut examples = Vec::new();
    for turn in ctx.prompt_turns() {
        total += 1;
        if turn.prompt_chars.unwrap_or(0) >= MIN_MESSAGE_LENGTH
            && text::count_phrase_hits(turn.user_message, text::FILLER_WORDS) >= 2
        {
            count += 1;
            if examples.len() < MAX_EXAMPLES {
                examples.push(example(
                    turn.user_message,
                    format!("{} chars", turn.prompt_chars.unwrap_or(0)),
                ));
            }
        }
    }
    (count > MIN_SAMPLE && count as f64 / total as f64 > MAX_RATIO).then(|| RuleHit {
        occurrences: count,
        total,
        pct: ratio_pct(count, total),
        stat: None,
        escalate: false,
        examples,
    })
}

/// Outlier turns referencing >=30 files (min 10 outliers). Only tools that
/// populate `referenced_files` enter the denominator.
fn excessive_file_context(ctx: &CoachContext) -> Option<RuleHit> {
    const MIN_FILES: u64 = 30;
    const MIN_OUTLIERS: u64 = 10;
    const MAX_RATIO: f64 = 0.005;

    let mut count = 0u64;
    let mut total = 0u64;
    let mut max_files = 0u64;
    let mut examples = Vec::new();
    for turn in ctx
        .turns()
        .filter(|t| crate::coach::signals::supports_file_refs(t.tool))
    {
        total += 1;
        max_files = max_files.max(turn.referenced_files);
        if turn.referenced_files >= MIN_FILES {
            count += 1;
            if examples.len() < MAX_EXAMPLES {
                examples.push(example(
                    turn.user_message,
                    format!("{} files", turn.referenced_files),
                ));
            }
        }
    }
    (count >= MIN_OUTLIERS && total > 0 && count as f64 / total as f64 >= MAX_RATIO).then(|| {
        RuleHit {
            occurrences: count,
            total,
            pct: ratio_pct(count, total),
            stat: Some(max_files.to_string()),
            escalate: false,
            examples,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::super::run_rules;
    use crate::coach::testutil::{call, ctx_calls};
    use crate::coach::CoachContext;

    fn triggered_ids(calls: &[crate::tools::ParsedCall]) -> Vec<&'static str> {
        let refs = ctx_calls(calls);
        let ctx = CoachContext::new(&refs);
        run_rules(&ctx).into_iter().map(|o| o.id).collect()
    }

    #[test]
    fn lazy_prompting_triggers_on_short_prompt_majority() {
        // 11 short prompts + 1 long: ratio > 0.3, count > 10.
        let mut calls = Vec::new();
        for i in 0..11 {
            calls.push(call(&format!("s{i}"), i, "fix bug"));
        }
        calls.push(call(
            "s-long",
            30,
            "Refactor the authentication middleware to use JWT tokens and add refresh token rotation",
        ));
        assert!(triggered_ids(&calls).contains(&"lazy-prompting"));
    }

    #[test]
    fn lazy_prompting_stays_clean_on_substantial_prompts() {
        let calls: Vec<_> = (0..12)
            .map(|i| {
                call(
                    &format!("s{i}"),
                    i,
                    "Refactor the parser module and keep the public API stable while adding tests",
                )
            })
            .collect();
        assert!(!triggered_ids(&calls).contains(&"lazy-prompting"));
    }

    #[test]
    fn caps_lock_and_frustration_trigger_on_shouting() {
        let calls = vec![
            call("s1", 0, "WHY WONT THIS WORK???"),
            call("s2", 1, "THIS IS SO BROKEN FIX IT NOW"),
        ];
        let ids = triggered_ids(&calls);
        assert!(ids.contains(&"caps-lock"));
        assert!(ids.contains(&"frustration-signals"));

        let calm = vec![
            call("s1", 0, "Please refactor the auth module carefully"),
            call("s2", 1, "Add error handling to the API endpoint"),
        ];
        let ids = triggered_ids(&calm);
        assert!(!ids.contains(&"caps-lock"));
        assert!(!ids.contains(&"frustration-signals"));
    }

    #[test]
    fn repeated_prompts_counts_duplicate_groups() {
        let calls: Vec<_> = (0..3)
            .map(|i| call(&format!("s{i}"), i, "please run the full test suite again"))
            .collect();
        assert!(triggered_ids(&calls).contains(&"repeated-prompts"));

        let distinct = vec![
            call("s1", 0, "please run the full test suite again"),
            call("s2", 1, "now check the formatting of the archive"),
            call("s3", 2, "finally build the desktop bundle for macos"),
        ];
        assert!(!triggered_ids(&distinct).contains(&"repeated-prompts"));
    }

    #[test]
    fn verbose_output_triggers_on_big_output_from_tiny_prompts() {
        let mut calls = Vec::new();
        for i in 0..11 {
            let mut c = call(&format!("s{i}"), i, "explain this file plz");
            c.output_tokens = 8_000;
            calls.push(c);
        }
        assert!(triggered_ids(&calls).contains(&"verbose-output"));
    }

    #[test]
    fn excessive_file_context_needs_file_ref_support() {
        let mut calls = Vec::new();
        for i in 0..10 {
            let mut c = call(
                &format!("s{i}"),
                i,
                "review all of these files for problems",
            );
            c.referenced_files = (0..40).map(|n| format!("src/file{n}.rs")).collect();
            calls.push(c);
        }
        assert!(triggered_ids(&calls).contains(&"excessive-file-context"));

        // Same shape on a tool without the signal must not trigger.
        for c in &mut calls {
            c.tool = crate::tools::cursor::config::TOOL_ID;
        }
        assert!(!triggered_ids(&calls).contains(&"excessive-file-context"));
    }
}
