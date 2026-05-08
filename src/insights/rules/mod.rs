use chrono::{DateTime, Utc};

use crate::pricing::PriceTable;
use crate::tools::{LimitSnapshot, ParsedCall};

use super::baselines::Baselines;
use super::recommendation::Recommendation;

pub mod anomalies;
pub mod cache;
pub mod model_rightsizing;
pub mod quota;

pub struct RuleContext<'a> {
    pub calls: &'a [ParsedCall],
    pub limits: &'a [LimitSnapshot],
    pub baselines: &'a Baselines,
    pub price_table: &'a PriceTable,
    pub now: DateTime<Utc>,
}

pub fn run_all(ctx: &RuleContext<'_>) -> Vec<Recommendation> {
    let mut out = Vec::new();
    out.extend(model_rightsizing::short_output_sonnet(ctx));
    out.extend(model_rightsizing::fast_mode_opus_excess(ctx));
    out.extend(model_rightsizing::reasoning_heavy_o_series(ctx));
    out.extend(cache::cache_hit_trend_drop(ctx));
    out.extend(cache::cache_write_overhead(ctx));
    out.extend(cache::low_hit_project_outlier(ctx));
    out.extend(cache::silenced_tools(ctx));
    out.extend(anomalies::outlier_session_cost(ctx));
    out.extend(anomalies::day_over_day_spend_zscore(ctx));
    out.extend(anomalies::project_mom_growth(ctx));
    out.extend(quota::claude_weekly_forecast(ctx));
    out.extend(quota::copilot_premium_pacing(ctx));
    out.extend(quota::limit_recently_hit(ctx));
    out
}
