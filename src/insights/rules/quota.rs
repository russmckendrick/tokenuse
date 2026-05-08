use chrono::Duration;

use crate::insights::recommendation::{Recommendation, RuleId, Scope, Severity};

use super::model_rightsizing::format_percent;
use super::RuleContext;

const CLAUDE_FORECAST_WARN: f64 = 0.90;
const CLAUDE_FORECAST_RISK: f64 = 1.0;
const COPILOT_FORECAST_THRESHOLD: f64 = 0.80;

pub fn claude_weekly_forecast(ctx: &RuleContext<'_>) -> Vec<Recommendation> {
    weekly_forecast(
        ctx,
        "claude-code",
        RuleId::ClaudeWeeklyForecast,
        CLAUDE_FORECAST_WARN,
        CLAUDE_FORECAST_RISK,
    )
}

pub fn copilot_premium_pacing(ctx: &RuleContext<'_>) -> Vec<Recommendation> {
    weekly_forecast(
        ctx,
        "copilot",
        RuleId::CopilotPremiumPacing,
        COPILOT_FORECAST_THRESHOLD,
        CLAUDE_FORECAST_RISK,
    )
}

fn weekly_forecast(
    ctx: &RuleContext<'_>,
    tool: &'static str,
    rule_id: RuleId,
    warn_threshold: f64,
    risk_threshold: f64,
) -> Vec<Recommendation> {
    let mut out = Vec::new();
    for snapshot in ctx.limits {
        if snapshot.tool != tool {
            continue;
        }
        let Some(window) = snapshot.primary else {
            continue;
        };
        let Some(resets_at) = window.resets_at else {
            continue;
        };
        let total_window = Duration::minutes(window.window_minutes as i64);
        if total_window <= Duration::zero() {
            continue;
        }
        let remaining = (resets_at - ctx.now).max(Duration::zero());
        let elapsed = (total_window - remaining).max(Duration::zero());
        if elapsed <= Duration::zero() {
            continue;
        }
        let elapsed_fraction = elapsed.num_seconds() as f64 / total_window.num_seconds() as f64;
        if elapsed_fraction <= 0.05 {
            continue;
        }
        let used = window.used_percent / 100.0;
        let projected = used / elapsed_fraction;
        if projected < warn_threshold {
            continue;
        }
        let severity = if projected >= risk_threshold {
            Severity::Risk
        } else {
            Severity::Warn
        };
        let limit_name = snapshot
            .limit_name
            .clone()
            .unwrap_or_else(|| snapshot.limit_id.clone());
        out.push(Recommendation {
            rule_id,
            severity,
            body_args: vec![
                ("limit", limit_name),
                ("used", format_percent(used)),
                ("projected", format_percent(projected)),
                ("elapsed", format_percent(elapsed_fraction)),
                (
                    "remaining_hours",
                    format!("{:.1}", remaining.num_minutes() as f64 / 60.0),
                ),
            ],
            est_savings_usd: None,
            est_savings_basis: None,
            scope: Scope::Tool { tool: tool.into() },
            silenced_reason_key: None,
            silenced_reason_args: Vec::new(),
        });
    }
    out
}

pub fn limit_recently_hit(ctx: &RuleContext<'_>) -> Vec<Recommendation> {
    let mut out = Vec::new();
    for snapshot in ctx.limits {
        let Some(hit_type) = snapshot.rate_limit_reached_type.as_deref() else {
            continue;
        };
        let Some(observed) = snapshot.observed_at else {
            continue;
        };
        if (ctx.now - observed) > Duration::hours(24) {
            continue;
        }
        let limit_name = snapshot
            .limit_name
            .clone()
            .unwrap_or_else(|| snapshot.limit_id.clone());
        out.push(Recommendation {
            rule_id: RuleId::LimitRecentlyHit,
            severity: Severity::Risk,
            body_args: vec![
                ("tool", snapshot.tool.to_string()),
                ("limit", limit_name),
                ("hit_type", hit_type.to_string()),
            ],
            est_savings_usd: None,
            est_savings_basis: None,
            scope: Scope::Tool {
                tool: snapshot.tool.into(),
            },
            silenced_reason_key: None,
            silenced_reason_args: Vec::new(),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::insights::baselines::Baselines;
    use crate::insights::fixtures::{limit_snapshot, limit_with_recent_hit, now};
    use crate::pricing::PriceTable;

    #[test]
    fn claude_weekly_forecast_fires_when_pacing_exceeds_warn() {
        // Half-elapsed window, 80% used → projected 160% > 100%.
        let snapshot = limit_snapshot("claude-code", "weekly", 80.0, 7 * 24 * 60, 7 * 24 * 30);
        let limits = vec![snapshot];
        let baselines = Baselines::build(&[], now());
        let ctx = RuleContext {
            calls: &[],
            limits: &limits,
            baselines: &baselines,
            price_table: PriceTable::embedded(),
            now: now(),
        };
        let recs = claude_weekly_forecast(&ctx);
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0].severity, Severity::Risk);
    }

    #[test]
    fn forecast_silent_when_window_just_started() {
        let snapshot = limit_snapshot("claude-code", "weekly", 5.0, 7 * 24 * 60, 7 * 24 * 60 - 5);
        let baselines = Baselines::build(&[], now());
        let ctx = RuleContext {
            calls: &[],
            limits: std::slice::from_ref(&snapshot),
            baselines: &baselines,
            price_table: PriceTable::embedded(),
            now: now(),
        };
        assert!(claude_weekly_forecast(&ctx).is_empty());
    }

    #[test]
    fn limit_recently_hit_emits_risk() {
        let snapshot = limit_with_recent_hit("claude-code", "weekly", "weekly_quota");
        let baselines = Baselines::build(&[], now());
        let ctx = RuleContext {
            calls: &[],
            limits: std::slice::from_ref(&snapshot),
            baselines: &baselines,
            price_table: PriceTable::embedded(),
            now: now(),
        };
        let recs = limit_recently_hit(&ctx);
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0].severity, Severity::Risk);
    }
}
