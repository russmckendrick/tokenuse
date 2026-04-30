use serde::Serialize;

use crate::app::{Period, ProjectFilter, Tool};
use crate::currency::CurrencyFormatter;

#[derive(Debug, Clone, Serialize)]
pub struct DashboardData {
    pub summary: Summary,
    pub daily: Vec<DailyMetric>,
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
    pub tools: String,
    pub prompt: String,
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

impl ProjectOption {
    pub fn all(cost: String, calls: u64) -> Self {
        Self {
            identity: None,
            label: "All".into(),
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

pub fn dashboard_data(
    period: Period,
    tool: Tool,
    project_filter: &ProjectFilter,
    currency: &CurrencyFormatter,
) -> DashboardData {
    let mut data = match period {
        Period::Today => today_data(),
        Period::Week => week_data(),
        Period::ThirtyDays => thirty_day_data(),
        Period::Month => month_data(),
        Period::AllTime => all_time_data(),
    };

    if tool == Tool::All {
        data.summary.calls = "1,038";
        data.summary.sessions = "27";
        data.summary.cache_hit = "96.0%";
        data.projects.insert(
            2,
            ProjectMetric {
                name: "openai/sidecar",
                cost: "$12.48",
                avg_per_session: "$2.08",
                sessions: 6,
                tool_mix: "Copilot $7.1  Codex $5.4",
                value: 28,
            },
        );
        data.project_tools.insert(
            3,
            ProjectToolMetric {
                project: "openai/sidecar",
                tool: "Copilot",
                cost: "$7.10",
                calls: 64,
                sessions: 4,
                avg_per_session: "$1.78",
                value: 18,
            },
        );
        data.project_tools.insert(
            4,
            ProjectToolMetric {
                project: "openai/sidecar",
                tool: "Codex",
                cost: "$5.38",
                calls: 41,
                sessions: 2,
                avg_per_session: "$2.69",
                value: 14,
            },
        );
    }

    apply_project_filter(&mut data, project_filter);
    apply_currency(&mut data, currency);

    data
}

pub fn project_options(
    period: Period,
    tool: Tool,
    currency: &CurrencyFormatter,
) -> Vec<ProjectOption> {
    let data = dashboard_data(period, tool, &ProjectFilter::All, currency);
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
    currency: &CurrencyFormatter,
) -> Vec<SessionOption> {
    let data = dashboard_data(period, tool, &ProjectFilter::All, currency);
    data.sessions
        .iter()
        .enumerate()
        .map(|(idx, session)| SessionOption {
            key: format!("sample:{idx}"),
            date: session.date.into(),
            project: session.project.into(),
            tool: "Sample",
            cost: session.cost.into(),
            calls: session.calls,
            value: session.value,
        })
        .collect()
}

pub fn session_detail(key: &str, _currency: &CurrencyFormatter) -> Option<SessionDetailView> {
    if !key.starts_with("sample:") {
        return None;
    }
    Some(SessionDetailView {
        key: key.into(),
        session_id: key.trim_start_matches("sample:").into(),
        project: "(sample data)".into(),
        tool: "Sample",
        date_range: "-".into(),
        total_cost: "$0.00".into(),
        total_calls: 0,
        total_input: "0".into(),
        total_output: "0".into(),
        total_cache_read: "0".into(),
        calls: Vec::new(),
        note: Some(
            "sample data does not include per-call records · run with live sessions to drill in"
                .into(),
        ),
    })
}

pub fn limits_data(_tool: Tool) -> LimitsData {
    let codex_limits = vec![
        LimitMetric {
            tool: "Codex",
            scope: "Codex",
            window: "5h",
            used: 17,
            left: "83% left",
            reset: "16:47",
            plan: "Pro Lite",
        },
        LimitMetric {
            tool: "Codex",
            scope: "Codex",
            window: "weekly",
            used: 6,
            left: "94% left",
            reset: "05 May 07:00",
            plan: "Pro Lite",
        },
        LimitMetric {
            tool: "Codex",
            scope: "GPT-5.3-Codex-Spark",
            window: "5h",
            used: 0,
            left: "100% left",
            reset: "19:37",
            plan: "-",
        },
        LimitMetric {
            tool: "Codex",
            scope: "GPT-5.3-Codex-Spark",
            window: "weekly",
            used: 0,
            left: "100% left",
            reset: "06 May 14:37",
            plan: "-",
        },
    ];

    LimitsData {
        sections: sample_limit_sections(codex_limits),
    }
}

fn sample_limit_sections(codex_limits: Vec<LimitMetric>) -> Vec<ToolLimitSection> {
    vec![
        ToolLimitSection {
            tool: "Codex",
            limits: codex_limits,
            usage: RecentUsageMetric {
                buckets: [
                    0, 0, 12, 24, 8, 0, 18, 30, 42, 17, 5, 0, 0, 0, 18, 48, 66, 21, 9, 0, 36, 75,
                    44, 11,
                ],
                calls: 41,
                tokens: "1.2M",
                cost: "$5.38",
                last_seen: "now",
            },
            models: vec![
                RecentModelMetric {
                    name: "GPT-5",
                    calls: 24,
                    tokens: "820K",
                    cost: "$3.91",
                    value: 100,
                },
                RecentModelMetric {
                    name: "GPT-5.3-Codex-Spark",
                    calls: 17,
                    tokens: "380K",
                    cost: "$1.47",
                    value: 46,
                },
            ],
        },
        ToolLimitSection {
            tool: "Claude Code",
            limits: Vec::new(),
            usage: RecentUsageMetric {
                buckets: [
                    8, 14, 0, 0, 21, 44, 36, 9, 0, 0, 12, 28, 0, 6, 18, 40, 55, 31, 12, 0, 9, 20,
                    48, 33,
                ],
                calls: 73,
                tokens: "5.8M",
                cost: "$11.42",
                last_seen: "18m",
            },
            models: vec![
                RecentModelMetric {
                    name: "Opus 4.7",
                    calls: 51,
                    tokens: "4.9M",
                    cost: "$10.70",
                    value: 100,
                },
                RecentModelMetric {
                    name: "Haiku 4.5",
                    calls: 22,
                    tokens: "900K",
                    cost: "$0.72",
                    value: 18,
                },
            ],
        },
        ToolLimitSection {
            tool: "Cursor",
            limits: Vec::new(),
            usage: RecentUsageMetric {
                buckets: [
                    0, 0, 0, 6, 10, 4, 0, 0, 0, 9, 13, 0, 0, 0, 0, 7, 15, 24, 8, 0, 0, 12, 18, 0,
                ],
                calls: 18,
                tokens: "184K",
                cost: "$0.92",
                last_seen: "47m",
            },
            models: vec![RecentModelMetric {
                name: "Sonnet 4.5",
                calls: 18,
                tokens: "184K",
                cost: "$0.92",
                value: 100,
            }],
        },
        ToolLimitSection {
            tool: "Copilot",
            limits: Vec::new(),
            usage: RecentUsageMetric {
                buckets: [
                    4, 7, 0, 0, 0, 16, 22, 5, 0, 8, 0, 0, 0, 0, 0, 10, 29, 20, 5, 0, 0, 7, 0, 0,
                ],
                calls: 29,
                tokens: "96K",
                cost: "$0.47",
                last_seen: "2h",
            },
            models: vec![
                RecentModelMetric {
                    name: "anthropic-auto",
                    calls: 17,
                    tokens: "60K",
                    cost: "$0.31",
                    value: 100,
                },
                RecentModelMetric {
                    name: "openai-auto",
                    calls: 12,
                    tokens: "36K",
                    cost: "$0.16",
                    value: 60,
                },
            ],
        },
    ]
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

fn week_data() -> DashboardData {
    DashboardData {
        summary: Summary {
            cost: "$65.87",
            calls: "474",
            sessions: "12",
            cache_hit: "98.9%",
            input: "10.3K",
            output: "324.9K",
            cached: "105.6M",
            written: "1.1M",
        },
        daily: vec![
            DailyMetric {
                day: "04-22",
                cost: "$1.15",
                calls: 32,
                value: 2,
            },
            DailyMetric {
                day: "04-26",
                cost: "$64.72",
                calls: 442,
                value: 100,
            },
        ],
        projects: vec![
            ProjectMetric {
                name: "asciinema/to/svg",
                cost: "$59.03",
                avg_per_session: "$11.81",
                sessions: 5,
                tool_mix: "Claude $59.0",
                value: 100,
            },
            ProjectMetric {
                name: "mckendrick/Code/skills",
                cost: "$3.80",
                avg_per_session: "$1.90",
                sessions: 2,
                tool_mix: "Claude $2.6  Codex $1.2",
                value: 12,
            },
            ProjectMetric {
                name: "mckendrick/Code/blog",
                cost: "$2.41",
                avg_per_session: "$0.603",
                sessions: 4,
                tool_mix: "Claude $1.8  Copilot $0.61",
                value: 8,
            },
            ProjectMetric {
                name: "Code/russ/fm",
                cost: "$0.624",
                avg_per_session: "$0.624",
                sessions: 1,
                tool_mix: "Codex $0.62",
                value: 3,
            },
        ],
        project_tools: vec![
            ProjectToolMetric {
                project: "asciinema/to/svg",
                tool: "Claude",
                cost: "$59.03",
                calls: 442,
                sessions: 5,
                avg_per_session: "$11.81",
                value: 100,
            },
            ProjectToolMetric {
                project: "mckendrick/Code/skills",
                tool: "Claude",
                cost: "$2.60",
                calls: 31,
                sessions: 1,
                avg_per_session: "$2.60",
                value: 4,
            },
            ProjectToolMetric {
                project: "mckendrick/Code/skills",
                tool: "Codex",
                cost: "$1.20",
                calls: 22,
                sessions: 1,
                avg_per_session: "$1.20",
                value: 2,
            },
            ProjectToolMetric {
                project: "mckendrick/Code/blog",
                tool: "Claude",
                cost: "$1.80",
                calls: 30,
                sessions: 3,
                avg_per_session: "$0.600",
                value: 3,
            },
            ProjectToolMetric {
                project: "mckendrick/Code/blog",
                tool: "Copilot",
                cost: "$0.610",
                calls: 10,
                sessions: 1,
                avg_per_session: "$0.610",
                value: 1,
            },
            ProjectToolMetric {
                project: "Code/russ/fm",
                tool: "Codex",
                cost: "$0.624",
                calls: 16,
                sessions: 1,
                avg_per_session: "$0.624",
                value: 1,
            },
        ],
        sessions: vec![
            SessionMetric {
                date: "2026-04-26",
                project: "asciinema/to/svg",
                cost: "$58.52",
                calls: 311,
                value: 100,
            },
            SessionMetric {
                date: "2026-04-26",
                project: "mckendrick/Code/skills",
                cost: "$3.33",
                calls: 53,
                value: 12,
            },
            SessionMetric {
                date: "2026-04-26",
                project: "mckendrick/Code/blog",
                cost: "$1.18",
                calls: 23,
                value: 5,
            },
            SessionMetric {
                date: "2026-04-22",
                project: "mckendrick/Code/blog",
                cost: "$1.03",
                calls: 17,
                value: 4,
            },
            SessionMetric {
                date: "2026-04-26",
                project: "Code/russ/fm",
                cost: "$0.624",
                calls: 16,
                value: 2,
            },
        ],
        models: vec![
            ModelMetric {
                name: "Opus 4.7",
                cost: "$65.35",
                cache: "99.2%",
                calls: 431,
                value: 100,
            },
            ModelMetric {
                name: "Haiku 4.5",
                cost: "$0.517",
                cache: "87.1%",
                calls: 42,
                value: 6,
            },
            ModelMetric {
                name: "<synthetic>",
                cost: "$0.0000",
                cache: "-",
                calls: 1,
                value: 1,
            },
        ],
        tools: vec![
            CountMetric {
                name: "Edit",
                calls: 76,
                value: 100,
            },
            CountMetric {
                name: "Bash",
                calls: 61,
                value: 80,
            },
            CountMetric {
                name: "Read",
                calls: 27,
                value: 36,
            },
            CountMetric {
                name: "TodoWrite",
                calls: 10,
                value: 13,
            },
            CountMetric {
                name: "Write",
                calls: 8,
                value: 11,
            },
            CountMetric {
                name: "ExitPlanMode",
                calls: 5,
                value: 7,
            },
            CountMetric {
                name: "ToolSearch",
                calls: 3,
                value: 4,
            },
            CountMetric {
                name: "WebFetch",
                calls: 3,
                value: 4,
            },
            CountMetric {
                name: "AskUserQuestion",
                calls: 2,
                value: 3,
            },
            CountMetric {
                name: "TaskUpdate",
                calls: 2,
                value: 3,
            },
        ],
        commands: vec![
            CountMetric {
                name: "tail",
                calls: 54,
                value: 100,
            },
            CountMetric {
                name: "cargo",
                calls: 52,
                value: 96,
            },
            CountMetric {
                name: "echo",
                calls: 19,
                value: 35,
            },
            CountMetric {
                name: "head",
                calls: 16,
                value: 30,
            },
            CountMetric {
                name: "git",
                calls: 10,
                value: 18,
            },
            CountMetric {
                name: "grep",
                calls: 10,
                value: 18,
            },
            CountMetric {
                name: "cat",
                calls: 5,
                value: 9,
            },
            CountMetric {
                name: "gh",
                calls: 5,
                value: 9,
            },
            CountMetric {
                name: "ls",
                calls: 4,
                value: 7,
            },
            CountMetric {
                name: "python3",
                calls: 3,
                value: 6,
            },
        ],
        mcp_servers: vec![CountMetric {
            name: "ccd_session",
            calls: 1,
            value: 100,
        }],
    }
}

fn today_data() -> DashboardData {
    let mut data = week_data();
    data.summary.cost = "$9.04";
    data.summary.calls = "246";
    data.summary.sessions = "4";
    data.daily = vec![DailyMetric {
        day: "04-29",
        cost: "$9.04",
        calls: 246,
        value: 100,
    }];
    data.projects.truncate(3);
    data.project_tools.truncate(4);
    data.sessions.truncate(3);
    data
}

fn thirty_day_data() -> DashboardData {
    let mut data = all_time_data();
    data.summary.cost = "$191.48";
    data.summary.calls = "2,818";
    data.summary.sessions = "61";
    data.summary.cache_hit = "97.8%";
    data.daily.truncate(10);
    data
}

fn month_data() -> DashboardData {
    let mut data = all_time_data();
    data.summary.cost = "$184.96";
    data.summary.calls = "2,626";
    data.summary.sessions = "57";
    data.summary.cache_hit = "97.6%";
    data
}

fn all_time_data() -> DashboardData {
    let mut data = week_data();
    data.summary = Summary {
        cost: "$558.13",
        calls: "9,522",
        sessions: "215",
        cache_hit: "96.0%",
        input: "30.2M",
        output: "4.5M",
        cached: "1054.0M",
        written: "14.2M",
    };
    data.daily = vec![
        DailyMetric {
            day: "04-06",
            cost: "$6.52",
            calls: 192,
            value: 4,
        },
        DailyMetric {
            day: "04-07",
            cost: "$8.31",
            calls: 384,
            value: 5,
        },
        DailyMetric {
            day: "04-08",
            cost: "$15.32",
            calls: 347,
            value: 10,
        },
        DailyMetric {
            day: "04-09",
            cost: "$4.19",
            calls: 195,
            value: 3,
        },
        DailyMetric {
            day: "04-11",
            cost: "$18.97",
            calls: 601,
            value: 12,
        },
        DailyMetric {
            day: "04-12",
            cost: "$38.41",
            calls: 602,
            value: 24,
        },
        DailyMetric {
            day: "04-14",
            cost: "$12.22",
            calls: 198,
            value: 8,
        },
        DailyMetric {
            day: "04-16",
            cost: "$19.89",
            calls: 202,
            value: 13,
        },
        DailyMetric {
            day: "04-18",
            cost: "$161.51",
            calls: 1408,
            value: 100,
        },
        DailyMetric {
            day: "04-19",
            cost: "$75.47",
            calls: 938,
            value: 47,
        },
        DailyMetric {
            day: "04-22",
            cost: "$9.04",
            calls: 246,
            value: 6,
        },
        DailyMetric {
            day: "04-25",
            cost: "$21.31",
            calls: 593,
            value: 13,
        },
        DailyMetric {
            day: "04-26",
            cost: "$75.60",
            calls: 792,
            value: 47,
        },
        DailyMetric {
            day: "04-28",
            cost: "$6.64",
            calls: 245,
            value: 4,
        },
    ];
    data.projects = vec![
        ProjectMetric {
            name: "ai/commit/dev",
            cost: "$117.91",
            avg_per_session: "$6.21",
            sessions: 19,
            tool_mix: "Claude $78  Codex $40",
            value: 100,
        },
        ProjectMetric {
            name: "Code/russ/fm",
            cost: "$115.49",
            avg_per_session: "$9.62",
            sessions: 12,
            tool_mix: "Claude $82  Cursor $33",
            value: 98,
        },
        ProjectMetric {
            name: "mckendrick/Code/blog",
            cost: "$68.17",
            avg_per_session: "$0.897",
            sessions: 76,
            tool_mix: "Claude $39  Copilot $18  Codex $11",
            value: 58,
        },
        ProjectMetric {
            name: "asciinema/to/svg",
            cost: "$59.03",
            avg_per_session: "$11.81",
            sessions: 5,
            tool_mix: "Claude $59",
            value: 50,
        },
        ProjectMetric {
            name: "Code/dvr",
            cost: "$42.24",
            avg_per_session: "$2.01",
            sessions: 21,
            tool_mix: "Cursor $28  Copilot $14",
            value: 36,
        },
        ProjectMetric {
            name: "mckendrick/Code/aicommit",
            cost: "$41.52",
            avg_per_session: "$1.54",
            sessions: 27,
            tool_mix: "Claude $42",
            value: 35,
        },
        ProjectMetric {
            name: "Code/aicommit",
            cost: "$37.59",
            avg_per_session: "$1.45",
            sessions: 26,
            tool_mix: "Codex $25  Copilot $13",
            value: 32,
        },
        ProjectMetric {
            name: "Code/russ/fm",
            cost: "$30.63",
            avg_per_session: "$3.40",
            sessions: 9,
            tool_mix: "Codex $31",
            value: 26,
        },
    ];
    data.project_tools = vec![
        ProjectToolMetric {
            project: "ai/commit/dev",
            tool: "Claude",
            cost: "$78.20",
            calls: 961,
            sessions: 12,
            avg_per_session: "$6.52",
            value: 95,
        },
        ProjectToolMetric {
            project: "ai/commit/dev",
            tool: "Codex",
            cost: "$39.71",
            calls: 623,
            sessions: 7,
            avg_per_session: "$5.67",
            value: 48,
        },
        ProjectToolMetric {
            project: "Code/russ/fm",
            tool: "Claude",
            cost: "$82.61",
            calls: 545,
            sessions: 4,
            avg_per_session: "$20.65",
            value: 100,
        },
        ProjectToolMetric {
            project: "Code/russ/fm",
            tool: "Cursor",
            cost: "$32.88",
            calls: 514,
            sessions: 8,
            avg_per_session: "$4.11",
            value: 40,
        },
        ProjectToolMetric {
            project: "mckendrick/Code/blog",
            tool: "Claude",
            cost: "$39.04",
            calls: 991,
            sessions: 41,
            avg_per_session: "$0.952",
            value: 47,
        },
        ProjectToolMetric {
            project: "mckendrick/Code/blog",
            tool: "Copilot",
            cost: "$18.40",
            calls: 682,
            sessions: 23,
            avg_per_session: "$0.800",
            value: 22,
        },
        ProjectToolMetric {
            project: "mckendrick/Code/blog",
            tool: "Codex",
            cost: "$10.73",
            calls: 341,
            sessions: 12,
            avg_per_session: "$0.894",
            value: 13,
        },
        ProjectToolMetric {
            project: "asciinema/to/svg",
            tool: "Claude",
            cost: "$59.03",
            calls: 442,
            sessions: 5,
            avg_per_session: "$11.81",
            value: 71,
        },
        ProjectToolMetric {
            project: "Code/dvr",
            tool: "Cursor",
            cost: "$28.15",
            calls: 455,
            sessions: 14,
            avg_per_session: "$2.01",
            value: 34,
        },
        ProjectToolMetric {
            project: "Code/dvr",
            tool: "Copilot",
            cost: "$14.09",
            calls: 188,
            sessions: 7,
            avg_per_session: "$2.01",
            value: 17,
        },
    ];
    data.sessions = vec![
        SessionMetric {
            date: "2026-04-18",
            project: "Code/russ/fm",
            cost: "$82.61",
            calls: 545,
            value: 100,
        },
        SessionMetric {
            date: "2026-04-26",
            project: "asciinema/to/svg",
            cost: "$58.52",
            calls: 311,
            value: 71,
        },
        SessionMetric {
            date: "2026-04-18",
            project: "ai/commit/dev",
            cost: "$31.28",
            calls: 175,
            value: 38,
        },
        SessionMetric {
            date: "2026-04-18",
            project: "Code/russ/fm",
            cost: "$31.20",
            calls: 234,
            value: 38,
        },
        SessionMetric {
            date: "2026-04-19",
            project: "ai/commit/dev",
            cost: "$27.94",
            calls: 230,
            value: 34,
        },
    ];
    data.models = vec![
        ModelMetric {
            name: "Opus 4.7",
            cost: "$304.74",
            cache: "98.5%",
            calls: 2345,
            value: 100,
        },
        ModelMetric {
            name: "Opus 4.6",
            cost: "$124.43",
            cache: "97.7%",
            calls: 2299,
            value: 41,
        },
        ModelMetric {
            name: "GPT-5",
            cost: "$121.95",
            cache: "93.1%",
            calls: 4159,
            value: 40,
        },
        ModelMetric {
            name: "Haiku 4.5",
            cost: "$7.00",
            cache: "87.6%",
            calls: 704,
            value: 2,
        },
        ModelMetric {
            name: "GPT-5.4",
            cost: "$0.0059",
            cache: "-",
            calls: 1,
            value: 1,
        },
        ModelMetric {
            name: "<synthetic>",
            cost: "$0.0000",
            cache: "-",
            calls: 14,
            value: 1,
        },
    ];
    data.tools = vec![
        CountMetric {
            name: "Bash",
            calls: 5119,
            value: 100,
        },
        CountMetric {
            name: "write_stdin",
            calls: 486,
            value: 9,
        },
        CountMetric {
            name: "Edit",
            calls: 463,
            value: 9,
        },
        CountMetric {
            name: "Read",
            calls: 410,
            value: 8,
        },
        CountMetric {
            name: "js",
            calls: 176,
            value: 3,
        },
        CountMetric {
            name: "Write",
            calls: 118,
            value: 2,
        },
        CountMetric {
            name: "Grep",
            calls: 86,
            value: 2,
        },
        CountMetric {
            name: "TaskUpdate",
            calls: 77,
            value: 2,
        },
        CountMetric {
            name: "update_plan",
            calls: 62,
            value: 1,
        },
        CountMetric {
            name: "TodoWrite",
            calls: 53,
            value: 1,
        },
    ];
    data.commands = vec![
        CountMetric {
            name: "tail",
            calls: 131,
            value: 100,
        },
        CountMetric {
            name: "cargo",
            calls: 129,
            value: 98,
        },
        CountMetric {
            name: "head",
            calls: 112,
            value: 85,
        },
        CountMetric {
            name: "grep",
            calls: 89,
            value: 68,
        },
        CountMetric {
            name: "echo",
            calls: 84,
            value: 64,
        },
        CountMetric {
            name: "ls",
            calls: 76,
            value: 58,
        },
        CountMetric {
            name: "pnpm",
            calls: 62,
            value: 47,
        },
        CountMetric {
            name: "curl",
            calls: 35,
            value: 27,
        },
        CountMetric {
            name: "git",
            calls: 33,
            value: 25,
        },
        CountMetric {
            name: "python3",
            calls: 25,
            value: 19,
        },
    ];
    data.mcp_servers = vec![
        CountMetric {
            name: "claude-in-chrome",
            calls: 215,
            value: 100,
        },
        CountMetric {
            name: "Claude_Preview",
            calls: 149,
            value: 69,
        },
        CountMetric {
            name: "ccd_session",
            calls: 1,
            value: 1,
        },
        CountMetric {
            name: "cowork",
            calls: 1,
            value: 1,
        },
    ];
    data
}
