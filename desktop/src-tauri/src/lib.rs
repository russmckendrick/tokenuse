use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

use serde::Serialize;
use tauri::{
    menu::{
        AboutMetadata, Menu, MenuItem, PredefinedMenuItem, Submenu, HELP_SUBMENU_ID,
        WINDOW_SUBMENU_ID,
    },
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, LogicalPosition, Manager, PhysicalPosition, RunEvent, Runtime, State, Theme,
    WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent,
};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};
use tauri_plugin_notification::NotificationExt;
use thiserror::Error;
use tokenuse::{
    app::{
        App, BackgroundUsageAlert, ConfigRowView, DataSource, Page, Period, ProjectFilter,
        SortMode, Tool,
    },
    data::{DashboardData, LimitsData, ProjectOption, SessionDetailView, SessionOption},
    export::ExportFormat,
    keymap::{self, KeyHint, KeyInput},
    runtime,
};

type SharedState = Arc<Mutex<DesktopState>>;
type CommandResult<T> = Result<T, CommandError>;

const APP_NAME: &str = "Token Use";
const AUTHOR: &str = "Russ McKendrick";
const HOMEPAGE_URL: &str = "https://www.tokenuse.app";
const MAIN_WINDOW_LABEL: &str = "main";
const TRAY_POPOVER_LABEL: &str = "tray-popover";
const TRAY_POPOVER_WIDTH: f64 = 340.0;
const TRAY_POPOVER_HEIGHT: f64 = 460.0;

struct DesktopState {
    app: App,
    quitting: bool,
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
    desktop_settings: DesktopSettingsState,
    export_dir: String,
    export_formats: Vec<OptionItem>,
    shortcut_footer: Vec<KeyHint>,
}

#[derive(Debug, Clone, Serialize)]
struct DesktopSettingsState {
    open_at_login: bool,
    show_dock_or_taskbar_icon: bool,
}

#[derive(Debug, Clone, Serialize)]
struct TraySnapshot {
    version: &'static str,
    status: Option<String>,
    currency: String,
    dashboard: DashboardData,
    usage: LimitsData,
}

#[derive(Debug, Clone, Serialize)]
struct ExportResponse {
    path: String,
    snapshot: DesktopSnapshot,
}

#[derive(Debug, Clone, Serialize)]
struct ShortcutResponse {
    handled: bool,
    effect: Option<&'static str>,
    snapshot: DesktopSnapshot,
}

#[tauri::command]
async fn get_snapshot(state: State<'_, SharedState>) -> CommandResult<DesktopSnapshot> {
    with_app(state, |app| Ok(snapshot(app))).await
}

#[tauri::command]
async fn get_tray_snapshot(state: State<'_, SharedState>) -> CommandResult<TraySnapshot> {
    with_app(state, |app| Ok(tray_snapshot(app))).await
}

#[tauri::command]
fn open_main_window(app_handle: AppHandle) -> CommandResult<()> {
    hide_tray_popover_window(&app_handle)?;
    restore_main_window(&app_handle);
    Ok(())
}

#[tauri::command]
fn hide_tray_popover(app_handle: AppHandle) -> CommandResult<()> {
    hide_tray_popover_window(&app_handle)
}

#[tauri::command]
async fn set_page(page: String, state: State<'_, SharedState>) -> CommandResult<DesktopSnapshot> {
    with_app(state, move |app| {
        let page = parse_page(&page)?;
        if page != Page::Session {
            app.session_view = None;
            app.session_scroll = 0;
        }
        app.set_page(page);
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
async fn set_open_at_login(
    enabled: bool,
    app_handle: AppHandle,
    state: State<'_, SharedState>,
) -> CommandResult<DesktopSnapshot> {
    sync_open_at_login(&app_handle, enabled)?;
    with_app(state, move |app| {
        app.settings.desktop.open_at_login = enabled;
        save_user_settings(app)?;
        app.status = Some(format!(
            "open at login {}",
            if enabled { "enabled" } else { "disabled" }
        ));
        Ok(snapshot(app))
    })
    .await
}

#[tauri::command]
async fn set_show_dock_or_taskbar_icon(
    enabled: bool,
    app_handle: AppHandle,
    state: State<'_, SharedState>,
) -> CommandResult<DesktopSnapshot> {
    apply_dock_or_taskbar_icon(&app_handle, enabled)?;
    with_app(state, move |app| {
        app.settings.desktop.show_dock_or_taskbar_icon = enabled;
        save_user_settings(app)?;
        app.status = Some(format!(
            "Dock/taskbar icon {}",
            if enabled { "shown" } else { "hidden" }
        ));
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

#[tauri::command]
async fn handle_shortcut(
    context: String,
    input: KeyInput,
    state: State<'_, SharedState>,
) -> CommandResult<ShortcutResponse> {
    let action = keymap::keymap()
        .resolve_input(&context, &input)
        .map(str::to_string);
    with_app(state, move |app| {
        let mut effect = None;
        let handled = match action.as_deref() {
            Some(keymap::ACTION_OPEN_PROJECT_PICKER) => {
                effect = Some("open_project_picker");
                true
            }
            Some(keymap::ACTION_OPEN_SESSION_PICKER) => {
                effect = Some("open_session_picker");
                true
            }
            Some(keymap::ACTION_OPEN_EXPORT_PICKER) => {
                effect = Some("open_export_picker");
                true
            }
            Some(keymap::ACTION_CLOSE_MODAL) => {
                effect = Some("close_modal");
                true
            }
            Some(keymap::ACTION_CLOSE_CALL_DETAIL) => {
                effect = Some("close_call_detail");
                true
            }
            Some(action) => app.apply_shortcut_action(action),
            None => false,
        };
        Ok(ShortcutResponse {
            handled,
            effect,
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

fn save_user_settings(app: &App) -> CommandResult<()> {
    app.settings
        .save(&app.paths)
        .map_err(|e| CommandError::Tokenuse(e.to_string()))
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
        desktop_settings: desktop_settings(app),
        export_dir: app.export_dir.display().to_string(),
        export_formats: ExportFormat::ALL
            .into_iter()
            .map(|format| OptionItem {
                value: export_format_id(format),
                label: format.label(),
            })
            .collect(),
        shortcut_footer: keymap::keymap().footer("desktop").to_vec(),
    }
}

fn desktop_settings(app: &App) -> DesktopSettingsState {
    DesktopSettingsState {
        open_at_login: app.settings.desktop.open_at_login,
        show_dock_or_taskbar_icon: app.settings.desktop.show_dock_or_taskbar_icon,
    }
}

fn tray_snapshot(app: &App) -> TraySnapshot {
    TraySnapshot {
        version: env!("CARGO_PKG_VERSION"),
        status: app.status.clone(),
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
        "html" => Ok(ExportFormat::Html),
        "pdf" => Ok(ExportFormat::Pdf),
        _ => Err(unknown("export format", value)),
    }
}

fn export_format_id(format: ExportFormat) -> &'static str {
    match format {
        ExportFormat::Json => "json",
        ExportFormat::Csv => "csv",
        ExportFormat::Svg => "svg",
        ExportFormat::Png => "png",
        ExportFormat::Html => "html",
        ExportFormat::Pdf => "pdf",
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

fn restore_main_window<R: Runtime>(app_handle: &AppHandle<R>) {
    if let Some(window) = app_handle.get_webview_window(MAIN_WINDOW_LABEL) {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn hide_tray_popover_window<R: Runtime>(app_handle: &AppHandle<R>) -> CommandResult<()> {
    if let Some(window) = app_handle.get_webview_window(TRAY_POPOVER_LABEL) {
        window
            .hide()
            .map_err(|e| CommandError::Tokenuse(e.to_string()))?;
    }
    Ok(())
}

fn handle_run_event<R: Runtime>(app_handle: &AppHandle<R>, event: RunEvent) {
    #[cfg(target_os = "macos")]
    if matches!(event, RunEvent::Reopen { .. }) {
        restore_main_window(app_handle);
    }

    #[cfg(not(target_os = "macos"))]
    let _ = (app_handle, event);
}

fn mark_quitting<R: Runtime>(app_handle: &AppHandle<R>) {
    let state = app_handle.state::<SharedState>();
    if let Ok(mut state) = state.inner().lock() {
        state.quitting = true;
    }
}

fn is_quitting<R: Runtime>(app_handle: &AppHandle<R>) -> bool {
    let state = app_handle.state::<SharedState>();
    state
        .inner()
        .lock()
        .map(|state| state.quitting)
        .unwrap_or(true)
}

fn setup_desktop_runtime<R: Runtime>(
    app: &mut tauri::App<R>,
    state: SharedState,
) -> tauri::Result<()> {
    setup_tray(app)?;
    apply_saved_desktop_settings(app.handle(), &state);
    spawn_background_monitor(app.handle().clone(), state);
    Ok(())
}

fn setup_tray<R: Runtime>(app: &mut tauri::App<R>) -> tauri::Result<()> {
    let show_item = MenuItem::with_id(app, "show", "Show Token Use", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit Token Use", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&show_item, &separator, &quit_item])?;

    TrayIconBuilder::new()
        .icon(tray_icon()?)
        .icon_as_template(cfg!(target_os = "macos"))
        .tooltip(APP_NAME)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => restore_main_window(app),
            "quit" => {
                mark_quitting(app);
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                position,
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let _ = toggle_tray_popover(tray.app_handle(), position);
            }
        })
        .build(app)?;

    Ok(())
}

fn toggle_tray_popover<R: Runtime>(
    app_handle: &AppHandle<R>,
    position: PhysicalPosition<f64>,
) -> tauri::Result<()> {
    let window = match app_handle.get_webview_window(TRAY_POPOVER_LABEL) {
        Some(window) => window,
        None => create_tray_popover_window(app_handle)?,
    };

    if window.is_visible().unwrap_or(false) {
        window.hide()?;
        return Ok(());
    }

    position_tray_popover(&window, app_handle, position)?;
    window.show()?;
    window.set_focus()?;
    Ok(())
}

fn create_tray_popover_window<R: Runtime>(
    app_handle: &AppHandle<R>,
) -> tauri::Result<WebviewWindow<R>> {
    WebviewWindowBuilder::new(
        app_handle,
        TRAY_POPOVER_LABEL,
        WebviewUrl::App("index.html".into()),
    )
    .title("Token Use")
    .inner_size(TRAY_POPOVER_WIDTH, TRAY_POPOVER_HEIGHT)
    .min_inner_size(TRAY_POPOVER_WIDTH, TRAY_POPOVER_HEIGHT)
    .max_inner_size(TRAY_POPOVER_WIDTH, TRAY_POPOVER_HEIGHT)
    .resizable(false)
    .maximizable(false)
    .minimizable(false)
    .decorations(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .visible(false)
    .focused(false)
    .theme(Some(Theme::Dark))
    .shadow(true)
    .build()
}

fn position_tray_popover<R: Runtime>(
    window: &WebviewWindow<R>,
    app_handle: &AppHandle<R>,
    position: PhysicalPosition<f64>,
) -> tauri::Result<()> {
    let Some(monitor) = app_handle
        .monitor_from_point(position.x, position.y)?
        .or(app_handle.primary_monitor()?)
    else {
        window.set_position(LogicalPosition::new(position.x, position.y))?;
        return Ok(());
    };

    let scale = monitor.scale_factor();
    let work_area = monitor.work_area();
    let work_x = f64::from(work_area.position.x) / scale;
    let work_y = f64::from(work_area.position.y) / scale;
    let work_width = f64::from(work_area.size.width) / scale;
    let work_height = f64::from(work_area.size.height) / scale;
    let click_x = position.x / scale;
    let click_y = position.y / scale;

    let x = clamp(
        click_x - (TRAY_POPOVER_WIDTH / 2.0),
        work_x + 8.0,
        work_x + work_width - TRAY_POPOVER_WIDTH - 8.0,
    );
    let y = if click_y < work_y + (work_height / 2.0) {
        click_y + 10.0
    } else {
        click_y - TRAY_POPOVER_HEIGHT - 10.0
    };
    let y = clamp(
        y,
        work_y + 8.0,
        work_y + work_height - TRAY_POPOVER_HEIGHT - 8.0,
    );

    window.set_position(LogicalPosition::new(x, y))?;
    Ok(())
}

fn clamp(value: f64, min: f64, max: f64) -> f64 {
    if max < min {
        min
    } else {
        value.clamp(min, max)
    }
}

fn apply_saved_desktop_settings<R: Runtime>(app_handle: &AppHandle<R>, state: &SharedState) {
    let settings = match state.lock() {
        Ok(state) => state.app.settings.desktop.clone(),
        Err(_) => return,
    };

    let mut notices = Vec::new();
    if let Err(e) = sync_open_at_login(app_handle, settings.open_at_login) {
        notices.push(e.to_string());
    }
    if let Err(e) = apply_dock_or_taskbar_icon(app_handle, settings.show_dock_or_taskbar_icon) {
        notices.push(e.to_string());
    }

    if !notices.is_empty() {
        if let Ok(mut state) = state.lock() {
            state.app.status = Some(notices.join(" · "));
        }
    }
}

fn sync_open_at_login<R: Runtime>(app_handle: &AppHandle<R>, enabled: bool) -> CommandResult<()> {
    let autostart = app_handle.autolaunch();
    let result = if enabled {
        autostart.enable()
    } else {
        autostart.disable()
    };
    result.map_err(|e| CommandError::Tokenuse(format!("open at login failed · {e}")))
}

fn apply_dock_or_taskbar_icon<R: Runtime>(
    app_handle: &AppHandle<R>,
    visible: bool,
) -> CommandResult<()> {
    #[cfg(target_os = "macos")]
    {
        app_handle
            .set_dock_visibility(visible)
            .map_err(|e| CommandError::Tokenuse(format!("Dock visibility failed · {e}")))?;
    }

    #[cfg(not(target_os = "macos"))]
    if let Some(window) = app_handle.get_webview_window(MAIN_WINDOW_LABEL) {
        window
            .set_skip_taskbar(!visible)
            .map_err(|e| CommandError::Tokenuse(format!("taskbar visibility failed · {e}")))?;
    }

    Ok(())
}

fn tray_icon() -> tauri::Result<tauri::image::Image<'static>> {
    #[cfg(target_os = "macos")]
    const TRAY_ICON: &[u8] = include_bytes!("../icons/tray-menubar.png");
    #[cfg(not(target_os = "macos"))]
    const TRAY_ICON: &[u8] = include_bytes!("../icons/tray-system.png");

    tauri::image::Image::from_bytes(TRAY_ICON)
}

fn spawn_background_monitor<R: Runtime>(app_handle: AppHandle<R>, state: SharedState) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(3));

        let alerts = {
            let mut state = match state.lock() {
                Ok(state) => state,
                Err(_) => return,
            };
            if state.quitting {
                return;
            }
            state.app.poll_reload();
            state.app.take_background_alerts()
        };

        for alert in alerts {
            send_background_alert(&app_handle, alert);
        }
    });
}

fn send_background_alert<R: Runtime>(app_handle: &AppHandle<R>, alert: BackgroundUsageAlert) {
    let _ = app_handle
        .notification()
        .builder()
        .title("Token Use usage alert")
        .body(background_alert_body(alert))
        .show();
}

fn background_alert_body(alert: BackgroundUsageAlert) -> String {
    let mut parts = Vec::new();
    if alert.cost_usd > 0.0 {
        parts.push(format!("${:.2}", alert.cost_usd));
    }
    if alert.tokens > 0 {
        parts.push(format!("{} tokens", format_compact_count(alert.tokens)));
    }
    if alert.calls > 0 {
        parts.push(format!(
            "{} {}",
            format_int(alert.calls),
            plural(alert.calls, "call", "calls")
        ));
    }

    let summary = if parts.is_empty() {
        "usage changed".into()
    } else {
        parts.join(", ")
    };
    format!("Usage jumped by {summary} since the last alert baseline.")
}

fn format_compact_count(n: u64) -> String {
    if n >= 1_000_000_000 {
        format!("{:.1}B", n as f64 / 1_000_000_000.0)
    } else if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        format_int(n)
    }
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

fn plural<'a>(count: u64, singular: &'a str, plural: &'a str) -> &'a str {
    if count == 1 {
        singular
    } else {
        plural
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_format_helpers_roundtrip_html_and_pdf() {
        assert!(matches!(
            parse_export_format("html").unwrap(),
            ExportFormat::Html
        ));
        assert_eq!(export_format_id(ExportFormat::Html), "html");
        assert!(matches!(
            parse_export_format("pdf").unwrap(),
            ExportFormat::Pdf
        ));
        assert_eq!(export_format_id(ExportFormat::Pdf), "pdf");
    }

    #[test]
    fn tray_snapshot_uses_24h_defaults_without_changing_main_state() {
        let mut app = App::default();
        app.period = Period::AllTime;
        app.tool = Tool::ClaudeCode;
        app.sort = SortMode::Tokens;
        app.project_filter = ProjectFilter::Selected {
            identity: "project-id".into(),
            label: "Project".into(),
        };

        let period = app.period;
        let tool = app.tool;
        let sort = app.sort;
        let project_filter = app.project_filter.clone();
        let expected = app.dashboard_for(
            Period::Today,
            Tool::All,
            &ProjectFilter::All,
            SortMode::Spend,
        );
        let snapshot = tray_snapshot(&app);

        assert_eq!(snapshot.dashboard.summary.cost, expected.summary.cost);
        assert_eq!(snapshot.dashboard.summary.calls, expected.summary.calls);
        assert_eq!(app.period, period);
        assert_eq!(app.tool, tool);
        assert_eq!(app.sort, sort);
        assert_eq!(app.project_filter, project_filter);
    }

    #[test]
    fn background_alert_body_formats_usage_delta() {
        let body = background_alert_body(BackgroundUsageAlert {
            calls: 25,
            tokens: 120_000,
            cost_usd: 1.25,
        });

        assert_eq!(
            body,
            "Usage jumped by $1.25, 120.0K tokens, 25 calls since the last alert baseline."
        );
    }

    #[test]
    fn background_alert_body_skips_zero_delta_parts() {
        let body = background_alert_body(BackgroundUsageAlert {
            calls: 1,
            tokens: 0,
            cost_usd: 0.0,
        });

        assert_eq!(
            body,
            "Usage jumped by 1 call since the last alert baseline."
        );
    }
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
    let shared_state = Arc::new(Mutex::new(DesktopState {
        app,
        quitting: false,
    }));
    let monitor_state = shared_state.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            restore_main_window(app);
        }))
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            app.set_theme(Some(Theme::Dark));
            setup_desktop_runtime(app, monitor_state.clone())?;
            Ok(())
        })
        .menu(desktop_menu)
        .manage(shared_state)
        .on_window_event(|window, event| {
            if window.label() == TRAY_POPOVER_LABEL {
                match event {
                    WindowEvent::CloseRequested { api, .. } => {
                        api.prevent_close();
                        let _ = window.hide();
                    }
                    WindowEvent::Focused(false) => {
                        let _ = window.hide();
                    }
                    _ => {}
                }
                return;
            }
            if window.label() != MAIN_WINDOW_LABEL {
                return;
            }
            if let WindowEvent::CloseRequested { api, .. } = event {
                let app_handle = window.app_handle();
                if !is_quitting(app_handle) {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_snapshot,
            get_tray_snapshot,
            open_main_window,
            hide_tray_popover,
            set_page,
            set_period,
            set_tool,
            set_sort,
            set_project,
            open_session,
            close_session,
            set_currency,
            set_open_at_login,
            set_show_dock_or_taskbar_icon,
            refresh_archive,
            refresh_currency_rates,
            refresh_pricing_snapshot,
            set_export_dir,
            export_current,
            handle_shortcut,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tokenuse desktop application")
        .run(handle_run_event);
}
