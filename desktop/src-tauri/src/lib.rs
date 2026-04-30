use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use serde::Serialize;
use tauri::{
    menu::{AboutMetadata, Menu, PredefinedMenuItem, Submenu, HELP_SUBMENU_ID, WINDOW_SUBMENU_ID},
    AppHandle, Runtime, State,
};
use thiserror::Error;
use tokenuse::{
    app::{App, ConfigRowView, DataSource, Page, Period, ProjectFilter, SortMode, Tool},
    data::{DashboardData, LimitsData, ProjectOption, SessionDetailView, SessionOption},
    export::ExportFormat,
    runtime,
};

type SharedState = Arc<Mutex<DesktopState>>;
type CommandResult<T> = Result<T, CommandError>;

const APP_NAME: &str = "Token Use";
const AUTHOR: &str = "Russ McKendrick";
const HOMEPAGE_URL: &str = "https://www.tokenuse.app";

struct DesktopState {
    app: App,
}

#[derive(Debug, Error)]
enum CommandError {
    #[error("desktop state is unavailable")]
    StatePoisoned,
    #[error("background task failed: {0}")]
    Join(String),
    #[error("unknown {kind}: {value}")]
    Unknown { kind: &'static str, value: String },
    #[error("{0}")]
    Tokenuse(String),
}

impl Serialize for CommandError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

#[derive(Debug, Clone, Serialize)]
struct OptionItem {
    value: &'static str,
    label: &'static str,
}

#[derive(Debug, Clone, Serialize)]
struct ProjectState {
    identity: Option<String>,
    label: String,
}

#[derive(Debug, Clone, Serialize)]
struct DesktopSnapshot {
    version: &'static str,
    source: &'static str,
    status: Option<String>,
    page: &'static str,
    period: &'static str,
    periods: Vec<OptionItem>,
    tool: &'static str,
    tools: Vec<OptionItem>,
    sort: &'static str,
    sorts: Vec<OptionItem>,
    project: ProjectState,
    dashboard: DashboardData,
    usage: LimitsData,
    projects: Vec<ProjectOption>,
    sessions: Vec<SessionOption>,
    session: Option<SessionDetailView>,
    config_rows: Vec<ConfigRowView>,
    currencies: Vec<String>,
    currency: String,
    export_dir: String,
    export_formats: Vec<OptionItem>,
}

#[derive(Debug, Clone, Serialize)]
struct ExportResponse {
    path: String,
    snapshot: DesktopSnapshot,
}

#[tauri::command]
async fn get_snapshot(state: State<'_, SharedState>) -> CommandResult<DesktopSnapshot> {
    with_app(state, |app| Ok(snapshot(app))).await
}

#[tauri::command]
async fn set_page(page: String, state: State<'_, SharedState>) -> CommandResult<DesktopSnapshot> {
    with_app(state, move |app| {
        let page = parse_page(&page)?;
        if page != Page::Session {
            app.session_view = None;
            app.session_scroll = 0;
        }
        app.page = page;
        Ok(snapshot(app))
    })
    .await
}

#[tauri::command]
async fn set_period(
    period: String,
    state: State<'_, SharedState>,
) -> CommandResult<DesktopSnapshot> {
    with_app(state, move |app| {
        app.set_period(parse_period(&period)?);
        Ok(snapshot(app))
    })
    .await
}

#[tauri::command]
async fn set_tool(tool: String, state: State<'_, SharedState>) -> CommandResult<DesktopSnapshot> {
    with_app(state, move |app| {
        app.set_tool(parse_tool(&tool)?);
        Ok(snapshot(app))
    })
    .await
}

#[tauri::command]
async fn set_sort(sort: String, state: State<'_, SharedState>) -> CommandResult<DesktopSnapshot> {
    with_app(state, move |app| {
        app.set_sort(parse_sort(&sort)?);
        Ok(snapshot(app))
    })
    .await
}

#[tauri::command]
async fn set_project(
    identity: Option<String>,
    state: State<'_, SharedState>,
) -> CommandResult<DesktopSnapshot> {
    with_app(state, move |app| {
        app.set_project_by_identity(identity.as_deref());
        Ok(snapshot(app))
    })
    .await
}

#[tauri::command]
async fn open_session(
    key: String,
    state: State<'_, SharedState>,
) -> CommandResult<DesktopSnapshot> {
    with_app(state, move |app| {
        app.enter_session(&key);
        Ok(snapshot(app))
    })
    .await
}

#[tauri::command]
async fn close_session(state: State<'_, SharedState>) -> CommandResult<DesktopSnapshot> {
    with_app(state, |app| {
        app.leave_session();
        Ok(snapshot(app))
    })
    .await
}

#[tauri::command]
async fn set_currency(
    code: String,
    state: State<'_, SharedState>,
) -> CommandResult<DesktopSnapshot> {
    with_app(state, move |app| {
        app.set_currency(&code);
        Ok(snapshot(app))
    })
    .await
}

#[tauri::command]
async fn refresh_archive(state: State<'_, SharedState>) -> CommandResult<DesktopSnapshot> {
    with_app(state, |app| {
        app.reload();
        Ok(snapshot(app))
    })
    .await
}

#[tauri::command]
async fn refresh_currency_rates(state: State<'_, SharedState>) -> CommandResult<DesktopSnapshot> {
    with_app(state, |app| {
        app.refresh_currency_rates();
        Ok(snapshot(app))
    })
    .await
}

#[tauri::command]
async fn refresh_pricing_snapshot(state: State<'_, SharedState>) -> CommandResult<DesktopSnapshot> {
    with_app(state, |app| {
        app.refresh_pricing_snapshot();
        Ok(snapshot(app))
    })
    .await
}

#[tauri::command]
async fn set_export_dir(
    path: String,
    state: State<'_, SharedState>,
) -> CommandResult<DesktopSnapshot> {
    with_app(state, move |app| {
        if path.trim().is_empty() {
            return Err(CommandError::Tokenuse("export folder path is empty".into()));
        }
        app.set_export_dir(PathBuf::from(path));
        Ok(snapshot(app))
    })
    .await
}

#[tauri::command]
async fn export_current(
    format: String,
    state: State<'_, SharedState>,
) -> CommandResult<ExportResponse> {
    with_app(state, move |app| {
        let format = parse_export_format(&format)?;
        let path = app
            .export_current(format)
            .map_err(|e| CommandError::Tokenuse(e.to_string()))?;
        Ok(ExportResponse {
            path: path.display().to_string(),
            snapshot: snapshot(app),
        })
    })
    .await
}

async fn with_app<T, F>(state: State<'_, SharedState>, f: F) -> CommandResult<T>
where
    T: Send + 'static,
    F: FnOnce(&mut App) -> CommandResult<T> + Send + 'static,
{
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut state = state.lock().map_err(|_| CommandError::StatePoisoned)?;
        state.app.poll_reload();
        f(&mut state.app)
    })
    .await
    .map_err(|e| CommandError::Join(e.to_string()))?
}

fn snapshot(app: &App) -> DesktopSnapshot {
    DesktopSnapshot {
        version: env!("CARGO_PKG_VERSION"),
        source: match app.source {
            DataSource::Live(_) => "live",
            DataSource::Sample => "sample",
        },
        status: app.status.clone(),
        page: page_id(app.page),
        period: period_id(app.period),
        periods: Period::ALL
            .into_iter()
            .map(|period| OptionItem {
                value: period_id(period),
                label: period.label(),
            })
            .collect(),
        tool: tool_id(app.tool),
        tools: [
            Tool::All,
            Tool::ClaudeCode,
            Tool::Cursor,
            Tool::Codex,
            Tool::Copilot,
        ]
        .into_iter()
        .map(|tool| OptionItem {
            value: tool_id(tool),
            label: tool.label(),
        })
        .collect(),
        sort: sort_id(app.sort),
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
                label: "All".into(),
            },
            ProjectFilter::Selected { identity, label } => ProjectState {
                identity: Some(identity.clone()),
                label: label.clone(),
            },
        },
        dashboard: app.dashboard(),
        usage: app.usage(),
        projects: app.project_options(),
        sessions: app.session_options(),
        session: app.session_view.clone(),
        config_rows: app.config_rows(),
        currencies: app.currency_table.codes(),
        currency: app.currency().code().to_string(),
        export_dir: app.export_dir.display().to_string(),
        export_formats: ExportFormat::ALL
            .into_iter()
            .map(|format| OptionItem {
                value: export_format_id(format),
                label: format.label(),
            })
            .collect(),
    }
}

fn parse_page(value: &str) -> CommandResult<Page> {
    match value {
        "overview" => Ok(Page::Overview),
        "deep-dive" => Ok(Page::DeepDive),
        "usage" => Ok(Page::Usage),
        "config" => Ok(Page::Config),
        "session" => Ok(Page::Session),
        _ => Err(unknown("page", value)),
    }
}

fn page_id(page: Page) -> &'static str {
    match page {
        Page::Overview => "overview",
        Page::DeepDive => "deep-dive",
        Page::Config => "config",
        Page::Usage => "usage",
        Page::Session => "session",
    }
}

fn parse_period(value: &str) -> CommandResult<Period> {
    match value {
        "today" => Ok(Period::Today),
        "week" => Ok(Period::Week),
        "thirty-days" => Ok(Period::ThirtyDays),
        "month" => Ok(Period::Month),
        "all-time" => Ok(Period::AllTime),
        _ => Err(unknown("period", value)),
    }
}

fn period_id(period: Period) -> &'static str {
    match period {
        Period::Today => "today",
        Period::Week => "week",
        Period::ThirtyDays => "thirty-days",
        Period::Month => "month",
        Period::AllTime => "all-time",
    }
}

fn parse_tool(value: &str) -> CommandResult<Tool> {
    match value {
        "all" => Ok(Tool::All),
        "claude-code" => Ok(Tool::ClaudeCode),
        "cursor" => Ok(Tool::Cursor),
        "codex" => Ok(Tool::Codex),
        "copilot" => Ok(Tool::Copilot),
        _ => Err(unknown("tool", value)),
    }
}

fn tool_id(tool: Tool) -> &'static str {
    match tool {
        Tool::All => "all",
        Tool::ClaudeCode => "claude-code",
        Tool::Cursor => "cursor",
        Tool::Codex => "codex",
        Tool::Copilot => "copilot",
    }
}

fn parse_sort(value: &str) -> CommandResult<SortMode> {
    match value {
        "spend" => Ok(SortMode::Spend),
        "date" => Ok(SortMode::Date),
        "tokens" => Ok(SortMode::Tokens),
        _ => Err(unknown("sort", value)),
    }
}

fn sort_id(sort: SortMode) -> &'static str {
    match sort {
        SortMode::Spend => "spend",
        SortMode::Date => "date",
        SortMode::Tokens => "tokens",
    }
}

fn parse_export_format(value: &str) -> CommandResult<ExportFormat> {
    match value {
        "json" => Ok(ExportFormat::Json),
        "csv" => Ok(ExportFormat::Csv),
        "svg" => Ok(ExportFormat::Svg),
        "png" => Ok(ExportFormat::Png),
        _ => Err(unknown("export format", value)),
    }
}

fn export_format_id(format: ExportFormat) -> &'static str {
    match format {
        ExportFormat::Json => "json",
        ExportFormat::Csv => "csv",
        ExportFormat::Svg => "svg",
        ExportFormat::Png => "png",
    }
}

fn unknown(kind: &'static str, value: &str) -> CommandError {
    CommandError::Unknown {
        kind,
        value: value.into(),
    }
}

fn app_version_label() -> String {
    format!("v{}", env!("CARGO_PKG_VERSION"))
}

fn about_metadata() -> AboutMetadata<'static> {
    let version = app_version_label();

    AboutMetadata {
        name: Some(APP_NAME.into()),
        version: Some(version),
        #[cfg(target_os = "macos")]
        short_version: Some(String::new()),
        authors: Some(vec![AUTHOR.into()]),
        comments: Some("Local AI token usage analytics.".into()),
        copyright: Some(format!("Author: {AUTHOR}")),
        website: Some(HOMEPAGE_URL.into()),
        website_label: Some("tokenuse.app".into()),
        ..Default::default()
    }
}

fn desktop_menu<R: Runtime>(app_handle: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let about_metadata = about_metadata();

    let window_menu = Submenu::with_id_and_items(
        app_handle,
        WINDOW_SUBMENU_ID,
        "Window",
        true,
        &[
            &PredefinedMenuItem::minimize(app_handle, None)?,
            &PredefinedMenuItem::maximize(app_handle, None)?,
            #[cfg(target_os = "macos")]
            &PredefinedMenuItem::separator(app_handle)?,
            &PredefinedMenuItem::close_window(app_handle, None)?,
        ],
    )?;

    let help_menu = Submenu::with_id_and_items(
        app_handle,
        HELP_SUBMENU_ID,
        "Help",
        true,
        &[
            #[cfg(not(target_os = "macos"))]
            &PredefinedMenuItem::about(app_handle, None, Some(about_metadata.clone()))?,
        ],
    )?;

    Menu::with_items(
        app_handle,
        &[
            #[cfg(target_os = "macos")]
            &Submenu::with_items(
                app_handle,
                APP_NAME,
                true,
                &[
                    &PredefinedMenuItem::about(
                        app_handle,
                        Some("About Token Use"),
                        Some(about_metadata),
                    )?,
                    &PredefinedMenuItem::separator(app_handle)?,
                    &PredefinedMenuItem::services(app_handle, None)?,
                    &PredefinedMenuItem::separator(app_handle)?,
                    &PredefinedMenuItem::hide(app_handle, None)?,
                    &PredefinedMenuItem::hide_others(app_handle, None)?,
                    &PredefinedMenuItem::separator(app_handle)?,
                    &PredefinedMenuItem::quit(app_handle, None)?,
                ],
            )?,
            #[cfg(not(any(
                target_os = "linux",
                target_os = "dragonfly",
                target_os = "freebsd",
                target_os = "netbsd",
                target_os = "openbsd"
            )))]
            &Submenu::with_items(
                app_handle,
                "File",
                true,
                &[
                    &PredefinedMenuItem::close_window(app_handle, None)?,
                    #[cfg(not(target_os = "macos"))]
                    &PredefinedMenuItem::quit(app_handle, None)?,
                ],
            )?,
            &Submenu::with_items(
                app_handle,
                "Edit",
                true,
                &[
                    &PredefinedMenuItem::undo(app_handle, None)?,
                    &PredefinedMenuItem::redo(app_handle, None)?,
                    &PredefinedMenuItem::separator(app_handle)?,
                    &PredefinedMenuItem::cut(app_handle, None)?,
                    &PredefinedMenuItem::copy(app_handle, None)?,
                    &PredefinedMenuItem::paste(app_handle, None)?,
                    &PredefinedMenuItem::select_all(app_handle, None)?,
                ],
            )?,
            #[cfg(target_os = "macos")]
            &Submenu::with_items(
                app_handle,
                "View",
                true,
                &[&PredefinedMenuItem::fullscreen(app_handle, None)?],
            )?,
            &window_menu,
            &help_menu,
        ],
    )
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = color_eyre::install();
    let startup = runtime::load_startup().expect("load tokenuse startup data");
    let app = App::with_runtime(
        startup.source,
        startup.status,
        startup.settings,
        startup.paths,
        startup.currency_table,
        startup.initial_refresh_delay,
        startup.refresh_source,
    );

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .menu(desktop_menu)
        .manage(Arc::new(Mutex::new(DesktopState { app })))
        .invoke_handler(tauri::generate_handler![
            get_snapshot,
            set_page,
            set_period,
            set_tool,
            set_sort,
            set_project,
            open_session,
            close_session,
            set_currency,
            refresh_archive,
            refresh_currency_rates,
            refresh_pricing_snapshot,
            set_export_dir,
            export_current,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tokenuse desktop application");
}
