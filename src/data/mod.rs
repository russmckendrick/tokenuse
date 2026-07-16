use std::sync::OnceLock;

use chrono::{Datelike, Duration, Local, NaiveDate, Timelike};
use serde::{Deserialize, Serialize};

use crate::currency::CurrencyFormatter;
use crate::{
    app::{Period, ProjectFilter, SortMode, Tool},
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
    pub date: &'static str,
    pub project: &'static str,
    pub cost: &'static str,
    pub calls: u64,
    pub value: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelMetric {
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

/// Sample-mode catalog rows: each sample model row belongs to one tool, so
/// the per-tool split is that single tool at full width.
fn sample_catalog(models: &[WireModelMetric]) -> Vec<ModelCatalogEntry> {
    models
        .iter()
        .map(|row| {
            let identity = crate::models::resolve(&row.tool, &row.id);
            let tool_label = crate::ingest::projects::tool_short_label(&row.tool);
            let cost = leak(row.cost.clone());
            ModelCatalogEntry {
                canonical_id: leak(identity.canonical_id),
                name: leak(identity.display),
                provider: identity.provider.id(),
                provider_label: identity.provider.label(),
                family: leak(identity.family),
                cost,
                calls: row.calls,
                tokens: "-",
                cache_hit: leak(row.cache.clone()),
                value: row.value,
                per_tool: vec![ModelToolBreakdown {
                    tool: leak(row.tool.clone()),
                    tool_label,
                    cost,
                    calls: row.calls,
                    value: 100,
                }],
            }
        })
        .collect()
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
    let data = dashboard_data(period, tool, project_filter, SortMode::Spend, &usd);

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
    let samples = sample_data();
    let mut entries = match period {
        Period::Today => samples.catalog_today.clone(),
        Period::Week => samples.catalog_week.clone(),
        Period::ThirtyDays => samples.catalog_thirty_days.clone(),
        Period::Month => samples.catalog_month.clone(),
        Period::AllTime => samples.catalog_all_time.clone(),
    };
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

    apply_project_filter(&mut data, project_filter);
    data.activity_timeline = sample_activity_timeline(&data.daily, period);
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
    let data = dashboard_data(period, tool, &ProjectFilter::All, sort, currency);
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

pub fn session_options(
    period: Period,
    tool: Tool,
    sort: SortMode,
    currency: &CurrencyFormatter,
) -> Vec<SessionOption> {
    let data = dashboard_data(period, tool, &ProjectFilter::All, sort, currency);
    data.sessions
        .iter()
        .enumerate()
        .map(|(idx, session)| SessionOption {
            key: format!("sample:{idx}"),
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
    _sort: SortMode,
    _currency: &CurrencyFormatter,
) -> Option<SessionDetailView> {
    if !key.starts_with("sample:") {
        return None;
    }
    Some(SessionDetailView {
        key: key.into(),
        session_id: key.trim_start_matches("sample:").into(),
        project: copy().session.sample_project.clone(),
        tool: copy().tools.sample.as_str(),
        date_range: copy().session.sample_date_range.clone(),
        total_cost: "$0.00".into(),
        total_calls: 0,
        total_input: "0".into(),
        total_output: "0".into(),
        total_cache_read: "0".into(),
        calls: Vec::new(),
        note: Some(copy().session.sample_note.clone()),
    })
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
                    name: "tokens",
                    calls: 3200,
                    value: 100,
                },
                CountMetric {
                    name: "russ.cloud",
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
                project: "tokens".into(),
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
                project: "russ.cloud".into(),
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
    fn sample_today_dates_are_relative_to_current_day() {
        let currency = CurrencyFormatter::usd();
        let data = dashboard_data(
            Period::Today,
            Tool::All,
            &ProjectFilter::All,
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
            SortMode::Spend,
            &currency,
        );
        let tokens = dashboard_data(
            Period::Week,
            Tool::All,
            &ProjectFilter::All,
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
        for banned in banned {
            assert!(
                !raw.contains(&banned),
                "sample data should not contain {banned}"
            );
        }
    }
}
