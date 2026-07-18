use serde::Serialize;
use tokenuse::{
    app::{App, ConfigRowView, DataSource, ModelFilter, Period, ProjectFilter, SortMode, Tool},
    copy::{self, CopyDeck},
    data::{
        CoachProjectActivity, DashboardData, LimitsData, ModelCatalogEntry, ModelPageDetail,
        ModelToolBreakdown, OutputSummary, ProjectOption, ProjectToolSplit, SessionOption,
    },
    ingest::ProjectInventoryRow,
    reports::ReportFormat,
};

use crate::ids::{period_id, report_format_id, sort_id, status_tone_id, tool_id};

#[derive(Debug, Clone, Serialize)]
pub(crate) struct OptionItem {
    pub(crate) value: &'static str,
    pub(crate) label: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ProjectState {
    pub(crate) identity: Option<String>,
    pub(crate) label: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct DesktopSnapshot {
    pub(crate) version: &'static str,
    pub(crate) copy: &'static CopyDeck,
    pub(crate) data_generation: u64,
    pub(crate) source: &'static str,
    pub(crate) status: Option<String>,
    pub(crate) status_tone: &'static str,
    pub(crate) period: &'static str,
    pub(crate) periods: Vec<OptionItem>,
    pub(crate) tool: &'static str,
    pub(crate) tools: Vec<OptionItem>,
    pub(crate) sort: &'static str,
    pub(crate) sorts: Vec<OptionItem>,
    pub(crate) project: ProjectState,
    pub(crate) dashboard: DashboardData,
    pub(crate) usage: LimitsData,
    pub(crate) projects: Vec<ProjectOption>,
    pub(crate) report_projects: Vec<ProjectOption>,
    pub(crate) sessions: Vec<SessionOption>,
    pub(crate) config_rows: Vec<ConfigRowView>,
    pub(crate) currencies: Vec<String>,
    pub(crate) currency: String,
    pub(crate) desktop_settings: DesktopSettingsState,
    pub(crate) desktop_updates: DesktopUpdateState,
    pub(crate) report_dir: String,
    pub(crate) report_formats: Vec<OptionItem>,
    pub(crate) subscription_cookies: SubscriptionCookieState,
    pub(crate) mcp_http: McpHttpState,
}

/// Bearer token deliberately absent — the frontend fetches it on demand via
/// `reveal_mcp_token`, mirroring the booleans-only cookie state below.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct McpHttpState {
    pub(crate) enabled: bool,
    pub(crate) port: u16,
    pub(crate) running: bool,
    pub(crate) endpoint: String,
    pub(crate) last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SubscriptionCookieState {
    pub(crate) supported: bool,
    pub(crate) claude_set: bool,
    pub(crate) codex_set: bool,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct DesktopSettingsState {
    pub(crate) open_at_login: bool,
    pub(crate) show_dock_or_taskbar_icon: bool,
    pub(crate) plan_prices: Vec<PlanPriceRow>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct PlanPriceRow {
    pub(crate) id: &'static str,
    pub(crate) label: &'static str,
    pub(crate) price: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct DesktopUpdateState {
    pub(crate) supported: bool,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct TraySnapshot {
    pub(crate) version: &'static str,
    pub(crate) copy: &'static CopyDeck,
    pub(crate) status: Option<String>,
    pub(crate) currency: String,
    pub(crate) dashboard: DashboardData,
    pub(crate) usage: LimitsData,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ReportResponse {
    pub(crate) path: String,
    pub(crate) snapshot: DesktopSnapshot,
}

/// Payload for the per-tool desktop page: the tool-filtered dashboard for
/// the active period plus that tool's limit console.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct ToolPageData {
    pub(crate) dashboard: DashboardData,
    pub(crate) usage: LimitsData,
}

/// Payload for the per-project desktop page: the project-filtered dashboard
/// for the active period across all tools, the project's full session list,
/// its discovered sources, and the Coach output/activity profile.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct ProjectPageData {
    pub(crate) identity: String,
    pub(crate) name: String,
    pub(crate) dashboard: DashboardData,
    pub(crate) sessions: Vec<SessionOption>,
    pub(crate) sources: Vec<ProjectInventoryRow>,
    pub(crate) tool_split: Vec<ProjectToolSplit>,
    pub(crate) output: OutputSummary,
    pub(crate) activity: Option<CoachProjectActivity>,
}

pub(crate) fn project_page(app: &App, identity: &str) -> ProjectPageData {
    let sources: Vec<ProjectInventoryRow> = app
        .project_inventory()
        .into_iter()
        .filter(|row| row.identity == identity)
        .collect();
    let name = sources
        .first()
        .map(|row| row.project.clone())
        .or_else(|| {
            app.project_options()
                .into_iter()
                .find(|option| option.identity.as_deref() == Some(identity))
                .map(|option| option.label)
        })
        .unwrap_or_else(|| identity.to_string());

    let filter = ProjectFilter::Selected {
        identity: identity.to_string(),
        label: name.clone(),
    };
    let coach = app.coach_for(app.period, Tool::All, &filter);

    ProjectPageData {
        identity: identity.to_string(),
        name,
        dashboard: app.dashboard_for(app.period, Tool::All, &filter, &ModelFilter::All, app.sort),
        sessions: app.session_options_for(
            app.period,
            Tool::All,
            &filter,
            &ModelFilter::All,
            app.sort,
        ),
        sources,
        tool_split: app.project_tool_split(&filter),
        // The coach payload is scoped to this one project, so its output
        // block and single activity row are already project-specific.
        activity: coach.projects.first().cloned(),
        output: coach.output,
    }
}

/// Payload for the per-model desktop page: the model-filtered dashboard for
/// the active period across all tools, the model's session list, its
/// per-tool split, and the token-composition/pricing extras.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct ModelPageData {
    pub(crate) canonical_id: String,
    pub(crate) name: String,
    pub(crate) provider: &'static str,
    pub(crate) provider_label: &'static str,
    pub(crate) family: &'static str,
    pub(crate) dashboard: DashboardData,
    pub(crate) sessions: Vec<SessionOption>,
    pub(crate) per_tool: Vec<ModelToolBreakdown>,
    pub(crate) detail: Option<ModelPageDetail>,
}

pub(crate) fn model_page(app: &App, canonical_id: &str) -> ModelPageData {
    let find = |entries: Vec<ModelCatalogEntry>| {
        entries
            .into_iter()
            .find(|entry| entry.canonical_id == canonical_id)
    };
    // The Models index lists all-time rows alongside the active period's, so
    // a period-inactive model stays reachable: fall back to the all-time
    // entry for its identity and per-tool split.
    let entry = find(app.model_catalog(app.period)).or_else(|| find(app.model_catalog(Period::AllTime)));

    let name = entry
        .as_ref()
        .map(|entry| entry.name.to_string())
        .unwrap_or_else(|| canonical_id.to_string());
    let filter = ModelFilter::Selected {
        canonical_id: canonical_id.to_string(),
        label: name.clone(),
    };

    ModelPageData {
        canonical_id: canonical_id.to_string(),
        name,
        provider: entry.as_ref().map(|entry| entry.provider).unwrap_or(""),
        provider_label: entry
            .as_ref()
            .map(|entry| entry.provider_label)
            .unwrap_or("-"),
        family: entry.as_ref().map(|entry| entry.family).unwrap_or(""),
        dashboard: app.dashboard_for(
            app.period,
            Tool::All,
            &ProjectFilter::All,
            &filter,
            app.sort,
        ),
        sessions: app.session_options_for(
            app.period,
            Tool::All,
            &ProjectFilter::All,
            &filter,
            app.sort,
        ),
        per_tool: entry.map(|entry| entry.per_tool).unwrap_or_default(),
        detail: app.model_detail(&filter),
    }
}

pub(crate) fn snapshot(app: &App) -> DesktopSnapshot {
    let tool = app.tool;
    let sort = app.sort;

    DesktopSnapshot {
        version: env!("CARGO_PKG_VERSION"),
        copy: copy::copy(),
        data_generation: app.data_generation(),
        source: match app.source {
            DataSource::Live(_) => "live",
            DataSource::Sample => "sample",
        },
        status: app.status_text().map(str::to_string),
        status_tone: status_tone_id(app.status_tone()),
        period: period_id(app.period),
        periods: Period::ALL
            .into_iter()
            .map(|period| OptionItem {
                value: period_id(period),
                label: period.label(),
            })
            .collect(),
        tool: tool_id(tool),
        tools: [
            Tool::All,
            Tool::ClaudeCode,
            Tool::Cursor,
            Tool::Codex,
            Tool::Copilot,
            Tool::Gemini,
        ]
        .into_iter()
        .map(|tool| OptionItem {
            value: tool_id(tool),
            label: tool.label(),
        })
        .collect(),
        sort: sort_id(sort),
        sorts: SortMode::ALL
            .into_iter()
            .map(|sort| OptionItem {
                value: sort_id(sort),
                label: sort.label(),
            })
            .collect(),
        project: match &app.project_filter {
            ProjectFilter::All => ProjectState {
                identity: None,
                label: copy::copy().tools.all.clone(),
            },
            ProjectFilter::Selected { identity, label } => ProjectState {
                identity: Some(identity.clone()),
                label: label.clone(),
            },
        },
        dashboard: app.dashboard(),
        usage: app.usage_for(tool, sort),
        projects: app.project_options(),
        report_projects: app.report_project_options(app.period),
        sessions: app.session_options(),
        config_rows: app.config_rows(),
        currencies: app.currency_table.codes(),
        currency: app.currency().code().to_string(),
        desktop_settings: desktop_settings(app),
        desktop_updates: desktop_updates(),
        report_dir: app.export_dir.display().to_string(),
        report_formats: ReportFormat::ALL
            .into_iter()
            .map(|format| OptionItem {
                value: report_format_id(format),
                label: format.label(),
            })
            .collect(),
        subscription_cookies: subscription_cookie_state(),
        mcp_http: mcp_http_state(app),
    }
}

fn mcp_http_state(app: &App) -> McpHttpState {
    let (running, last_error) = crate::mcp_http::status();
    McpHttpState {
        enabled: app.settings.mcp.http_enabled,
        port: app.settings.mcp.http_port,
        running,
        endpoint: tokenuse::mcp::http::endpoint_url(app.settings.mcp.http_port),
        last_error,
    }
}

#[cfg(feature = "quota-sync")]
fn subscription_cookie_state() -> SubscriptionCookieState {
    SubscriptionCookieState {
        supported: true,
        claude_set: matches!(
            tokenuse::secrets::read(tokenuse::tools::claude_subscription::config::KEYRING_ACCOUNT),
            Ok(Some(_))
        ),
        codex_set: matches!(
            tokenuse::secrets::read(tokenuse::tools::codex_subscription::config::KEYRING_ACCOUNT),
            Ok(Some(_))
        ),
    }
}

#[cfg(not(feature = "quota-sync"))]
fn subscription_cookie_state() -> SubscriptionCookieState {
    SubscriptionCookieState {
        supported: false,
        claude_set: false,
        codex_set: false,
    }
}

fn desktop_settings(app: &App) -> DesktopSettingsState {
    DesktopSettingsState {
        open_at_login: app.settings.desktop.open_at_login,
        show_dock_or_taskbar_icon: app.settings.desktop.show_dock_or_taskbar_icon,
        plan_prices: tokenuse::ingest::LIMIT_SECTION_TOOLS
            .into_iter()
            .map(|(id, label)| PlanPriceRow {
                id,
                label,
                price: app.settings.plan_prices.get(id).copied(),
            })
            .collect(),
    }
}

fn desktop_updates() -> DesktopUpdateState {
    DesktopUpdateState {
        supported: cfg!(any(windows, target_os = "linux")),
    }
}

pub(crate) fn tray_snapshot(app: &App) -> TraySnapshot {
    TraySnapshot {
        version: env!("CARGO_PKG_VERSION"),
        copy: copy::copy(),
        status: app.status_text().map(str::to_string),
        currency: app.currency().code().to_string(),
        dashboard: app.dashboard_for(
            Period::Today,
            Tool::All,
            &ProjectFilter::All,
            &ModelFilter::All,
            SortMode::Spend,
        ),
        usage: app.usage_for(Tool::All, SortMode::Spend),
    }
}

