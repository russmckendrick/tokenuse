use std::collections::{BTreeMap, HashMap, HashSet};

use chrono::{DateTime, Datelike, Duration, Local, NaiveDate};
use color_eyre::Result;

use crate::app::{Period, Provider};
use crate::data::{
    CountMetric, DailyMetric, DashboardData, ModelMetric, ProjectMetric, ProjectProviderMetric,
    SessionMetric, Summary,
};
use crate::providers::{self, ParsedCall};

pub struct Ingested {
    pub calls: Vec<ParsedCall>,
}

pub fn load() -> Result<Ingested> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut calls: Vec<ParsedCall> = Vec::new();

    for provider in providers::registry() {
        let sources = match provider.discover() {
            Ok(s) => s,
            Err(_) => continue,
        };
        for source in sources {
            match provider.parse(&source, &mut seen) {
                Ok(mut more) => calls.append(&mut more),
                Err(_) => continue,
            }
        }
    }

    Ok(Ingested { calls })
}

impl Ingested {
    pub fn dashboard(&self, period: Period, provider: Provider) -> DashboardData {
        let now = Local::now();
        let filtered: Vec<&ParsedCall> = self
            .calls
            .iter()
            .filter(|c| matches_provider(c, provider) && in_period(c, period, now))
            .collect();
        build_dashboard(&filtered)
    }

    pub fn is_empty(&self) -> bool {
        self.calls.is_empty()
    }
}

fn matches_provider(call: &ParsedCall, provider: Provider) -> bool {
    match provider {
        Provider::All => true,
        Provider::ClaudeCode => call.provider == "claude-code",
        Provider::Cursor => call.provider == "cursor",
        Provider::Codex => call.provider == "codex",
        Provider::Copilot => call.provider == "copilot",
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

    let daily = aggregate_daily(calls);
    let projects = aggregate_projects(calls);
    let project_providers = aggregate_project_providers(calls);
    let sessions = aggregate_sessions(calls);
    let models = aggregate_models(calls);
    let tools = aggregate_tools(calls);
    let commands = aggregate_commands(calls);
    let mcp_servers = aggregate_mcp(calls);

    DashboardData {
        summary,
        daily,
        projects,
        project_providers,
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
        project_providers: Vec::new(),
        sessions: Vec::new(),
        models: Vec::new(),
        tools: Vec::new(),
        commands: Vec::new(),
        mcp_servers: Vec::new(),
    }
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

fn aggregate_projects(calls: &[&ParsedCall]) -> Vec<ProjectMetric> {
    #[derive(Default)]
    struct Acc {
        cost: f64,
        sessions: HashSet<String>,
        providers: HashMap<&'static str, f64>,
    }
    let mut by_project: HashMap<String, Acc> = HashMap::new();
    for c in calls {
        let entry = by_project.entry(canonical_project(&c.project)).or_default();
        entry.cost += c.cost_usd;
        if let Some(key) = session_key(c) {
            entry.sessions.insert(key);
        }
        *entry.providers.entry(c.provider).or_default() += c.cost_usd;
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
        .map(|(name, acc)| {
            let session_count = acc.sessions.len().max(1) as u64;
            let avg = acc.cost / session_count as f64;
            ProjectMetric {
                name: leak(name),
                cost: leak(format_money(acc.cost)),
                avg_per_session: leak(format_money(avg)),
                sessions: session_count,
                provider_mix: leak(format_provider_mix(&acc.providers)),
                value: scale(acc.cost, max),
            }
        })
        .collect()
}

fn aggregate_project_providers(calls: &[&ParsedCall]) -> Vec<ProjectProviderMetric> {
    #[derive(Default)]
    struct Acc {
        cost: f64,
        calls: u64,
        sessions: HashSet<String>,
    }

    let mut project_totals: HashMap<String, f64> = HashMap::new();
    let mut by_pair: HashMap<(String, &'static str), Acc> = HashMap::new();

    for c in calls {
        let project = canonical_project(&c.project);
        *project_totals.entry(project.clone()).or_default() += c.cost_usd;

        let entry = by_pair.entry((project, c.provider)).or_default();
        entry.cost += c.cost_usd;
        entry.calls += 1;
        if let Some(key) = session_key(c) {
            entry.sessions.insert(key);
        }
    }

    let mut rows: Vec<(String, &'static str, f64, Acc)> = by_pair
        .into_iter()
        .map(|((project, provider), acc)| {
            let total = *project_totals.get(&project).unwrap_or(&0.0);
            (project, provider, total, acc)
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
            .then_with(|| provider_short_label(a.1).cmp(provider_short_label(b.1)))
    });
    let max = rows.iter().map(|r| r.3.cost).fold(0.0_f64, f64::max);

    rows.into_iter()
        .take(12)
        .map(|(project, provider, _, acc)| {
            let session_count = acc.sessions.len().max(1) as u64;
            let avg = acc.cost / session_count as f64;
            ProjectProviderMetric {
                project: leak(project),
                provider: provider_short_label(provider),
                cost: leak(format_money(acc.cost)),
                calls: acc.calls,
                sessions: session_count,
                avg_per_session: leak(format_money(avg)),
                value: scale(acc.cost, max),
            }
        })
        .collect()
}

fn aggregate_sessions(calls: &[&ParsedCall]) -> Vec<SessionMetric> {
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
            entry.project = canonical_project(&c.project);
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
            project: leak(acc.project),
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
    let registry = providers::registry();
    let mut display_lookup: HashMap<&'static str, Box<dyn providers::Provider>> = HashMap::new();
    for p in registry {
        display_lookup.insert(p.id(), p);
    }

    let mut by_model: HashMap<String, Acc> = HashMap::new();
    for c in calls {
        let display = display_lookup
            .get(c.provider)
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
            let head = providers::jsonl::first_word(cmd);
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
        Some(format!("{}:{}", call.provider, call.session_id))
    }
}

fn canonical_project(raw: &str) -> String {
    let normalized = raw.trim().replace('\\', "/");
    let trimmed = normalized.trim_end_matches('/');
    let display = short_project(trimmed);
    if display.is_empty() {
        "(unknown)".into()
    } else {
        display
    }
}

fn format_provider_mix(providers: &HashMap<&'static str, f64>) -> String {
    let mut rows: Vec<(&'static str, f64)> = providers
        .iter()
        .map(|(provider, cost)| (*provider, *cost))
        .collect();
    rows.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| provider_short_label(a.0).cmp(provider_short_label(b.0)))
    });

    if rows.is_empty() {
        return "-".into();
    }

    rows.into_iter()
        .take(3)
        .map(|(provider, cost)| {
            format!(
                "{} {}",
                provider_short_label(provider),
                format_money_short(cost)
            )
        })
        .collect::<Vec<_>>()
        .join("  ")
}

fn provider_short_label(provider: &str) -> &'static str {
    match provider {
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

fn short_project(raw: &str) -> String {
    let cleaned = raw.trim_start_matches('/').replace("/Users/", "");
    let parts: Vec<&str> = cleaned.split('/').filter(|s| !s.is_empty()).collect();
    if parts.len() <= 3 {
        return parts.join("/");
    }
    let tail = &parts[parts.len() - 3..];
    tail.join("/")
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

    fn mk_call(provider: &'static str, ts: &str, cost: f64) -> ParsedCall {
        ParsedCall {
            provider,
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
        provider: &'static str,
        session_id: &str,
        project: &str,
        cost: f64,
    ) -> ParsedCall {
        ParsedCall {
            provider,
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
    fn short_project_keeps_tail_three() {
        assert_eq!(
            short_project("/Users/me/Code/asciinema/to/svg"),
            "asciinema/to/svg"
        );
    }

    #[test]
    fn project_costs_roll_up_across_providers() {
        let ingested = Ingested {
            calls: vec![
                mk_project_call("claude-code", "s1", "/Users/me/Code/widgets", 2.0),
                mk_project_call("codex", "s1", "me/Code/widgets", 3.0),
                mk_project_call("cursor", "s2", "/Users/me/Code/widgets", 5.0),
            ],
        };

        let data = ingested.dashboard(Period::AllTime, Provider::All);

        assert_eq!(data.projects.len(), 1);
        assert_eq!(data.projects[0].name, "me/Code/widgets");
        assert_eq!(data.projects[0].cost, "$10.00");
        assert_eq!(data.projects[0].sessions, 3);
        assert!(data.projects[0].provider_mix.contains("Cursor $5.00"));
        assert!(data.projects[0].provider_mix.contains("Codex $3.00"));
        assert!(data.projects[0].provider_mix.contains("Claude $2.00"));
        assert_eq!(data.project_providers.len(), 3);
    }

    #[test]
    fn provider_filter_keeps_project_costs_provider_local() {
        let ingested = Ingested {
            calls: vec![
                mk_project_call("claude-code", "s1", "/Users/me/Code/widgets", 2.0),
                mk_project_call("codex", "s1", "/Users/me/Code/widgets", 3.0),
            ],
        };

        let data = ingested.dashboard(Period::AllTime, Provider::Codex);

        assert_eq!(data.projects.len(), 1);
        assert_eq!(data.projects[0].cost, "$3.00");
        assert_eq!(data.projects[0].provider_mix, "Codex $3.00");
        assert_eq!(data.project_providers.len(), 1);
        assert_eq!(data.project_providers[0].provider, "Codex");
    }

    #[test]
    fn session_counts_are_provider_qualified() {
        let ingested = Ingested {
            calls: vec![
                mk_project_call("claude-code", "shared", "/Users/me/Code/widgets", 1.0),
                mk_project_call("claude-code", "shared", "/Users/me/Code/widgets", 2.0),
                mk_project_call("codex", "shared", "/Users/me/Code/widgets", 3.0),
            ],
        };

        let data = ingested.dashboard(Period::AllTime, Provider::All);

        assert_eq!(data.summary.sessions, "2");
        assert_eq!(data.projects[0].sessions, 2);
        assert_eq!(data.projects[0].avg_per_session, "$3.00");
    }

    #[test]
    fn project_provider_rows_sort_by_project_total_then_provider_cost() {
        let ingested = Ingested {
            calls: vec![
                mk_project_call("claude-code", "a1", "/Users/me/Code/a", 2.0),
                mk_project_call("codex", "a2", "/Users/me/Code/a", 9.0),
                mk_project_call("cursor", "b1", "/Users/me/Code/b", 10.0),
            ],
        };

        let data = ingested.dashboard(Period::AllTime, Provider::All);

        assert_eq!(data.project_providers[0].project, "me/Code/a");
        assert_eq!(data.project_providers[0].provider, "Codex");
        assert_eq!(data.project_providers[1].project, "me/Code/a");
        assert_eq!(data.project_providers[1].provider, "Claude");
        assert_eq!(data.project_providers[2].project, "me/Code/b");
        assert_eq!(data.project_providers[2].provider, "Cursor");
    }
}
