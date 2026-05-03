use serde::Serialize;
use tokenuse::{
    app::{App, ConfigRowView, DataSource, Page, Period, ProjectFilter, SortMode, Tool},
    copy::{self, CopyDeck, CopyKeyHint},
    data::{DashboardData, LimitsData, ProjectOption, SessionDetailView, SessionOption},
    export::ExportFormat,
};

use crate::ids::{export_format_id, page_id, period_id, sort_id, status_tone_id, tool_id};

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
    pub(crate) source: &'static str,
    pub(crate) status: Option<String>,
    pub(crate) status_tone: &'static str,
    pub(crate) page: &'static str,
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
    pub(crate) sessions: Vec<SessionOption>,
    pub(crate) session: Option<SessionDetailView>,
    pub(crate) config_rows: Vec<ConfigRowView>,
    pub(crate) currencies: Vec<String>,
    pub(crate) currency: String,
    pub(crate) desktop_settings: DesktopSettingsState,
    pub(crate) export_dir: String,
    pub(crate) export_formats: Vec<OptionItem>,
    pub(crate) shortcut_footer: Vec<CopyKeyHint>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct DesktopSettingsState {
    pub(crate) open_at_login: bool,
    pub(crate) show_dock_or_taskbar_icon: bool,
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
pub(crate) struct ExportResponse {
    pub(crate) path: String,
    pub(crate) snapshot: DesktopSnapshot,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ShortcutResponse {
    pub(crate) handled: bool,
    pub(crate) effect: Option<&'static str>,
    pub(crate) snapshot: DesktopSnapshot,
}

pub(crate) fn snapshot(app: &App) -> DesktopSnapshot {
    let uses_fixed_usage_filters = app.page == Page::Usage;
    let tool = if uses_fixed_usage_filters {
        Tool::All
    } else {
        app.tool
    };
    let sort = if uses_fixed_usage_filters {
        SortMode::Spend
    } else {
        app.sort
    };

    DesktopSnapshot {
        version: env!("CARGO_PKG_VERSION"),
        copy: copy::copy(),
        source: match app.source {
            DataSource::Live(_) => "live",
            DataSource::Sample => "sample",
        },
        status: app.status_text().map(str::to_string),
        status_tone: status_tone_id(app.status_tone()),
        page: page_id(app.page),
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
        project: if uses_fixed_usage_filters {
            ProjectState {
                identity: None,
                label: copy::copy().tools.all.clone(),
            }
        } else {
            match &app.project_filter {
                ProjectFilter::All => ProjectState {
                    identity: None,
                    label: copy::copy().tools.all.clone(),
                },
                ProjectFilter::Selected { identity, label } => ProjectState {
                    identity: Some(identity.clone()),
                    label: label.clone(),
                },
            }
        },
        dashboard: app.dashboard(),
        usage: app.usage_for(tool, sort),
        projects: app.project_options(),
        sessions: app.session_options(),
        session: app.session_view.clone(),
        config_rows: app.config_rows(),
        currencies: app.currency_table.codes(),
        currency: app.currency().code().to_string(),
        desktop_settings: desktop_settings(app),
        export_dir: app.export_dir.display().to_string(),
        export_formats: ExportFormat::ALL
            .into_iter()
            .map(|format| OptionItem {
                value: export_format_id(format),
                label: format.label(),
            })
            .collect(),
        shortcut_footer: copy::copy().footer(desktop_footer_name(app)).to_vec(),
    }
}

fn desktop_footer_name(app: &App) -> &'static str {
    match app.page {
        Page::Usage => "desktop_usage",
        Page::Config => "desktop_config",
        _ => "desktop",
    }
}

fn desktop_settings(app: &App) -> DesktopSettingsState {
    DesktopSettingsState {
        open_at_login: app.settings.desktop.open_at_login,
        show_dock_or_taskbar_icon: app.settings.desktop.show_dock_or_taskbar_icon,
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
            SortMode::Spend,
        ),
        usage: app.usage_for(Tool::All, SortMode::Spend),
    }
}
