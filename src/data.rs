use crate::app::{Period, Provider};

#[derive(Debug, Clone)]
pub struct DashboardData {
    pub summary: Summary,
    pub daily: Vec<DailyMetric>,
    pub projects: Vec<ProjectMetric>,
    pub sessions: Vec<SessionMetric>,
    pub activities: Vec<ActivityMetric>,
    pub models: Vec<ModelMetric>,
    pub tools: Vec<CountMetric>,
    pub commands: Vec<CountMetric>,
    pub mcp_servers: Vec<CountMetric>,
}

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
pub struct DailyMetric {
    pub day: &'static str,
    pub cost: &'static str,
    pub calls: u64,
    pub value: u64,
}

#[derive(Debug, Clone)]
pub struct ProjectMetric {
    pub name: &'static str,
    pub cost: &'static str,
    pub avg_per_session: &'static str,
    pub sessions: u64,
    pub overhead: &'static str,
    pub value: u64,
}

#[derive(Debug, Clone)]
pub struct SessionMetric {
    pub date: &'static str,
    pub project: &'static str,
    pub cost: &'static str,
    pub calls: u64,
    pub value: u64,
}

#[derive(Debug, Clone)]
pub struct ActivityMetric {
    pub name: &'static str,
    pub cost: &'static str,
    pub turns: u64,
    pub one_shot: &'static str,
    pub value: u64,
    pub accent: ActivityAccent,
}

#[derive(Debug, Clone, Copy)]
pub enum ActivityAccent {
    Blue,
    Green,
    Muted,
    Cyan,
    Yellow,
    Red,
    Magenta,
}

#[derive(Debug, Clone)]
pub struct ModelMetric {
    pub name: &'static str,
    pub cost: &'static str,
    pub cache: &'static str,
    pub calls: u64,
    pub value: u64,
}

#[derive(Debug, Clone)]
pub struct CountMetric {
    pub name: &'static str,
    pub calls: u64,
    pub value: u64,
}

pub fn dashboard_data(period: Period, provider: Provider) -> DashboardData {
    let mut data = match period {
        Period::Today => today_data(),
        Period::Week => week_data(),
        Period::ThirtyDays => thirty_day_data(),
        Period::Month => month_data(),
        Period::AllTime => all_time_data(),
    };

    if provider == Provider::All {
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
                overhead: "10.8K",
                value: 28,
            },
        );
    }

    data
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
                overhead: "11.6K",
                value: 100,
            },
            ProjectMetric {
                name: "mckendrick/Code/skills",
                cost: "$3.80",
                avg_per_session: "$1.90",
                sessions: 2,
                overhead: "11.6K",
                value: 12,
            },
            ProjectMetric {
                name: "mckendrick/Code/blog",
                cost: "$2.41",
                avg_per_session: "$0.603",
                sessions: 4,
                overhead: "12.1K",
                value: 8,
            },
            ProjectMetric {
                name: "Code/russ/fm",
                cost: "$0.624",
                avg_per_session: "$0.624",
                sessions: 1,
                overhead: "11.9K",
                value: 3,
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
        activities: vec![
            ActivityMetric {
                name: "Coding",
                cost: "$58.62",
                turns: 14,
                one_shot: "60%",
                value: 100,
                accent: ActivityAccent::Blue,
            },
            ActivityMetric {
                name: "Feature Dev",
                cost: "$2.47",
                turns: 4,
                one_shot: "100%",
                value: 10,
                accent: ActivityAccent::Green,
            },
            ActivityMetric {
                name: "Planning",
                cost: "$2.32",
                turns: 2,
                one_shot: "-",
                value: 9,
                accent: ActivityAccent::Blue,
            },
            ActivityMetric {
                name: "Debugging",
                cost: "$0.871",
                turns: 2,
                one_shot: "100%",
                value: 4,
                accent: ActivityAccent::Red,
            },
            ActivityMetric {
                name: "Conversation",
                cost: "$0.768",
                turns: 3,
                one_shot: "-",
                value: 3,
                accent: ActivityAccent::Muted,
            },
            ActivityMetric {
                name: "Exploration",
                cost: "$0.510",
                turns: 3,
                one_shot: "-",
                value: 2,
                accent: ActivityAccent::Cyan,
            },
            ActivityMetric {
                name: "Brainstorming",
                cost: "$0.307",
                turns: 3,
                one_shot: "-",
                value: 1,
                accent: ActivityAccent::Magenta,
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
            overhead: "11.5K",
            value: 100,
        },
        ProjectMetric {
            name: "Code/russ/fm",
            cost: "$115.49",
            avg_per_session: "$9.62",
            sessions: 12,
            overhead: "11.9K",
            value: 98,
        },
        ProjectMetric {
            name: "mckendrick/Code/blog",
            cost: "$68.17",
            avg_per_session: "$0.897",
            sessions: 76,
            overhead: "12.1K",
            value: 58,
        },
        ProjectMetric {
            name: "asciinema/to/svg",
            cost: "$59.03",
            avg_per_session: "$11.81",
            sessions: 5,
            overhead: "11.6K",
            value: 50,
        },
        ProjectMetric {
            name: "Code/dvr",
            cost: "$42.24",
            avg_per_session: "$2.01",
            sessions: 21,
            overhead: "-",
            value: 36,
        },
        ProjectMetric {
            name: "mckendrick/Code/aicommit",
            cost: "$41.52",
            avg_per_session: "$1.54",
            sessions: 27,
            overhead: "11.6K",
            value: 35,
        },
        ProjectMetric {
            name: "Code/aicommit",
            cost: "$37.59",
            avg_per_session: "$1.45",
            sessions: 26,
            overhead: "-",
            value: 32,
        },
        ProjectMetric {
            name: "Code/russ/fm",
            cost: "$30.63",
            avg_per_session: "$3.40",
            sessions: 9,
            overhead: "-",
            value: 26,
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
    data.activities = vec![
        ActivityMetric {
            name: "Coding",
            cost: "$267.52",
            turns: 2053,
            one_shot: "85%",
            value: 100,
            accent: ActivityAccent::Blue,
        },
        ActivityMetric {
            name: "Feature Dev",
            cost: "$70.98",
            turns: 126,
            one_shot: "86%",
            value: 27,
            accent: ActivityAccent::Green,
        },
        ActivityMetric {
            name: "Conversation",
            cost: "$70.37",
            turns: 2079,
            one_shot: "-",
            value: 26,
            accent: ActivityAccent::Muted,
        },
        ActivityMetric {
            name: "Exploration",
            cost: "$50.91",
            turns: 148,
            one_shot: "-",
            value: 19,
            accent: ActivityAccent::Cyan,
        },
        ActivityMetric {
            name: "Delegation",
            cost: "$34.17",
            turns: 13,
            one_shot: "63%",
            value: 13,
            accent: ActivityAccent::Yellow,
        },
        ActivityMetric {
            name: "Debugging",
            cost: "$33.63",
            turns: 70,
            one_shot: "89%",
            value: 13,
            accent: ActivityAccent::Red,
        },
        ActivityMetric {
            name: "Refactoring",
            cost: "$18.20",
            turns: 17,
            one_shot: "60%",
            value: 7,
            accent: ActivityAccent::Yellow,
        },
        ActivityMetric {
            name: "Brainstorming",
            cost: "$4.00",
            turns: 41,
            one_shot: "-",
            value: 2,
            accent: ActivityAccent::Magenta,
        },
        ActivityMetric {
            name: "Testing",
            cost: "$3.24",
            turns: 51,
            one_shot: "-",
            value: 1,
            accent: ActivityAccent::Magenta,
        },
        ActivityMetric {
            name: "Planning",
            cost: "$2.32",
            turns: 2,
            one_shot: "-",
            value: 1,
            accent: ActivityAccent::Blue,
        },
        ActivityMetric {
            name: "Build/Deploy",
            cost: "$0.582",
            turns: 5,
            one_shot: "-",
            value: 1,
            accent: ActivityAccent::Green,
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
