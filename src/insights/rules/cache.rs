use std::collections::{BTreeSet, HashMap};

use crate::insights::baselines::tool_reports_cache;
use crate::insights::recommendation::{Recommendation, RuleId, SavingsBasis, Scope, Severity};

use super::model_rightsizing::format_percent;
use super::RuleContext;

const HIT_DROP_THRESHOLD: f64 = 0.15;
const HIT_DROP_BASELINE_FLOOR: f64 = 0.50;

const WRITE_RATIO_THRESHOLD: f64 = 0.5;
const WRITE_RATIO_MIN_EVENTS: u64 = 100;

const LOW_HIT_RATIO: f64 = 0.5;
const LOW_HIT_MIN_SESSIONS: u64 = 5;

pub fn cache_hit_trend_drop(ctx: &RuleContext<'_>) -> Vec<Recommendation> {
    let mut out = Vec::new();
    for ((tool, project), window) in &ctx.baselines.cache_hit {
        let (Some(prior), Some(recent)) = (window.prior_30d, window.last_7d) else {
            continue;
        };
        if prior < HIT_DROP_BASELINE_FLOOR {
            continue;
        }
        let drop = prior - recent;
        if drop < HIT_DROP_THRESHOLD {
            continue;
        }

        // Estimate savings: missed cache reads × (input − cache_read) per token.
        // We approximate input/cache_read prices via the embedded fallback for the tool.
        let price = ctx.price_table.lookup_for(tool, "claude-sonnet-4-5", None);
        let per_token_delta = (price.input - price.cache_read).max(0.0);
        let recovered_tokens =
            window.last_7d_input_tokens as f64 * (drop / prior.max(0.0001)).clamp(0.0, 1.0);
        let savings = recovered_tokens * per_token_delta;
        if savings < 0.5 {
            continue;
        }

        let severity = if drop >= 0.30 {
            Severity::Risk
        } else {
            Severity::Warn
        };

        out.push(Recommendation {
            rule_id: RuleId::CacheHitTrendDrop,
            severity,
            body_args: vec![
                ("tool", (*tool).to_string()),
                ("project", project.clone()),
                ("recent", format_percent(recent)),
                ("prior", format_percent(prior)),
                ("drop", format_percent(drop)),
            ],
            est_savings_usd: Some(savings),
            est_savings_basis: Some(SavingsBasis::PerWeek),
            scope: Scope::Project {
                name: project.clone(),
            },
            silenced_reason_key: None,
            silenced_reason_args: Vec::new(),
        });
    }
    out
}

pub fn cache_write_overhead(ctx: &RuleContext<'_>) -> Vec<Recommendation> {
    let mut out = Vec::new();
    for (tool, ratio_acc) in &ctx.baselines.cache_write_ratio_by_tool {
        if ratio_acc.events < WRITE_RATIO_MIN_EVENTS {
            continue;
        }
        let Some(ratio) = ratio_acc.ratio() else {
            continue;
        };
        if ratio < WRITE_RATIO_THRESHOLD {
            continue;
        }
        // Excess writes = creation tokens we didn't earn back via reads.
        let price = ctx.price_table.lookup_for(tool, "claude-sonnet-4-5", None);
        let per_token_delta = (price.cache_write - price.cache_read).max(0.0);
        let excess_tokens =
            (ratio_acc.cache_creation_tokens as f64 - ratio_acc.cache_read_tokens as f64).max(0.0);
        let savings = excess_tokens * per_token_delta;
        if savings < 0.5 {
            continue;
        }
        out.push(Recommendation {
            rule_id: RuleId::CacheWriteOverhead,
            severity: Severity::Warn,
            body_args: vec![
                ("tool", (*tool).to_string()),
                ("ratio", format!("{ratio:.2}")),
                ("events", ratio_acc.events.to_string()),
            ],
            est_savings_usd: Some(savings),
            est_savings_basis: Some(SavingsBasis::PerWeek),
            scope: Scope::Tool {
                tool: (*tool).into(),
            },
            silenced_reason_key: None,
            silenced_reason_args: Vec::new(),
        });
    }
    out
}

pub fn low_hit_project_outlier(ctx: &RuleContext<'_>) -> Vec<Recommendation> {
    // Count sessions per project (last 7 days, cache-reporting tools only).
    let today = ctx.now.date_naive();
    let mut sessions_by_project: HashMap<String, BTreeSet<String>> = HashMap::new();
    for call in ctx.calls {
        if !tool_reports_cache(call.tool) {
            continue;
        }
        let Some(ts) = call.timestamp else { continue };
        if (today - ts.date_naive()).num_days() >= 7 {
            continue;
        }
        sessions_by_project
            .entry(call.project.clone())
            .or_default()
            .insert(call.session_id.clone());
    }

    let mut out = Vec::new();
    for ((tool, project), window) in &ctx.baselines.cache_hit {
        let Some(rate) = window.last_7d else { continue };
        let Some(median) = ctx
            .baselines
            .tool_cache_medians
            .get(tool)
            .and_then(|m| m.median_hit_rate)
        else {
            continue;
        };
        if median <= 0.0 {
            continue;
        }
        if rate >= median * LOW_HIT_RATIO {
            continue;
        }
        let session_count = sessions_by_project
            .get(project)
            .map(|s| s.len() as u64)
            .unwrap_or(0);
        if session_count < LOW_HIT_MIN_SESSIONS {
            continue;
        }
        let price = ctx.price_table.lookup_for(tool, "claude-sonnet-4-5", None);
        let per_token_delta = (price.input - price.cache_read).max(0.0);
        let gap = (median - rate).max(0.0);
        let savings = window.last_7d_input_tokens as f64 * gap * per_token_delta;
        if savings < 0.5 {
            continue;
        }
        out.push(Recommendation {
            rule_id: RuleId::LowHitProjectOutlier,
            severity: Severity::Info,
            body_args: vec![
                ("project", project.clone()),
                ("tool", (*tool).to_string()),
                ("rate", format_percent(rate)),
                ("median", format_percent(median)),
                ("sessions", session_count.to_string()),
            ],
            est_savings_usd: Some(savings),
            est_savings_basis: Some(SavingsBasis::PerWeek),
            scope: Scope::Project {
                name: project.clone(),
            },
            silenced_reason_key: None,
            silenced_reason_args: Vec::new(),
        });
    }
    out
}

/// Emit one Info card per cache-blind tool (Cursor, Copilot) with calls in window so the
/// user understands cache rules don't apply to those tools.
pub fn silenced_tools(ctx: &RuleContext<'_>) -> Vec<Recommendation> {
    let today = ctx.now.date_naive();
    let mut tools_with_traffic: BTreeSet<&'static str> = BTreeSet::new();
    for call in ctx.calls {
        if matches!(call.tool, "cursor" | "copilot") {
            if let Some(ts) = call.timestamp {
                if (today - ts.date_naive()).num_days() < 7 {
                    tools_with_traffic.insert(call.tool);
                }
            }
        }
    }
    let mut out = Vec::new();
    for tool in tools_with_traffic {
        out.push(Recommendation {
            rule_id: RuleId::CacheToolSilenced,
            severity: Severity::Info,
            body_args: vec![("tool", tool.to_string())],
            est_savings_usd: None,
            est_savings_basis: None,
            scope: Scope::Tool { tool: tool.into() },
            silenced_reason_key: Some("no_cache_data"),
            silenced_reason_args: vec![("tool", tool.to_string())],
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::insights::baselines::Baselines;
    use crate::insights::fixtures::{now, CallBuilder};
    use crate::pricing::PriceTable;

    #[test]
    fn cache_hit_drop_silent_with_insufficient_baseline() {
        let calls = vec![
            CallBuilder::new("claude-code", "claude-sonnet-4-5", "alpha")
                .at(2)
                .input(1_000)
                .cache_read(100)
                .cost(0.05)
                .build(),
        ];
        let baselines = Baselines::build(&calls, now());
        let ctx = RuleContext {
            calls: &calls,
            limits: &[],
            baselines: &baselines,
            price_table: PriceTable::embedded(),
            now: now(),
        };
        assert!(cache_hit_trend_drop(&ctx).is_empty());
    }

    #[test]
    fn silenced_tools_reports_cursor_and_copilot() {
        let calls = vec![
            CallBuilder::new("cursor", "claude-sonnet-4-5", "alpha")
                .at(1)
                .input(1_000)
                .cost(0.05)
                .build(),
            CallBuilder::new("copilot", "gpt-4o", "alpha")
                .at(2)
                .input(1_000)
                .cost(0.05)
                .build(),
            CallBuilder::new("claude-code", "claude-sonnet-4-5", "alpha")
                .at(1)
                .input(1_000)
                .cost(0.05)
                .build(),
        ];
        let baselines = Baselines::build(&calls, now());
        let ctx = RuleContext {
            calls: &calls,
            limits: &[],
            baselines: &baselines,
            price_table: PriceTable::embedded(),
            now: now(),
        };
        let recs = silenced_tools(&ctx);
        assert_eq!(recs.len(), 2);
        assert!(recs
            .iter()
            .all(|r| r.silenced_reason_key == Some("no_cache_data")));
    }

    #[test]
    fn cache_write_overhead_silent_below_event_threshold() {
        let calls = vec![
            CallBuilder::new("claude-code", "claude-sonnet-4-5", "alpha")
                .at(2)
                .cache_write(10_000)
                .cache_read(100)
                .cost(0.05)
                .build(),
        ];
        let baselines = Baselines::build(&calls, now());
        let ctx = RuleContext {
            calls: &calls,
            limits: &[],
            baselines: &baselines,
            price_table: PriceTable::embedded(),
            now: now(),
        };
        assert!(cache_write_overhead(&ctx).is_empty());
    }
}
