use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::Path;

use chrono::{DateTime, Datelike, Duration, Local, NaiveDate, TimeZone, Utc};
use color_eyre::Result;

use crate::app::{Period, ProjectFilter, SortMode, Tool};
use crate::currency::CurrencyFormatter;
use crate::data::{
    ActivityMetric, CountMetric, DailyMetric, DashboardData, LimitMetric, LimitsData, ModelMetric,
    ProjectMetric, ProjectOption, ProjectToolMetric, RecentModelMetric, RecentUsageMetric,
    SessionDetail, SessionDetailView, SessionMetric, SessionOption, Summary, ToolLimitSection,
};
use crate::tools::{self, LimitSnapshot, LimitWindow, ParsedCall};

#[derive(Clone)]
pub struct Ingested {
    pub calls: Vec<ParsedCall>,
    pub limits: Vec<LimitSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectInventoryRow {
    pub project: String,
    pub tool: &'static str,
    pub raw_project: String,
    pub calls: u64,
    pub sessions: u64,
    pub cost: String,
}

pub fn load() -> Result<Ingested> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut calls: Vec<ParsedCall> = Vec::new();
    let mut limits: Vec<LimitSnapshot> = Vec::new();

    for tool in tools::registry() {
        let sources = match tool.discover() {
            Ok(s) => s,
            Err(_) => continue,
        };
        for source in sources {
            if let Ok(mut more) = tool.parse(&source, &mut seen) {
                calls.append(&mut more);
            }
            if let Ok(mut more) = tool.parse_limits(&source) {
                limits.append(&mut more);
            }
        }
    }

    Ok(Ingested { calls, limits })
}

impl Ingested {
    pub fn dashboard(
        &self,
        period: Period,
        tool: Tool,
        project_filter: &ProjectFilter,
        sort: SortMode,
        currency: &CurrencyFormatter,
    ) -> DashboardData {
        let now = Local::now();
        let filtered: Vec<&ParsedCall> = self
            .calls
            .iter()
            .filter(|c| {
                matches_tool(c, tool)
                    && matches_project(c, project_filter)
                    && in_period(c, period, now)
            })
            .collect();
        build_dashboard(&filtered, period, now, sort, currency)
    }

    pub fn project_options(
        &self,
        period: Period,
        tool: Tool,
        sort: SortMode,
        currency: &CurrencyFormatter,
    ) -> Vec<ProjectOption> {
        let now = Local::now();
        let filtered: Vec<&ParsedCall> = self
            .calls
            .iter()
            .filter(|c| matches_tool(c, tool) && in_period(c, period, now))
            .collect();
        build_project_options(&filtered, sort, currency)
    }

    pub fn limits(&self, tool: Tool, sort: SortMode, currency: &CurrencyFormatter) -> LimitsData {
        build_limits_data(&self.limits, &self.calls, tool, sort, currency)
    }

    pub fn session_options(
        &self,
        period: Period,
        tool: Tool,
        project_filter: &ProjectFilter,
        sort: SortMode,
        currency: &CurrencyFormatter,
    ) -> Vec<SessionOption> {
        let now = Local::now();
        let filtered: Vec<&ParsedCall> = self
            .calls
            .iter()
            .filter(|c| {
                matches_tool(c, tool)
                    && matches_project(c, project_filter)
                    && in_period(c, period, now)
            })
            .collect();
        build_session_options(&filtered, sort, currency)
    }

    pub fn session_detail(
        &self,
        key: &str,
        sort: SortMode,
        currency: &CurrencyFormatter,
    ) -> Option<SessionDetailView> {
        let matching: Vec<&ParsedCall> = self
            .calls
            .iter()
            .filter(|c| session_key(c).as_deref() == Some(key))
            .collect();
        if matching.is_empty() {
            return None;
        }
        Some(build_session_detail(key, &matching, sort, currency))
    }

    pub fn is_empty(&self) -> bool {
        self.calls.is_empty() && self.limits.is_empty()
    }

    pub fn project_inventory(&self) -> Vec<ProjectInventoryRow> {
        #[derive(Default)]
        struct Acc {
            calls: u64,
            cost: f64,
            sessions: HashSet<String>,
        }

        let labels = project_label_lookup(self.calls.iter().map(|call| &call.project));
        let mut by_project: BTreeMap<(String, &'static str, String), Acc> = BTreeMap::new();

        for call in &self.calls {
            let key = (
                project_identity(&call.project),
                call.tool,
                raw_project_display(&call.project),
            );
            let entry = by_project.entry(key).or_default();
            entry.calls += 1;
            entry.cost += call.cost_usd;
            if let Some(key) = session_key(call) {
                entry.sessions.insert(key);
            }
        }

        by_project
            .into_iter()
            .map(|((project, tool, raw_project), acc)| {
                let label = project_label(&labels, &project);
                let currency = CurrencyFormatter::usd();
                ProjectInventoryRow {
                    project: label,
                    tool: tool_short_label(tool),
                    raw_project,
                    calls: acc.calls,
                    sessions: acc.sessions.len() as u64,
                    cost: currency.format_money(acc.cost),
                }
            })
            .collect()
    }
}

fn matches_tool(call: &ParsedCall, tool: Tool) -> bool {
    match tool {
        Tool::All => true,
        Tool::ClaudeCode => call.tool == "claude-code",
        Tool::Cursor => call.tool == "cursor",
        Tool::Codex => call.tool == "codex",
        Tool::Copilot => call.tool == "copilot",
    }
}

fn matches_project(call: &ParsedCall, project_filter: &ProjectFilter) -> bool {
    match project_filter {
        ProjectFilter::All => true,
        ProjectFilter::Selected { identity, .. } => project_identity(&call.project) == *identity,
    }
}

fn in_period(call: &ParsedCall, period: Period, now: DateTime<Local>) -> bool {
    let Some(ts) = call.timestamp else {
        return matches!(period, Period::AllTime);
    };
    let local: DateTime<Local> = ts.with_timezone(&Local);
    let today = now.date_naive();
    let date = local.date_naive();

    match period {
        Period::Today => local >= now - Duration::hours(24) && local <= now,
        Period::Week => date >= today - Duration::days(6),
        Period::ThirtyDays => date >= today - Duration::days(29),
        Period::Month => date.year() == today.year() && date.month() == today.month(),
        Period::AllTime => true,
    }
}

fn build_limits_data(
    limits: &[LimitSnapshot],
    calls: &[ParsedCall],
    _tool: Tool,
    sort: SortMode,
    currency: &CurrencyFormatter,
) -> LimitsData {
    let mut latest: HashMap<(&'static str, String), &LimitSnapshot> = HashMap::new();

    for limit in limits {
        let key = (limit.tool, limit.limit_id.clone());
        match latest.get(&key) {
            Some(existing) if !limit_is_newer(limit, existing) => {}
            _ => {
                latest.insert(key, limit);
            }
        }
    }

    let mut rows = Vec::new();
    for limit in latest.into_values() {
        if let Some(window) = limit.primary {
            rows.push(limit_metric(limit, window));
        }
        if let Some(window) = limit.secondary {
            rows.push(limit_metric(limit, window));
        }
    }

    rows.sort_by(|a, b| {
        a.tool
            .cmp(b.tool)
            .then_with(|| a.scope.cmp(b.scope))
            .then_with(|| window_rank(a.window).cmp(&window_rank(b.window)))
            .then_with(|| a.window.cmp(b.window))
    });

    let mut limits_by_tool: HashMap<&'static str, Vec<LimitMetric>> = HashMap::new();
    for row in rows {
        limits_by_tool.entry(row.tool).or_default().push(row);
    }

    LimitsData {
        sections: build_tool_limit_sections(calls, limits_by_tool, sort, currency),
    }
}

#[derive(Debug, Clone, Default)]
struct SortStats {
    cost: f64,
    tokens: f64,
    latest: Option<DateTime<Utc>>,
}

impl SortStats {
    fn add_call(&mut self, call: &ParsedCall) {
        self.cost += call.cost_usd;
        self.tokens += activity_tokens(call) as f64;
        self.latest = latest_timestamp(self.latest, call.timestamp);
    }

    fn add_share(&mut self, call: &ParsedCall, denominator: usize) {
        if denominator == 0 {
            return;
        }
        let denominator = denominator as f64;
        self.cost += call.cost_usd / denominator;
        self.tokens += activity_tokens(call) as f64 / denominator;
        self.latest = latest_timestamp(self.latest, call.timestamp);
    }
}

fn latest_timestamp(
    current: Option<DateTime<Utc>>,
    candidate: Option<DateTime<Utc>>,
) -> Option<DateTime<Utc>> {
    match (current, candidate) {
        (Some(current), Some(candidate)) => Some(current.max(candidate)),
        (None, Some(candidate)) => Some(candidate),
        (Some(current), None) => Some(current),
        (None, None) => None,
    }
}

fn compare_sort_stats(a: &SortStats, b: &SortStats, sort: SortMode) -> Ordering {
    let primary = match sort {
        SortMode::Spend => cmp_f64_desc(a.cost, b.cost),
        SortMode::Date => cmp_date_desc(a.latest, b.latest),
        SortMode::Tokens => cmp_f64_desc(a.tokens, b.tokens),
    };
    if primary != Ordering::Equal {
        return primary;
    }

    match sort {
        SortMode::Spend => {
            cmp_f64_desc(a.tokens, b.tokens).then_with(|| cmp_date_desc(a.latest, b.latest))
        }
        SortMode::Date => {
            cmp_f64_desc(a.cost, b.cost).then_with(|| cmp_f64_desc(a.tokens, b.tokens))
        }
        SortMode::Tokens => {
            cmp_f64_desc(a.cost, b.cost).then_with(|| cmp_date_desc(a.latest, b.latest))
        }
    }
}

fn compare_labeled_stats(
    a: &SortStats,
    b: &SortStats,
    a_label: &str,
    b_label: &str,
    sort: SortMode,
) -> Ordering {
    compare_sort_stats(a, b, sort).then_with(|| a_label.cmp(b_label))
}

fn cmp_f64_desc(a: f64, b: f64) -> Ordering {
    b.partial_cmp(&a).unwrap_or(Ordering::Equal)
}

fn cmp_date_desc(a: Option<DateTime<Utc>>, b: Option<DateTime<Utc>>) -> Ordering {
    b.cmp(&a)
}

fn max_primary_value<'a, I>(rows: I, sort: SortMode) -> f64
where
    I: IntoIterator<Item = &'a SortStats>,
{
    match sort {
        SortMode::Spend => rows.into_iter().map(|stats| stats.cost).fold(0.0, f64::max),
        SortMode::Tokens => rows
            .into_iter()
            .map(|stats| stats.tokens)
            .fold(0.0, f64::max),
        SortMode::Date => 0.0,
    }
}

fn sort_bar_value(stats: &SortStats, sort: SortMode, max: f64, rank: usize, total: usize) -> u64 {
    match sort {
        SortMode::Spend => scale(stats.cost, max),
        SortMode::Tokens => scale(stats.tokens, max),
        SortMode::Date if stats.latest.is_none() || total == 0 => 0,
        SortMode::Date => {
            let value = ((total.saturating_sub(rank) as f64 / total as f64) * 100.0).round();
            value.clamp(1.0, 100.0) as u64
        }
    }
}

#[derive(Default)]
struct ModelAcc {
    calls: u64,
    tokens: u64,
    cost: f64,
    latest: Option<DateTime<Utc>>,
}

fn build_tool_limit_sections(
    calls: &[ParsedCall],
    mut limits_by_tool: HashMap<&'static str, Vec<LimitMetric>>,
    sort: SortMode,
    currency: &CurrencyFormatter,
) -> Vec<ToolLimitSection> {
    #[derive(Default)]
    struct Acc {
        buckets: [u64; 24],
        calls: u64,
        tokens: u64,
        cost: f64,
        last_seen: Option<DateTime<Local>>,
        latest: Option<DateTime<Utc>>,
        models: HashMap<String, ModelAcc>,
    }

    const TOOLS: [(&str, &str); 4] = [
        ("codex", "Codex"),
        ("claude-code", "Claude Code"),
        ("cursor", "Cursor"),
        ("copilot", "Copilot"),
    ];

    let now = Local::now();
    let mut by_tool: HashMap<&str, Acc> =
        TOOLS.iter().map(|(id, _)| (*id, Acc::default())).collect();
    let display_lookup = tool_display_lookup();

    for call in calls {
        let Some(acc) = by_tool.get_mut(call.tool) else {
            continue;
        };
        let Some(ts) = call.timestamp else {
            continue;
        };
        let local = ts.with_timezone(&Local);
        if local > now {
            continue;
        }
        let elapsed_hours = now.signed_duration_since(local).num_hours();
        if !(0..24).contains(&elapsed_hours) {
            continue;
        }

        let tokens = activity_tokens(call).max(1);
        let bucket = 23usize.saturating_sub(elapsed_hours as usize);
        acc.buckets[bucket] = acc.buckets[bucket].saturating_add(tokens);
        acc.calls += 1;
        acc.tokens = acc.tokens.saturating_add(tokens);
        acc.cost += call.cost_usd;
        acc.last_seen = Some(acc.last_seen.map(|prev| prev.max(local)).unwrap_or(local));
        acc.latest = latest_timestamp(acc.latest, call.timestamp);

        let model = display_lookup
            .get(call.tool)
            .map(|adapter| adapter.model_display(&call.model))
            .unwrap_or_else(|| call.model.clone());
        let model_acc = acc.models.entry(model).or_default();
        model_acc.calls += 1;
        model_acc.tokens = model_acc.tokens.saturating_add(tokens);
        model_acc.cost += call.cost_usd;
        model_acc.latest = latest_timestamp(model_acc.latest, call.timestamp);
    }

    let mut ordered: Vec<(usize, &str, &str, Acc)> = TOOLS
        .into_iter()
        .enumerate()
        .map(|(idx, (id, label))| (idx, id, label, by_tool.remove(id).unwrap_or_default()))
        .collect();
    ordered.sort_by(|a, b| {
        let a_stats = SortStats {
            cost: a.3.cost,
            tokens: a.3.tokens as f64,
            latest: a.3.latest,
        };
        let b_stats = SortStats {
            cost: b.3.cost,
            tokens: b.3.tokens as f64,
            latest: b.3.latest,
        };
        compare_sort_stats(&a_stats, &b_stats, sort).then_with(|| a.0.cmp(&b.0))
    });

    ordered
        .into_iter()
        .map(|(_, _, label, acc)| ToolLimitSection {
            tool: label,
            limits: limits_by_tool.remove(label).unwrap_or_default(),
            usage: RecentUsageMetric {
                buckets: scale_buckets(acc.buckets),
                calls: acc.calls,
                tokens: leak(format_compact(acc.tokens)),
                cost: leak(currency.format_money(acc.cost)),
                last_seen: leak(format_last_seen(acc.last_seen, now)),
            },
            models: recent_model_rows(acc.models, sort, currency),
        })
        .collect()
}

fn tool_display_lookup() -> HashMap<&'static str, Box<dyn tools::ToolAdapter>> {
    let mut display_lookup: HashMap<&'static str, Box<dyn tools::ToolAdapter>> = HashMap::new();
    for adapter in tools::registry() {
        display_lookup.insert(adapter.id(), adapter);
    }
    display_lookup
}

fn recent_model_rows(
    models: HashMap<String, ModelAcc>,
    sort: SortMode,
    currency: &CurrencyFormatter,
) -> Vec<RecentModelMetric> {
    let mut rows: Vec<(String, ModelAcc)> = models.into_iter().collect();
    rows.sort_by(|a, b| {
        let a_stats = SortStats {
            cost: a.1.cost,
            tokens: a.1.tokens as f64,
            latest: a.1.latest,
        };
        let b_stats = SortStats {
            cost: b.1.cost,
            tokens: b.1.tokens as f64,
            latest: b.1.latest,
        };
        compare_labeled_stats(&a_stats, &b_stats, &a.0, &b.0, sort)
    });
    let max = match sort {
        SortMode::Spend => rows.iter().map(|row| row.1.cost).fold(0.0, f64::max),
        SortMode::Tokens => rows
            .iter()
            .map(|row| row.1.tokens as f64)
            .fold(0.0, f64::max),
        SortMode::Date => 0.0,
    };
    let total = rows.len();

    rows.into_iter()
        .take(3)
        .enumerate()
        .map(|(idx, (name, acc))| RecentModelMetric {
            name: leak(name),
            calls: acc.calls,
            tokens: leak(format_compact(acc.tokens)),
            cost: leak(currency.format_money(acc.cost)),
            value: sort_bar_value(
                &SortStats {
                    cost: acc.cost,
                    tokens: acc.tokens as f64,
                    latest: acc.latest,
                },
                sort,
                max,
                idx,
                total,
            ),
        })
        .collect()
}

fn activity_tokens(call: &ParsedCall) -> u64 {
    call.input_tokens
        .saturating_add(call.output_tokens)
        .saturating_add(call.cache_creation_input_tokens)
        .saturating_add(call.cache_read_input_tokens)
}

fn scale_buckets<const N: usize>(buckets: [u64; N]) -> [u64; N] {
    let max = buckets.iter().copied().max().unwrap_or(0);
    if max == 0 {
        return buckets;
    }

    buckets.map(|value| {
        if value == 0 {
            0
        } else {
            ((value as f64 / max as f64) * 100.0)
                .round()
                .clamp(1.0, 100.0) as u64
        }
    })
}

fn format_last_seen(last_seen: Option<DateTime<Local>>, now: DateTime<Local>) -> String {
    let Some(last_seen) = last_seen else {
        return "-".into();
    };
    let minutes = now.signed_duration_since(last_seen).num_minutes().max(0);
    match minutes {
        0 => "now".into(),
        1..=59 => format!("{minutes}m"),
        60..=1439 => format!("{}h", minutes / 60),
        _ => format!("{}d", minutes / 1440),
    }
}

fn limit_is_newer(candidate: &LimitSnapshot, existing: &LimitSnapshot) -> bool {
    match (candidate.observed_at, existing.observed_at) {
        (Some(candidate), Some(existing)) => candidate > existing,
        (Some(_), None) => true,
        (None, Some(_)) => false,
        (None, None) => false,
    }
}

fn limit_metric(limit: &LimitSnapshot, window: LimitWindow) -> LimitMetric {
    let used = window.used_percent.round().clamp(0.0, 100.0) as u64;
    let left = (100.0 - window.used_percent).round().clamp(0.0, 100.0) as u64;
    LimitMetric {
        tool: tool_short_label(limit.tool),
        scope: leak(
            limit
                .limit_name
                .clone()
                .unwrap_or_else(|| tool_short_label(limit.tool).to_string()),
        ),
        window: leak(format_window(window.window_minutes)),
        used,
        left: leak(format!("{left}% left")),
        reset: leak(format_reset(window.resets_at)),
        plan: leak(
            limit
                .plan_type
                .as_deref()
                .map(format_plan_type)
                .unwrap_or_else(|| "-".into()),
        ),
    }
}

fn window_rank(label: &str) -> u8 {
    match label {
        "5h" => 0,
        "weekly" => 1,
        _ => 2,
    }
}

fn format_window(minutes: u64) -> String {
    match minutes {
        300 => "5h".into(),
        10080 => "weekly".into(),
        m if m >= 1440 && m % 1440 == 0 => format!("{}d", m / 1440),
        m if m >= 60 && m % 60 == 0 => format!("{}h", m / 60),
        m => format!("{m}m"),
    }
}

fn format_reset(ts: Option<chrono::DateTime<chrono::Utc>>) -> String {
    let Some(ts) = ts else {
        return "-".into();
    };
    let local = ts.with_timezone(&Local);
    if local.date_naive() == Local::now().date_naive() {
        local.format("%H:%M").to_string()
    } else {
        local.format("%d %b %H:%M").to_string()
    }
}

fn format_plan_type(plan: &str) -> String {
    match plan {
        "prolite" => "Pro Lite".into(),
        "plus" => "Plus".into(),
        "pro" => "Pro".into(),
        other => other
            .split(['_', '-'])
            .filter(|part| !part.is_empty())
            .map(|part| {
                let mut chars = part.chars();
                match chars.next() {
                    Some(first) => {
                        first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase()
                    }
                    None => String::new(),
                }
            })
            .collect::<Vec<_>>()
            .join(" "),
    }
}

fn build_dashboard(
    calls: &[&ParsedCall],
    period: Period,
    now: DateTime<Local>,
    sort: SortMode,
    currency: &CurrencyFormatter,
) -> DashboardData {
    if calls.is_empty() {
        return empty_dashboard(currency);
    }

    let total_cost: f64 = calls.iter().map(|c| c.cost_usd).sum();
    let total_input: u64 = calls.iter().map(|c| c.input_tokens).sum();
    let total_output: u64 = calls.iter().map(|c| c.output_tokens).sum();
    let total_cache_read: u64 = calls.iter().map(|c| c.cache_read_input_tokens).sum();
    let total_cache_write: u64 = calls.iter().map(|c| c.cache_creation_input_tokens).sum();

    let cache_denom = total_input + total_cache_read + total_cache_write;
    let cache_hit_pct = if cache_denom > 0 {
        (total_cache_read as f64 / cache_denom as f64) * 100.0
    } else {
        0.0
    };

    let sessions_set: HashSet<String> = calls.iter().filter_map(|c| session_key(c)).collect();

    let summary = Summary {
        cost: leak(currency.format_money(total_cost)),
        calls: leak(format_int(calls.len() as u64)),
        sessions: leak(format_int(sessions_set.len() as u64)),
        cache_hit: leak(format!("{:.1}%", cache_hit_pct)),
        input: leak(format_compact(total_input)),
        output: leak(format_compact(total_output)),
        cached: leak(format_compact(total_cache_read)),
        written: leak(format_compact(total_cache_write)),
    };

    let project_labels = project_label_lookup(calls.iter().map(|call| &call.project));

    let daily = aggregate_daily(calls, sort, currency);
    let activity_timeline = aggregate_activity_timeline(calls, period, now, currency);
    let projects = aggregate_projects(calls, &project_labels, sort, currency);
    let project_tools = aggregate_project_tools(calls, &project_labels, sort, currency);
    let sessions = aggregate_sessions(calls, &project_labels, sort, currency);
    let models = aggregate_models(calls, sort, currency);
    let tools = aggregate_tools(calls, sort);
    let commands = aggregate_commands(calls, sort);
    let mcp_servers = aggregate_mcp(calls, sort);

    DashboardData {
        summary,
        daily,
        activity_timeline,
        projects,
        project_tools,
        sessions,
        models,
        tools,
        commands,
        mcp_servers,
    }
}

fn empty_dashboard(currency: &CurrencyFormatter) -> DashboardData {
    DashboardData {
        summary: Summary {
            cost: leak(currency.format_money(0.0)),
            calls: "0",
            sessions: "0",
            cache_hit: "-",
            input: "0",
            output: "0",
            cached: "0",
            written: "0",
        },
        daily: Vec::new(),
        activity_timeline: Vec::new(),
        projects: Vec::new(),
        project_tools: Vec::new(),
        sessions: Vec::new(),
        models: Vec::new(),
        tools: Vec::new(),
        commands: Vec::new(),
        mcp_servers: Vec::new(),
    }
}

fn build_project_options(
    calls: &[&ParsedCall],
    sort: SortMode,
    currency: &CurrencyFormatter,
) -> Vec<ProjectOption> {
    #[derive(Default)]
    struct Acc {
        cost: f64,
        calls: u64,
        stats: SortStats,
    }

    let total_cost: f64 = calls.iter().map(|c| c.cost_usd).sum();
    let labels = project_label_lookup(calls.iter().map(|call| call.project.as_str()));
    let mut by_project: HashMap<String, Acc> = HashMap::new();

    for call in calls {
        let entry = by_project
            .entry(project_identity(&call.project))
            .or_default();
        entry.cost += call.cost_usd;
        entry.calls += 1;
        entry.stats.add_call(call);
    }

    let mut rows: Vec<(String, String, Acc)> = by_project
        .into_iter()
        .map(|(identity, acc)| {
            let label = project_label(&labels, &identity);
            (identity, label, acc)
        })
        .collect();

    rows.sort_by(|a, b| {
        compare_labeled_stats(&a.2.stats, &b.2.stats, &a.1, &b.1, sort)
            .then_with(|| b.2.calls.cmp(&a.2.calls))
    });

    let mut options = vec![ProjectOption::all(
        currency.format_money(total_cost),
        calls.len() as u64,
    )];
    options.extend(rows.into_iter().map(|(identity, label, acc)| {
        ProjectOption::selected(identity, label, currency.format_money(acc.cost), acc.calls)
    }));
    options
}

fn aggregate_daily(
    calls: &[&ParsedCall],
    sort: SortMode,
    currency: &CurrencyFormatter,
) -> Vec<DailyMetric> {
    #[derive(Default)]
    struct Acc {
        cost: f64,
        calls: u64,
        stats: SortStats,
    }

    let mut by_day: BTreeMap<NaiveDate, Acc> = BTreeMap::new();
    for c in calls {
        let Some(ts) = c.timestamp else { continue };
        let date = ts.with_timezone(&Local).date_naive();
        let entry = by_day.entry(date).or_default();
        entry.cost += c.cost_usd;
        entry.calls += 1;
        entry.stats.add_call(c);
    }
    let mut rows: Vec<(NaiveDate, Acc)> = by_day.into_iter().collect();
    rows.sort_by(|a, b| {
        compare_sort_stats(&a.1.stats, &b.1.stats, sort).then_with(|| b.0.cmp(&a.0))
    });
    let max = max_primary_value(rows.iter().map(|row| &row.1.stats), sort);
    let total = rows.len();
    rows.into_iter()
        .enumerate()
        .map(|(idx, (date, acc))| DailyMetric {
            day: leak(date.format("%m-%d").to_string()),
            cost: leak(currency.format_money(acc.cost)),
            calls: acc.calls,
            value: sort_bar_value(&acc.stats, sort, max, idx, total),
        })
        .collect()
}

fn aggregate_activity_timeline(
    calls: &[&ParsedCall],
    period: Period,
    now: DateTime<Local>,
    currency: &CurrencyFormatter,
) -> Vec<ActivityMetric> {
    match period {
        Period::Today => aggregate_hourly_timeline(calls, 24, true, now, currency),
        Period::Week => aggregate_hourly_timeline(calls, 24 * 7, false, now, currency),
        Period::ThirtyDays | Period::Month | Period::AllTime => {
            aggregate_daily_timeline(calls, currency)
        }
    }
}

fn aggregate_hourly_timeline(
    calls: &[&ParsedCall],
    hours: i64,
    hour_only_label: bool,
    now: DateTime<Local>,
    currency: &CurrencyFormatter,
) -> Vec<ActivityMetric> {
    #[derive(Default)]
    struct Acc {
        cost: f64,
        calls: u64,
    }

    let now_hour = now.timestamp() / 3600;
    let start_hour = now_hour.saturating_sub(hours.saturating_sub(1));
    let mut by_hour: BTreeMap<i64, Acc> = BTreeMap::new();

    for c in calls {
        let Some(ts) = c.timestamp else { continue };
        let hour = ts.with_timezone(&Local).timestamp() / 3600;
        if hour < start_hour || hour > now_hour {
            continue;
        }
        let entry = by_hour.entry(hour).or_default();
        entry.cost += c.cost_usd;
        entry.calls += 1;
    }

    let max_cost = by_hour.values().map(|acc| acc.cost).fold(0.0, f64::max);
    (start_hour..=now_hour)
        .map(|hour| {
            let acc = by_hour.remove(&hour).unwrap_or_default();
            ActivityMetric {
                label: leak(format_hour_bucket(hour, hour_only_label)),
                cost: leak(currency.format_money(acc.cost)),
                calls: acc.calls,
                value: scale(acc.cost, max_cost),
            }
        })
        .collect()
}

fn aggregate_daily_timeline(
    calls: &[&ParsedCall],
    currency: &CurrencyFormatter,
) -> Vec<ActivityMetric> {
    #[derive(Default)]
    struct Acc {
        cost: f64,
        calls: u64,
    }

    let mut by_day: BTreeMap<NaiveDate, Acc> = BTreeMap::new();
    for c in calls {
        let Some(ts) = c.timestamp else { continue };
        let date = ts.with_timezone(&Local).date_naive();
        let entry = by_day.entry(date).or_default();
        entry.cost += c.cost_usd;
        entry.calls += 1;
    }

    let max_cost = by_day.values().map(|acc| acc.cost).fold(0.0, f64::max);
    by_day
        .into_iter()
        .map(|(date, acc)| ActivityMetric {
            label: leak(date.format("%m-%d").to_string()),
            cost: leak(currency.format_money(acc.cost)),
            calls: acc.calls,
            value: scale(acc.cost, max_cost),
        })
        .collect()
}

fn format_hour_bucket(hour: i64, hour_only: bool) -> String {
    let Some(utc) = Utc.timestamp_opt(hour.saturating_mul(3600), 0).single() else {
        return "-".into();
    };
    let local = utc.with_timezone(&Local);
    if hour_only {
        local.format("%Hh").to_string()
    } else {
        local.format("%m-%d %Hh").to_string()
    }
}

fn aggregate_projects(
    calls: &[&ParsedCall],
    project_labels: &HashMap<String, String>,
    sort: SortMode,
    currency: &CurrencyFormatter,
) -> Vec<ProjectMetric> {
    #[derive(Default)]
    struct Acc {
        cost: f64,
        sessions: HashSet<String>,
        tools: HashMap<&'static str, f64>,
        stats: SortStats,
    }
    let mut by_project: HashMap<String, Acc> = HashMap::new();
    for c in calls {
        let entry = by_project.entry(project_identity(&c.project)).or_default();
        entry.cost += c.cost_usd;
        entry.stats.add_call(c);
        if let Some(key) = session_key(c) {
            entry.sessions.insert(key);
        }
        *entry.tools.entry(c.tool).or_default() += c.cost_usd;
    }

    let mut rows: Vec<(String, Acc)> = by_project.into_iter().collect();
    rows.sort_by(|a, b| {
        let a_label = project_label(project_labels, &a.0);
        let b_label = project_label(project_labels, &b.0);
        compare_labeled_stats(&a.1.stats, &b.1.stats, &a_label, &b_label, sort)
    });
    let max = max_primary_value(rows.iter().map(|row| &row.1.stats), sort);
    let total = rows.len();

    rows.into_iter()
        .take(10)
        .enumerate()
        .map(|(idx, (project, acc))| {
            let session_count = acc.sessions.len().max(1) as u64;
            let avg = acc.cost / session_count as f64;
            ProjectMetric {
                name: leak(project_label(project_labels, &project)),
                cost: leak(currency.format_money(acc.cost)),
                avg_per_session: leak(currency.format_money(avg)),
                sessions: session_count,
                tool_mix: leak(format_tool_mix(&acc.tools, currency)),
                value: sort_bar_value(&acc.stats, sort, max, idx, total),
            }
        })
        .collect()
}

fn aggregate_project_tools(
    calls: &[&ParsedCall],
    project_labels: &HashMap<String, String>,
    sort: SortMode,
    currency: &CurrencyFormatter,
) -> Vec<ProjectToolMetric> {
    #[derive(Default)]
    struct Acc {
        cost: f64,
        calls: u64,
        sessions: HashSet<String>,
        stats: SortStats,
    }

    let mut by_pair: HashMap<(String, &'static str), Acc> = HashMap::new();

    for c in calls {
        let project = project_identity(&c.project);
        let entry = by_pair.entry((project, c.tool)).or_default();
        entry.cost += c.cost_usd;
        entry.calls += 1;
        entry.stats.add_call(c);
        if let Some(key) = session_key(c) {
            entry.sessions.insert(key);
        }
    }

    let mut rows: Vec<(String, &'static str, Acc)> = by_pair
        .into_iter()
        .map(|((project, tool), acc)| (project, tool, acc))
        .collect();

    rows.sort_by(|a, b| {
        let a_label = format!(
            "{} {}",
            project_label(project_labels, &a.0),
            tool_short_label(a.1)
        );
        let b_label = format!(
            "{} {}",
            project_label(project_labels, &b.0),
            tool_short_label(b.1)
        );
        compare_labeled_stats(&a.2.stats, &b.2.stats, &a_label, &b_label, sort)
    });
    let max = max_primary_value(rows.iter().map(|row| &row.2.stats), sort);
    let total = rows.len();

    rows.into_iter()
        .take(12)
        .enumerate()
        .map(|(idx, (project, tool, acc))| {
            let session_count = acc.sessions.len().max(1) as u64;
            let avg = acc.cost / session_count as f64;
            ProjectToolMetric {
                project: leak(project_label(project_labels, &project)),
                tool: tool_short_label(tool),
                cost: leak(currency.format_money(acc.cost)),
                calls: acc.calls,
                sessions: session_count,
                avg_per_session: leak(currency.format_money(avg)),
                value: sort_bar_value(&acc.stats, sort, max, idx, total),
            }
        })
        .collect()
}

fn aggregate_sessions(
    calls: &[&ParsedCall],
    project_labels: &HashMap<String, String>,
    sort: SortMode,
    currency: &CurrencyFormatter,
) -> Vec<SessionMetric> {
    #[derive(Default)]
    struct Acc {
        cost: f64,
        calls: u64,
        date: Option<NaiveDate>,
        project: String,
        stats: SortStats,
    }
    let mut by_session: HashMap<String, Acc> = HashMap::new();
    for c in calls {
        let Some(key) = session_key(c) else {
            continue;
        };
        let entry = by_session.entry(key).or_default();
        entry.cost += c.cost_usd;
        entry.calls += 1;
        entry.stats.add_call(c);
        if entry.project.is_empty() {
            entry.project = project_identity(&c.project);
        }
        if let Some(ts) = c.timestamp {
            let d = ts.with_timezone(&Local).date_naive();
            entry.date = Some(entry.date.map(|prev| prev.max(d)).unwrap_or(d));
        }
    }

    let mut rows: Vec<Acc> = by_session.into_values().collect();
    rows.sort_by(|a, b| {
        let a_label = project_label(project_labels, &a.project);
        let b_label = project_label(project_labels, &b.project);
        compare_labeled_stats(&a.stats, &b.stats, &a_label, &b_label, sort)
    });
    let max = max_primary_value(rows.iter().map(|row| &row.stats), sort);
    let total = rows.len();

    rows.into_iter()
        .take(10)
        .enumerate()
        .map(|(idx, acc)| SessionMetric {
            date: leak(
                acc.date
                    .map(|d| d.format("%Y-%m-%d").to_string())
                    .unwrap_or_else(|| "-".into()),
            ),
            project: leak(project_label(project_labels, &acc.project)),
            cost: leak(currency.format_money(acc.cost)),
            calls: acc.calls,
            value: sort_bar_value(&acc.stats, sort, max, idx, total),
        })
        .collect()
}

fn build_session_options(
    calls: &[&ParsedCall],
    sort: SortMode,
    currency: &CurrencyFormatter,
) -> Vec<SessionOption> {
    #[derive(Default)]
    struct Acc {
        cost: f64,
        calls: u64,
        date: Option<NaiveDate>,
        project: String,
        tool: &'static str,
        stats: SortStats,
    }
    let labels = project_label_lookup(calls.iter().map(|call| call.project.as_str()));
    let mut by_session: HashMap<String, Acc> = HashMap::new();
    for c in calls {
        let Some(key) = session_key(c) else {
            continue;
        };
        let entry = by_session.entry(key).or_default();
        entry.cost += c.cost_usd;
        entry.calls += 1;
        entry.stats.add_call(c);
        if entry.project.is_empty() {
            entry.project = project_identity(&c.project);
        }
        if entry.tool.is_empty() {
            entry.tool = c.tool;
        }
        if let Some(ts) = c.timestamp {
            let d = ts.with_timezone(&Local).date_naive();
            entry.date = Some(entry.date.map(|prev| prev.max(d)).unwrap_or(d));
        }
    }

    let mut rows: Vec<(String, Acc)> = by_session.into_iter().collect();
    rows.sort_by(|a, b| {
        let a_label = project_label(&labels, &a.1.project);
        let b_label = project_label(&labels, &b.1.project);
        compare_labeled_stats(&a.1.stats, &b.1.stats, &a_label, &b_label, sort)
            .then_with(|| b.1.calls.cmp(&a.1.calls))
    });
    let max = max_primary_value(rows.iter().map(|row| &row.1.stats), sort);
    let total = rows.len();

    rows.into_iter()
        .enumerate()
        .map(|(idx, (key, acc))| SessionOption {
            key,
            date: acc
                .date
                .map(|d| d.format("%Y-%m-%d").to_string())
                .unwrap_or_else(|| "-".into()),
            project: project_label(&labels, &acc.project),
            tool: tool_short_label(acc.tool),
            cost: currency.format_money(acc.cost),
            calls: acc.calls,
            value: sort_bar_value(&acc.stats, sort, max, idx, total),
        })
        .collect()
}

fn build_session_detail(
    key: &str,
    calls: &[&ParsedCall],
    sort: SortMode,
    currency: &CurrencyFormatter,
) -> SessionDetailView {
    let mut sorted: Vec<&ParsedCall> = calls.to_vec();
    sorted.sort_by(|a, b| compare_calls(a, b, sort));

    let display_lookup = tool_display_lookup();

    let total_cost: f64 = sorted.iter().map(|c| c.cost_usd).sum();
    let total_calls = sorted.len() as u64;
    let total_input: u64 = sorted
        .iter()
        .map(|c| c.input_tokens.saturating_add(c.cached_input_tokens))
        .sum();
    let total_output: u64 = sorted.iter().map(|c| c.output_tokens).sum();
    let total_cache_read: u64 = sorted.iter().map(|c| c.cache_read_input_tokens).sum();

    let first_date = sorted.iter().filter_map(|c| c.timestamp).min();
    let last_date = sorted.iter().filter_map(|c| c.timestamp).max();
    let date_range = match (first_date, last_date) {
        (Some(a), Some(b)) if a.date_naive() == b.date_naive() => {
            a.with_timezone(&Local).format("%Y-%m-%d").to_string()
        }
        (Some(a), Some(b)) => format!(
            "{} → {}",
            a.with_timezone(&Local).format("%Y-%m-%d"),
            b.with_timezone(&Local).format("%Y-%m-%d")
        ),
        _ => "-".into(),
    };

    let project = sorted
        .first()
        .map(|c| project_identity(&c.project))
        .unwrap_or_default();
    let labels = project_label_lookup(std::iter::once(project.as_str()));
    let project_label = project_label(&labels, &project);
    let session_id = sorted
        .first()
        .map(|c| c.session_id.clone())
        .unwrap_or_default();
    let tool_label = sorted
        .first()
        .map(|c| tool_short_label(c.tool))
        .unwrap_or("Other");

    let detail_calls = sorted
        .iter()
        .map(|c| {
            let model = display_lookup
                .get(c.tool)
                .map(|adapter| adapter.model_display(&c.model))
                .unwrap_or_else(|| c.model.clone());
            let timestamp = c
                .timestamp
                .map(|ts| ts.with_timezone(&Local).format("%m-%d %H:%M").to_string())
                .unwrap_or_else(|| "-".into());
            let mut tools = c
                .tools
                .iter()
                .filter(|t| !t.starts_with("mcp__"))
                .cloned()
                .collect::<Vec<_>>();
            for t in &c.tools {
                if let Some(rest) = t.strip_prefix("mcp__") {
                    let server = rest.split("__").next().unwrap_or(rest);
                    tools.push(format!("mcp:{server}"));
                }
            }
            let tools_text = if tools.is_empty() {
                "-".into()
            } else {
                tools.join(", ")
            };
            SessionDetail {
                timestamp,
                model,
                cost: currency.format_money(c.cost_usd),
                input_tokens: c.input_tokens.saturating_add(c.cached_input_tokens),
                output_tokens: c.output_tokens,
                cache_read: c.cache_read_input_tokens,
                cache_write: c.cache_creation_input_tokens,
                reasoning_tokens: c.reasoning_tokens,
                web_search_requests: c.web_search_requests,
                tools: tools_text,
                bash_commands: c.bash_commands.clone(),
                prompt: snippet(&c.user_message, 120),
                prompt_full: clean_text(&c.user_message),
            }
        })
        .collect();

    SessionDetailView {
        key: key.into(),
        session_id,
        project: project_label,
        tool: tool_label,
        date_range,
        total_cost: currency.format_money(total_cost),
        total_calls,
        total_input: format_compact(total_input),
        total_output: format_compact(total_output),
        total_cache_read: format_compact(total_cache_read),
        calls: detail_calls,
        note: None,
    }
}

fn compare_calls(a: &ParsedCall, b: &ParsedCall, sort: SortMode) -> Ordering {
    let a_stats = SortStats {
        cost: a.cost_usd,
        tokens: activity_tokens(a) as f64,
        latest: a.timestamp,
    };
    let b_stats = SortStats {
        cost: b.cost_usd,
        tokens: activity_tokens(b) as f64,
        latest: b.timestamp,
    };
    compare_labeled_stats(&a_stats, &b_stats, &a.model, &b.model, sort)
}

fn snippet(text: &str, max: usize) -> String {
    let cleaned = clean_text(text);
    if cleaned.chars().count() <= max {
        cleaned
    } else {
        let mut out: String = cleaned.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

fn clean_text(text: &str) -> String {
    let cleaned: String = text
        .chars()
        .map(|c| {
            if c == '\n' || c == '\r' || c == '\t' || c.is_control() {
                ' '
            } else {
                c
            }
        })
        .collect();
    let mut compacted = String::with_capacity(cleaned.len());
    let mut last_space = false;
    for c in cleaned.chars() {
        if c == ' ' {
            if !last_space {
                compacted.push(' ');
            }
            last_space = true;
        } else {
            compacted.push(c);
            last_space = false;
        }
    }
    compacted.trim().to_string()
}

fn aggregate_models(
    calls: &[&ParsedCall],
    sort: SortMode,
    currency: &CurrencyFormatter,
) -> Vec<ModelMetric> {
    #[derive(Default)]
    struct Acc {
        cost: f64,
        calls: u64,
        cache_read: u64,
        input: u64,
        stats: SortStats,
    }
    let registry = tools::registry();
    let mut display_lookup: HashMap<&'static str, Box<dyn tools::ToolAdapter>> = HashMap::new();
    for p in registry {
        display_lookup.insert(p.id(), p);
    }

    let mut by_model: HashMap<String, Acc> = HashMap::new();
    for c in calls {
        let display = display_lookup
            .get(c.tool)
            .map(|p| p.model_display(&c.model))
            .unwrap_or_else(|| c.model.clone());
        let entry = by_model.entry(display).or_default();
        entry.cost += c.cost_usd;
        entry.calls += 1;
        entry.cache_read += c.cache_read_input_tokens;
        entry.input += c.input_tokens + c.cache_read_input_tokens + c.cache_creation_input_tokens;
        entry.stats.add_call(c);
    }

    let mut rows: Vec<(String, Acc)> = by_model.into_iter().collect();
    rows.sort_by(|a, b| compare_labeled_stats(&a.1.stats, &b.1.stats, &a.0, &b.0, sort));
    let max = max_primary_value(rows.iter().map(|row| &row.1.stats), sort);
    let total = rows.len();

    rows.into_iter()
        .enumerate()
        .map(|(idx, (name, acc))| ModelMetric {
            name: leak(name),
            cost: leak(currency.format_money(acc.cost)),
            cache: leak(if acc.input == 0 {
                "-".into()
            } else {
                format!("{:.1}%", (acc.cache_read as f64 / acc.input as f64) * 100.0)
            }),
            calls: acc.calls,
            value: sort_bar_value(&acc.stats, sort, max, idx, total),
        })
        .collect()
}

#[derive(Default)]
struct CountAcc {
    calls: u64,
    stats: SortStats,
}

fn aggregate_tools(calls: &[&ParsedCall], sort: SortMode) -> Vec<CountMetric> {
    let mut counts: HashMap<String, CountAcc> = HashMap::new();
    for c in calls {
        let names: Vec<&String> = c.tools.iter().filter(|t| !t.starts_with("mcp__")).collect();
        for t in &names {
            let entry = counts.entry((*t).clone()).or_default();
            entry.calls += 1;
            entry.stats.add_share(c, names.len());
        }
    }
    top_counts(counts, sort, 10)
}

fn aggregate_commands(calls: &[&ParsedCall], sort: SortMode) -> Vec<CountMetric> {
    let mut counts: HashMap<String, CountAcc> = HashMap::new();
    for c in calls {
        let heads: Vec<String> = c
            .bash_commands
            .iter()
            .map(|cmd| tools::jsonl::first_word(cmd))
            .filter(|head| !head.is_empty())
            .collect();
        for head in &heads {
            let entry = counts.entry(head.clone()).or_default();
            entry.calls += 1;
            entry.stats.add_share(c, heads.len());
        }
    }
    top_counts(counts, sort, 10)
}

fn aggregate_mcp(calls: &[&ParsedCall], sort: SortMode) -> Vec<CountMetric> {
    let mut counts: HashMap<String, CountAcc> = HashMap::new();
    for c in calls {
        let servers: Vec<String> = c
            .tools
            .iter()
            .filter_map(|t| {
                t.strip_prefix("mcp__")
                    .map(|rest| rest.split("__").next().unwrap_or(rest).to_string())
            })
            .collect();
        for server in &servers {
            let entry = counts.entry(server.clone()).or_default();
            entry.calls += 1;
            entry.stats.add_share(c, servers.len());
        }
    }
    top_counts(counts, sort, 10)
}

fn top_counts(counts: HashMap<String, CountAcc>, sort: SortMode, limit: usize) -> Vec<CountMetric> {
    let mut rows: Vec<(String, CountAcc)> = counts.into_iter().collect();
    rows.sort_by(|a, b| compare_labeled_stats(&a.1.stats, &b.1.stats, &a.0, &b.0, sort));
    let max = max_primary_value(rows.iter().map(|row| &row.1.stats), sort);
    let total = rows.len();
    rows.into_iter()
        .take(limit)
        .enumerate()
        .map(|(idx, (name, acc))| CountMetric {
            name: leak(name),
            calls: acc.calls,
            value: sort_bar_value(&acc.stats, sort, max, idx, total),
        })
        .collect()
}

fn session_key(call: &ParsedCall) -> Option<String> {
    if call.session_id.is_empty() {
        None
    } else {
        Some(format!("{}:{}", call.tool, call.session_id))
    }
}

fn project_identity(raw: &str) -> String {
    let normalized = normalized_project_path(raw);
    nearest_git_root(&normalized).unwrap_or(normalized)
}

fn raw_project_display(raw: &str) -> String {
    normalized_project_path(raw)
}

fn normalized_project_path(raw: &str) -> String {
    let normalized = raw.trim().replace('\\', "/");
    let trimmed = normalized.trim_end_matches('/');
    if trimmed.is_empty() {
        "(unknown)".into()
    } else {
        trimmed.to_string()
    }
}

fn nearest_git_root(project: &str) -> Option<String> {
    let path = Path::new(project);
    if !path.is_absolute() {
        return None;
    }

    path.ancestors()
        .find(|ancestor| ancestor.join(".git").exists())
        .map(path_to_project_string)
}

fn path_to_project_string(path: &Path) -> String {
    let normalized = path.to_string_lossy().replace('\\', "/");
    let trimmed = normalized.trim_end_matches('/');
    if trimmed.is_empty() {
        normalized
    } else {
        trimmed.to_string()
    }
}

fn project_label_lookup<I, S>(raw_projects: I) -> HashMap<String, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let identities: BTreeSet<String> = raw_projects
        .into_iter()
        .map(|raw| project_identity(raw.as_ref()))
        .collect();

    identities
        .iter()
        .map(|identity| {
            (
                identity.clone(),
                shortest_unique_project_label(identity, &identities),
            )
        })
        .collect()
}

fn project_label(labels: &HashMap<String, String>, identity: &str) -> String {
    labels.get(identity).cloned().unwrap_or_else(|| {
        shortest_unique_project_label(identity, &BTreeSet::from([identity.to_string()]))
    })
}

fn shortest_unique_project_label(identity: &str, identities: &BTreeSet<String>) -> String {
    let parts = project_parts(identity);
    if parts.is_empty() {
        return "(unknown)".into();
    }

    for suffix_len in 1..=parts.len() {
        let candidate = project_suffix(&parts, suffix_len);
        let conflicts = identities
            .iter()
            .filter(|other| other.as_str() != identity)
            .any(|other| {
                let other_parts = project_parts(other);
                other_parts.len() >= suffix_len
                    && project_suffix(&other_parts, suffix_len) == candidate
            });

        if !conflicts {
            return candidate;
        }
    }

    parts.join("/")
}

fn project_parts(identity: &str) -> Vec<&str> {
    if identity == "(unknown)" {
        return vec![identity];
    }
    identity
        .trim_start_matches('/')
        .split('/')
        .filter(|part| !part.is_empty())
        .collect()
}

fn project_suffix(parts: &[&str], suffix_len: usize) -> String {
    parts[parts.len().saturating_sub(suffix_len)..].join("/")
}

fn format_tool_mix(tools: &HashMap<&'static str, f64>, currency: &CurrencyFormatter) -> String {
    let mut rows: Vec<(&'static str, f64)> =
        tools.iter().map(|(tool, cost)| (*tool, *cost)).collect();
    rows.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| tool_short_label(a.0).cmp(tool_short_label(b.0)))
    });

    if rows.is_empty() {
        return "-".into();
    }

    rows.into_iter()
        .take(3)
        .map(|(tool, cost)| {
            format!(
                "{} {}",
                tool_short_label(tool),
                currency.format_money_short(cost)
            )
        })
        .collect::<Vec<_>>()
        .join("  ")
}

fn tool_short_label(tool: &str) -> &'static str {
    match tool {
        "claude-code" => "Claude",
        "cursor" => "Cursor",
        "codex" => "Codex",
        "copilot" => "Copilot",
        _ => "Other",
    }
}

fn scale(value: f64, max: f64) -> u64 {
    if max <= 0.0 {
        return 0;
    }
    let v = (value / max * 100.0).round() as i64;
    v.clamp(1, 100) as u64
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

#[cfg(test)]
mod tests {
    use super::*;

    struct TempDir(std::path::PathBuf);

    impl TempDir {
        fn new(name: &str) -> Self {
            use std::sync::atomic::{AtomicU64, Ordering};
            static SEQ: AtomicU64 = AtomicU64::new(0);
            let seq = SEQ.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "tokenuse-ingest-{}-{}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                seq,
                name
            ));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &std::path::Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn mk_call(tool: &'static str, ts: &str, cost: f64) -> ParsedCall {
        ParsedCall {
            tool,
            timestamp: DateTime::parse_from_rfc3339(ts)
                .ok()
                .map(|d| d.with_timezone(&chrono::Utc)),
            cost_usd: cost,
            session_id: "s1".into(),
            project: "/Users/me/Code/widgets".into(),
            model: "claude-opus-4-7".into(),
            ..ParsedCall::default()
        }
    }

    fn mk_project_call(
        tool: &'static str,
        session_id: &str,
        project: &str,
        cost: f64,
    ) -> ParsedCall {
        ParsedCall {
            tool,
            timestamp: DateTime::parse_from_rfc3339("2026-04-29T08:00:00Z")
                .ok()
                .map(|d| d.with_timezone(&chrono::Utc)),
            cost_usd: cost,
            session_id: session_id.into(),
            project: project.into(),
            model: "test-model".into(),
            ..ParsedCall::default()
        }
    }

    fn mk_limit(
        id: &str,
        name: Option<&str>,
        observed_at: &str,
        primary_used: f64,
        secondary_used: f64,
    ) -> LimitSnapshot {
        LimitSnapshot {
            tool: "codex",
            limit_id: id.into(),
            limit_name: name.map(Into::into),
            plan_type: Some("prolite".into()),
            observed_at: DateTime::parse_from_rfc3339(observed_at)
                .ok()
                .map(|d| d.with_timezone(&chrono::Utc)),
            primary: Some(LimitWindow {
                used_percent: primary_used,
                window_minutes: 300,
                resets_at: DateTime::parse_from_rfc3339("2026-04-29T15:47:00Z")
                    .ok()
                    .map(|d| d.with_timezone(&chrono::Utc)),
            }),
            secondary: Some(LimitWindow {
                used_percent: secondary_used,
                window_minutes: 10080,
                resets_at: DateTime::parse_from_rfc3339("2026-05-05T06:00:00Z")
                    .ok()
                    .map(|d| d.with_timezone(&chrono::Utc)),
            }),
            credits: None,
            rate_limit_reached_type: None,
        }
    }

    fn mk_recent_call(tool: &'static str, cost: f64, input_tokens: u64) -> ParsedCall {
        ParsedCall {
            tool,
            timestamp: Some(chrono::Utc::now()),
            cost_usd: cost,
            input_tokens,
            session_id: "recent".into(),
            project: "/Users/me/Code/widgets".into(),
            model: "test-model".into(),
            ..ParsedCall::default()
        }
    }

    fn mk_sort_call(
        tool: &'static str,
        session_id: &str,
        project: &str,
        model: &str,
        ts: &str,
        cost: f64,
        input_tokens: u64,
    ) -> ParsedCall {
        ParsedCall {
            tool,
            timestamp: DateTime::parse_from_rfc3339(ts)
                .ok()
                .map(|d| d.with_timezone(&chrono::Utc)),
            cost_usd: cost,
            input_tokens,
            session_id: session_id.into(),
            project: project.into(),
            model: model.into(),
            ..ParsedCall::default()
        }
    }

    #[test]
    fn period_today_filters_correctly() {
        use chrono::TimeZone;
        let now = Local.with_ymd_and_hms(2026, 4, 29, 12, 0, 0).unwrap();
        let now_utc = now.with_timezone(&chrono::Utc);
        let same_day = ParsedCall {
            timestamp: Some(now_utc - Duration::hours(4)),
            ..mk_call("claude-code", "2026-04-29T08:00:00Z", 1.0)
        };
        let previous_day_within_24h = ParsedCall {
            timestamp: Some(now_utc - Duration::hours(23)),
            ..mk_call("claude-code", "2026-04-28T13:00:00Z", 1.0)
        };
        let previous_day_outside_24h = ParsedCall {
            timestamp: Some(now_utc - Duration::hours(25)),
            ..mk_call("claude-code", "2026-04-28T08:00:00Z", 1.0)
        };
        let future = ParsedCall {
            timestamp: Some(now_utc + Duration::minutes(1)),
            ..mk_call("claude-code", "2026-04-29T12:01:00Z", 1.0)
        };
        assert!(in_period(&same_day, Period::Today, now));
        assert!(in_period(&previous_day_within_24h, Period::Today, now));
        assert!(!in_period(&previous_day_outside_24h, Period::Today, now));
        assert!(!in_period(&future, Period::Today, now));
        assert!(in_period(&previous_day_outside_24h, Period::Week, now));
    }

    #[test]
    fn project_labels_use_leaf_names_when_unique() {
        let raw = "/Users/me/Code/ai-commit-dev".to_string();
        let labels = project_label_lookup([&raw]);

        assert_eq!(
            labels.get("/Users/me/Code/ai-commit-dev").unwrap(),
            "ai-commit-dev"
        );
    }

    #[test]
    fn limits_use_latest_snapshot_and_flatten_windows() {
        let ingested = Ingested {
            calls: Vec::new(),
            limits: vec![
                mk_limit("codex", None, "2026-04-29T08:00:00Z", 80.0, 40.0),
                mk_limit("codex", None, "2026-04-29T09:00:00Z", 17.0, 6.0),
            ],
        };

        let data = ingested.limits(Tool::All, SortMode::Spend, &CurrencyFormatter::usd());

        let codex = data
            .sections
            .iter()
            .find(|section| section.tool == "Codex")
            .unwrap();
        assert_eq!(codex.limits.len(), 2);
        assert_eq!(codex.limits[0].tool, "Codex");
        assert_eq!(codex.limits[0].scope, "Codex");
        assert_eq!(codex.limits[0].window, "5h");
        assert_eq!(codex.limits[0].used, 17);
        assert_eq!(codex.limits[0].left, "83% left");
        assert_eq!(codex.limits[0].plan, "Pro Lite");
        assert_eq!(codex.limits[1].window, "weekly");
        assert_eq!(codex.limits[1].used, 6);
        assert_eq!(codex.limits[1].left, "94% left");
        assert_eq!(data.sections.len(), 4);
        assert_eq!(data.sections[0].tool, "Codex");
        assert_eq!(data.sections[1].tool, "Claude Code");
    }

    #[test]
    fn limits_keep_model_specific_rows_and_honor_tool_filter() {
        let ingested = Ingested {
            calls: Vec::new(),
            limits: vec![
                mk_limit("codex", None, "2026-04-29T08:00:00Z", 17.0, 6.0),
                mk_limit(
                    "codex_bengalfox",
                    Some("GPT-5.3-Codex-Spark"),
                    "2026-04-29T08:01:00Z",
                    0.0,
                    0.0,
                ),
            ],
        };

        let codex = ingested.limits(Tool::Codex, SortMode::Spend, &CurrencyFormatter::usd());
        let claude = ingested.limits(Tool::ClaudeCode, SortMode::Spend, &CurrencyFormatter::usd());

        let codex_section = codex
            .sections
            .iter()
            .find(|section| section.tool == "Codex")
            .unwrap();
        let claude_section = claude
            .sections
            .iter()
            .find(|section| section.tool == "Claude Code")
            .unwrap();
        assert_eq!(codex_section.limits.len(), 4);
        assert!(codex
            .sections
            .iter()
            .flat_map(|section| section.limits.iter())
            .any(|row| row.scope == "GPT-5.3-Codex-Spark"));
        assert!(claude_section.limits.is_empty());
        assert_eq!(claude.sections[0].tool, "Codex");
        assert_eq!(claude.sections.len(), 4);
    }

    #[test]
    fn usage_sections_show_last_24_hours_for_all_tools_sorted_by_usage() {
        let ingested = Ingested {
            calls: vec![
                mk_recent_call("codex", 1.0, 100),
                mk_recent_call("claude-code", 2.0, 200),
                mk_recent_call("cursor", 3.0, 300),
                mk_recent_call("copilot", 4.0, 400),
            ],
            limits: Vec::new(),
        };

        let data = ingested.limits(
            Tool::ClaudeCode,
            SortMode::Tokens,
            &CurrencyFormatter::usd(),
        );
        let tools: Vec<&str> = data.sections.iter().map(|row| row.tool).collect();

        assert_eq!(tools, vec!["Copilot", "Cursor", "Claude Code", "Codex"]);
        assert!(data
            .sections
            .iter()
            .all(|section| section.limits.is_empty()));
        assert!(data.sections.iter().all(|section| section.usage.calls == 1));
        assert!(data
            .sections
            .iter()
            .all(|section| section.usage.buckets[23] == 100));
        let claude = data
            .sections
            .iter()
            .find(|section| section.tool == "Claude Code")
            .unwrap();
        assert_eq!(claude.usage.tokens, "200");
        assert_eq!(claude.usage.cost, "$2.00");
        assert_eq!(claude.models[0].name, "test-model");
        assert_eq!(claude.models[0].calls, 1);
    }

    #[test]
    fn dashboard_sort_modes_order_usage_backed_rows() {
        let ingested = Ingested {
            calls: vec![
                mk_sort_call(
                    "codex",
                    "spend",
                    "/Users/me/Code/spend",
                    "spend-model",
                    "2026-04-01T08:00:00Z",
                    10.0,
                    100,
                ),
                mk_sort_call(
                    "codex",
                    "date",
                    "/Users/me/Code/date",
                    "date-model",
                    "2026-04-29T08:00:00Z",
                    5.0,
                    200,
                ),
                mk_sort_call(
                    "codex",
                    "tokens",
                    "/Users/me/Code/tokens",
                    "tokens-model",
                    "2026-04-15T08:00:00Z",
                    1.0,
                    1_000,
                ),
            ],
            limits: Vec::new(),
        };

        let spend = ingested.dashboard(
            Period::AllTime,
            Tool::All,
            &ProjectFilter::All,
            SortMode::Spend,
            &CurrencyFormatter::usd(),
        );
        let date = ingested.dashboard(
            Period::AllTime,
            Tool::All,
            &ProjectFilter::All,
            SortMode::Date,
            &CurrencyFormatter::usd(),
        );
        let tokens = ingested.dashboard(
            Period::AllTime,
            Tool::All,
            &ProjectFilter::All,
            SortMode::Tokens,
            &CurrencyFormatter::usd(),
        );

        assert_eq!(spend.projects[0].name, "spend");
        assert_eq!(date.projects[0].name, "date");
        assert_eq!(tokens.projects[0].name, "tokens");
        assert_eq!(spend.daily[0].day, "04-01");
        assert_eq!(date.daily[0].day, "04-29");
        assert_eq!(tokens.daily[0].day, "04-15");
        assert_eq!(
            tokens
                .activity_timeline
                .iter()
                .map(|row| row.label)
                .collect::<Vec<_>>(),
            vec!["04-01", "04-15", "04-29"]
        );
        assert_eq!(tokens.activity_timeline[0].value, 100);
        assert_eq!(tokens.activity_timeline[1].value, 10);
        assert_eq!(tokens.activity_timeline[2].value, 50);
        assert_eq!(spend.models[0].name, "spend-model");
        assert_eq!(date.models[0].name, "date-model");
        assert_eq!(tokens.models[0].name, "tokens-model");
        assert_eq!(spend.sessions[0].project, "spend");
        assert_eq!(date.sessions[0].project, "date");
        assert_eq!(tokens.sessions[0].project, "tokens");
    }

    #[test]
    fn short_period_activity_timeline_uses_hourly_buckets() {
        let ingested = Ingested {
            calls: vec![mk_recent_call("codex", 1.0, 100)],
            limits: Vec::new(),
        };

        let today = ingested.dashboard(
            Period::Today,
            Tool::All,
            &ProjectFilter::All,
            SortMode::Spend,
            &CurrencyFormatter::usd(),
        );
        let week = ingested.dashboard(
            Period::Week,
            Tool::All,
            &ProjectFilter::All,
            SortMode::Spend,
            &CurrencyFormatter::usd(),
        );

        assert_eq!(today.activity_timeline.len(), 24);
        assert_eq!(week.activity_timeline.len(), 24 * 7);
        assert_eq!(today.activity_timeline.last().unwrap().calls, 1);
        assert_eq!(week.activity_timeline.last().unwrap().calls, 1);
    }

    #[test]
    fn count_tables_sort_by_attributed_spend_and_tokens() {
        let mut shared = mk_sort_call(
            "codex",
            "shared",
            "/Users/me/Code/widgets",
            "shared-model",
            "2026-04-29T08:00:00Z",
            9.0,
            900,
        );
        shared.tools = vec!["Alpha".into(), "Beta".into()];
        shared.bash_commands = vec!["alpha run".into(), "beta run".into()];

        let mut single = mk_sort_call(
            "codex",
            "single",
            "/Users/me/Code/widgets",
            "single-model",
            "2026-04-28T08:00:00Z",
            5.0,
            100,
        );
        single.tools = vec!["Gamma".into()];
        single.bash_commands = vec!["gamma run".into()];

        let ingested = Ingested {
            calls: vec![shared, single],
            limits: Vec::new(),
        };

        let spend = ingested.dashboard(
            Period::AllTime,
            Tool::All,
            &ProjectFilter::All,
            SortMode::Spend,
            &CurrencyFormatter::usd(),
        );
        let tokens = ingested.dashboard(
            Period::AllTime,
            Tool::All,
            &ProjectFilter::All,
            SortMode::Tokens,
            &CurrencyFormatter::usd(),
        );

        assert_eq!(spend.tools[0].name, "Gamma");
        assert_eq!(tokens.tools[0].name, "Alpha");
        assert_eq!(spend.commands[0].name, "gamma");
        assert_eq!(tokens.commands[0].name, "alpha");
        assert_eq!(tokens.tools[0].calls, 1);
    }

    #[test]
    fn usage_sections_can_sort_by_latest_activity() {
        let now = chrono::Utc::now();
        let mut codex = mk_recent_call("codex", 10.0, 100);
        codex.timestamp = Some(now - chrono::Duration::hours(3));
        let mut cursor = mk_recent_call("cursor", 1.0, 50);
        cursor.timestamp = Some(now - chrono::Duration::minutes(5));

        let ingested = Ingested {
            calls: vec![codex, cursor],
            limits: Vec::new(),
        };

        let data = ingested.limits(Tool::All, SortMode::Date, &CurrencyFormatter::usd());

        assert_eq!(data.sections[0].tool, "Cursor");
        assert_eq!(data.sections[1].tool, "Codex");
    }

    #[test]
    fn session_detail_uses_active_sort_mode() {
        let ingested = Ingested {
            calls: vec![
                mk_sort_call(
                    "other",
                    "s1",
                    "/Users/me/Code/widgets",
                    "spend-model",
                    "2026-04-01T08:00:00Z",
                    10.0,
                    100,
                ),
                mk_sort_call(
                    "other",
                    "s1",
                    "/Users/me/Code/widgets",
                    "date-model",
                    "2026-04-29T08:00:00Z",
                    2.0,
                    200,
                ),
                mk_sort_call(
                    "other",
                    "s1",
                    "/Users/me/Code/widgets",
                    "tokens-model",
                    "2026-04-15T08:00:00Z",
                    1.0,
                    1_000,
                ),
            ],
            limits: Vec::new(),
        };

        let spend = ingested
            .session_detail("other:s1", SortMode::Spend, &CurrencyFormatter::usd())
            .unwrap();
        let date = ingested
            .session_detail("other:s1", SortMode::Date, &CurrencyFormatter::usd())
            .unwrap();
        let tokens = ingested
            .session_detail("other:s1", SortMode::Tokens, &CurrencyFormatter::usd())
            .unwrap();

        assert_eq!(spend.calls[0].model, "spend-model");
        assert_eq!(date.calls[0].model, "date-model");
        assert_eq!(tokens.calls[0].model, "tokens-model");
    }

    #[test]
    fn project_costs_roll_up_across_tools() {
        let ingested = Ingested {
            calls: vec![
                mk_project_call("claude-code", "s1", "/Users/me/Code/widgets", 2.0),
                mk_project_call("codex", "s1", "/Users/me/Code/widgets", 3.0),
                mk_project_call("cursor", "s2", "/Users/me/Code/widgets", 5.0),
            ],
            limits: Vec::new(),
        };

        let data = ingested.dashboard(
            Period::AllTime,
            Tool::All,
            &ProjectFilter::All,
            SortMode::Spend,
            &CurrencyFormatter::usd(),
        );

        assert_eq!(data.projects.len(), 1);
        assert_eq!(data.projects[0].name, "widgets");
        assert_eq!(data.projects[0].cost, "$10.00");
        assert_eq!(data.projects[0].sessions, 3);
        assert!(data.projects[0].tool_mix.contains("Cursor $5.00"));
        assert!(data.projects[0].tool_mix.contains("Codex $3.00"));
        assert!(data.projects[0].tool_mix.contains("Claude $2.00"));
        assert_eq!(data.project_tools.len(), 3);
    }

    #[test]
    fn project_costs_roll_up_claude_codex_real_cwd_with_hyphens() {
        let ingested = Ingested {
            calls: vec![
                mk_project_call("claude-code", "s1", "/Users/me/Code/ai-commit-dev", 2.0),
                mk_project_call("codex", "s2", "/Users/me/Code/ai-commit-dev", 3.0),
            ],
            limits: Vec::new(),
        };

        let data = ingested.dashboard(
            Period::AllTime,
            Tool::All,
            &ProjectFilter::All,
            SortMode::Spend,
            &CurrencyFormatter::usd(),
        );

        assert_eq!(data.projects.len(), 1);
        assert_eq!(data.projects[0].name, "ai-commit-dev");
        assert_eq!(data.projects[0].cost, "$5.00");
        assert_eq!(data.project_tools.len(), 2);
    }

    #[test]
    fn session_detail_exposes_modal_call_fields() {
        let mut call = mk_project_call("codex", "s1", "/Users/me/Code/widgets", 1.0);
        call.input_tokens = 100;
        call.output_tokens = 50;
        call.cache_read_input_tokens = 20;
        call.cache_creation_input_tokens = 5;
        call.reasoning_tokens = 7;
        call.web_search_requests = 2;
        call.tools = vec!["exec_command".into()];
        call.bash_commands = vec!["cargo test".into()];
        call.user_message = "run the checks\nand show me failures".into();
        let ingested = Ingested {
            calls: vec![call],
            limits: Vec::new(),
        };

        let detail = ingested
            .session_detail("codex:s1", SortMode::Spend, &CurrencyFormatter::usd())
            .unwrap();
        let call = &detail.calls[0];

        assert_eq!(call.reasoning_tokens, 7);
        assert_eq!(call.web_search_requests, 2);
        assert_eq!(call.bash_commands, vec!["cargo test"]);
        assert_eq!(call.prompt_full, "run the checks and show me failures");
        assert_eq!(call.prompt, "run the checks and show me failures");
    }

    #[test]
    fn project_inventory_uses_compact_labels_and_trims_raw_projects() {
        let ingested = Ingested {
            calls: vec![
                mk_project_call("claude-code", "s1", "/Users/me/Code/widgets", 2.0),
                mk_project_call("codex", "s2", "/Users/me/Code/widgets", 3.0),
                mk_project_call("codex", "s3", "/Users/me/Code/widgets/", 5.0),
            ],
            limits: Vec::new(),
        };

        let rows = ingested.project_inventory();

        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(|row| row.project == "widgets"));
        assert!(rows
            .iter()
            .any(|row| row.tool == "Claude" && row.raw_project == "/Users/me/Code/widgets"));
        assert!(rows
            .iter()
            .any(|row| row.tool == "Codex" && row.raw_project == "/Users/me/Code/widgets"));
    }

    #[test]
    fn project_options_are_unique_across_tools() {
        let ingested = Ingested {
            calls: vec![
                mk_project_call("claude-code", "s1", "/Users/me/Code/widgets", 2.0),
                mk_project_call("codex", "s2", "/Users/me/Code/widgets", 3.0),
                mk_project_call("codex", "s3", "/Users/me/Code/widgets/", 5.0),
            ],
            limits: Vec::new(),
        };

        let options = ingested.project_options(
            Period::AllTime,
            Tool::All,
            SortMode::Spend,
            &CurrencyFormatter::usd(),
        );

        assert_eq!(options.len(), 2);
        assert_eq!(options[0].identity, None);
        assert_eq!(options[0].label, "All");
        assert_eq!(options[1].label, "widgets");
        assert_eq!(options[1].calls, 3);
        assert_eq!(options[1].cost, "$10.00");
    }

    #[test]
    fn project_labels_disambiguate_leaf_collisions_with_shortest_suffixes() {
        let ingested = Ingested {
            calls: vec![
                mk_project_call("claude-code", "s1", "/Users/me/Code/tokens", 2.0),
                mk_project_call("codex", "s2", "/Users/me/Code/dvr/tokens", 3.0),
            ],
            limits: Vec::new(),
        };

        let data = ingested.dashboard(
            Period::AllTime,
            Tool::All,
            &ProjectFilter::All,
            SortMode::Spend,
            &CurrencyFormatter::usd(),
        );
        let names: HashSet<&str> = data.projects.iter().map(|project| project.name).collect();

        assert_eq!(data.projects.len(), 2);
        assert!(names.contains("Code/tokens"));
        assert!(names.contains("dvr/tokens"));
    }

    #[test]
    fn project_identity_uses_existing_git_root_for_nested_cwd() {
        let tmp = TempDir::new("git-root");
        let dvr = tmp.path().join("dvr");
        let nested_dvr = dvr.join("tokens");
        let tokens = tmp.path().join("tokens");
        std::fs::create_dir_all(dvr.join(".git")).unwrap();
        std::fs::create_dir_all(&nested_dvr).unwrap();
        std::fs::create_dir_all(tokens.join(".git")).unwrap();

        let nested_dvr = path_to_project_string(&nested_dvr);
        let dvr = path_to_project_string(&dvr);
        let tokens = path_to_project_string(&tokens);

        assert_eq!(project_identity(&nested_dvr), dvr);

        let ingested = Ingested {
            calls: vec![
                mk_project_call("codex", "s1", &nested_dvr, 2.0),
                mk_project_call("claude-code", "s2", &tokens, 3.0),
            ],
            limits: Vec::new(),
        };

        let data = ingested.dashboard(
            Period::AllTime,
            Tool::All,
            &ProjectFilter::All,
            SortMode::Spend,
            &CurrencyFormatter::usd(),
        );
        let names: HashSet<&str> = data.projects.iter().map(|project| project.name).collect();
        assert_eq!(data.projects.len(), 2);
        assert!(names.contains("dvr"));
        assert!(names.contains("tokens"));

        let rows = ingested.project_inventory();
        assert!(rows
            .iter()
            .any(|row| row.project == "dvr" && row.raw_project == nested_dvr));
    }

    #[test]
    fn tool_filter_keeps_project_costs_tool_local() {
        let ingested = Ingested {
            calls: vec![
                mk_project_call("claude-code", "s1", "/Users/me/Code/widgets", 2.0),
                mk_project_call("codex", "s1", "/Users/me/Code/widgets", 3.0),
            ],
            limits: Vec::new(),
        };

        let data = ingested.dashboard(
            Period::AllTime,
            Tool::Codex,
            &ProjectFilter::All,
            SortMode::Spend,
            &CurrencyFormatter::usd(),
        );

        assert_eq!(data.projects.len(), 1);
        assert_eq!(data.projects[0].cost, "$3.00");
        assert_eq!(data.projects[0].tool_mix, "Codex $3.00");
        assert_eq!(data.project_tools.len(), 1);
        assert_eq!(data.project_tools[0].tool, "Codex");
    }

    #[test]
    fn project_filter_applies_before_all_aggregations() {
        let mut widgets = mk_project_call("codex", "s1", "/Users/me/Code/widgets", 2.0);
        widgets.model = "gpt-5".into();
        widgets.tools = vec!["Bash".into(), "mcp__linear__search".into()];
        widgets.bash_commands = vec!["cargo test".into()];

        let mut blog = mk_project_call("claude-code", "s2", "/Users/me/Code/blog", 5.0);
        blog.model = "claude-opus-4-7".into();
        blog.tools = vec!["Read".into(), "mcp__github__search".into()];
        blog.bash_commands = vec!["rg widgets".into()];

        let ingested = Ingested {
            calls: vec![widgets, blog],
            limits: Vec::new(),
        };
        let filter = ProjectFilter::Selected {
            identity: "/Users/me/Code/widgets".into(),
            label: "widgets".into(),
        };

        let data = ingested.dashboard(
            Period::AllTime,
            Tool::All,
            &filter,
            SortMode::Spend,
            &CurrencyFormatter::usd(),
        );

        assert_eq!(data.summary.calls, "1");
        assert_eq!(data.summary.cost, "$2.00");
        assert_eq!(data.projects.len(), 1);
        assert_eq!(data.projects[0].name, "widgets");
        assert_eq!(data.sessions.len(), 1);
        assert_eq!(data.sessions[0].project, "widgets");
        assert_eq!(data.models.len(), 1);
        assert_eq!(data.models[0].name, "GPT-5");
        assert_eq!(data.tools.len(), 1);
        assert_eq!(data.tools[0].name, "Bash");
        assert_eq!(data.commands.len(), 1);
        assert_eq!(data.commands[0].name, "cargo");
        assert_eq!(data.mcp_servers.len(), 1);
        assert_eq!(data.mcp_servers[0].name, "linear");
    }

    #[test]
    fn session_counts_are_tool_qualified() {
        let ingested = Ingested {
            calls: vec![
                mk_project_call("claude-code", "shared", "/Users/me/Code/widgets", 1.0),
                mk_project_call("claude-code", "shared", "/Users/me/Code/widgets", 2.0),
                mk_project_call("codex", "shared", "/Users/me/Code/widgets", 3.0),
            ],
            limits: Vec::new(),
        };

        let data = ingested.dashboard(
            Period::AllTime,
            Tool::All,
            &ProjectFilter::All,
            SortMode::Spend,
            &CurrencyFormatter::usd(),
        );

        assert_eq!(data.summary.sessions, "2");
        assert_eq!(data.projects[0].sessions, 2);
        assert_eq!(data.projects[0].avg_per_session, "$3.00");
    }

    #[test]
    fn project_tool_rows_sort_by_row_spend() {
        let ingested = Ingested {
            calls: vec![
                mk_project_call("claude-code", "a1", "/Users/me/Code/a", 2.0),
                mk_project_call("codex", "a2", "/Users/me/Code/a", 9.0),
                mk_project_call("cursor", "b1", "/Users/me/Code/b", 10.0),
            ],
            limits: Vec::new(),
        };

        let data = ingested.dashboard(
            Period::AllTime,
            Tool::All,
            &ProjectFilter::All,
            SortMode::Spend,
            &CurrencyFormatter::usd(),
        );

        assert_eq!(data.project_tools[0].project, "b");
        assert_eq!(data.project_tools[0].tool, "Cursor");
        assert_eq!(data.project_tools[1].project, "a");
        assert_eq!(data.project_tools[1].tool, "Codex");
        assert_eq!(data.project_tools[2].project, "a");
        assert_eq!(data.project_tools[2].tool, "Claude");
    }
}
