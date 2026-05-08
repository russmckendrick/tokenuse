use std::collections::HashMap;

use crate::insights::recommendation::{Recommendation, RuleId, SavingsBasis, Scope, Severity};
use crate::pricing::ModelPrice;
use crate::tools::{ParsedCall, Speed};

use super::RuleContext;

fn price_call(call: &ParsedCall, price: &ModelPrice, speed: Speed) -> f64 {
    let multiplier = match (speed, price.fast_multiplier) {
        (Speed::Fast, Some(m)) => m,
        _ => 1.0,
    };
    multiplier
        * ((call.input_tokens as f64) * price.input
            + (call.output_tokens as f64) * price.output
            + (call.cache_creation_input_tokens as f64) * price.cache_write
            + (call.cache_read_input_tokens as f64) * price.cache_read
            + (call.web_search_requests as f64) * price.web_search)
}

const HAIKU_MODEL: &str = "claude-haiku-4-5";
const SHORT_OUTPUT_TOKENS: u64 = 200;
const SHORT_INPUT_TOKENS: u64 = 4_000;
const SHORT_MIN_CALLS: usize = 50;
const SHORT_MIN_PROJECT_SHARE: f64 = 0.30;

const FAST_OPUS_MIN_USD: f64 = 5.0;
const FAST_OPUS_MIN_RATIO: f64 = 2.0;

const REASONING_MIN_RATIO: f64 = 3.0;
const REASONING_MIN_CALLS: usize = 20;
const REASONING_MIN_SHARE: f64 = 0.40;

fn is_sonnet(model: &str) -> bool {
    model.to_ascii_lowercase().contains("sonnet")
}

fn is_opus(model: &str) -> bool {
    model.to_ascii_lowercase().contains("opus")
}

fn is_o_series(model: &str) -> bool {
    let lower = model.to_ascii_lowercase();
    lower.starts_with('o') && lower.chars().nth(1).is_some_and(|c| c.is_ascii_digit())
        || lower.contains("gpt-5") && lower.contains("codex")
}

pub fn short_output_sonnet(ctx: &RuleContext<'_>) -> Vec<Recommendation> {
    #[derive(Default)]
    struct Acc {
        short: Vec<usize>,
        total_sonnet: usize,
    }
    let mut by_project: HashMap<String, Acc> = HashMap::new();

    for (idx, call) in ctx.calls.iter().enumerate() {
        if !is_sonnet(&call.model) {
            continue;
        }
        if call.tool != "claude-code" {
            continue;
        }
        let entry = by_project.entry(call.project.clone()).or_default();
        entry.total_sonnet += 1;
        if call.output_tokens < SHORT_OUTPUT_TOKENS
            && call.input_tokens < SHORT_INPUT_TOKENS
            && call.reasoning_tokens == 0
        {
            entry.short.push(idx);
        }
    }

    let mut out = Vec::new();
    for (project, acc) in by_project {
        if acc.short.len() < SHORT_MIN_CALLS {
            continue;
        }
        if (acc.short.len() as f64) / (acc.total_sonnet.max(1) as f64) < SHORT_MIN_PROJECT_SHARE {
            continue;
        }

        let mut current_cost = 0.0;
        let mut projected_cost = 0.0;
        for &idx in &acc.short {
            let call = &ctx.calls[idx];
            current_cost += call.cost_usd;
            let haiku_price =
                ctx.price_table
                    .lookup_for("claude-code", HAIKU_MODEL, call.timestamp);
            projected_cost += price_call(call, haiku_price, Speed::Standard);
        }

        let savings = (current_cost - projected_cost).max(0.0);
        if savings < 0.5 {
            continue;
        }

        let weekly_savings = scale_to_period(savings, ctx, SavingsBasis::PerWeek);
        out.push(Recommendation {
            rule_id: RuleId::ShortOutputSonnet,
            severity: Severity::Warn,
            body_args: vec![
                ("project", project.clone()),
                ("calls", acc.short.len().to_string()),
                ("current", format_money(current_cost)),
                ("projected", format_money(projected_cost)),
            ],
            est_savings_usd: Some(weekly_savings),
            est_savings_basis: Some(SavingsBasis::PerWeek),
            scope: Scope::Project { name: project },
            silenced_reason_key: None,
            silenced_reason_args: Vec::new(),
        });
    }
    out
}

pub fn fast_mode_opus_excess(ctx: &RuleContext<'_>) -> Vec<Recommendation> {
    #[derive(Default)]
    struct Acc {
        fast_cost: f64,
        standard_cost: f64,
        fast_calls: usize,
        fast_indices: Vec<usize>,
    }
    let mut by_project: HashMap<String, Acc> = HashMap::new();

    for (idx, call) in ctx.calls.iter().enumerate() {
        if !is_opus(&call.model) {
            continue;
        }
        let entry = by_project.entry(call.project.clone()).or_default();
        if matches!(call.speed, Speed::Fast) {
            entry.fast_cost += call.cost_usd;
            entry.fast_calls += 1;
            entry.fast_indices.push(idx);
        } else {
            entry.standard_cost += call.cost_usd;
        }
    }

    let mut out = Vec::new();
    for (project, acc) in by_project {
        if acc.fast_cost < FAST_OPUS_MIN_USD {
            continue;
        }
        let standard_basis = acc.standard_cost.max(0.01);
        if acc.fast_cost / standard_basis < FAST_OPUS_MIN_RATIO {
            continue;
        }
        // Estimate savings as the multiplier overhead removed.
        let mut savings = 0.0;
        for &idx in &acc.fast_indices {
            let call = &ctx.calls[idx];
            let price = ctx
                .price_table
                .lookup_for(call.tool, &call.model, call.timestamp);
            let multiplier = price.fast_multiplier.unwrap_or(1.0);
            if multiplier <= 1.0 {
                continue;
            }
            let standard_cost = call.cost_usd / multiplier;
            savings += call.cost_usd - standard_cost;
        }
        if savings < 0.5 {
            continue;
        }
        let weekly = scale_to_period(savings, ctx, SavingsBasis::PerWeek);
        out.push(Recommendation {
            rule_id: RuleId::FastModeOpusExcess,
            severity: Severity::Warn,
            body_args: vec![
                ("project", project.clone()),
                ("fast_calls", acc.fast_calls.to_string()),
                ("fast_cost", format_money(acc.fast_cost)),
                ("standard_cost", format_money(acc.standard_cost)),
            ],
            est_savings_usd: Some(weekly),
            est_savings_basis: Some(SavingsBasis::PerWeek),
            scope: Scope::Project { name: project },
            silenced_reason_key: None,
            silenced_reason_args: Vec::new(),
        });
    }
    out
}

pub fn reasoning_heavy_o_series(ctx: &RuleContext<'_>) -> Vec<Recommendation> {
    #[derive(Default)]
    struct Acc {
        calls: usize,
        reasoning: u64,
        output: u64,
        reasoning_cost: f64,
        total_cost: f64,
    }
    let mut by_model: HashMap<String, Acc> = HashMap::new();

    for call in ctx.calls {
        if call.tool != "codex" {
            continue;
        }
        if !is_o_series(&call.model) {
            continue;
        }
        if call.output_tokens == 0 {
            continue;
        }
        if (call.reasoning_tokens as f64) / (call.output_tokens as f64) <= REASONING_MIN_RATIO {
            continue;
        }
        let entry = by_model.entry(call.model.clone()).or_default();
        entry.calls += 1;
        entry.reasoning += call.reasoning_tokens;
        entry.output += call.output_tokens;
        entry.total_cost += call.cost_usd;
        let price = ctx
            .price_table
            .lookup_for(call.tool, &call.model, call.timestamp);
        entry.reasoning_cost += (call.reasoning_tokens as f64) * price.output;
    }

    let mut out = Vec::new();
    for (model, acc) in by_model {
        if acc.calls < REASONING_MIN_CALLS {
            continue;
        }
        if acc.total_cost < 0.01 {
            continue;
        }
        if acc.reasoning_cost / acc.total_cost < REASONING_MIN_SHARE {
            continue;
        }
        // Conservative: assume halving reasoning tokens via lower-effort tier.
        let savings = (acc.reasoning_cost / 2.0).max(0.0);
        let weekly = scale_to_period(savings, ctx, SavingsBasis::PerWeek);
        out.push(Recommendation {
            rule_id: RuleId::ReasoningHeavyOSeries,
            severity: Severity::Info,
            body_args: vec![
                ("model", model.clone()),
                ("calls", acc.calls.to_string()),
                (
                    "reasoning_share",
                    format_percent(acc.reasoning_cost / acc.total_cost),
                ),
                ("reasoning_cost", format_money(acc.reasoning_cost)),
            ],
            est_savings_usd: Some(weekly),
            est_savings_basis: Some(SavingsBasis::PerWeek),
            scope: Scope::Tool {
                tool: "codex".into(),
            },
            silenced_reason_key: None,
            silenced_reason_args: Vec::new(),
        });
    }
    out
}

fn scale_to_period(window_usd: f64, ctx: &RuleContext<'_>, target: SavingsBasis) -> f64 {
    let observed_days = observed_window_days(ctx).max(1.0);
    let target_days = match target {
        SavingsBasis::PerWeek => 7.0,
        SavingsBasis::PerMonth => 30.0,
        SavingsBasis::OneOff => return window_usd,
    };
    window_usd * (target_days / observed_days)
}

fn observed_window_days(ctx: &RuleContext<'_>) -> f64 {
    let today = ctx.now.date_naive();
    let mut earliest = today;
    let mut found = false;
    for call in ctx.calls {
        if let Some(ts) = call.timestamp {
            let date = ts.date_naive();
            if date < earliest {
                earliest = date;
            }
            found = true;
        }
    }
    if !found {
        return 7.0;
    }
    ((today - earliest).num_days() as f64 + 1.0).max(1.0)
}

pub(crate) fn format_money(amount: f64) -> String {
    if amount >= 100.0 {
        format!("${amount:.0}")
    } else if amount >= 10.0 {
        format!("${amount:.1}")
    } else {
        format!("${amount:.2}")
    }
}

pub(crate) fn format_percent(ratio: f64) -> String {
    format!("{:.0}%", (ratio * 100.0).clamp(0.0, 100.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::insights::baselines::Baselines;
    use crate::insights::fixtures::{at, now, CallBuilder};
    use crate::pricing::PriceTable;

    #[test]
    fn short_output_sonnet_fires_when_threshold_met() {
        let mut calls = Vec::new();
        for i in 0..60 {
            calls.push(
                CallBuilder::new("claude-code", "claude-sonnet-4-5", "alpha")
                    .session(&format!("alpha-{i}"))
                    .at(2)
                    .input(1_000)
                    .output(120)
                    .cost(0.05)
                    .build(),
            );
        }
        // Add a few non-short Sonnet calls to keep the share above 30%.
        for i in 0..10 {
            calls.push(
                CallBuilder::new("claude-code", "claude-sonnet-4-5", "alpha")
                    .session(&format!("alpha-long-{i}"))
                    .at(3)
                    .input(50_000)
                    .output(8_000)
                    .cost(0.5)
                    .build(),
            );
        }

        let baselines = Baselines::build(&calls, now());
        let ctx = RuleContext {
            calls: &calls,
            limits: &[],
            baselines: &baselines,
            price_table: PriceTable::embedded(),
            now: now(),
        };
        let recs = short_output_sonnet(&ctx);
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0].rule_id, RuleId::ShortOutputSonnet);
        assert!(recs[0].est_savings_usd.unwrap() > 0.0);
    }

    #[test]
    fn short_output_sonnet_silent_when_too_few_calls() {
        let calls: Vec<_> = (0..10)
            .map(|i| {
                CallBuilder::new("claude-code", "claude-sonnet-4-5", "alpha")
                    .session(&format!("alpha-{i}"))
                    .at(2)
                    .input(500)
                    .output(50)
                    .cost(0.01)
                    .build()
            })
            .collect();
        let baselines = Baselines::build(&calls, now());
        let ctx = RuleContext {
            calls: &calls,
            limits: &[],
            baselines: &baselines,
            price_table: PriceTable::embedded(),
            now: now(),
        };
        assert!(short_output_sonnet(&ctx).is_empty());
    }

    #[test]
    fn fast_mode_opus_silent_without_fast_traffic() {
        let calls = vec![CallBuilder::new("claude-code", "claude-opus-4-7", "alpha")
            .at(1)
            .input(1_000)
            .output(500)
            .cost(2.0)
            .build()];
        let baselines = Baselines::build(&calls, now());
        let ctx = RuleContext {
            calls: &calls,
            limits: &[],
            baselines: &baselines,
            price_table: PriceTable::embedded(),
            now: now(),
        };
        assert!(fast_mode_opus_excess(&ctx).is_empty());
    }

    #[test]
    fn reasoning_heavy_silent_below_call_threshold() {
        let calls: Vec<_> = (0..5)
            .map(|i| {
                CallBuilder::new("codex", "gpt-5-codex", "alpha")
                    .session(&format!("alpha-{i}"))
                    .at(1)
                    .input(1_000)
                    .output(500)
                    .reasoning(5_000)
                    .cost(0.1)
                    .build()
            })
            .collect();
        let baselines = Baselines::build(&calls, now());
        let ctx = RuleContext {
            calls: &calls,
            limits: &[],
            baselines: &baselines,
            price_table: PriceTable::embedded(),
            now: now(),
        };
        assert!(reasoning_heavy_o_series(&ctx).is_empty());
    }

    #[test]
    fn observed_window_uses_earliest_call() {
        let calls = vec![
            CallBuilder::new("claude-code", "x", "alpha").at(0).build(),
            CallBuilder::new("claude-code", "x", "alpha").at(13).build(),
        ];
        let baselines = Baselines::build(&calls, now());
        let ctx = RuleContext {
            calls: &calls,
            limits: &[],
            baselines: &baselines,
            price_table: PriceTable::embedded(),
            now: now(),
        };
        let days = observed_window_days(&ctx);
        assert!((days - 14.0).abs() < 0.01);
    }

    #[test]
    fn at_helper_returns_relative_timestamp() {
        let one_day = at(1).unwrap();
        assert!(one_day < now());
    }
}
