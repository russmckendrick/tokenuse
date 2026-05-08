use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::copy::{copy, template, CopyDeck};
use crate::pricing::PriceTable;
use crate::tools::{LimitSnapshot, ParsedCall};

pub mod baselines;
#[cfg(test)]
pub mod fixtures;
pub mod recommendation;
pub mod rules;

pub use baselines::Baselines;
pub use recommendation::{
    Category, InsightsBundle, Recommendation, RuleId, SavingsBasis, Scope, Severity,
};

use rules::{run_all, RuleContext};

#[derive(Debug, Clone, Serialize)]
pub struct InsightsView {
    pub generated_at: DateTime<Utc>,
    pub baseline_window_days: u32,
    pub summary: InsightsSummary,
    pub recommendations: Vec<RecommendationView>,
}

#[derive(Debug, Clone, Serialize)]
pub struct InsightsSummary {
    pub total_est_savings_usd: f64,
    pub total_est_savings: String,
    pub by_category: Vec<CategoryCount>,
    pub by_severity: Vec<SeverityCount>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CategoryCount {
    pub id: &'static str,
    pub label: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct SeverityCount {
    pub id: &'static str,
    pub label: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecommendationView {
    pub id: String,
    pub rule_id: &'static str,
    pub category: &'static str,
    pub category_label: String,
    pub severity: &'static str,
    pub severity_label: String,
    pub title: String,
    pub body: String,
    pub assumption: Option<String>,
    pub savings: Option<String>,
    pub savings_amount_usd: Option<f64>,
    pub scope: ScopeView,
    pub silenced_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScopeView {
    pub kind: &'static str,
    pub label: Option<String>,
    pub project: Option<String>,
    pub session: Option<String>,
    pub tool: Option<String>,
    pub model: Option<String>,
}

impl ScopeView {
    fn from_scope(scope: &Scope) -> Self {
        match scope {
            Scope::All => Self {
                kind: "all",
                label: None,
                project: None,
                session: None,
                tool: None,
                model: None,
            },
            Scope::Project { name } => Self {
                kind: "project",
                label: Some(name.clone()),
                project: Some(name.clone()),
                session: None,
                tool: None,
                model: None,
            },
            Scope::ProjectModel { project, model } => Self {
                kind: "project_model",
                label: Some(format!("{project} · {model}")),
                project: Some(project.clone()),
                session: None,
                tool: None,
                model: Some(model.clone()),
            },
            Scope::Tool { tool } => Self {
                kind: "tool",
                label: Some(tool.clone()),
                project: None,
                session: None,
                tool: Some(tool.clone()),
                model: None,
            },
            Scope::Session { id, project } => Self {
                kind: "session",
                label: Some(format!("{project} · {id}")),
                project: Some(project.clone()),
                session: Some(id.clone()),
                tool: None,
                model: None,
            },
        }
    }
}

pub fn compute_insights(
    calls: &[ParsedCall],
    limits: &[LimitSnapshot],
    price_table: &PriceTable,
    now: DateTime<Utc>,
) -> InsightsBundle {
    let baselines = Baselines::build(calls, now);
    let ctx = RuleContext {
        calls,
        limits,
        baselines: &baselines,
        price_table,
        now,
    };
    let mut bundle = InsightsBundle::empty();
    bundle.extend(run_all(&ctx));
    bundle.finalise()
}

pub fn build_view(bundle: &InsightsBundle, now: DateTime<Utc>) -> InsightsView {
    let copy = copy();
    let recommendations: Vec<RecommendationView> = bundle
        .recommendations
        .iter()
        .map(|rec| render_recommendation(rec, copy))
        .collect();

    let total_est_savings_usd: f64 = recommendations
        .iter()
        .filter_map(|r| r.savings_amount_usd)
        .sum();

    let summary = InsightsSummary {
        total_est_savings_usd,
        total_est_savings: format_money_total(total_est_savings_usd),
        by_category: build_category_counts(&recommendations, copy),
        by_severity: build_severity_counts(&recommendations, copy),
    };

    InsightsView {
        generated_at: now,
        baseline_window_days: 30,
        summary,
        recommendations,
    }
}

fn render_recommendation(rec: &Recommendation, copy: &CopyDeck) -> RecommendationView {
    let key = rec.rule_id.key();
    let rule = copy.insights.rules.get(key);
    let category = rec.rule_id.category();
    let category_label = copy
        .insights
        .categories
        .get(category.id())
        .cloned()
        .unwrap_or_else(|| category.id().to_string());
    let severity_label = copy
        .insights
        .severity
        .get(rec.severity.id())
        .cloned()
        .unwrap_or_else(|| rec.severity.id().to_string());

    let body_args_borrow: Vec<(&str, String)> =
        rec.body_args.iter().map(|(k, v)| (*k, v.clone())).collect();
    let title = rule
        .map(|r| template(&r.title, &body_args_borrow))
        .unwrap_or_else(|| key.to_string());
    let body = rule
        .map(|r| template(&r.body, &body_args_borrow))
        .unwrap_or_default();
    let assumption = rule
        .and_then(|r| r.assumption.as_ref())
        .map(|a| template(a, &body_args_borrow));

    let savings = rec.est_savings_usd.and_then(|amount| {
        let basis = rec.est_savings_basis?;
        let template_str = match basis {
            SavingsBasis::PerWeek => &copy.insights.savings.per_week,
            SavingsBasis::PerMonth => &copy.insights.savings.per_month,
            SavingsBasis::OneOff => &copy.insights.savings.one_off,
        };
        Some(template(
            template_str,
            &[("amount", format_money_total(amount))],
        ))
    });

    let silenced_reason = rec.silenced_reason_key.and_then(|key| {
        let template_str = copy.insights.silenced.get(key)?;
        let args: Vec<(&str, String)> = rec
            .silenced_reason_args
            .iter()
            .map(|(k, v)| (*k, v.clone()))
            .collect();
        Some(template(template_str, &args))
    });

    RecommendationView {
        id: rec.id(),
        rule_id: key,
        category: category.id(),
        category_label,
        severity: rec.severity.id(),
        severity_label,
        title,
        body,
        assumption,
        savings,
        savings_amount_usd: rec.est_savings_usd,
        scope: ScopeView::from_scope(&rec.scope),
        silenced_reason,
    }
}

fn build_category_counts(recs: &[RecommendationView], copy: &CopyDeck) -> Vec<CategoryCount> {
    Category::ALL
        .iter()
        .map(|category| {
            let id = category.id();
            let label = copy
                .insights
                .categories
                .get(id)
                .cloned()
                .unwrap_or_else(|| id.to_string());
            let count = recs.iter().filter(|r| r.category == id).count();
            CategoryCount { id, label, count }
        })
        .collect()
}

fn build_severity_counts(recs: &[RecommendationView], copy: &CopyDeck) -> Vec<SeverityCount> {
    [Severity::Risk, Severity::Warn, Severity::Info]
        .iter()
        .map(|severity| {
            let id = severity.id();
            let label = copy
                .insights
                .severity
                .get(id)
                .cloned()
                .unwrap_or_else(|| id.to_string());
            let count = recs.iter().filter(|r| r.severity == id).count();
            SeverityCount { id, label, count }
        })
        .collect()
}

fn format_money_total(amount: f64) -> String {
    if amount >= 100.0 {
        format!("${amount:.0}")
    } else if amount >= 10.0 {
        format!("${amount:.1}")
    } else {
        format!("${amount:.2}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::insights::fixtures::{now, CallBuilder};
    use crate::pricing::PriceTable;

    #[test]
    fn compute_insights_empty_for_no_data() {
        let bundle = compute_insights(&[], &[], PriceTable::embedded(), now());
        assert!(bundle.recommendations.is_empty());
    }

    #[test]
    fn build_view_renders_summary_zero_savings() {
        let bundle = InsightsBundle::empty();
        let view = build_view(&bundle, now());
        assert!(view.recommendations.is_empty());
        assert_eq!(view.summary.total_est_savings_usd, 0.0);
        assert_eq!(view.summary.by_category.len(), 4);
        assert_eq!(view.summary.by_severity.len(), 3);
    }

    #[test]
    fn rule_id_keys_are_unique_and_contiguous() {
        let mut keys: Vec<_> = RuleId::ALL.iter().map(|r| r.key()).collect();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), RuleId::ALL.len());
    }

    #[test]
    fn every_rule_has_copy_keys() {
        let copy = crate::copy::copy();
        for rule in RuleId::ALL {
            let key = rule.key();
            let entry = copy
                .insights
                .rules
                .get(key)
                .unwrap_or_else(|| panic!("missing insights.rules.{key}"));
            assert!(!entry.title.is_empty(), "{key}.title is empty");
            assert!(!entry.body.is_empty(), "{key}.body is empty");
        }
    }

    #[test]
    fn no_cache_data_silenced_template_exists() {
        let copy = crate::copy::copy();
        assert!(copy.insights.silenced.contains_key("no_cache_data"));
    }

    #[test]
    fn ordering_prefers_severity_then_savings() {
        let mut bundle = InsightsBundle::empty();
        bundle.push(Recommendation {
            rule_id: RuleId::ShortOutputSonnet,
            severity: Severity::Info,
            body_args: vec![],
            est_savings_usd: Some(100.0),
            est_savings_basis: Some(SavingsBasis::PerWeek),
            scope: Scope::All,
            silenced_reason_key: None,
            silenced_reason_args: vec![],
        });
        bundle.push(Recommendation {
            rule_id: RuleId::ClaudeWeeklyForecast,
            severity: Severity::Risk,
            body_args: vec![],
            est_savings_usd: None,
            est_savings_basis: None,
            scope: Scope::All,
            silenced_reason_key: None,
            silenced_reason_args: vec![],
        });
        let bundle = bundle.finalise();
        assert_eq!(bundle.recommendations[0].severity, Severity::Risk);
    }

    #[test]
    fn synthetic_run_renders_view() {
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
        let bundle = compute_insights(&calls, &[], PriceTable::embedded(), now());
        let view = build_view(&bundle, now());
        assert!(!view.recommendations.is_empty());
        let r = &view.recommendations[0];
        assert!(!r.title.is_empty(), "title should be non-empty: {:?}", r);
        assert!(r.savings.is_some());
    }
}
