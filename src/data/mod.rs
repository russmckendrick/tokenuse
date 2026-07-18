use std::sync::OnceLock;

use chrono::{Datelike, Duration, Local, NaiveDate, Timelike};
use serde::{Deserialize, Serialize};

use crate::currency::CurrencyFormatter;
use crate::{
    app::{ModelFilter, Period, ProjectFilter, SortMode, Tool},
    copy::copy,
};

#[derive(Debug, Clone, Serialize)]
pub struct DashboardData {
    pub summary: Summary,
    pub daily: Vec<DailyMetric>,
    pub activity_timeline: Vec<ActivityMetric>,
    pub projects: Vec<ProjectMetric>,
    pub project_tools: Vec<ProjectToolMetric>,
    pub sessions: Vec<SessionMetric>,
    pub models: Vec<ModelMetric>,
    pub tools: Vec<CountMetric>,
    pub commands: Vec<CountMetric>,
    pub mcp_servers: Vec<CountMetric>,
    /// Deterministic task categories (coding, debugging, exploration, …)
    /// classified per call from tool usage and the stored prompt prefix.
    pub by_activity: Vec<ActivityMetric>,
    /// Distinct `tool · model` pairs in the visible period whose pricing fell
    /// through to the book's fallback model — usually proxy-renamed models
    /// that need an alias or override before their cost is real.
    pub fallback_priced_models: Vec<&'static str>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LimitsData {
    pub sections: Vec<ToolLimitSection>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolLimitSection {
    pub tool: &'static str,
    pub limits: Vec<LimitMetric>,
    pub usage: RecentUsageMetric,
    pub models: Vec<RecentModelMetric>,
    pub plan_value: Option<PlanValueMetric>,
}

/// API-equivalent calendar-month spend against the tool's subscription
/// price: what this month's tokens would have cost at API rates versus what
/// the plan actually costs. Present only when a price is known (configured
/// in `UserConfig::plan_prices` or derived from a detected plan SKU).
#[derive(Debug, Clone, Serialize)]
pub struct PlanValueMetric {
    pub price: &'static str,
    pub month_cost: &'static str,
    pub multiple: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct LimitMetric {
    pub tool: &'static str,
    pub scope: &'static str,
    pub window: &'static str,
    pub used: u64,
    pub left: &'static str,
    pub reset: &'static str,
    pub plan: &'static str,
    pub used_credits: Option<f64>,
    pub remaining_credits: Option<f64>,
    pub total_credits: Option<f64>,
    pub additional_usage: Option<bool>,
    pub stale: bool,
    pub as_of: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecentUsageMetric {
    pub buckets: [u64; 24],
    pub calls: u64,
    pub tokens: &'static str,
    pub cost: &'static str,
    pub last_seen: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecentModelMetric {
    pub name: &'static str,
    pub provider: &'static str,
    pub calls: u64,
    pub tokens: &'static str,
    pub cost: &'static str,
    pub value: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Summary {
    pub cost: &'static str,
    pub calls: &'static str,
    pub sessions: &'static str,
    pub cache_hit: &'static str,
    pub input: &'static str,
    pub output: &'static str,
    pub cached: &'static str,
    pub written: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct DailyMetric {
    pub day: &'static str,
    pub cost: &'static str,
    pub calls: u64,
    pub value: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ActivityMetric {
    pub label: &'static str,
    pub cost: &'static str,
    pub calls: u64,
    pub value: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectMetric {
    pub name: &'static str,
    pub cost: &'static str,
    pub avg_per_session: &'static str,
    pub sessions: u64,
    pub tool_mix: &'static str,
    pub value: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectToolMetric {
    pub project: &'static str,
    pub tool: &'static str,
    pub cost: &'static str,
    pub calls: u64,
    pub sessions: u64,
    pub avg_per_session: &'static str,
    pub value: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionMetric {
    /// Drill-down key accepted by `session_detail`; empty when the session
    /// cannot be resolved (a row is then display-only).
    pub key: &'static str,
    pub date: &'static str,
    pub project: &'static str,
    pub cost: &'static str,
    pub calls: u64,
    pub value: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelMetric {
    /// Registry fold key; rows navigate to the per-model page with it.
    pub canonical_id: &'static str,
    pub name: &'static str,
    pub provider: &'static str,
    pub provider_label: &'static str,
    pub family: &'static str,
    pub cost: &'static str,
    pub cache: &'static str,
    pub cache_rate: &'static str,
    pub calls: u64,
    pub value: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct CountMetric {
    pub name: &'static str,
    pub calls: u64,
    pub value: u64,
}

/// One canonical model aggregated across every tool for a period, with the
/// per-tool split that produced it. Powers the unified model catalog view.
#[derive(Debug, Clone, Serialize)]
pub struct ModelCatalogEntry {
    pub canonical_id: &'static str,
    pub name: &'static str,
    pub provider: &'static str,
    pub provider_label: &'static str,
    pub family: &'static str,
    pub cost: &'static str,
    pub calls: u64,
    pub tokens: &'static str,
    pub cache_hit: &'static str,
    pub value: u64,
    pub per_tool: Vec<ModelToolBreakdown>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelToolBreakdown {
    pub tool: &'static str,
    pub tool_label: &'static str,
    pub cost: &'static str,
    pub calls: u64,
    pub value: u64,
}

/// Raw token-bucket totals for one model plus compact display labels, so the
/// UI can chart shares without parsing formatted strings.
#[derive(Debug, Clone, Serialize)]
pub struct TokenComposition {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    pub input_label: &'static str,
    pub output_label: &'static str,
    pub cache_read_label: &'static str,
    pub cache_write_label: &'static str,
}

/// Effective pricing for one model in the visible period: per-Mtok rates,
/// cache-rate labels, and the blended average cost per call.
#[derive(Debug, Clone, Serialize)]
pub struct ModelPricingInfo {
    pub input_per_mtok: &'static str,
    pub output_per_mtok: &'static str,
    pub cache_read_per_mtok: &'static str,
    pub cache_write_per_mtok: &'static str,
    pub cache_read_rate: &'static str,
    pub cache_write_rate: &'static str,
    pub avg_cost_per_call: &'static str,
    /// True when any (tool, raw model) pair in scope was priced via the
    /// book's fallback row — the cost is a guess until an alias lands.
    pub fallback: bool,
}

/// Model-page extras that a scoped dashboard cannot provide: the token
/// composition split and the model's effective pricing.
#[derive(Debug, Clone, Serialize)]
pub struct ModelPageDetail {
    pub composition: TokenComposition,
    pub pricing: ModelPricingInfo,
}

/// Time-explorer aggregates for the desktop Analytics page.
#[derive(Debug, Clone, Serialize)]
pub struct AnalyticsData {
    pub daily_by_tool: Vec<StackedDayMetric>,
    /// Activity tokens by weekday (Monday = 0) and local hour of day.
    pub hour_day: [[u64; 24]; 7],
    pub provider_share: Vec<ShareMetric>,
    pub tool_share: Vec<ShareMetric>,
}

/// Coach page payload: overall grade, practice scores, findings, flow/pace
/// summaries, AI code output, and the activity calendar behind the timeline
/// day picker. All wording is resolved client-side from copy.json via the
/// ids carried here.
#[derive(Debug, Clone, Serialize)]
pub struct CoachData {
    pub overall: CoachOverall,
    pub practice_groups: Vec<PracticeGroupScore>,
    pub findings: Vec<CoachFinding>,
    /// Advisory configuration findings (unused MCP servers, CLAUDE.md bloat,
    /// wasteful reads). Deliberately unscored: they describe the setup as it
    /// is now, so they never move the practice grade.
    pub setup: Vec<CoachSetupFinding>,
    pub flow: FlowSummary,
    pub pace: PaceSummary,
    pub output: OutputSummary,
    /// Turns per active day for the activity calendar, oldest first, spanning
    /// the period's calendar window (a trailing ~9-week context for scoped
    /// periods, the trailing year for All Time); days without activity are
    /// omitted and rendered empty client-side.
    pub timeline_grid: Vec<TimelineGridDay>,
    /// Per-project activity detail for the Activity Projects view; joined
    /// client-side onto the dashboard project rows by short label.
    pub projects: Vec<CoachProjectActivity>,
}

/// One project's activity profile: estimated active time, request turns,
/// tech stack, most-edited files, and a coarse work-pattern classification.
#[derive(Debug, Clone, Serialize)]
pub struct CoachProjectActivity {
    /// Dashboard-consistent short label (joins `DashboardData::projects`).
    pub name: &'static str,
    /// Formatted block-based active time ("14.2h", "45m").
    pub active_hours: &'static str,
    pub turns: u64,
    /// Code-output languages by LoC, descending - the observed tech stack.
    pub languages: Vec<CountMetric>,
    /// Most-edited file paths, descending by edit count.
    pub hot_files: Vec<&'static str>,
    /// Copy id for the weekday/weekend mix ("mostly_weekdays" |
    /// "mostly_weekends" | "mixed_days"); empty without timestamps.
    pub days_id: &'static str,
    /// Copy id for the dominant daypart ("mornings" | "afternoons" |
    /// "evenings" | "late_nights"); empty when none dominates.
    pub time_id: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct TimelineGridDay {
    pub day: &'static str,
    pub turns: u64,
    /// False for context days outside the selected period (rendered dimmed).
    pub in_period: bool,
}

/// Composite report-card grade: rules-weighted mean of the group scores.
#[derive(Debug, Clone, Serialize)]
pub struct CoachOverall {
    pub score: u64,
    /// Grade id ("a_plus".."f"); copy maps ids to letters.
    pub grade_id: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct PracticeGroupScore {
    pub id: &'static str,
    pub score: u64,
    /// Grade id ("a_plus".."f"); copy maps ids to letters.
    pub grade_id: &'static str,
    /// Pre-formatted week-over-week / month-over-month deltas ("+13%", "–").
    pub wow: &'static str,
    pub mom: &'static str,
    /// Weekly score series, oldest-first, for the trend sparkline.
    pub trend: Vec<u64>,
    pub triggered: u64,
    pub total_rules: u64,
    /// Rule id of the heaviest triggered rule; empty when clean.
    pub top_rule_id: &'static str,
}

/// One advisory setup finding with a heuristic token-savings estimate.
#[derive(Debug, Clone, Serialize)]
pub struct CoachSetupFinding {
    pub id: &'static str,
    pub title: &'static str,
    pub detail: &'static str,
    pub savings_tokens: u64,
    /// Pre-formatted estimate, e.g. "~1.2M tokens".
    pub savings_label: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct CoachFinding {
    pub rule_id: &'static str,
    pub group: &'static str,
    pub severity: &'static str,
    pub occurrences: u64,
    pub total: u64,
    /// Pre-formatted percentage ("42%"); empty when not meaningful.
    pub pct: &'static str,
    /// Rule-specific stat for the {stat} copy slot; empty when unused.
    pub stat: &'static str,
    pub examples: Vec<FindingExample>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FindingExample {
    pub text: &'static str,
    pub detail: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct FlowSummary {
    pub overall_score: u64,
    pub label_id: &'static str,
    pub avg_followup: &'static str,
    pub avg_block: &'static str,
    pub deep_days: u64,
    pub fragmented_days: u64,
    pub total_days: u64,
    pub days: Vec<FlowDayMetric>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FlowDayMetric {
    pub day: &'static str,
    pub score: u64,
    pub label_id: &'static str,
    pub longest_block_min: u64,
    pub active_min: u64,
    pub sessions: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct PaceSummary {
    pub current_streak: u64,
    pub longest_streak: u64,
    pub late_night_pct: u64,
    pub weekend_pct: u64,
    pub risk_id: &'static str,
    pub alert_ids: Vec<&'static str>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OutputSummary {
    pub total_loc: &'static str,
    pub by_language: Vec<CountMetric>,
    /// name = local day, calls = LoC.
    pub by_day: Vec<CountMetric>,
    /// Period-aware output trend: half-hourly for 24 hours, hourly for seven
    /// days, and daily for longer ranges. Newest bucket first.
    pub trend: Vec<CountMetric>,
    pub by_project: Vec<CountMetric>,
    pub by_model: Vec<CountMetric>,
    /// Comma-separated tool labels with no code-output signal; empty when
    /// every tool contributes.
    pub uncovered_tools: &'static str,
}

/// One day's session Gantt for the Coach page timeline panel.
#[derive(Debug, Clone, Serialize)]
pub struct CoachTimelineDay {
    pub day: String,
    pub max_concurrent: u64,
    /// Minutes since local midnight bounding the day's activity.
    pub window_start_min: u64,
    pub window_end_min: u64,
    /// Formatted spend across the day's sessions.
    pub total_cost: String,
    pub rows: Vec<TimelineSessionRow>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TimelineSessionRow {
    pub session_key: String,
    pub project: String,
    pub tool: String,
    pub tool_label: String,
    pub turns: u64,
    pub cost: String,
    pub blocks: Vec<TimelineBlock>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TimelineBlock {
    pub start_min: u64,
    pub end_min: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct StackedDayMetric {
    pub day: &'static str,
    pub total_cost: &'static str,
    pub segments: Vec<StackSegment>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StackSegment {
    pub tool: &'static str,
    pub tool_label: &'static str,
    pub cost: &'static str,
    /// Stacking magnitude in 1/10000 USD so segments compare across days.
    pub amount: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ShareMetric {
    pub key: &'static str,
    pub label: &'static str,
    pub cost: &'static str,
    pub calls: u64,
    /// Share of the total in permille (0-1000).
    pub share: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProjectOption {
    pub identity: Option<String>,
    pub label: String,
    pub cost: String,
    pub calls: u64,
}

/// One row of the model picker: `canonical_id` is `None` for the "All" row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ModelOption {
    pub canonical_id: Option<String>,
    pub label: String,
    pub cost: String,
    pub calls: u64,
}

/// One tool's slice of a project's spend, with numeric magnitudes so charts
/// can draw proportions without re-parsing formatted currency strings.
#[derive(Debug, Clone, Serialize)]
pub struct ProjectToolSplit {
    /// Tool id ("claude-code") for chart colors; `label` is the display name.
    pub key: &'static str,
    pub label: &'static str,
    pub cost: String,
    pub avg_per_session: String,
    pub calls: u64,
    pub sessions: u64,
    /// Currency-invariant magnitudes used only for chart proportions.
    pub cost_value: f64,
    pub avg_value: f64,
}

/// One row of the full Projects index page: every project in scope, uncapped,
/// with the normalized identity retained for the drill-down route.
#[derive(Debug, Clone, Serialize)]
pub struct ProjectIndexRow {
    pub identity: String,
    pub name: String,
    pub cost: String,
    pub avg_per_session: String,
    pub sessions: u64,
    pub calls: u64,
    pub last_active: String,
    pub tool_mix: String,
    pub value: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionOption {
    pub key: String,
    pub date: String,
    pub project: String,
    pub tool: &'static str,
    pub cost: String,
    pub calls: u64,
    pub value: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionDetail {
    pub timestamp: String,
    pub model: String,
    pub cost: String,
    pub cache_read_rate: String,
    pub cache_write_rate: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    pub reasoning_tokens: u64,
    pub web_search_requests: u64,
    pub tools: String,
    pub interaction_mode: String,
    pub token_quality: String,
    pub timestamp_quality: String,
    pub bash_commands: Vec<String>,
    pub prompt: String,
    pub prompt_full: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionDetailView {
    pub key: String,
    pub session_id: String,
    pub project: String,
    pub tool: &'static str,
    pub date_range: String,
    pub total_cost: String,
    pub total_calls: u64,
    pub total_input: String,
    pub total_output: String,
    pub total_cache_read: String,
    pub calls: Vec<SessionDetail>,
    pub note: Option<String>,
}

#[derive(Debug, Deserialize)]
struct WireSampleData {
    periods: WireSamplePeriods,
    limits: WireLimitsData,
}

#[derive(Debug, Deserialize)]
struct WireSamplePeriods {
    today: WireDashboardData,
    week: WireDashboardData,
    thirty_days: WireDashboardData,
    month: WireDashboardData,
    all_time: WireDashboardData,
}

#[derive(Debug, Deserialize)]
struct WireDashboardData {
    summary: WireSummary,
    daily: Vec<WireDailyMetric>,
    projects: Vec<WireProjectMetric>,
    project_tools: Vec<WireProjectToolMetric>,
    sessions: Vec<WireSessionMetric>,
    models: Vec<WireModelMetric>,
    tools: Vec<WireCountMetric>,
    commands: Vec<WireCountMetric>,
    mcp_servers: Vec<WireCountMetric>,
}

#[derive(Debug, Deserialize)]
struct WireLimitsData {
    sections: Vec<WireToolLimitSection>,
}

#[derive(Debug, Deserialize)]
struct WireToolLimitSection {
    tool: String,
    limits: Vec<WireLimitMetric>,
    usage: WireRecentUsageMetric,
    models: Vec<WireRecentModelMetric>,
    #[serde(default)]
    plan_value: Option<WirePlanValueMetric>,
}

#[derive(Debug, Deserialize)]
struct WirePlanValueMetric {
    price: String,
    month_cost: String,
    multiple: String,
}

#[derive(Debug, Deserialize)]
struct WireLimitMetric {
    tool: String,
    scope: String,
    window: String,
    used: u64,
    left: String,
    reset: String,
    plan: String,
    #[serde(default)]
    used_credits: Option<f64>,
    #[serde(default)]
    remaining_credits: Option<f64>,
    #[serde(default)]
    total_credits: Option<f64>,
    #[serde(default)]
    additional_usage: Option<bool>,
    #[serde(default)]
    stale: bool,
    #[serde(default)]
    as_of: String,
}

#[derive(Debug, Deserialize)]
struct WireRecentUsageMetric {
    buckets: [u64; 24],
    calls: u64,
    tokens: String,
    cost: String,
    last_seen: String,
}

#[derive(Debug, Deserialize)]
struct WireRecentModelMetric {
    id: String,
    #[serde(default)]
    tool: String,
    calls: u64,
    tokens: String,
    cost: String,
    value: u64,
}

#[derive(Debug, Deserialize)]
struct WireSummary {
    cost: String,
    calls: String,
    sessions: String,
    cache_hit: String,
    input: String,
    output: String,
    cached: String,
    written: String,
}

#[derive(Debug, Deserialize)]
struct WireDailyMetric {
    day: String,
    cost: String,
    calls: u64,
    value: u64,
}

#[derive(Debug, Deserialize)]
struct WireProjectMetric {
    name: String,
    cost: String,
    avg_per_session: String,
    sessions: u64,
    tool_mix: String,
    value: u64,
}

#[derive(Debug, Deserialize)]
struct WireProjectToolMetric {
    project: String,
    tool: String,
    cost: String,
    calls: u64,
    sessions: u64,
    avg_per_session: String,
    value: u64,
}

#[derive(Debug, Deserialize)]
struct WireSessionMetric {
    date: String,
    project: String,
    cost: String,
    calls: u64,
    value: u64,
}

#[derive(Debug, Deserialize)]
struct WireModelMetric {
    id: String,
    #[serde(default)]
    tool: String,
    cost: String,
    cache: String,
    cache_rate: String,
    calls: u64,
    value: u64,
}

#[derive(Debug, Deserialize)]
struct WireCountMetric {
    name: String,
    calls: u64,
    value: u64,
}

struct SampleData {
    today: DashboardData,
    week: DashboardData,
    thirty_days: DashboardData,
    month: DashboardData,
    all_time: DashboardData,
    catalog_today: Vec<ModelCatalogEntry>,
    catalog_week: Vec<ModelCatalogEntry>,
    catalog_thirty_days: Vec<ModelCatalogEntry>,
    catalog_month: Vec<ModelCatalogEntry>,
    catalog_all_time: Vec<ModelCatalogEntry>,
    limits: LimitsData,
}

impl ProjectOption {
    pub fn all(cost: String, calls: u64) -> Self {
        Self {
            identity: None,
            label: copy().tools.all.clone(),
            cost,
            calls,
        }
    }

    pub fn selected(identity: String, label: String, cost: String, calls: u64) -> Self {
        Self {
            identity: Some(identity),
            label,
            cost,
            calls,
        }
    }
}

impl ModelOption {
    pub fn all(cost: String, calls: u64) -> Self {
        Self {
            canonical_id: None,
            label: copy().tools.all.clone(),
            cost,
            calls,
        }
    }

    pub fn selected(canonical_id: String, label: String, cost: String, calls: u64) -> Self {
        Self {
            canonical_id: Some(canonical_id),
            label,
            cost,
            calls,
        }
    }
}

fn sample_data() -> &'static SampleData {
    static SAMPLE: OnceLock<SampleData> = OnceLock::new();
    SAMPLE.get_or_init(|| {
        let wire: WireSampleData = serde_json::from_str(include_str!("sample_data.json"))
            .expect("embedded sample data must be valid JSON");
        SampleData::from(wire)
    })
}

impl From<WireSampleData> for SampleData {
    fn from(wire: WireSampleData) -> Self {
        let catalog_today = sample_catalog(&wire.periods.today.models);
        let catalog_week = sample_catalog(&wire.periods.week.models);
        let catalog_thirty_days = sample_catalog(&wire.periods.thirty_days.models);
        let catalog_month = sample_catalog(&wire.periods.month.models);
        let catalog_all_time = sample_catalog(&wire.periods.all_time.models);
        Self {
            today: wire.periods.today.into(),
            week: wire.periods.week.into(),
            thirty_days: wire.periods.thirty_days.into(),
            month: wire.periods.month.into(),
            all_time: wire.periods.all_time.into(),
            catalog_today,
            catalog_week,
            catalog_thirty_days,
            catalog_month,
            catalog_all_time,
            limits: wire.limits.into(),
        }
    }
}

/// Sample-mode catalog rows: wire model rows fold by canonical id exactly
/// like the live pipeline, so a model used from several tools carries a real
/// per-tool split (e.g. Sonnet under both Claude Code and Cursor).
fn sample_catalog(models: &[WireModelMetric]) -> Vec<ModelCatalogEntry> {
    struct Acc {
        identity: crate::models::ModelIdentity,
        cost_units: u64,
        calls: u64,
        cache: String,
        /// Cost units of the row the cache label came from, so the label of
        /// the dominant tool wins when tools disagree.
        cache_units: u64,
        per_tool: Vec<(String, u64, u64)>,
    }

    let mut rows: Vec<Acc> = Vec::new();
    for row in models {
        let identity = crate::models::resolve(&row.tool, &row.id);
        let units = parse_money_sort_value(&row.cost);
        match rows
            .iter_mut()
            .find(|acc| acc.identity.canonical_id == identity.canonical_id)
        {
            Some(acc) => {
                acc.cost_units += units;
                acc.calls += row.calls;
                if row.cache != "-" && units > acc.cache_units {
                    acc.cache = row.cache.clone();
                    acc.cache_units = units;
                }
                match acc.per_tool.iter_mut().find(|(tool, ..)| *tool == row.tool) {
                    Some(split) => {
                        split.1 += units;
                        split.2 += row.calls;
                    }
                    None => acc.per_tool.push((row.tool.clone(), units, row.calls)),
                }
            }
            None => rows.push(Acc {
                cache_units: if row.cache == "-" { 0 } else { units },
                cache: row.cache.clone(),
                per_tool: vec![(row.tool.clone(), units, row.calls)],
                identity,
                cost_units: units,
                calls: row.calls,
            }),
        }
    }

    rows.sort_by(|a, b| {
        b.cost_units
            .cmp(&a.cost_units)
            .then_with(|| a.identity.display.cmp(&b.identity.display))
    });
    let max_units = rows.first().map(|acc| acc.cost_units).unwrap_or(0).max(1);
    rows.into_iter()
        .map(|mut acc| {
            acc.per_tool
                .sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
            let split_max = acc
                .per_tool
                .first()
                .map(|split| split.1)
                .unwrap_or(0)
                .max(1);
            let per_tool = acc
                .per_tool
                .iter()
                .map(|(tool, units, calls)| ModelToolBreakdown {
                    tool: leak(tool.clone()),
                    tool_label: crate::ingest::projects::tool_short_label(tool),
                    cost: leak(format!("${:.2}", *units as f64 / 10_000.0)),
                    calls: *calls,
                    value: (units * 100 / split_max).clamp(1, 100),
                })
                .collect();
            ModelCatalogEntry {
                canonical_id: leak(acc.identity.canonical_id),
                name: leak(acc.identity.display),
                provider: acc.identity.provider.id(),
                provider_label: acc.identity.provider.label(),
                family: leak(acc.identity.family),
                cost: leak(format!("${:.2}", acc.cost_units as f64 / 10_000.0)),
                calls: acc.calls,
                tokens: "-",
                cache_hit: leak(if acc.cache_units == 0 {
                    "-".to_string()
                } else {
                    acc.cache.clone()
                }),
                value: (acc.cost_units * 100 / max_units).clamp(1, 100),
                per_tool,
            }
        })
        .collect()
}

/// The bundled catalog rows for one period, in USD.
fn sample_catalog_for(period: Period) -> &'static [ModelCatalogEntry] {
    let samples = sample_data();
    match period {
        Period::Today => &samples.catalog_today,
        Period::Week => &samples.catalog_week,
        Period::ThirtyDays => &samples.catalog_thirty_days,
        Period::Month => &samples.catalog_month,
        Period::AllTime => &samples.catalog_all_time,
    }
}

/// Sample-mode analytics: daily totals split across tools by the period's
/// tool proportions, a plausible work-hours heat pattern, and shares parsed
/// back out of the sample money strings.
pub fn analytics_data(
    period: Period,
    tool: Tool,
    project_filter: &ProjectFilter,
    currency: &CurrencyFormatter,
) -> AnalyticsData {
    // Aggregate in USD, then format each derived amount once with the real
    // currency, so converted sample strings are never parsed and re-converted.
    let usd = CurrencyFormatter::usd();
    let data = dashboard_data(
        period,
        tool,
        project_filter,
        &ModelFilter::All,
        SortMode::Spend,
        &usd,
    );

    let mut tool_totals: Vec<(&'static str, &'static str, u64, u64)> = Vec::new();
    for row in &data.project_tools {
        let amount = parse_money_sort_value(row.cost);
        match tool_totals
            .iter_mut()
            .find(|(label, ..)| *label == row.tool)
        {
            Some(entry) => {
                entry.2 += amount;
                entry.3 += row.calls;
            }
            None => tool_totals.push((row.tool, row.tool, amount, row.calls)),
        }
    }
    tool_totals.sort_by_key(|t| std::cmp::Reverse(t.2));
    let tool_total_amount: u64 = tool_totals.iter().map(|t| t.2).sum();

    let daily_by_tool = data
        .daily
        .iter()
        .rev()
        .map(|day| {
            let day_amount = parse_money_sort_value(day.cost);
            let segments = tool_totals
                .iter()
                .filter(|(_, _, amount, _)| *amount > 0)
                .map(|(tool_id, label, amount, _)| {
                    let share = (day_amount * amount)
                        .checked_div(tool_total_amount)
                        .unwrap_or(0);
                    StackSegment {
                        tool: tool_id,
                        tool_label: label,
                        cost: leak(currency.format_money(share as f64 / 10_000.0)),
                        amount: share,
                    }
                })
                .collect();
            StackedDayMetric {
                day: day.day,
                total_cost: leak(currency.format_money(day_amount as f64 / 10_000.0)),
                segments,
            }
        })
        .collect();

    let total_calls: u64 = data.daily.iter().map(|d| d.calls).sum();
    let mut hour_day = [[0u64; 24]; 7];
    for (weekday, row) in hour_day.iter_mut().enumerate() {
        let weekday_weight: u64 = if weekday < 5 { 10 } else { 3 };
        for (hour, cell) in row.iter_mut().enumerate() {
            let hour_weight: u64 = match hour {
                9..=11 | 14..=17 => 10,
                8 | 12 | 13 | 18 | 19 => 6,
                20..=22 => 3,
                _ => 1,
            };
            *cell = total_calls * weekday_weight * hour_weight / 100;
        }
    }

    let provider_share = share_rows(
        data.models.iter().map(|m| {
            (
                m.provider,
                m.provider_label,
                parse_money_sort_value(m.cost),
                m.calls,
            )
        }),
        currency,
    );
    let tool_share = share_rows(
        tool_totals
            .iter()
            .map(|(id, label, amount, calls)| (*id, *label, *amount, *calls)),
        currency,
    );

    AnalyticsData {
        daily_by_tool,
        hour_day,
        provider_share,
        tool_share,
    }
}

fn share_rows(
    rows: impl Iterator<Item = (&'static str, &'static str, u64, u64)>,
    currency: &CurrencyFormatter,
) -> Vec<ShareMetric> {
    let mut folded: Vec<(&'static str, &'static str, u64, u64)> = Vec::new();
    for (key, label, amount, calls) in rows {
        match folded.iter_mut().find(|(k, ..)| *k == key) {
            Some(entry) => {
                entry.2 += amount;
                entry.3 += calls;
            }
            None => folded.push((key, label, amount, calls)),
        }
    }
    folded.sort_by_key(|r| std::cmp::Reverse(r.2));
    let total: u64 = folded.iter().map(|r| r.2).sum();
    folded
        .into_iter()
        .map(|(key, label, amount, calls)| ShareMetric {
            key,
            label,
            cost: leak(currency.format_money(amount as f64 / 10_000.0)),
            calls,
            share: (amount * 1000).checked_div(total).unwrap_or(0),
        })
        .collect()
}

pub fn model_catalog_data(period: Period, currency: &CurrencyFormatter) -> Vec<ModelCatalogEntry> {
    let mut entries = sample_catalog_for(period).to_vec();
    if !currency.is_usd() {
        for entry in &mut entries {
            entry.cost = convert_money_text(entry.cost, currency, false);
            for split in &mut entry.per_tool {
                split.cost = convert_money_text(split.cost, currency, false);
            }
        }
    }
    entries
}

/// Sample-mode model-page extras: the token composition comes from the
/// model-scoped sample dashboard's summary buckets; pricing comes from the
/// real books via the model's per-tool ids, so synthetic sample ids surface
/// the genuine fallback note.
pub fn model_detail_data(
    period: Period,
    canonical_id: &str,
    currency: &CurrencyFormatter,
) -> Option<ModelPageDetail> {
    use std::collections::HashSet;

    let entry = sample_catalog_for(period)
        .iter()
        .find(|entry| entry.canonical_id == canonical_id)?;

    let filter = ModelFilter::Selected {
        canonical_id: canonical_id.to_string(),
        label: entry.name.to_string(),
    };
    let usd = CurrencyFormatter::usd();
    let data = dashboard_data(
        period,
        Tool::All,
        &ProjectFilter::All,
        &filter,
        SortMode::Spend,
        &usd,
    );
    let input = parse_compact_sort_value(data.summary.input);
    let output = parse_compact_sort_value(data.summary.output);
    let cache_read = parse_compact_sort_value(data.summary.cached);
    let cache_write = parse_compact_sort_value(data.summary.written);

    let per_mtok = |rate: f64| -> String {
        if rate <= 0.0 {
            "-".into()
        } else {
            currency.format_money(rate * 1_000_000.0)
        }
    };
    let uniform = |rates: HashSet<String>| -> String {
        if rates.is_empty() {
            "-".into()
        } else if rates.len() == 1 {
            rates.into_iter().next().unwrap_or_else(|| "-".into())
        } else {
            copy().metrics.mixed.clone()
        }
    };
    let mut input_rates = HashSet::new();
    let mut output_rates = HashSet::new();
    let mut cache_read_prices = HashSet::new();
    let mut cache_write_prices = HashSet::new();
    let mut cache_read_rates = HashSet::new();
    let mut cache_write_rates = HashSet::new();
    let mut fallback = false;
    for split in &entry.per_tool {
        let price = crate::pricing::price_for(split.tool, canonical_id, None);
        input_rates.insert(per_mtok(price.input));
        output_rates.insert(per_mtok(price.output));
        cache_read_prices.insert(per_mtok(price.cache_read));
        cache_write_prices.insert(per_mtok(price.cache_write));
        cache_read_rates.insert(crate::pricing::cache_read_rate_label_for(
            split.tool,
            canonical_id,
            None,
        ));
        cache_write_rates.insert(crate::pricing::cache_write_rate_label_for(
            split.tool,
            canonical_id,
            None,
        ));
        fallback |= crate::pricing::uses_fallback(split.tool, canonical_id, None);
    }

    let cost_usd = parse_money_sort_value(entry.cost) as f64 / 10_000.0;
    Some(ModelPageDetail {
        composition: TokenComposition {
            input,
            output,
            cache_read,
            cache_write,
            input_label: leak(format_compact(input)),
            output_label: leak(format_compact(output)),
            cache_read_label: leak(format_compact(cache_read)),
            cache_write_label: leak(format_compact(cache_write)),
        },
        pricing: ModelPricingInfo {
            input_per_mtok: leak(uniform(input_rates)),
            output_per_mtok: leak(uniform(output_rates)),
            cache_read_per_mtok: leak(uniform(cache_read_prices)),
            cache_write_per_mtok: leak(uniform(cache_write_prices)),
            cache_read_rate: leak(uniform(cache_read_rates)),
            cache_write_rate: leak(uniform(cache_write_rates)),
            avg_cost_per_call: leak(currency.format_money(cost_usd / entry.calls.max(1) as f64)),
            fallback,
        },
    })
}

impl From<WireDashboardData> for DashboardData {
    fn from(wire: WireDashboardData) -> Self {
        let daily: Vec<DailyMetric> = wire.daily.into_iter().map(Into::into).collect();
        Self {
            summary: wire.summary.into(),
            activity_timeline: daily.iter().map(ActivityMetric::from_daily).collect(),
            daily,
            projects: wire.projects.into_iter().map(Into::into).collect(),
            project_tools: wire.project_tools.into_iter().map(Into::into).collect(),
            sessions: wire.sessions.into_iter().map(Into::into).collect(),
            models: wire.models.into_iter().map(Into::into).collect(),
            tools: wire.tools.into_iter().map(Into::into).collect(),
            commands: wire.commands.into_iter().map(Into::into).collect(),
            mcp_servers: wire.mcp_servers.into_iter().map(Into::into).collect(),
            // `by_activity` is synthesized per period in `dashboard_data`
            // (see `sample_by_activity`); the bundled data carries no per-call
            // classification signals and never exercises the pricing fallback.
            by_activity: Vec::new(),
            fallback_priced_models: Vec::new(),
        }
    }
}

impl From<WireLimitsData> for LimitsData {
    fn from(wire: WireLimitsData) -> Self {
        Self {
            sections: wire.sections.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<WireToolLimitSection> for ToolLimitSection {
    fn from(wire: WireToolLimitSection) -> Self {
        Self {
            tool: leak(wire.tool),
            limits: wire.limits.into_iter().map(Into::into).collect(),
            usage: wire.usage.into(),
            models: wire.models.into_iter().map(Into::into).collect(),
            plan_value: wire.plan_value.map(Into::into),
        }
    }
}

impl From<WirePlanValueMetric> for PlanValueMetric {
    fn from(wire: WirePlanValueMetric) -> Self {
        Self {
            price: leak(wire.price),
            month_cost: leak(wire.month_cost),
            multiple: leak(wire.multiple),
        }
    }
}

impl From<WireLimitMetric> for LimitMetric {
    fn from(wire: WireLimitMetric) -> Self {
        Self {
            tool: leak(wire.tool),
            scope: leak(wire.scope),
            window: leak(wire.window),
            used: wire.used,
            left: leak(wire.left),
            reset: leak(wire.reset),
            plan: leak(wire.plan),
            used_credits: wire.used_credits,
            remaining_credits: wire.remaining_credits,
            total_credits: wire.total_credits,
            additional_usage: wire.additional_usage,
            stale: wire.stale,
            as_of: if wire.as_of.is_empty() {
                "-"
            } else {
                leak(wire.as_of)
            },
        }
    }
}

impl From<WireRecentUsageMetric> for RecentUsageMetric {
    fn from(wire: WireRecentUsageMetric) -> Self {
        Self {
            buckets: wire.buckets,
            calls: wire.calls,
            tokens: leak(wire.tokens),
            cost: leak(wire.cost),
            last_seen: leak(wire.last_seen),
        }
    }
}

impl From<WireRecentModelMetric> for RecentModelMetric {
    fn from(wire: WireRecentModelMetric) -> Self {
        let identity = crate::models::resolve(&wire.tool, &wire.id);
        Self {
            name: leak(identity.display),
            provider: identity.provider.id(),
            calls: wire.calls,
            tokens: leak(wire.tokens),
            cost: leak(wire.cost),
            value: wire.value,
        }
    }
}

impl From<WireSummary> for Summary {
    fn from(wire: WireSummary) -> Self {
        Self {
            cost: leak(wire.cost),
            calls: leak(wire.calls),
            sessions: leak(wire.sessions),
            cache_hit: leak(wire.cache_hit),
            input: leak(wire.input),
            output: leak(wire.output),
            cached: leak(wire.cached),
            written: leak(wire.written),
        }
    }
}

impl From<WireDailyMetric> for DailyMetric {
    fn from(wire: WireDailyMetric) -> Self {
        Self {
            day: leak(wire.day),
            cost: leak(wire.cost),
            calls: wire.calls,
            value: wire.value,
        }
    }
}

impl ActivityMetric {
    fn from_daily(row: &DailyMetric) -> Self {
        Self {
            label: row.day,
            cost: row.cost,
            calls: row.calls,
            value: row.value,
        }
    }
}

impl From<WireProjectMetric> for ProjectMetric {
    fn from(wire: WireProjectMetric) -> Self {
        Self {
            name: leak(wire.name),
            cost: leak(wire.cost),
            avg_per_session: leak(wire.avg_per_session),
            sessions: wire.sessions,
            tool_mix: leak(wire.tool_mix),
            value: wire.value,
        }
    }
}

impl From<WireProjectToolMetric> for ProjectToolMetric {
    fn from(wire: WireProjectToolMetric) -> Self {
        Self {
            project: leak(wire.project),
            tool: leak(wire.tool),
            cost: leak(wire.cost),
            calls: wire.calls,
            sessions: wire.sessions,
            avg_per_session: leak(wire.avg_per_session),
            value: wire.value,
        }
    }
}

impl From<WireSessionMetric> for SessionMetric {
    fn from(wire: WireSessionMetric) -> Self {
        Self {
            key: "",
            date: leak(wire.date),
            project: leak(wire.project),
            cost: leak(wire.cost),
            calls: wire.calls,
            value: wire.value,
        }
    }
}

impl From<WireModelMetric> for ModelMetric {
    fn from(wire: WireModelMetric) -> Self {
        let identity = crate::models::resolve(&wire.tool, &wire.id);
        Self {
            canonical_id: leak(identity.canonical_id),
            name: leak(identity.display),
            provider: identity.provider.id(),
            provider_label: identity.provider.label(),
            family: leak(identity.family),
            cost: leak(wire.cost),
            cache: leak(wire.cache),
            cache_rate: leak(wire.cache_rate),
            calls: wire.calls,
            value: wire.value,
        }
    }
}

impl From<WireCountMetric> for CountMetric {
    fn from(wire: WireCountMetric) -> Self {
        Self {
            name: leak(wire.name),
            calls: wire.calls,
            value: wire.value,
        }
    }
}

fn sample_base_date() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 4, 29).expect("sample base date is valid")
}

fn sample_date_delta() -> Duration {
    Local::now()
        .date_naive()
        .signed_duration_since(sample_base_date())
}

fn rebase_dashboard_dates(data: &mut DashboardData, base: NaiveDate, delta: Duration) {
    for row in &mut data.daily {
        if let Some(date) = parse_sample_day(row.day, base) {
            row.day = leak(format_sample_day(date + delta));
        }
    }
    for row in &mut data.activity_timeline {
        if let Some(date) = parse_sample_day(row.label, base) {
            row.label = leak(format_sample_day(date + delta));
        }
    }
    for row in &mut data.sessions {
        if let Ok(date) = NaiveDate::parse_from_str(row.date, "%Y-%m-%d") {
            row.date = leak((date + delta).format("%Y-%m-%d").to_string());
        }
    }
}

fn rebase_limit_dates(data: &mut LimitsData, base: NaiveDate, delta: Duration) {
    for section in &mut data.sections {
        for limit in &mut section.limits {
            if let Some(reset) = rebase_reset_text(limit.reset, base, delta) {
                limit.reset = leak(reset);
            }
        }
    }
}

fn sample_activity_timeline(rows: &[DailyMetric], period: Period) -> Vec<ActivityMetric> {
    if !period.uses_hourly_activity_timeline(Local::now()) {
        return rows.iter().map(ActivityMetric::from_daily).collect();
    }

    // Sample data is daily-only, so create a deterministic hourly contour to
    // exercise the same short-range graph density as live data.
    const HOURLY_SHAPE: [u64; 24] = [
        0, 0, 0, 0, 4, 12, 18, 8, 0, 6, 14, 10, 0, 0, 8, 24, 40, 30, 12, 0, 4, 14, 26, 10,
    ];
    let shape_total = HOURLY_SHAPE.iter().sum::<u64>().max(1);

    rows.iter()
        .flat_map(|row| {
            HOURLY_SHAPE.iter().enumerate().map(move |(hour, weight)| {
                let value = if row.value == 0 || *weight == 0 {
                    0
                } else {
                    (row.value * *weight).div_ceil(100).max(1)
                };
                ActivityMetric {
                    label: leak(format!("{} {:02}h", row.day, hour)),
                    cost: row.cost,
                    calls: row.calls.saturating_mul(*weight) / shape_total,
                    value,
                }
            })
        })
        .collect()
}

/// Sample task-category split for the By Activity view: distributes the
/// visible period total across a fixed set of categories so the panel renders
/// with the same wording (`TaskCategory::label`) and bar shape as live data.
/// Built in USD from the period summary; `apply_currency` converts the costs.
fn sample_by_activity(summary: &Summary) -> Vec<ActivityMetric> {
    use crate::categories::TaskCategory;
    const WEIGHTS: [(TaskCategory, u64); 9] = [
        (TaskCategory::Coding, 30),
        (TaskCategory::Debugging, 16),
        (TaskCategory::Feature, 14),
        (TaskCategory::Exploration, 12),
        (TaskCategory::Refactoring, 10),
        (TaskCategory::Testing, 8),
        (TaskCategory::Planning, 5),
        (TaskCategory::Git, 3),
        (TaskCategory::BuildDeploy, 2),
    ];
    let max_weight = 30;
    let total_units = parse_money_sort_value(summary.cost); // USD * 10_000
    let total_calls = parse_count(summary.calls);
    if total_units == 0 {
        return Vec::new();
    }
    WEIGHTS
        .iter()
        .map(|(category, weight)| ActivityMetric {
            label: category.label(),
            cost: leak(format!(
                "${:.2}",
                (total_units * weight / 100) as f64 / 10_000.0
            )),
            calls: total_calls * weight / 100,
            value: (weight * 100 / max_weight).clamp(1, 100),
        })
        .collect()
}

fn parse_sample_day(value: &str, base: NaiveDate) -> Option<NaiveDate> {
    let (month, day) = value.split_once('-')?;
    let month = month.parse::<u32>().ok()?;
    let day = day.parse::<u32>().ok()?;
    let mut date = NaiveDate::from_ymd_opt(base.year(), month, day)?;
    if date > base {
        date = NaiveDate::from_ymd_opt(base.year() - 1, month, day)?;
    }
    Some(date)
}

fn format_sample_day(date: NaiveDate) -> String {
    date.format("%m-%d").to_string()
}

fn rebase_reset_text(value: &str, base: NaiveDate, delta: Duration) -> Option<String> {
    let mut parts = value.split_whitespace();
    let day = parts.next()?.parse::<u32>().ok()?;
    let month = month_number(parts.next()?)?;
    let time = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    let date = NaiveDate::from_ymd_opt(base.year(), month, day)?;
    Some(format!(
        "{} {} {}",
        (date + delta).format("%d"),
        month_name(date + delta),
        time
    ))
}

fn month_number(name: &str) -> Option<u32> {
    match name {
        "Jan" => Some(1),
        "Feb" => Some(2),
        "Mar" => Some(3),
        "Apr" => Some(4),
        "May" => Some(5),
        "Jun" => Some(6),
        "Jul" => Some(7),
        "Aug" => Some(8),
        "Sep" => Some(9),
        "Oct" => Some(10),
        "Nov" => Some(11),
        "Dec" => Some(12),
        _ => None,
    }
}

fn month_name(date: NaiveDate) -> &'static str {
    match date.month() {
        1 => "Jan",
        2 => "Feb",
        3 => "Mar",
        4 => "Apr",
        5 => "May",
        6 => "Jun",
        7 => "Jul",
        8 => "Aug",
        9 => "Sep",
        10 => "Oct",
        11 => "Nov",
        12 => "Dec",
        _ => unreachable!("chrono months are always 1-12"),
    }
}

pub fn dashboard_data(
    period: Period,
    _tool: Tool,
    project_filter: &ProjectFilter,
    model_filter: &ModelFilter,
    sort: SortMode,
    currency: &CurrencyFormatter,
) -> DashboardData {
    let samples = sample_data();
    let mut data = match period {
        Period::Today => samples.today.clone(),
        Period::Week => samples.week.clone(),
        Period::ThirtyDays => samples.thirty_days.clone(),
        Period::Month => samples.month.clone(),
        Period::AllTime => samples.all_time.clone(),
    };
    rebase_dashboard_dates(&mut data, sample_base_date(), sample_date_delta());

    // Keys are minted before any filter so a retained row keeps its unscoped
    // index: `session_detail` always resolves against the unscoped list
    // (matching live semantics, where a session drill-down shows the whole
    // session regardless of the active filters).
    for (idx, session) in data.sessions.iter_mut().enumerate() {
        session.key = leak(format!("sample:{idx}"));
    }

    apply_project_filter(&mut data, project_filter);
    // Model scoping runs before the timeline and by-activity synthesis so
    // both derive from the scoped daily rows and summary.
    apply_model_filter(&mut data, model_filter, period);
    data.activity_timeline = sample_activity_timeline(&data.daily, period);
    data.by_activity = sample_by_activity(&data.summary);
    apply_sample_sort(&mut data, sort);
    apply_currency(&mut data, currency);

    data
}

pub fn project_options(
    period: Period,
    tool: Tool,
    sort: SortMode,
    currency: &CurrencyFormatter,
) -> Vec<ProjectOption> {
    let data = dashboard_data(
        period,
        tool,
        &ProjectFilter::All,
        &ModelFilter::All,
        sort,
        currency,
    );
    let mut options = vec![ProjectOption::all(
        data.summary.cost.into(),
        parse_count(data.summary.calls),
    )];

    options.extend(data.projects.iter().map(|project| {
        let calls = data
            .project_tools
            .iter()
            .filter(|row| row.project == project.name)
            .map(|row| row.calls)
            .sum();
        ProjectOption::selected(
            project.name.into(),
            project.name.into(),
            project.cost.into(),
            calls,
        )
    }));

    options
}

/// Sample-mode per-tool split for one project, derived from the sample
/// dashboard's project/tool rows; numeric magnitudes are recovered from the
/// formatted money strings, which is fine for demo proportions.
pub fn project_tool_split(
    period: Period,
    project: &str,
    currency: &CurrencyFormatter,
) -> Vec<ProjectToolSplit> {
    fn money_value(text: &str) -> f64 {
        text.chars()
            .filter(|c| c.is_ascii_digit() || *c == '.')
            .collect::<String>()
            .parse()
            .unwrap_or(0.0)
    }
    fn tool_key(label: &str) -> &'static str {
        match label {
            "Claude" => "claude-code",
            "Cursor" => "cursor",
            "Codex" => "codex",
            "Copilot" => "copilot",
            "Gemini" => "gemini",
            _ => "",
        }
    }

    let data = dashboard_data(
        period,
        Tool::All,
        &ProjectFilter::All,
        &ModelFilter::All,
        SortMode::Spend,
        currency,
    );
    data.project_tools
        .iter()
        .filter(|row| row.project == project)
        .map(|row| {
            let cost_value = money_value(row.cost);
            ProjectToolSplit {
                key: tool_key(row.tool),
                label: row.tool,
                cost: row.cost.into(),
                avg_per_session: row.avg_per_session.into(),
                calls: row.calls,
                sessions: row.sessions,
                cost_value,
                avg_value: cost_value / row.sessions.max(1) as f64,
            }
        })
        .collect()
}

/// Sample-mode Projects index: derived from the sample dashboard, with the
/// label doubling as the identity and last-active dates taken from the
/// sample session rows.
pub fn project_index(
    period: Period,
    tool: Tool,
    sort: SortMode,
    currency: &CurrencyFormatter,
) -> Vec<ProjectIndexRow> {
    let data = dashboard_data(
        period,
        tool,
        &ProjectFilter::All,
        &ModelFilter::All,
        sort,
        currency,
    );
    data.projects
        .iter()
        .map(|project| {
            let calls = data
                .project_tools
                .iter()
                .filter(|row| row.project == project.name)
                .map(|row| row.calls)
                .sum();
            let last_active = data
                .sessions
                .iter()
                .filter(|session| session.project == project.name)
                .map(|session| session.date)
                .max()
                .unwrap_or("-")
                .to_string();
            ProjectIndexRow {
                identity: project.name.into(),
                name: project.name.into(),
                cost: project.cost.into(),
                avg_per_session: project.avg_per_session.into(),
                sessions: project.sessions,
                calls,
                last_active,
                tool_mix: project.tool_mix.into(),
                value: project.value,
            }
        })
        .collect()
}

pub fn session_options(
    period: Period,
    tool: Tool,
    model_filter: &ModelFilter,
    sort: SortMode,
    currency: &CurrencyFormatter,
) -> Vec<SessionOption> {
    // The model filter is forwarded so the picker lists exactly the scoped
    // dashboard's sessions; rows reuse the keys stamped by `dashboard_data`,
    // which `session_detail` resolves against the unscoped list.
    let data = dashboard_data(
        period,
        tool,
        &ProjectFilter::All,
        model_filter,
        sort,
        currency,
    );
    data.sessions
        .iter()
        .map(|session| SessionOption {
            key: session.key.to_string(),
            date: session.date.into(),
            project: session.project.into(),
            tool: copy().tools.sample.as_str(),
            cost: session.cost.into(),
            calls: session.calls,
            value: session.value,
        })
        .collect()
}

pub fn session_detail(
    key: &str,
    period: Period,
    sort: SortMode,
    currency: &CurrencyFormatter,
) -> Option<SessionDetailView> {
    let idx: usize = key.strip_prefix("sample:")?.parse().ok()?;

    // Resolve the exact picker row (project / date / cost) the key points at
    // by key match against the unscoped list (keys are minted before filters
    // and sorting, so a scoped picker's key still resolves here), computed in
    // USD so the synthesized per-call ledger reconciles with the session list
    // regardless of the display currency.
    let usd = CurrencyFormatter::usd();
    let data = dashboard_data(
        period,
        Tool::All,
        &ProjectFilter::All,
        &ModelFilter::All,
        sort,
        &usd,
    );
    let session = data.sessions.iter().find(|session| session.key == key)?;
    let total_usd = parse_money_sort_value(session.cost) as f64 / 10_000.0;
    // Match the picker row's call count so the ledger length, the "of {total}"
    // header, and the session KPI all agree (guarded against pathological rows;
    // the largest bundled sample session is well under this ceiling).
    let count = session.calls.clamp(1, 500);

    // Per-call templates cycled across the ledger: tool, model id, mode,
    // prompt, and whether the call ran shell commands.
    const TEMPLATES: [(&str, &str, InteractionKind, &str, &str); 5] = [
        (
            "claude-code",
            "claude-opus-4-7",
            InteractionKind::Agent,
            "Refactor the ingest pipeline to stream rows instead of buffering",
            "cargo test --workspace",
        ),
        (
            "codex",
            "gpt-5",
            InteractionKind::Chat,
            "Explain why the currency conversion double-counts cached tokens",
            "",
        ),
        (
            "claude-code",
            "claude-sonnet-4-5",
            InteractionKind::Agent,
            "Add a regression test for the fallback pricing path",
            "cargo clippy --all-targets",
        ),
        (
            "gemini",
            "gemini-2.5-pro",
            InteractionKind::Plan,
            "Plan the migration to the v7 archive schema",
            "",
        ),
        (
            "cursor",
            "claude-sonnet-4-5",
            InteractionKind::Agent,
            "Wire the By Activity panel into the desktop snapshot",
            "pnpm run check",
        ),
    ];
    // Weight cycle: a few heavy calls, mostly light ones. Costs are split by
    // these weights so the ledger sums back to the picker row's total.
    const WEIGHTS: [u64; 6] = [5, 1, 2, 1, 3, 1];
    let weight_total: u64 = (0..count)
        .map(|i| WEIGHTS[(i as usize) % WEIGHTS.len()])
        .sum();

    let sess = &copy().session;
    let mode_label = |kind: InteractionKind| match kind {
        InteractionKind::Agent => sess.mode_agent.clone(),
        InteractionKind::Chat => sess.mode_chat.clone(),
        InteractionKind::Plan => sess.mode_plan.clone(),
    };

    let mut calls = Vec::with_capacity(count as usize);
    let mut total_input = 0u64;
    let mut total_output = 0u64;
    let mut total_cache_read = 0u64;
    let mut minute = 9 * 60; // first call at 09:00 local
    for i in 0..count {
        let template = &TEMPLATES[(i as usize) % TEMPLATES.len()];
        let weight = WEIGHTS[(i as usize) % WEIGHTS.len()];
        let cost_usd = total_usd * weight as f64 / weight_total as f64;
        let input = (cost_usd * 38_000.0) as u64;
        let output = (cost_usd * 2_600.0) as u64;
        let cache_read = (cost_usd * 520_000.0) as u64;
        let cache_write = (cost_usd * 9_000.0) as u64;
        let reasoning = if matches!(template.2, InteractionKind::Plan) {
            output / 2
        } else {
            0
        };
        total_input += input;
        total_output += output;
        total_cache_read += cache_read;

        let bash: Vec<String> = if template.4.is_empty() {
            Vec::new()
        } else {
            vec![template.4.to_string()]
        };
        let tools = if bash.is_empty() {
            "Read, Edit".to_string()
        } else {
            "Bash, Read, Edit".to_string()
        };
        let hour = (minute / 60) % 24;
        let timestamp = format!("{} {:02}:{:02}", session.date, hour, minute % 60);
        minute += 7;

        let identity = crate::models::resolve(template.0, template.1);
        calls.push(SessionDetail {
            timestamp,
            model: identity.display,
            cost: currency.format_money(cost_usd),
            cache_read_rate: "90%".into(),
            cache_write_rate: "10%".into(),
            input_tokens: input,
            output_tokens: output,
            cache_read,
            cache_write,
            reasoning_tokens: reasoning,
            web_search_requests: 0,
            tools,
            interaction_mode: mode_label(template.2),
            token_quality: sess.quality_exact.clone(),
            timestamp_quality: sess.quality_session.clone(),
            bash_commands: bash,
            prompt: template.3.into(),
            prompt_full: template.3.into(),
        });
    }

    Some(SessionDetailView {
        key: key.into(),
        session_id: format!("sample-{idx}"),
        project: session.project.into(),
        tool: copy().tools.sample.as_str(),
        date_range: session.date.into(),
        total_cost: currency.format_money(total_usd),
        total_calls: count,
        total_input: format_compact(total_input),
        total_output: format_compact(total_output),
        total_cache_read: format_compact(total_cache_read),
        calls,
        note: None,
    })
}

/// Interaction mode for a sample call ledger row; mapped to copy labels.
#[derive(Debug, Clone, Copy)]
enum InteractionKind {
    Agent,
    Chat,
    Plan,
}

pub fn limits_data(_tool: Tool, sort: SortMode, currency: &CurrencyFormatter) -> LimitsData {
    let mut data = sample_data().limits.clone();
    rebase_limit_dates(&mut data, sample_base_date(), sample_date_delta());

    data.sections.sort_by(|a, b| {
        sample_usage_sort_value(&b.usage, sort)
            .cmp(&sample_usage_sort_value(&a.usage, sort))
            .then_with(|| a.tool.cmp(b.tool))
    });
    apply_limits_currency(&mut data, currency);
    data
}

fn apply_sample_sort(data: &mut DashboardData, sort: SortMode) {
    match sort {
        SortMode::Spend => {}
        SortMode::Date => {
            data.daily.sort_by(|a, b| b.day.cmp(a.day));
            data.sessions
                .sort_by(|a, b| b.date.cmp(a.date).then_with(|| a.project.cmp(b.project)));
        }
        SortMode::Tokens => {
            data.daily
                .sort_by_key(|entry| std::cmp::Reverse(entry.value));
            data.projects
                .sort_by(|a, b| b.value.cmp(&a.value).then_with(|| a.name.cmp(b.name)));
            data.project_tools.sort_by(|a, b| {
                b.value
                    .cmp(&a.value)
                    .then_with(|| a.project.cmp(b.project))
                    .then_with(|| a.tool.cmp(b.tool))
            });
            data.sessions.sort_by(|a, b| {
                b.value
                    .cmp(&a.value)
                    .then_with(|| b.calls.cmp(&a.calls))
                    .then_with(|| a.project.cmp(b.project))
            });
            data.models
                .sort_by(|a, b| b.value.cmp(&a.value).then_with(|| a.name.cmp(b.name)));
            data.tools
                .sort_by(|a, b| b.value.cmp(&a.value).then_with(|| a.name.cmp(b.name)));
            data.commands
                .sort_by(|a, b| b.value.cmp(&a.value).then_with(|| a.name.cmp(b.name)));
            data.mcp_servers
                .sort_by(|a, b| b.value.cmp(&a.value).then_with(|| a.name.cmp(b.name)));
        }
    }
}

fn sample_usage_sort_value(usage: &RecentUsageMetric, sort: SortMode) -> u64 {
    match sort {
        SortMode::Spend => parse_money_sort_value(usage.cost),
        SortMode::Date => last_seen_sort_value(usage.last_seen),
        SortMode::Tokens => parse_compact_sort_value(usage.tokens),
    }
}

fn parse_money_sort_value(value: &str) -> u64 {
    let numeric = value
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '.')
        .collect::<String>();
    numeric
        .parse::<f64>()
        .map(|n| (n * 10_000.0).round() as u64)
        .unwrap_or(0)
}

fn parse_compact_sort_value(value: &str) -> u64 {
    let trimmed = value.trim();
    let (number, multiplier) = match trimmed.chars().last() {
        Some('K') => (&trimmed[..trimmed.len().saturating_sub(1)], 1_000.0),
        Some('M') => (&trimmed[..trimmed.len().saturating_sub(1)], 1_000_000.0),
        Some('B') => (&trimmed[..trimmed.len().saturating_sub(1)], 1_000_000_000.0),
        _ => (trimmed, 1.0),
    };
    number
        .parse::<f64>()
        .map(|n| (n * multiplier).round() as u64)
        .unwrap_or(0)
}

fn last_seen_sort_value(value: &str) -> u64 {
    match value {
        "now" => u64::MAX,
        "-" => 0,
        _ if value.ends_with('m') => 1_000_000_u64.saturating_sub(parse_count(value)),
        _ if value.ends_with('h') => 100_000_u64.saturating_sub(parse_count(value)),
        _ if value.ends_with('d') => 10_000_u64.saturating_sub(parse_count(value)),
        _ => 0,
    }
}

fn apply_project_filter(data: &mut DashboardData, project_filter: &ProjectFilter) {
    let ProjectFilter::Selected { label, .. } = project_filter else {
        return;
    };

    if let Some(project) = data.projects.iter().find(|project| project.name == label) {
        let calls: u64 = data
            .project_tools
            .iter()
            .filter(|row| row.project == label)
            .map(|row| row.calls)
            .sum();
        data.summary.cost = project.cost;
        data.summary.calls = leak(format_int(calls));
        data.summary.sessions = leak(format_int(project.sessions));
    } else {
        data.summary.cost = "$0.00";
        data.summary.calls = "0";
        data.summary.sessions = "0";
        data.summary.cache_hit = "-";
    }

    data.projects.retain(|project| project.name == label);
    data.project_tools.retain(|row| row.project == label);
    data.sessions.retain(|row| row.project == label);
}

/// Sample-mode model scoping: keeps the selected model's rows and rescales
/// every other panel deterministically so the scoped payload still
/// reconciles (summary == sum of retained parts, remainders assigned to the
/// largest row). Live data filters per call; sample data derives the same
/// shape from the bundled rows.
fn apply_model_filter(data: &mut DashboardData, model_filter: &ModelFilter, period: Period) {
    let ModelFilter::Selected { canonical_id, .. } = model_filter else {
        return;
    };

    let entry = sample_catalog_for(period)
        .iter()
        .find(|entry| entry.canonical_id == canonical_id.as_str());
    // The model's tool short labels ("Claude", "Cursor") match the sample
    // project_tools rows' tool column.
    let tool_labels: Vec<&'static str> = entry
        .map(|entry| {
            entry
                .per_tool
                .iter()
                .map(|split| split.tool_label)
                .collect()
        })
        .unwrap_or_default();
    let retained_units: u64 = data
        .project_tools
        .iter()
        .filter(|row| tool_labels.contains(&row.tool))
        .map(|row| parse_money_sort_value(row.cost))
        .sum();
    let (Some(entry), true) = (entry, retained_units > 0) else {
        data.summary.cost = "$0.00";
        data.summary.calls = "0";
        data.summary.sessions = "0";
        data.summary.cache_hit = "-";
        data.summary.input = "0";
        data.summary.output = "0";
        data.summary.cached = "0";
        data.summary.written = "0";
        data.daily.clear();
        data.projects.clear();
        data.project_tools.clear();
        data.sessions.clear();
        data.models.clear();
        data.tools.clear();
        data.commands.clear();
        data.mcp_servers.clear();
        return;
    };

    let total_units = parse_money_sort_value(data.summary.cost).max(1);
    let total_calls = parse_count(data.summary.calls).max(1);
    let model_units = parse_money_sort_value(entry.cost).min(total_units);
    let model_calls = entry.calls.min(total_calls);
    let cost_ratio = model_units as f64 / total_units as f64;
    let call_ratio = model_calls as f64 / total_calls as f64;

    // Retain the model's tools' project rows and rescale their costs and
    // calls so they sum exactly to the model row (remainder onto the largest
    // row, so the invariant is exact by construction).
    data.project_tools
        .retain(|row| tool_labels.contains(&row.tool));
    let retained_calls: u64 = data
        .project_tools
        .iter()
        .map(|row| row.calls)
        .sum::<u64>()
        .max(1);
    // Costs are floored to cent granularity so each row's formatted "$x.yz"
    // string round-trips exactly; the sub-cent remainder joins the largest
    // row along with the integer-division remainder.
    let mut scaled: Vec<(u64, u64)> = data
        .project_tools
        .iter()
        .map(|row| {
            (
                parse_money_sort_value(row.cost) * model_units / retained_units / 100 * 100,
                row.calls * model_calls / retained_calls,
            )
        })
        .collect();
    let largest = data
        .project_tools
        .iter()
        .enumerate()
        .max_by_key(|(_, row)| parse_money_sort_value(row.cost))
        .map(|(idx, _)| idx)
        .unwrap_or(0);
    let unit_sum: u64 = scaled.iter().map(|(units, _)| units).sum();
    let call_sum: u64 = scaled.iter().map(|(_, calls)| calls).sum();
    scaled[largest].0 += model_units - unit_sum.min(model_units);
    scaled[largest].1 += model_calls - call_sum.min(model_calls);
    for (row, (units, calls)) in data.project_tools.iter_mut().zip(&scaled) {
        row.cost = leak(format!("${:.2}", *units as f64 / 10_000.0));
        row.calls = *calls;
        row.avg_per_session = leak(format!(
            "${:.2}",
            *units as f64 / 10_000.0 / row.sessions.max(1) as f64
        ));
    }

    // Rebuild the projects panel from the retained rows: a project used from
    // several of the model's tools folds back into one row. Track each
    // project's old/new cost so its sessions scale by the same ratio.
    struct FoldedProject {
        name: &'static str,
        units: u64,
        sessions: u64,
        mix: Vec<(&'static str, u64)>,
    }
    let usd = CurrencyFormatter::usd();
    let mut folded: Vec<FoldedProject> = Vec::new();
    for row in &data.project_tools {
        let units = parse_money_sort_value(row.cost);
        match folded
            .iter_mut()
            .find(|project| project.name == row.project)
        {
            Some(project) => {
                project.units += units;
                project.sessions += row.sessions;
                project.mix.push((row.tool, units));
            }
            None => folded.push(FoldedProject {
                name: row.project,
                units,
                sessions: row.sessions,
                mix: vec![(row.tool, units)],
            }),
        }
    }
    folded.sort_by(|a, b| b.units.cmp(&a.units).then_with(|| a.name.cmp(b.name)));
    let project_max = folded
        .first()
        .map(|project| project.units)
        .unwrap_or(0)
        .max(1);
    let old_project_units: Vec<(&'static str, u64)> = data
        .projects
        .iter()
        .map(|project| (project.name, parse_money_sort_value(project.cost)))
        .collect();
    data.projects = folded
        .iter()
        .map(|project| {
            let mut mix = project.mix.clone();
            mix.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
            let tool_mix = mix
                .iter()
                .take(3)
                .map(|(tool, units)| {
                    format!(
                        "{tool} {}",
                        usd.format_money_short(*units as f64 / 10_000.0)
                    )
                })
                .collect::<Vec<_>>()
                .join("  ");
            ProjectMetric {
                name: project.name,
                cost: leak(format!("${:.2}", project.units as f64 / 10_000.0)),
                avg_per_session: leak(format!(
                    "${:.2}",
                    project.units as f64 / 10_000.0 / project.sessions.max(1) as f64
                )),
                sessions: project.sessions,
                tool_mix: leak(tool_mix),
                value: (project.units * 100 / project_max).clamp(1, 100),
            }
        })
        .collect();

    // Sessions: keep rows in the retained projects and scale each cost by
    // its project's own before/after ratio.
    data.sessions
        .retain(|session| folded.iter().any(|project| project.name == session.project));
    for session in &mut data.sessions {
        let old = old_project_units
            .iter()
            .find(|(name, _)| *name == session.project)
            .map(|(_, units)| *units)
            .unwrap_or(0)
            .max(1);
        let new = folded
            .iter()
            .find(|project| project.name == session.project)
            .map(|project| project.units)
            .unwrap_or(0);
        let units = parse_money_sort_value(session.cost) * new / old;
        session.cost = leak(format!("${:.2}", units as f64 / 10_000.0));
        session.calls = (session.calls as f64 * new as f64 / old as f64) as u64;
    }
    let session_max = data
        .sessions
        .iter()
        .map(|session| parse_money_sort_value(session.cost))
        .max()
        .unwrap_or(0)
        .max(1);
    for session in &mut data.sessions {
        session.value = (parse_money_sort_value(session.cost) * 100 / session_max).clamp(1, 100);
    }

    // Daily trend and the count panels scale by the model's overall share.
    for day in &mut data.daily {
        let units = (parse_money_sort_value(day.cost) as f64 * cost_ratio) as u64;
        day.cost = leak(format!("${:.2}", units as f64 / 10_000.0));
        day.calls = (day.calls as f64 * call_ratio) as u64;
    }
    for row in &mut data.tools {
        row.calls = (row.calls as f64 * call_ratio) as u64;
    }
    for row in &mut data.commands {
        row.calls = (row.calls as f64 * call_ratio) as u64;
    }
    for row in &mut data.mcp_servers {
        row.calls = (row.calls as f64 * call_ratio) as u64;
    }

    // The By Model panel keeps only the selected model's per-tool rows.
    data.models.retain(|model| model.name == entry.name);
    let model_max = data
        .models
        .iter()
        .map(|model| parse_money_sort_value(model.cost))
        .max()
        .unwrap_or(0)
        .max(1);
    for model in &mut data.models {
        model.value = (parse_money_sort_value(model.cost) * 100 / model_max).clamp(1, 100);
    }

    data.summary.cost = leak(format!("${:.2}", model_units as f64 / 10_000.0));
    data.summary.calls = leak(format_int(model_calls));
    data.summary.sessions = leak(format_int(
        folded.iter().map(|project| project.sessions).sum(),
    ));
    data.summary.cache_hit = entry.cache_hit;
    data.summary.input = leak(format_compact(
        (parse_compact_sort_value(data.summary.input) as f64 * cost_ratio) as u64,
    ));
    data.summary.output = leak(format_compact(
        (parse_compact_sort_value(data.summary.output) as f64 * cost_ratio) as u64,
    ));
    data.summary.cached = leak(format_compact(
        (parse_compact_sort_value(data.summary.cached) as f64 * cost_ratio) as u64,
    ));
    data.summary.written = leak(format_compact(
        (parse_compact_sort_value(data.summary.written) as f64 * cost_ratio) as u64,
    ));
}

fn apply_currency(data: &mut DashboardData, currency: &CurrencyFormatter) {
    if currency.is_usd() {
        return;
    }

    data.summary.cost = convert_money_text(data.summary.cost, currency, false);
    for row in &mut data.daily {
        row.cost = convert_money_text(row.cost, currency, false);
    }
    for row in &mut data.activity_timeline {
        row.cost = convert_money_text(row.cost, currency, false);
    }
    for row in &mut data.by_activity {
        row.cost = convert_money_text(row.cost, currency, false);
    }
    for row in &mut data.projects {
        row.cost = convert_money_text(row.cost, currency, false);
        row.avg_per_session = convert_money_text(row.avg_per_session, currency, false);
        row.tool_mix = convert_money_text(row.tool_mix, currency, true);
    }
    for row in &mut data.project_tools {
        row.cost = convert_money_text(row.cost, currency, false);
        row.avg_per_session = convert_money_text(row.avg_per_session, currency, false);
    }
    for row in &mut data.sessions {
        row.cost = convert_money_text(row.cost, currency, false);
    }
    for row in &mut data.models {
        row.cost = convert_money_text(row.cost, currency, false);
    }
}

fn apply_limits_currency(data: &mut LimitsData, currency: &CurrencyFormatter) {
    if currency.is_usd() {
        return;
    }

    for section in &mut data.sections {
        section.usage.cost = convert_money_text(section.usage.cost, currency, false);
        for model in &mut section.models {
            model.cost = convert_money_text(model.cost, currency, false);
        }
        if let Some(plan_value) = &mut section.plan_value {
            plan_value.price = convert_money_text(plan_value.price, currency, false);
            plan_value.month_cost = convert_money_text(plan_value.month_cost, currency, false);
        }
    }
}

fn convert_money_text(
    value: &'static str,
    currency: &CurrencyFormatter,
    short: bool,
) -> &'static str {
    let mut out = String::with_capacity(value.len() + 8);
    let mut chars = value.chars().peekable();
    let mut changed = false;

    while let Some(ch) = chars.next() {
        if ch != '$' {
            out.push(ch);
            continue;
        }

        let mut number = String::new();
        while let Some(next) = chars.peek() {
            if next.is_ascii_digit() || *next == '.' {
                number.push(*next);
                chars.next();
            } else {
                break;
            }
        }

        match number.parse::<f64>() {
            Ok(amount) => {
                changed = true;
                if short {
                    out.push_str(&currency.format_money_short(amount));
                } else {
                    out.push_str(&currency.format_money(amount));
                }
            }
            Err(_) => {
                out.push('$');
                out.push_str(&number);
            }
        }
    }

    if changed {
        leak(out)
    } else {
        value
    }
}

fn parse_count(value: &str) -> u64 {
    value
        .chars()
        .filter(|c| c.is_ascii_digit())
        .collect::<String>()
        .parse()
        .unwrap_or(0)
}

fn format_int(n: u64) -> String {
    let s = n.to_string();
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 && (bytes.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(*b as char);
    }
    out
}

fn format_compact(n: u64) -> String {
    if n >= 1_000_000_000 {
        format!("{:.1}B", n as f64 / 1_000_000_000.0)
    } else if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

fn leak(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

/// Hand-authored coach payload for sample mode (Shift+D) so the page renders
/// deterministic demo content without touching real archives.
pub fn coach_sample(period: Period) -> CoachData {
    CoachData {
        // Rules-weighted mean of the group scores below: 2356/27 -> 87.
        overall: CoachOverall {
            score: 87,
            grade_id: "b_plus",
        },
        // Setup findings scan the local machine; sample mode shows none.
        setup: Vec::new(),
        practice_groups: vec![
            PracticeGroupScore {
                id: "prompt_quality",
                score: 78,
                grade_id: "c",
                wow: "+4%",
                mom: "+9%",
                trend: vec![64, 66, 69, 65, 70, 68, 71, 72, 75, 78],
                triggered: 2,
                total_rules: 8,
                top_rule_id: "lazy-prompting",
            },
            PracticeGroupScore {
                id: "session_hygiene",
                score: 88,
                grade_id: "b_plus",
                wow: "-2%",
                mom: "+1%",
                trend: vec![82, 85, 84, 87, 86, 89, 91, 90, 90, 88],
                triggered: 1,
                total_rules: 9,
                top_rule_id: "late-night-coding",
            },
            PracticeGroupScore {
                id: "code_review",
                score: 100,
                grade_id: "a_plus",
                wow: "–",
                mom: "–",
                trend: vec![100, 100, 100, 100, 100, 100, 100, 100],
                triggered: 0,
                total_rules: 5,
                top_rule_id: "",
            },
            PracticeGroupScore {
                id: "tool_mastery",
                score: 88,
                grade_id: "b_plus",
                wow: "+6%",
                mom: "–",
                trend: vec![72, 75, 74, 78, 80, 79, 82, 83, 88],
                triggered: 1,
                total_rules: 5,
                top_rule_id: "cache-hit-starvation",
            },
        ],
        findings: vec![
            CoachFinding {
                rule_id: "lazy-prompting",
                group: "prompt_quality",
                severity: "medium",
                occurrences: 14,
                total: 42,
                pct: "33%",
                stat: "",
                examples: vec![
                    FindingExample {
                        text: "fix bug",
                        detail: "7 chars",
                    },
                    FindingExample {
                        text: "do it again",
                        detail: "11 chars",
                    },
                ],
            },
            CoachFinding {
                rule_id: "late-night-coding",
                group: "session_hygiene",
                severity: "low",
                occurrences: 12,
                total: 96,
                pct: "13%",
                stat: "",
                examples: vec![FindingExample {
                    text: "one more refactor before bed",
                    detail: "2026-06-11 01:24",
                }],
            },
            CoachFinding {
                rule_id: "cache-hit-starvation",
                group: "tool_mastery",
                severity: "medium",
                occurrences: 23,
                total: 23,
                pct: "",
                stat: "6.2%",
                examples: Vec::new(),
            },
        ],
        flow: FlowSummary {
            overall_score: 64,
            label_id: "moderate",
            avg_followup: "38s",
            avg_block: "52 min",
            deep_days: 3,
            fragmented_days: 2,
            total_days: 10,
            days: vec![
                FlowDayMetric {
                    day: "2026-06-12",
                    score: 74,
                    label_id: "deep",
                    longest_block_min: 96,
                    active_min: 210,
                    sessions: 3,
                },
                FlowDayMetric {
                    day: "2026-06-11",
                    score: 41,
                    label_id: "shallow",
                    longest_block_min: 22,
                    active_min: 75,
                    sessions: 5,
                },
                FlowDayMetric {
                    day: "2026-06-10",
                    score: 82,
                    label_id: "deep",
                    longest_block_min: 118,
                    active_min: 260,
                    sessions: 2,
                },
                FlowDayMetric {
                    day: "2026-06-09",
                    score: 57,
                    label_id: "moderate",
                    longest_block_min: 44,
                    active_min: 150,
                    sessions: 4,
                },
                FlowDayMetric {
                    day: "2026-06-08",
                    score: 22,
                    label_id: "fragmented",
                    longest_block_min: 12,
                    active_min: 48,
                    sessions: 6,
                },
                FlowDayMetric {
                    day: "2026-06-07",
                    score: 68,
                    label_id: "moderate",
                    longest_block_min: 61,
                    active_min: 190,
                    sessions: 3,
                },
                FlowDayMetric {
                    day: "2026-06-06",
                    score: 75,
                    label_id: "deep",
                    longest_block_min: 102,
                    active_min: 240,
                    sessions: 2,
                },
                FlowDayMetric {
                    day: "2026-06-05",
                    score: 49,
                    label_id: "moderate",
                    longest_block_min: 35,
                    active_min: 120,
                    sessions: 4,
                },
                FlowDayMetric {
                    day: "2026-06-04",
                    score: 18,
                    label_id: "fragmented",
                    longest_block_min: 9,
                    active_min: 40,
                    sessions: 7,
                },
                FlowDayMetric {
                    day: "2026-06-03",
                    score: 33,
                    label_id: "shallow",
                    longest_block_min: 20,
                    active_min: 88,
                    sessions: 5,
                },
            ],
        },
        pace: PaceSummary {
            current_streak: 6,
            longest_streak: 11,
            late_night_pct: 9,
            weekend_pct: 18,
            risk_id: "low",
            alert_ids: Vec::new(),
        },
        output: OutputSummary {
            total_loc: "4,812",
            by_language: vec![
                CountMetric {
                    name: "rust",
                    calls: 2760,
                    value: 100,
                },
                CountMetric {
                    name: "typescript",
                    calls: 1240,
                    value: 45,
                },
                CountMetric {
                    name: "markdown",
                    calls: 812,
                    value: 29,
                },
            ],
            by_day: sample_output_days(),
            trend: sample_output_trend(period),
            by_project: vec![
                CountMetric {
                    name: "acme/atlas-dashboard",
                    calls: 3200,
                    value: 100,
                },
                CountMetric {
                    name: "northstar/cli",
                    calls: 1612,
                    value: 50,
                },
            ],
            by_model: vec![CountMetric {
                name: "claude-opus-4-7",
                calls: 4812,
                value: 100,
            }],
            uncovered_tools: "Cursor",
        },
        timeline_grid: sample_timeline_grid(period),
        projects: vec![
            CoachProjectActivity {
                name: "acme/atlas-dashboard",
                active_hours: "14.2h",
                turns: 96,
                languages: vec![
                    CountMetric {
                        name: "rust",
                        calls: 2140,
                        value: 100,
                    },
                    CountMetric {
                        name: "typescript",
                        calls: 660,
                        value: 31,
                    },
                    CountMetric {
                        name: "css",
                        calls: 210,
                        value: 10,
                    },
                ],
                hot_files: vec![
                    "src/app.rs",
                    "src/routes/overview.svelte",
                    "src/data/mod.rs",
                ],
                days_id: "mixed_days",
                time_id: "mornings",
            },
            CoachProjectActivity {
                name: "northstar/cli",
                active_hours: "6.8h",
                turns: 41,
                languages: vec![
                    CountMetric {
                        name: "rust",
                        calls: 620,
                        value: 100,
                    },
                    CountMetric {
                        name: "markdown",
                        calls: 150,
                        value: 24,
                    },
                ],
                hot_files: vec!["src/main.rs", "docs/usage.md"],
                days_id: "mostly_weekdays",
                time_id: "evenings",
            },
            CoachProjectActivity {
                name: "brightlane/docs",
                active_hours: "2.4h",
                turns: 18,
                languages: vec![CountMetric {
                    name: "markdown",
                    calls: 480,
                    value: 100,
                }],
                hot_files: vec!["guides/getting-started.md"],
                days_id: "mostly_weekends",
                time_id: "",
            },
        ],
    }
}

fn sample_output_days() -> Vec<CountMetric> {
    [
        ("2026-06-12", 1420, 72),
        ("2026-06-11", 1980, 100),
        ("2026-06-10", 1610, 81),
        ("2026-06-09", 880, 44),
        ("2026-06-08", 240, 12),
        ("2026-06-07", 1120, 57),
        ("2026-06-06", 1540, 78),
    ]
    .into_iter()
    .map(|(name, calls, value)| CountMetric { name, calls, value })
    .collect()
}

fn sample_output_trend(period: Period) -> Vec<CountMetric> {
    const DAILY_TOTALS: [u64; 7] = [1420, 1980, 1610, 880, 240, 1120, 1540];
    let end_day = NaiveDate::from_ymd_opt(2026, 6, 12).expect("valid sample output day");

    let raw: Vec<(String, u64)> = match period {
        Period::Today => {
            let end = end_day
                .and_hms_opt(23, 30, 0)
                .expect("valid sample half-hour");
            (0..48)
                .map(|index| {
                    let timestamp = end - Duration::minutes(i64::from(index) * 30);
                    let active_index = if (8..18).contains(&timestamp.hour()) {
                        Some((timestamp.hour() - 8) * 2 + timestamp.minute() / 30)
                    } else {
                        None
                    };
                    let calls = active_index
                        .map(|slot| {
                            DAILY_TOTALS[0] / 20 + u64::from(slot < DAILY_TOTALS[0] as u32 % 20)
                        })
                        .unwrap_or(0);
                    (timestamp.format("%Y-%m-%d %H:%M").to_string(), calls)
                })
                .collect()
        }
        Period::Week => {
            let end = end_day.and_hms_opt(23, 0, 0).expect("valid sample hour");
            (0..168)
                .map(|index| {
                    let timestamp = end - Duration::hours(i64::from(index));
                    let day_index = (end_day - timestamp.date()).num_days() as usize;
                    let total = DAILY_TOTALS[day_index];
                    // `then`, not `then_some`: the subtraction must stay lazy
                    // or pre-9am hours underflow.
                    let active_index = (9..17)
                        .contains(&timestamp.hour())
                        .then(|| timestamp.hour() - 9);
                    let calls = active_index
                        .map(|slot| total / 8 + u64::from(slot < total as u32 % 8))
                        .unwrap_or(0);
                    (timestamp.format("%Y-%m-%d %H:%M").to_string(), calls)
                })
                .collect()
        }
        Period::ThirtyDays | Period::Month => return sample_output_days(),
        Period::AllTime => {
            return [
                ("2026-06", 4812, 100),
                ("2026-05", 3920, 81),
                ("2026-04", 2780, 58),
                ("2026-03", 3410, 71),
                ("2026-02", 2240, 47),
                ("2026-01", 1680, 35),
            ]
            .into_iter()
            .map(|(name, calls, value)| CountMetric { name, calls, value })
            .collect();
        }
    };

    let max = raw.iter().map(|(_, calls)| *calls).max().unwrap_or(0);
    raw.into_iter()
        .map(|(name, calls)| CountMetric {
            name: leak(name),
            calls,
            value: if calls == 0 || max == 0 {
                0
            } else {
                (calls * 100 / max).clamp(1, 100)
            },
        })
        .collect()
}

/// Deterministic pseudo-random activity for the sample calendar: most
/// weekdays active, sparser weekends, turn counts cycling 1-28, windowed
/// and period-flagged like the live grid (anchored at the sample end day).
fn sample_timeline_grid(period: Period) -> Vec<TimelineGridDay> {
    let end = NaiveDate::from_ymd_opt(2026, 6, 12).expect("valid sample end day");
    let window = crate::coach::timeline::grid_window_days(period);
    let start = end - Duration::days(window.min(364) - 1);
    let (period_start, period_end) = period.day_bounds(end);
    let mut grid = Vec::new();
    let mut day = start;
    let mut i: u64 = 0;
    while day <= end {
        // The offset keeps the newest sample day active so the calendar's
        // default selection always has session rows.
        let pulse = (i * 37 + 13) % 29;
        let weekend = day.weekday().num_days_from_monday() >= 5;
        let active = if weekend {
            pulse.is_multiple_of(3)
        } else {
            !pulse.is_multiple_of(5)
        };
        if active {
            grid.push(TimelineGridDay {
                day: Box::leak(day.format("%Y-%m-%d").to_string().into_boxed_str()),
                turns: 1 + pulse % 28,
                in_period: period_start.is_none_or(|s| day >= s) && day <= period_end,
            });
        }
        i += 1;
        day += Duration::days(1);
    }
    grid
}

/// Sample timeline day for any active day on the sample calendar.
pub fn coach_timeline_sample(day: &str) -> Option<CoachTimelineDay> {
    if !sample_timeline_grid(Period::AllTime)
        .iter()
        .any(|d| d.day == day)
    {
        return None;
    }
    Some(CoachTimelineDay {
        day: day.to_string(),
        max_concurrent: 2,
        window_start_min: 9 * 60,
        window_end_min: 17 * 60 + 30,
        total_cost: "$15.45".into(),
        rows: vec![
            TimelineSessionRow {
                session_key: "claude-code:sample-1".into(),
                project: "acme/atlas-dashboard".into(),
                tool: "claude-code".into(),
                tool_label: "Claude Code".into(),
                turns: 14,
                cost: "$12.40".into(),
                blocks: vec![
                    TimelineBlock {
                        start_min: 9 * 60,
                        end_min: 10 * 60 + 45,
                    },
                    TimelineBlock {
                        start_min: 13 * 60,
                        end_min: 15 * 60 + 10,
                    },
                ],
            },
            TimelineSessionRow {
                session_key: "codex:sample-2".into(),
                project: "northstar/cli".into(),
                tool: "codex".into(),
                tool_label: "Codex".into(),
                turns: 6,
                cost: "$3.05".into(),
                blocks: vec![TimelineBlock {
                    start_min: 10 * 60,
                    end_min: 11 * 60 + 20,
                }],
            },
        ],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_sample_data_loads_all_periods() {
        let currency = CurrencyFormatter::usd();

        for period in Period::ALL {
            let data = dashboard_data(
                period,
                Tool::All,
                &ProjectFilter::All,
                &ModelFilter::All,
                SortMode::Spend,
                &currency,
            );
            assert!(!data.projects.is_empty());
            assert!(!data.project_tools.is_empty());
            assert!(!data.sessions.is_empty());
            assert!(data.models.iter().all(|model| !model.cache_rate.is_empty()));
        }

        assert!(!limits_data(Tool::All, SortMode::Spend, &currency)
            .sections
            .is_empty());
    }

    #[test]
    fn sample_model_filter_reconciles_summary_with_scoped_rows() {
        let currency = CurrencyFormatter::usd();

        for period in Period::ALL {
            for entry in model_catalog_data(period, &currency) {
                let filter = ModelFilter::Selected {
                    canonical_id: entry.canonical_id.to_string(),
                    label: entry.name.to_string(),
                };
                let data = dashboard_data(
                    period,
                    Tool::All,
                    &ProjectFilter::All,
                    &filter,
                    SortMode::Spend,
                    &currency,
                );

                // The scoped summary is exactly the catalog row's cost/calls.
                assert_eq!(
                    data.summary.cost, entry.cost,
                    "period {period:?} model {}",
                    entry.canonical_id
                );
                assert_eq!(parse_count(data.summary.calls), entry.calls);

                // Reconcile invariant: retained project rows sum exactly to
                // the scoped summary (remainders are assigned, not dropped).
                let project_tool_sum: u64 = data
                    .project_tools
                    .iter()
                    .map(|row| parse_money_sort_value(row.cost))
                    .sum();
                assert_eq!(
                    project_tool_sum,
                    parse_money_sort_value(data.summary.cost),
                    "period {period:?} model {}",
                    entry.canonical_id
                );
                let project_sum: u64 = data
                    .projects
                    .iter()
                    .map(|row| parse_money_sort_value(row.cost))
                    .sum();
                assert_eq!(project_sum, project_tool_sum);

                // Only the selected model's rows survive in the By Model panel.
                assert!(!data.models.is_empty());
                assert!(data.models.iter().all(|model| model.name == entry.name));
            }
        }
    }

    #[test]
    fn sample_catalog_folds_shared_canonical_ids_across_tools() {
        let currency = CurrencyFormatter::usd();
        for period in Period::ALL {
            let entries = model_catalog_data(period, &currency);
            let mut seen = std::collections::HashSet::new();
            for entry in &entries {
                assert!(
                    seen.insert(entry.canonical_id),
                    "duplicate canonical id {} in {period:?} catalog",
                    entry.canonical_id
                );
                // Each entry's per-tool split sums back to its own totals.
                let split_calls: u64 = entry.per_tool.iter().map(|split| split.calls).sum();
                assert_eq!(split_calls, entry.calls);
                let split_cost: u64 = entry
                    .per_tool
                    .iter()
                    .map(|split| parse_money_sort_value(split.cost))
                    .sum();
                assert_eq!(split_cost, parse_money_sort_value(entry.cost));
            }
        }
        // The bundled week data uses Sonnet from two tools — the fold must
        // produce one entry with a genuine multi-tool split.
        assert!(model_catalog_data(Period::Week, &currency)
            .iter()
            .any(|entry| entry.per_tool.len() > 1));
    }

    #[test]
    fn sample_model_detail_matches_scoped_summary_composition() {
        let currency = CurrencyFormatter::usd();
        let entries = model_catalog_data(Period::Week, &currency);
        let entry = entries.first().expect("sample catalog has entries");

        let detail = model_detail_data(Period::Week, entry.canonical_id, &currency)
            .expect("catalog model has detail");
        let filter = ModelFilter::Selected {
            canonical_id: entry.canonical_id.to_string(),
            label: entry.name.to_string(),
        };
        let data = dashboard_data(
            Period::Week,
            Tool::All,
            &ProjectFilter::All,
            &filter,
            SortMode::Spend,
            &currency,
        );
        assert_eq!(
            detail.composition.input_label, data.summary.input,
            "composition mirrors the scoped summary"
        );
        assert!(!detail.pricing.avg_cost_per_call.is_empty());

        assert!(model_detail_data(Period::Week, "never-used-model", &currency).is_none());
    }

    #[test]
    fn sample_today_dates_are_relative_to_current_day() {
        let currency = CurrencyFormatter::usd();
        let data = dashboard_data(
            Period::Today,
            Tool::All,
            &ProjectFilter::All,
            &ModelFilter::All,
            SortMode::Spend,
            &currency,
        );
        let today = Local::now().date_naive();

        assert_eq!(data.daily[0].day, today.format("%m-%d").to_string());
        assert!(data
            .sessions
            .iter()
            .all(|session| session.date == today.format("%Y-%m-%d").to_string()));
    }

    #[test]
    fn sample_activity_timeline_expands_short_ranges_and_ignores_table_sort() {
        let currency = CurrencyFormatter::usd();
        let spend = dashboard_data(
            Period::Week,
            Tool::All,
            &ProjectFilter::All,
            &ModelFilter::All,
            SortMode::Spend,
            &currency,
        );
        let tokens = dashboard_data(
            Period::Week,
            Tool::All,
            &ProjectFilter::All,
            &ModelFilter::All,
            SortMode::Tokens,
            &currency,
        );

        let spend_days = spend.daily.iter().map(|row| row.day).collect::<Vec<_>>();
        let sorted_days = tokens.daily.iter().map(|row| row.day).collect::<Vec<_>>();
        let timeline_labels = tokens
            .activity_timeline
            .iter()
            .map(|row| row.label)
            .collect::<Vec<_>>();

        assert_ne!(sorted_days, spend_days);
        assert_eq!(timeline_labels.len(), spend_days.len() * 24);
        assert!(timeline_labels[0].starts_with(spend_days[0]));
        assert!(timeline_labels
            .last()
            .is_some_and(|label| label.starts_with(spend_days[spend_days.len() - 1])));
    }

    #[test]
    fn sample_calendar_windows_and_flags_follow_the_period() {
        let scoped = coach_sample(Period::Week).timeline_grid;
        assert!(!scoped.is_empty());
        assert!(
            scoped.iter().any(|d| !d.in_period) && scoped.iter().any(|d| d.in_period),
            "a scoped sample mixes context and in-period days"
        );

        let year = coach_sample(Period::AllTime).timeline_grid;
        assert!(year.iter().all(|d| d.in_period));
        assert!(year.len() > scoped.len(), "All Time spans the wider window");
    }

    #[test]
    fn sample_usage_costs_honor_currency() {
        let table = crate::currency::CurrencyTable::embedded().unwrap();
        let currency = table.formatter("GBP");
        let data = limits_data(Tool::All, SortMode::Spend, &currency);

        assert!(data.sections.iter().any(|section| {
            section.usage.cost.contains('£')
                || section.models.iter().any(|model| model.cost.contains('£'))
        }));
        assert!(data.sections.iter().all(|section| {
            !section.usage.cost.contains('$')
                && section.models.iter().all(|model| !model.cost.contains('$'))
        }));
    }

    #[test]
    fn embedded_sample_data_has_no_personal_project_names() {
        let raw = include_str!("sample_data.json").to_lowercase();
        let banned = [
            ["ru", "ss"].concat(),
            ["mcken", "drick"].concat(),
            ["openai", "/sidecar"].concat(),
            ["ascii", "nema"].concat(),
            ["code/", "ru", "ss"].concat(),
            ["ai/", "commit"].concat(),
            ["ai", "commit"].concat(),
            ["code/", "dvr"].concat(),
        ];
        // The hand-authored Rust samples (coach payload, session ledger,
        // timeline) must stay anonymized too — the JSON scan above misses them.
        let currency = CurrencyFormatter::usd();
        let mut rust_samples = serde_json::to_string(&coach_sample(Period::AllTime)).unwrap();
        if let Some(day) = coach_sample(Period::AllTime).timeline_grid.first() {
            if let Some(timeline) = coach_timeline_sample(day.day) {
                rust_samples.push_str(&serde_json::to_string(&timeline).unwrap());
            }
        }
        if let Some(detail) =
            session_detail("sample:0", Period::AllTime, SortMode::Spend, &currency)
        {
            rust_samples.push_str(&serde_json::to_string(&detail).unwrap());
        }
        let rust_samples = rust_samples.to_lowercase();
        for banned in banned {
            assert!(
                !raw.contains(&banned),
                "sample data should not contain {banned}"
            );
            assert!(
                !rust_samples.contains(&banned),
                "rust sample payloads should not contain {banned}"
            );
        }
    }
}
