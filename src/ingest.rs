use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::Path;

use chrono::{DateTime, Datelike, Duration, Local, NaiveDate};
use color_eyre::Result;

use crate::app::{Period, ProjectFilter, Tool};
use crate::currency::CurrencyFormatter;
use crate::data::{
    CountMetric, DailyMetric, DashboardData, LimitMetric, LimitsData, ModelMetric, ProjectMetric,
    ProjectOption, ProjectToolMetric, RecentModelMetric, RecentUsageMetric, SessionDetail,
    SessionDetailView, SessionMetric, SessionOption, Summary, ToolLimitSection,
};
use crate::tools::{self, LimitSnapshot, LimitWindow, ParsedCall};

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
        build_dashboard(&filtered, currency)
    }

    pub fn project_options(
        &self,
        period: Period,
        tool: Tool,
        currency: &CurrencyFormatter,
    ) -> Vec<ProjectOption> {
        let now = Local::now();
        let filtered: Vec<&ParsedCall> = self
            .calls
            .iter()
            .filter(|c| matches_tool(c, tool) && in_period(c, period, now))
            .collect();
        build_project_options(&filtered, currency)
    }

    pub fn limits(&self, tool: Tool, currency: &CurrencyFormatter) -> LimitsData {
        build_limits_data(&self.limits, &self.calls, tool, currency)
    }

    pub fn session_options(
        &self,
        period: Period,
        tool: Tool,
        project_filter: &ProjectFilter,
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
        build_session_options(&filtered, currency)
    }

    pub fn session_detail(
        &self,
        key: &str,
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
        Some(build_session_detail(key, &matching, currency))
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
        Period::Today => date == today,
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
        sections: build_tool_limit_sections(calls, limits_by_tool, currency),
    }
}

#[derive(Default)]
struct ModelAcc {
    calls: u64,
    tokens: u64,
    cost: f64,
}

fn build_tool_limit_sections(
    calls: &[ParsedCall],
    mut limits_by_tool: HashMap<&'static str, Vec<LimitMetric>>,
    currency: &CurrencyFormatter,
) -> Vec<ToolLimitSection> {
    #[derive(Default)]
    struct Acc {
        buckets: [u64; 24],
        calls: u64,
        tokens: u64,
        cost: f64,
        last_seen: Option<DateTime<Local>>,
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

        let model = display_lookup
            .get(call.tool)
            .map(|adapter| adapter.model_display(&call.model))
            .unwrap_or_else(|| call.model.clone());
        let model_acc = acc.models.entry(model).or_default();
        model_acc.calls += 1;
        model_acc.tokens = model_acc.tokens.saturating_add(tokens);
        model_acc.cost += call.cost_usd;
    }

    let mut ordered: Vec<(usize, &str, &str, Acc)> = TOOLS
        .into_iter()
        .enumerate()
        .map(|(idx, (id, label))| (idx, id, label, by_tool.remove(id).unwrap_or_default()))
        .collect();
    ordered.sort_by(|a, b| b.3.tokens.cmp(&a.3.tokens).then_with(|| a.0.cmp(&b.0)));

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
            models: recent_model_rows(acc.models, currency),
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
    currency: &CurrencyFormatter,
) -> Vec<RecentModelMetric> {
    let mut rows: Vec<(String, u64, u64, f64)> = models
        .into_iter()
        .map(|(name, acc)| (name, acc.calls, acc.tokens, acc.cost))
        .collect();
    rows.sort_by(|a, b| {
        b.2.cmp(&a.2)
            .then_with(|| b.3.partial_cmp(&a.3).unwrap_or(std::cmp::Ordering::Equal))
            .then_with(|| b.1.cmp(&a.1))
            .then_with(|| a.0.cmp(&b.0))
    });
    let max = rows.first().map(|row| row.2).unwrap_or(0);

    rows.into_iter()
        .take(3)
        .map(|(name, calls, tokens, cost)| RecentModelMetric {
            name: leak(name),
            calls,
            tokens: leak(format_compact(tokens)),
            cost: leak(currency.format_money(cost)),
            value: if max == 0 {
                0
            } else {
                ((tokens as f64 / max as f64) * 100.0)
                    .round()
                    .clamp(1.0, 100.0) as u64
            },
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

fn build_dashboard(calls: &[&ParsedCall], currency: &CurrencyFormatter) -> DashboardData {
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

    let daily = aggregate_daily(calls, currency);
    let projects = aggregate_projects(calls, &project_labels, currency);
    let project_tools = aggregate_project_tools(calls, &project_labels, currency);
    let sessions = aggregate_sessions(calls, &project_labels, currency);
    let models = aggregate_models(calls, currency);
    let tools = aggregate_tools(calls);
    let commands = aggregate_commands(calls);
    let mcp_servers = aggregate_mcp(calls);

    DashboardData {
        summary,
        daily,
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
    currency: &CurrencyFormatter,
) -> Vec<ProjectOption> {
    #[derive(Default)]
    struct Acc {
        cost: f64,
        calls: u64,
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
    }

    let mut rows: Vec<(String, String, Acc)> = by_project
        .into_iter()
        .map(|(identity, acc)| {
            let label = project_label(&labels, &identity);
            (identity, label, acc)
        })
        .collect();

    rows.sort_by(|a, b| {
        b.2.cost
            .partial_cmp(&a.2.cost)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.2.calls.cmp(&a.2.calls))
            .then_with(|| a.1.cmp(&b.1))
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

fn aggregate_daily(calls: &[&ParsedCall], currency: &CurrencyFormatter) -> Vec<DailyMetric> {
    let mut by_day: BTreeMap<NaiveDate, (f64, u64)> = BTreeMap::new();
    for c in calls {
        let Some(ts) = c.timestamp else { continue };
        let date = ts.with_timezone(&Local).date_naive();
        let entry = by_day.entry(date).or_insert((0.0, 0));
        entry.0 += c.cost_usd;
        entry.1 += 1;
    }
    let max = by_day
        .values()
        .map(|(cost, _)| *cost)
        .fold(0.0_f64, f64::max);
    by_day
        .into_iter()
        .map(|(date, (cost, calls))| DailyMetric {
            day: leak(date.format("%m-%d").to_string()),
            cost: leak(currency.format_money(cost)),
            calls,
            value: scale(cost, max),
        })
        .collect()
}

fn aggregate_projects(
    calls: &[&ParsedCall],
    project_labels: &HashMap<String, String>,
    currency: &CurrencyFormatter,
) -> Vec<ProjectMetric> {
    #[derive(Default)]
    struct Acc {
        cost: f64,
        sessions: HashSet<String>,
        tools: HashMap<&'static str, f64>,
    }
    let mut by_project: HashMap<String, Acc> = HashMap::new();
    for c in calls {
        let entry = by_project.entry(project_identity(&c.project)).or_default();
        entry.cost += c.cost_usd;
        if let Some(key) = session_key(c) {
            entry.sessions.insert(key);
        }
        *entry.tools.entry(c.tool).or_default() += c.cost_usd;
    }

    let mut rows: Vec<(String, Acc)> = by_project.into_iter().collect();
    rows.sort_by(|a, b| {
        b.1.cost
            .partial_cmp(&a.1.cost)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0.cmp(&b.0))
    });
    let max = rows.first().map(|r| r.1.cost).unwrap_or(0.0);

    rows.into_iter()
        .take(10)
        .map(|(project, acc)| {
            let session_count = acc.sessions.len().max(1) as u64;
            let avg = acc.cost / session_count as f64;
            ProjectMetric {
                name: leak(project_label(project_labels, &project)),
                cost: leak(currency.format_money(acc.cost)),
                avg_per_session: leak(currency.format_money(avg)),
                sessions: session_count,
                tool_mix: leak(format_tool_mix(&acc.tools, currency)),
                value: scale(acc.cost, max),
            }
        })
        .collect()
}

fn aggregate_project_tools(
    calls: &[&ParsedCall],
    project_labels: &HashMap<String, String>,
    currency: &CurrencyFormatter,
) -> Vec<ProjectToolMetric> {
    #[derive(Default)]
    struct Acc {
        cost: f64,
        calls: u64,
        sessions: HashSet<String>,
    }

    let mut project_totals: HashMap<String, f64> = HashMap::new();
    let mut by_pair: HashMap<(String, &'static str), Acc> = HashMap::new();

    for c in calls {
        let project = project_identity(&c.project);
        *project_totals.entry(project.clone()).or_default() += c.cost_usd;

        let entry = by_pair.entry((project, c.tool)).or_default();
        entry.cost += c.cost_usd;
        entry.calls += 1;
        if let Some(key) = session_key(c) {
            entry.sessions.insert(key);
        }
    }

    let mut rows: Vec<(String, &'static str, f64, Acc)> = by_pair
        .into_iter()
        .map(|((project, tool), acc)| {
            let total = *project_totals.get(&project).unwrap_or(&0.0);
            (project, tool, total, acc)
        })
        .collect();

    rows.sort_by(|a, b| {
        b.2.partial_cmp(&a.2)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                b.3.cost
                    .partial_cmp(&a.3.cost)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| a.0.cmp(&b.0))
            .then_with(|| tool_short_label(a.1).cmp(tool_short_label(b.1)))
    });
    let max = rows.iter().map(|r| r.3.cost).fold(0.0_f64, f64::max);

    rows.into_iter()
        .take(12)
        .map(|(project, tool, _, acc)| {
            let session_count = acc.sessions.len().max(1) as u64;
            let avg = acc.cost / session_count as f64;
            ProjectToolMetric {
                project: leak(project_label(project_labels, &project)),
                tool: tool_short_label(tool),
                cost: leak(currency.format_money(acc.cost)),
                calls: acc.calls,
                sessions: session_count,
                avg_per_session: leak(currency.format_money(avg)),
                value: scale(acc.cost, max),
            }
        })
        .collect()
}

fn aggregate_sessions(
    calls: &[&ParsedCall],
    project_labels: &HashMap<String, String>,
    currency: &CurrencyFormatter,
) -> Vec<SessionMetric> {
    #[derive(Default)]
    struct Acc {
        cost: f64,
        calls: u64,
        date: Option<NaiveDate>,
        project: String,
    }
    let mut by_session: HashMap<String, Acc> = HashMap::new();
    for c in calls {
        let Some(key) = session_key(c) else {
            continue;
        };
        let entry = by_session.entry(key).or_default();
        entry.cost += c.cost_usd;
        entry.calls += 1;
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
        b.cost
            .partial_cmp(&a.cost)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let max = rows.first().map(|r| r.cost).unwrap_or(0.0);

    rows.into_iter()
        .take(10)
        .map(|acc| SessionMetric {
            date: leak(
                acc.date
                    .map(|d| d.format("%Y-%m-%d").to_string())
                    .unwrap_or_else(|| "-".into()),
            ),
            project: leak(project_label(project_labels, &acc.project)),
            cost: leak(currency.format_money(acc.cost)),
            calls: acc.calls,
            value: scale(acc.cost, max),
        })
        .collect()
}

fn build_session_options(
    calls: &[&ParsedCall],
    currency: &CurrencyFormatter,
) -> Vec<SessionOption> {
    #[derive(Default)]
    struct Acc {
        cost: f64,
        calls: u64,
        date: Option<NaiveDate>,
        project: String,
        tool: &'static str,
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
        b.1.cost
            .partial_cmp(&a.1.cost)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.1.calls.cmp(&a.1.calls))
    });
    let max = rows.first().map(|r| r.1.cost).unwrap_or(0.0);

    rows.into_iter()
        .map(|(key, acc)| SessionOption {
            key,
            date: acc
                .date
                .map(|d| d.format("%Y-%m-%d").to_string())
                .unwrap_or_else(|| "-".into()),
            project: project_label(&labels, &acc.project),
            tool: tool_short_label(acc.tool),
            cost: currency.format_money(acc.cost),
            calls: acc.calls,
            value: scale(acc.cost, max),
        })
        .collect()
}

fn build_session_detail(
    key: &str,
    calls: &[&ParsedCall],
    currency: &CurrencyFormatter,
) -> SessionDetailView {
    let mut sorted: Vec<&ParsedCall> = calls.to_vec();
    sorted.sort_by_key(|c| c.timestamp);

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
                tools: tools_text,
                prompt: snippet(&c.user_message, 120),
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

fn snippet(text: &str, max: usize) -> String {
    let cleaned: String = text
        .chars()
        .map(|c| {
            if c == '\n' || c == '\r' || c == '\t' {
                ' '
            } else if c.is_control() {
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
    let trimmed = compacted.trim();
    if trimmed.chars().count() <= max {
        trimmed.to_string()
    } else {
        let mut out: String = trimmed.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

fn aggregate_models(calls: &[&ParsedCall], currency: &CurrencyFormatter) -> Vec<ModelMetric> {
    #[derive(Default)]
    struct Acc {
        cost: f64,
        calls: u64,
        cache_read: u64,
        input: u64,
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
    }

    let mut rows: Vec<(String, Acc)> = by_model.into_iter().collect();
    rows.sort_by(|a, b| {
        b.1.cost
            .partial_cmp(&a.1.cost)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let max = rows.first().map(|r| r.1.cost).unwrap_or(0.0);

    rows.into_iter()
        .map(|(name, acc)| ModelMetric {
            name: leak(name),
            cost: leak(currency.format_money(acc.cost)),
            cache: leak(if acc.input == 0 {
                "-".into()
            } else {
                format!("{:.1}%", (acc.cache_read as f64 / acc.input as f64) * 100.0)
            }),
            calls: acc.calls,
            value: scale(acc.cost, max),
        })
        .collect()
}

fn aggregate_tools(calls: &[&ParsedCall]) -> Vec<CountMetric> {
    let mut counts: HashMap<String, u64> = HashMap::new();
    for c in calls {
        for t in &c.tools {
            if t.starts_with("mcp__") {
                continue;
            }
            *counts.entry(t.clone()).or_default() += 1;
        }
    }
    top_counts(counts, 10)
}

fn aggregate_commands(calls: &[&ParsedCall]) -> Vec<CountMetric> {
    let mut counts: HashMap<String, u64> = HashMap::new();
    for c in calls {
        for cmd in &c.bash_commands {
            let head = tools::jsonl::first_word(cmd);
            if head.is_empty() {
                continue;
            }
            *counts.entry(head).or_default() += 1;
        }
    }
    top_counts(counts, 10)
}

fn aggregate_mcp(calls: &[&ParsedCall]) -> Vec<CountMetric> {
    let mut counts: HashMap<String, u64> = HashMap::new();
    for c in calls {
        for t in &c.tools {
            if let Some(rest) = t.strip_prefix("mcp__") {
                let server = rest.split("__").next().unwrap_or(rest).to_string();
                *counts.entry(server).or_default() += 1;
            }
        }
    }
    top_counts(counts, 10)
}

fn top_counts(counts: HashMap<String, u64>, limit: usize) -> Vec<CountMetric> {
    let mut rows: Vec<(String, u64)> = counts.into_iter().collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1));
    let max = rows.first().map(|r| r.1).unwrap_or(0);
    rows.into_iter()
        .take(limit)
        .map(|(name, calls)| CountMetric {
            name: leak(name),
            calls,
            value: if max == 0 {
                0
            } else {
                (calls * 100 / max).max(1)
            },
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
        if i > 0 && (bytes.len() - i) % 3 == 0 {
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

    #[test]
    fn period_today_filters_correctly() {
        use chrono::TimeZone;
        let now = Local.with_ymd_and_hms(2026, 4, 29, 12, 0, 0).unwrap();
        let same_day = mk_call("claude-code", "2026-04-29T08:00:00Z", 1.0);
        let yesterday = mk_call("claude-code", "2026-04-28T08:00:00Z", 1.0);
        assert!(in_period(&same_day, Period::Today, now));
        assert!(!in_period(&yesterday, Period::Today, now));
        assert!(in_period(&yesterday, Period::Week, now));
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

        let data = ingested.limits(Tool::All, &CurrencyFormatter::usd());

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

        let codex = ingested.limits(Tool::Codex, &CurrencyFormatter::usd());
        let claude = ingested.limits(Tool::ClaudeCode, &CurrencyFormatter::usd());

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

        let data = ingested.limits(Tool::ClaudeCode, &CurrencyFormatter::usd());
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
            &CurrencyFormatter::usd(),
        );

        assert_eq!(data.projects.len(), 1);
        assert_eq!(data.projects[0].name, "ai-commit-dev");
        assert_eq!(data.projects[0].cost, "$5.00");
        assert_eq!(data.project_tools.len(), 2);
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

        let options =
            ingested.project_options(Period::AllTime, Tool::All, &CurrencyFormatter::usd());

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
            &CurrencyFormatter::usd(),
        );

        assert_eq!(data.summary.sessions, "2");
        assert_eq!(data.projects[0].sessions, 2);
        assert_eq!(data.projects[0].avg_per_session, "$3.00");
    }

    #[test]
    fn project_tool_rows_sort_by_project_total_then_tool_cost() {
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
            &CurrencyFormatter::usd(),
        );

        assert_eq!(data.project_tools[0].project, "a");
        assert_eq!(data.project_tools[0].tool, "Codex");
        assert_eq!(data.project_tools[1].project, "a");
        assert_eq!(data.project_tools[1].tool, "Claude");
        assert_eq!(data.project_tools[2].project, "b");
        assert_eq!(data.project_tools[2].tool, "Cursor");
    }
}
