use std::collections::HashMap;

use crate::insights::recommendation::{Recommendation, RuleId, Scope, Severity};

use super::model_rightsizing::{format_money, format_percent};
use super::RuleContext;

const SESSION_OUTLIER_MIN_BASELINE: usize = 20;
const ZSCORE_WARN: f64 = 2.5;
const ZSCORE_RISK: f64 = 3.5;
const ZSCORE_MIN_DAYS: usize = 14;
const MOM_GROWTH_THRESHOLD: f64 = 0.5;
const MOM_GROWTH_MIN_USD: f64 = 10.0;
const MOM_MIN_CALLS: u64 = 10;

pub fn outlier_session_cost(ctx: &RuleContext<'_>) -> Vec<Recommendation> {
    if ctx.baselines.session_cost_overall.n < SESSION_OUTLIER_MIN_BASELINE {
        return Vec::new();
    }
    let threshold = ctx.baselines.session_cost_overall.outlier_threshold();
    if threshold <= 0.0 {
        return Vec::new();
    }

    let today = ctx.now.date_naive();
    let mut session_costs: HashMap<String, (f64, String)> = HashMap::new();
    for call in ctx.calls {
        let Some(ts) = call.timestamp else { continue };
        if (today - ts.date_naive()).num_days() >= 30 {
            continue;
        }
        if call.session_id.is_empty() {
            continue;
        }
        let entry = session_costs
            .entry(call.session_id.clone())
            .or_insert((0.0, call.project.clone()));
        entry.0 += call.cost_usd;
    }

    let mut out: Vec<(String, f64, String)> = session_costs
        .into_iter()
        .filter(|(_, (cost, _))| *cost > threshold)
        .map(|(id, (cost, project))| (id, cost, project))
        .collect();
    out.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    out.truncate(3);

    out.into_iter()
        .map(|(id, cost, project)| Recommendation {
            rule_id: RuleId::OutlierSessionCost,
            severity: Severity::Info,
            body_args: vec![
                ("session", id.clone()),
                ("project", project.clone()),
                ("cost", format_money(cost)),
                ("threshold", format_money(threshold)),
            ],
            est_savings_usd: None,
            est_savings_basis: None,
            scope: Scope::Session { id, project },
            silenced_reason_key: None,
            silenced_reason_args: Vec::new(),
        })
        .collect()
}

pub fn day_over_day_spend_zscore(ctx: &RuleContext<'_>) -> Vec<Recommendation> {
    let today_total = ctx.baselines.today_total(ctx.calls);
    let stats = &ctx.baselines.daily_cost_overall;
    let Some(z) = stats.flagged_zscore(today_total, ZSCORE_MIN_DAYS) else {
        return Vec::new();
    };
    if z < ZSCORE_WARN {
        return Vec::new();
    }
    let severity = if z >= ZSCORE_RISK {
        Severity::Risk
    } else {
        Severity::Warn
    };
    let pct = if stats.mean > 0.0 {
        (today_total - stats.mean) / stats.mean
    } else {
        0.0
    };
    vec![Recommendation {
        rule_id: RuleId::DayOverDaySpendZscore,
        severity,
        body_args: vec![
            ("today", format_money(today_total)),
            ("baseline", format_money(stats.mean)),
            ("zscore", format!("{z:.1}")),
            ("delta_pct", format_percent(pct.abs())),
        ],
        est_savings_usd: None,
        est_savings_basis: None,
        scope: Scope::All,
        silenced_reason_key: None,
        silenced_reason_args: Vec::new(),
    }]
}

pub fn project_mom_growth(ctx: &RuleContext<'_>) -> Vec<Recommendation> {
    let mut out = Vec::new();
    for (project, monthly) in &ctx.baselines.project_monthly_cost {
        if monthly.current_calls < MOM_MIN_CALLS || monthly.prior_calls < MOM_MIN_CALLS {
            continue;
        }
        if monthly.prior_month <= 0.0 {
            continue;
        }
        let growth = (monthly.current_month - monthly.prior_month) / monthly.prior_month;
        if growth < MOM_GROWTH_THRESHOLD {
            continue;
        }
        if monthly.current_month < MOM_GROWTH_MIN_USD {
            continue;
        }
        out.push(Recommendation {
            rule_id: RuleId::ProjectMomGrowth,
            severity: Severity::Warn,
            body_args: vec![
                ("project", project.clone()),
                ("current", format_money(monthly.current_month)),
                ("prior", format_money(monthly.prior_month)),
                ("growth", format_percent(growth)),
            ],
            est_savings_usd: None,
            est_savings_basis: None,
            scope: Scope::Project {
                name: project.clone(),
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
    use crate::insights::fixtures::{now, CallBuilder};
    use crate::pricing::PriceTable;

    #[test]
    fn outlier_session_silent_below_min_baseline() {
        let calls: Vec<_> = (0..5)
            .map(|i| {
                CallBuilder::new("claude-code", "claude-sonnet-4-5", "alpha")
                    .session(&format!("s-{i}"))
                    .at(2)
                    .cost(1.0)
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
        assert!(outlier_session_cost(&ctx).is_empty());
    }

    #[test]
    fn day_over_day_silent_below_min_days() {
        let calls = vec![CallBuilder::new("claude-code", "x", "alpha")
            .at(0)
            .cost(50.0)
            .build()];
        let baselines = Baselines::build(&calls, now());
        let ctx = RuleContext {
            calls: &calls,
            limits: &[],
            baselines: &baselines,
            price_table: PriceTable::embedded(),
            now: now(),
        };
        assert!(day_over_day_spend_zscore(&ctx).is_empty());
    }

    #[test]
    fn project_mom_growth_silent_when_under_threshold() {
        let mut calls = Vec::new();
        for i in 0..15 {
            calls.push(
                CallBuilder::new("claude-code", "x", "alpha")
                    .session(&format!("c-{i}"))
                    .at(2)
                    .cost(1.0)
                    .build(),
            );
            calls.push(
                CallBuilder::new("claude-code", "x", "alpha")
                    .session(&format!("p-{i}"))
                    .at(40)
                    .cost(1.0)
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
        assert!(project_mom_growth(&ctx).is_empty());
    }
}
