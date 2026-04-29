use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::Path;

use chrono::{DateTime, Datelike, Duration, Local, NaiveDate};
use color_eyre::Result;

use crate::app::{Period, ProjectFilter, Tool};
use crate::data::{
    CountMetric, DailyMetric, DashboardData, ModelMetric, ProjectMetric, ProjectOption,
    ProjectToolMetric, SessionMetric, Summary,
};
use crate::tools::{self, ParsedCall};

pub struct Ingested {
    pub calls: Vec<ParsedCall>,
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

    for tool in tools::registry() {
        let sources = match tool.discover() {
            Ok(s) => s,
            Err(_) => continue,
        };
        for source in sources {
            match tool.parse(&source, &mut seen) {
                Ok(mut more) => calls.append(&mut more),
                Err(_) => continue,
            }
        }
    }

    Ok(Ingested { calls })
}

impl Ingested {
    pub fn dashboard(
        &self,
        period: Period,
        tool: Tool,
        project_filter: &ProjectFilter,
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
        build_dashboard(&filtered)
    }

    pub fn project_options(&self, period: Period, tool: Tool) -> Vec<ProjectOption> {
        let now = Local::now();
        let filtered: Vec<&ParsedCall> = self
            .calls
            .iter()
            .filter(|c| matches_tool(c, tool) && in_period(c, period, now))
            .collect();
        build_project_options(&filtered)
    }

    pub fn is_empty(&self) -> bool {
        self.calls.is_empty()
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
                ProjectInventoryRow {
                    project: label,
                    tool: tool_short_label(tool),
                    raw_project,
                    calls: acc.calls,
                    sessions: acc.sessions.len() as u64,
                    cost: format_money(acc.cost),
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

fn build_dashboard(calls: &[&ParsedCall]) -> DashboardData {
    if calls.is_empty() {
        return empty_dashboard();
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
        cost: leak(format_money(total_cost)),
        calls: leak(format_int(calls.len() as u64)),
        sessions: leak(format_int(sessions_set.len() as u64)),
        cache_hit: leak(format!("{:.1}%", cache_hit_pct)),
        input: leak(format_compact(total_input)),
        output: leak(format_compact(total_output)),
        cached: leak(format_compact(total_cache_read)),
        written: leak(format_compact(total_cache_write)),
    };

    let project_labels = project_label_lookup(calls.iter().map(|call| &call.project));

    let daily = aggregate_daily(calls);
    let projects = aggregate_projects(calls, &project_labels);
    let project_tools = aggregate_project_tools(calls, &project_labels);
    let sessions = aggregate_sessions(calls, &project_labels);
    let models = aggregate_models(calls);
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

fn empty_dashboard() -> DashboardData {
    DashboardData {
        summary: Summary {
            cost: "$0.00",
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

fn build_project_options(calls: &[&ParsedCall]) -> Vec<ProjectOption> {
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
        format_money(total_cost),
        calls.len() as u64,
    )];
    options.extend(rows.into_iter().map(|(identity, label, acc)| {
        ProjectOption::selected(identity, label, format_money(acc.cost), acc.calls)
    }));
    options
}

fn aggregate_daily(calls: &[&ParsedCall]) -> Vec<DailyMetric> {
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
            cost: leak(format_money(cost)),
            calls,
            value: scale(cost, max),
        })
        .collect()
}

fn aggregate_projects(
    calls: &[&ParsedCall],
    project_labels: &HashMap<String, String>,
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
                cost: leak(format_money(acc.cost)),
                avg_per_session: leak(format_money(avg)),
                sessions: session_count,
                tool_mix: leak(format_tool_mix(&acc.tools)),
                value: scale(acc.cost, max),
            }
        })
        .collect()
}

fn aggregate_project_tools(
    calls: &[&ParsedCall],
    project_labels: &HashMap<String, String>,
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
                cost: leak(format_money(acc.cost)),
                calls: acc.calls,
                sessions: session_count,
                avg_per_session: leak(format_money(avg)),
                value: scale(acc.cost, max),
            }
        })
        .collect()
}

fn aggregate_sessions(
    calls: &[&ParsedCall],
    project_labels: &HashMap<String, String>,
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
            cost: leak(format_money(acc.cost)),
            calls: acc.calls,
            value: scale(acc.cost, max),
        })
        .collect()
}

fn aggregate_models(calls: &[&ParsedCall]) -> Vec<ModelMetric> {
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
            cost: leak(format_money(acc.cost)),
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

fn format_tool_mix(tools: &HashMap<&'static str, f64>) -> String {
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
        .map(|(tool, cost)| format!("{} {}", tool_short_label(tool), format_money_short(cost)))
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

fn format_money(amount: f64) -> String {
    if amount >= 100.0 {
        format!("${:.2}", amount)
    } else if amount >= 1.0 {
        format!("${:.2}", amount)
    } else if amount >= 0.01 {
        format!("${:.3}", amount)
    } else {
        format!("${:.4}", amount)
    }
}

fn format_money_short(amount: f64) -> String {
    if amount >= 100.0 {
        format!("${:.0}", amount)
    } else if amount >= 10.0 {
        format!("${:.1}", amount)
    } else if amount >= 1.0 {
        format!("${:.2}", amount)
    } else if amount >= 0.01 {
        format!("${:.2}", amount)
    } else {
        format!("${:.4}", amount)
    }
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
            let path = std::env::temp_dir().join(format!(
                "tokenuse-ingest-{}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
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
    fn project_costs_roll_up_across_tools() {
        let ingested = Ingested {
            calls: vec![
                mk_project_call("claude-code", "s1", "/Users/me/Code/widgets", 2.0),
                mk_project_call("codex", "s1", "/Users/me/Code/widgets", 3.0),
                mk_project_call("cursor", "s2", "/Users/me/Code/widgets", 5.0),
            ],
        };

        let data = ingested.dashboard(Period::AllTime, Tool::All, &ProjectFilter::All);

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
        };

        let data = ingested.dashboard(Period::AllTime, Tool::All, &ProjectFilter::All);

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
        };

        let options = ingested.project_options(Period::AllTime, Tool::All);

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
        };

        let data = ingested.dashboard(Period::AllTime, Tool::All, &ProjectFilter::All);
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
        };

        let data = ingested.dashboard(Period::AllTime, Tool::All, &ProjectFilter::All);
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
        };

        let data = ingested.dashboard(Period::AllTime, Tool::Codex, &ProjectFilter::All);

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
        };
        let filter = ProjectFilter::Selected {
            identity: "/Users/me/Code/widgets".into(),
            label: "widgets".into(),
        };

        let data = ingested.dashboard(Period::AllTime, Tool::All, &filter);

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
        };

        let data = ingested.dashboard(Period::AllTime, Tool::All, &ProjectFilter::All);

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
        };

        let data = ingested.dashboard(Period::AllTime, Tool::All, &ProjectFilter::All);

        assert_eq!(data.project_tools[0].project, "a");
        assert_eq!(data.project_tools[0].tool, "Codex");
        assert_eq!(data.project_tools[1].project, "a");
        assert_eq!(data.project_tools[1].tool, "Claude");
        assert_eq!(data.project_tools[2].project, "b");
        assert_eq!(data.project_tools[2].tool, "Cursor");
    }
}
