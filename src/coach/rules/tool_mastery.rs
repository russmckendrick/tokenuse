//! Tool-mastery rules.
//!
//! Ported from Microsoft's AI-Engineering-Coach rule files (MIT, Copyright
//! (c) Microsoft Corporation): mcp-tool-bloat (reimplemented to its
//! documented semantics - the reference DSL referenced a nonexistent field
//! and could never fire), model-overreliance, reasoning-effort-overuse,
//! premium-waste (merged with premium-for-lookup-questions), and
//! cache-hit-starvation. "Premium" is derived from tokens' own pricing book
//! instead of the reference's hardcoded MODEL_TIERS table.

use super::{example, ratio_pct, RuleDef, RuleGroup, RuleHit, Severity, MAX_EXAMPLES};
use crate::coach::{signals, text, CoachContext};

/// Output rate at or above which a model counts as premium ($ per 1M tokens).
const PREMIUM_OUTPUT_RATE: f64 = 10.0;

pub fn rules() -> Vec<RuleDef> {
    vec![
        RuleDef {
            id: "mcp-tool-bloat",
            group: RuleGroup::ToolMastery,
            severity: Severity::Medium,
            detect: mcp_tool_bloat,
        },
        RuleDef {
            id: "model-overreliance",
            group: RuleGroup::ToolMastery,
            severity: Severity::Medium,
            detect: model_overreliance,
        },
        RuleDef {
            id: "reasoning-effort-overuse",
            group: RuleGroup::ToolMastery,
            severity: Severity::Medium,
            detect: reasoning_effort_overuse,
        },
        RuleDef {
            id: "premium-waste",
            group: RuleGroup::ToolMastery,
            severity: Severity::Medium,
            detect: premium_waste,
        },
        RuleDef {
            id: "cache-hit-starvation",
            group: RuleGroup::ToolMastery,
            severity: Severity::Medium,
            detect: cache_hit_starvation,
        },
    ]
}

fn is_premium_model(model: &str) -> bool {
    let table = crate::pricing::PriceTable::configured()
        .read()
        .expect("pricing table lock");
    table.lookup(model).output >= PREMIUM_OUTPUT_RATE
}

/// Sessions invoking more than 40 distinct tools, three or more times.
fn mcp_tool_bloat(ctx: &CoachContext) -> Option<RuleHit> {
    const MAX_TOOLS_PER_SESSION: usize = 40;
    const MIN_SESSIONS: u64 = 3;

    let mut count = 0u64;
    let mut total = 0u64;
    let mut biggest = 0usize;
    let mut examples = Vec::new();
    for session in ctx.conversational_sessions() {
        total += 1;
        let distinct = session.distinct_tool_count();
        biggest = biggest.max(distinct);
        if distinct > MAX_TOOLS_PER_SESSION {
            count += 1;
            if examples.len() < MAX_EXAMPLES {
                examples.push(example(
                    &session.project,
                    format!("{distinct} distinct tools"),
                ));
            }
        }
    }
    (count >= MIN_SESSIONS).then(|| RuleHit {
        occurrences: count,
        total,
        pct: None,
        stat: Some(biggest.to_string()),
        escalate: false,
        examples,
    })
}

/// More than 80% of turns on one model with fewer than 3 models seen (min 10 turns).
fn model_overreliance(ctx: &CoachContext) -> Option<RuleHit> {
    const MAX_TOP_MODEL_RATE: f64 = 0.8;
    const MIN_SAMPLE: u64 = 10;
    const MIN_MODELS: usize = 3;

    let mut by_model: std::collections::HashMap<&str, u64> = std::collections::HashMap::new();
    let mut total = 0u64;
    for turn in ctx.turns() {
        if turn.model.is_empty() {
            continue;
        }
        total += 1;
        *by_model.entry(turn.model).or_insert(0) += 1;
    }
    let (top_model, top_count) = by_model
        .iter()
        .max_by_key(|(_, count)| **count)
        .map(|(m, c)| (m.to_string(), *c))?;

    (total > MIN_SAMPLE
        && by_model.len() < MIN_MODELS
        && top_count as f64 / total as f64 > MAX_TOP_MODEL_RATE)
        .then(|| RuleHit {
            occurrences: top_count,
            total,
            pct: ratio_pct(top_count, total),
            stat: Some(top_model),
            escalate: false,
            examples: Vec::new(),
        })
}

/// Reasoning effort inferred from model-name suffixes only, never guessed.
fn effort_from_model(model: &str) -> Option<&'static str> {
    let lower = model.to_lowercase();
    let suffix = lower.rsplit(['-', ':']).next().unwrap_or("").trim();
    match suffix {
        "low" => Some("low"),
        "medium" => Some("medium"),
        "high" => Some("high"),
        "xhigh" | "max" => Some("max"),
        _ => None,
    }
}

/// More than 50% of effort-known turns at high/max reasoning (min 20 known).
fn reasoning_effort_overuse(ctx: &CoachContext) -> Option<RuleHit> {
    const MIN_SAMPLE: u64 = 20;
    const MAX_RATIO: f64 = 0.5;

    let mut premium = 0u64;
    let mut known = 0u64;
    let mut examples = Vec::new();
    for turn in ctx.turns() {
        let Some(effort) = effort_from_model(turn.model) else {
            continue;
        };
        known += 1;
        if effort == "high" || effort == "max" {
            premium += 1;
            if examples.len() < MAX_EXAMPLES {
                examples.push(example(turn.model, format!("effort: {effort}")));
            }
        }
    }
    (known > MIN_SAMPLE && premium as f64 / known as f64 > MAX_RATIO).then(|| RuleHit {
        occurrences: premium,
        total: known,
        pct: ratio_pct(premium, known),
        stat: None,
        escalate: false,
        examples,
    })
}

/// Premium models on trivial requests: very short prompts with no code
/// output, or short lookup-style questions that used no tools (merged
/// reference rules premium-waste + premium-for-lookup-questions).
fn premium_waste(ctx: &CoachContext) -> Option<RuleHit> {
    const MAX_SHORT_PROMPT: u64 = 50;
    const MAX_LOOKUP_PROMPT: u64 = 120;
    const MIN_SAMPLE: u64 = 10;

    let mut count = 0u64;
    let mut total = 0u64;
    let mut examples = Vec::new();
    for turn in ctx.prompt_turns() {
        total += 1;
        if turn.ai_loc > 0 {
            continue;
        }
        let chars = turn.prompt_chars.unwrap_or(0);
        let short_and_simple = chars < MAX_SHORT_PROMPT;
        let lookup = chars < MAX_LOOKUP_PROMPT
            && turn.tool_calls == 0
            && text::is_lookup_question(turn.user_message);
        if (short_and_simple || lookup) && is_premium_model(turn.model) {
            count += 1;
            if examples.len() < MAX_EXAMPLES {
                examples.push(example(turn.user_message, turn.model.to_string()));
            }
        }
    }
    (count > MIN_SAMPLE).then(|| RuleHit {
        occurrences: count,
        total,
        pct: ratio_pct(count, total),
        stat: None,
        escalate: false,
        examples,
    })
}

/// Long-prompt turns (>5K input context) with under 10% served from cache
/// (min 20 such turns). Only tools reporting cache reads are counted.
fn cache_hit_starvation(ctx: &CoachContext) -> Option<RuleHit> {
    const MIN_PROMPT_TOKENS: u64 = 5_000;
    const MIN_SAMPLE: u64 = 20;
    const MIN_CACHE_RATE: f64 = 0.1;

    let mut count = 0u64;
    let mut prompt_sum = 0u64;
    let mut cache_sum = 0u64;
    let mut examples = Vec::new();
    for turn in ctx.turns().filter(|t| signals::supports_cache_read(t.tool)) {
        if turn.full_prompt_tokens <= MIN_PROMPT_TOKENS {
            continue;
        }
        count += 1;
        prompt_sum += turn.full_prompt_tokens;
        cache_sum += turn.cache_read_tokens;
        if examples.len() < MAX_EXAMPLES && !turn.user_message.is_empty() {
            examples.push(example(
                turn.user_message,
                format!(
                    "{} prompt tokens, {} cached",
                    turn.full_prompt_tokens, turn.cache_read_tokens
                ),
            ));
        }
    }
    if count <= MIN_SAMPLE || prompt_sum == 0 {
        return None;
    }
    let cache_rate = cache_sum as f64 / prompt_sum as f64;
    (cache_rate < MIN_CACHE_RATE).then(|| RuleHit {
        occurrences: count,
        total: count,
        pct: None,
        stat: Some(format!("{:.1}%", cache_rate * 100.0)),
        escalate: false,
        examples,
    })
}

#[cfg(test)]
mod tests {
    use super::super::run_rules;
    use super::effort_from_model;
    use crate::coach::testutil::{call, ctx_calls};
    use crate::coach::CoachContext;

    fn triggered_ids(calls: &[crate::tools::ParsedCall]) -> Vec<&'static str> {
        let refs = ctx_calls(calls);
        let ctx = CoachContext::new(&refs);
        run_rules(&ctx).into_iter().map(|o| o.id).collect()
    }

    #[test]
    fn effort_suffixes_parse_and_bare_models_stay_unknown() {
        assert_eq!(effort_from_model("claude-opus-4-7-high"), Some("high"));
        assert_eq!(effort_from_model("claude-opus-4.7-xhigh"), Some("max"));
        assert_eq!(effort_from_model("gpt-5.4:low"), Some("low"));
        assert_eq!(effort_from_model("claude-opus-4-7"), None);
        assert_eq!(effort_from_model("gpt-5"), None);
    }

    #[test]
    fn reasoning_effort_overuse_needs_known_effort_majority() {
        let mut calls = Vec::new();
        for i in 0..21 {
            let mut c = call(&format!("s{i}"), i, &format!("do heavy reasoning task {i}"));
            c.model = "claude-opus-4-7-high".into();
            calls.push(c);
        }
        assert!(triggered_ids(&calls).contains(&"reasoning-effort-overuse"));

        for c in &mut calls {
            c.model = "claude-opus-4-7".into();
        }
        assert!(!triggered_ids(&calls).contains(&"reasoning-effort-overuse"));
    }

    #[test]
    fn mcp_tool_bloat_counts_distinct_tools_per_session() {
        let mut calls = Vec::new();
        for s in 0..3 {
            for i in 0..3 {
                let mut c = call(&format!("bloat{s}"), s * 10 + i, &format!("step {i}"));
                c.tools = (0..15)
                    .map(|n| format!("mcp__server{i}__tool{n}"))
                    .collect();
                calls.push(c);
            }
        }
        // 3 turns x 15 distinct names each = 45 distinct per session.
        assert!(triggered_ids(&calls).contains(&"mcp-tool-bloat"));
    }

    #[test]
    fn model_overreliance_requires_low_diversity() {
        let mut calls = Vec::new();
        for i in 0..12 {
            calls.push(call(
                &format!("s{i}"),
                i,
                &format!("routine task number {i}"),
            ));
        }
        // 12 turns all on the same single model.
        assert!(triggered_ids(&calls).contains(&"model-overreliance"));

        for (i, c) in calls.iter_mut().enumerate() {
            c.model = format!("model-{}", i % 4);
        }
        assert!(!triggered_ids(&calls).contains(&"model-overreliance"));
    }

    #[test]
    fn cache_hit_starvation_flags_uncached_long_prompts() {
        let mut calls = Vec::new();
        for i in 0..21 {
            let mut c = call(&format!("s{i}"), i, &format!("giant context request {i}"));
            c.input_tokens = 20_000;
            c.cache_read_input_tokens = 0;
            calls.push(c);
        }
        assert!(triggered_ids(&calls).contains(&"cache-hit-starvation"));

        for c in &mut calls {
            c.cache_read_input_tokens = 18_000;
        }
        assert!(!triggered_ids(&calls).contains(&"cache-hit-starvation"));
    }
}
