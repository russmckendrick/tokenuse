use std::sync::OnceLock;

use chrono::{Datelike, Duration, Local, NaiveDate};
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
    pub cost: &'static str,
    pub cache: &'static str,
    pub calls: u64,
    pub value: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct CountMetric {
    pub name: &'static str,
    pub calls: u64,
    pub value: u64,
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
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    pub reasoning_tokens: u64,
    pub web_search_requests: u64,
    pub tools: String,
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
    name: String,
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
    name: String,
    cost: String,
    cache: String,
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
        Self {
            today: wire.periods.today.into(),
            week: wire.periods.week.into(),
            thirty_days: wire.periods.thirty_days.into(),
            month: wire.periods.month.into(),
            all_time: wire.periods.all_time.into(),
            limits: wire.limits.into(),
        }
    }
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
        Self {
            name: leak(wire.name),
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
        Self {
            name: leak(wire.name),
            cost: leak(wire.cost),
            cache: leak(wire.cache),
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
