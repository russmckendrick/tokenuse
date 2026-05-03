use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, Monitor, PhysicalPosition, Rect, RunEvent, Runtime, Theme, WebviewUrl,
    WebviewWindow, WebviewWindowBuilder, WindowEvent,
};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};
use tokenuse::{
    app::{App, AppStatus, StatusTone},
    copy, runtime,
};

mod commands;
mod ids;
mod menu;
mod notifications;
mod snapshot;
mod state;

use menu::desktop_menu;
use notifications::send_background_alert;
use state::{is_quitting, mark_quitting, CommandError, CommandResult, DesktopState, SharedState};

const MAIN_WINDOW_LABEL: &str = "main";
const TRAY_POPOVER_LABEL: &str = "tray-popover";
const TRAY_POPOVER_WIDTH: f64 = 340.0;
const TRAY_POPOVER_HEIGHT: f64 = 520.0;

pub(crate) fn restore_main_window<R: Runtime>(app_handle: &AppHandle<R>) {
    if let Some(window) = app_handle.get_webview_window(MAIN_WINDOW_LABEL) {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

pub(crate) fn hide_tray_popover_window<R: Runtime>(app_handle: &AppHandle<R>) -> CommandResult<()> {
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
    let show_item = MenuItem::with_id(
        app,
        "show",
        copy::copy().actions.show_app.as_str(),
        true,
        None::<&str>,
    )?;
    let quit_item = MenuItem::with_id(
        app,
        "quit",
        copy::copy().actions.quit_app.as_str(),
        true,
        None::<&str>,
    )?;
    let separator = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&show_item, &separator, &quit_item])?;

    TrayIconBuilder::new()
        .icon(tray_icon()?)
        .icon_as_template(cfg!(target_os = "macos"))
        .tooltip(copy::copy().brand.name.as_str())
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
                rect,
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let _ = toggle_tray_popover(tray.app_handle(), position, rect);
            }
        })
        .build(app)?;

    Ok(())
}

fn toggle_tray_popover<R: Runtime>(
    app_handle: &AppHandle<R>,
    position: PhysicalPosition<f64>,
    rect: Rect,
) -> tauri::Result<()> {
    let window = match app_handle.get_webview_window(TRAY_POPOVER_LABEL) {
        Some(window) => window,
        None => create_tray_popover_window(app_handle)?,
    };

    if window.is_visible().unwrap_or(false) {
        window.hide()?;
        return Ok(());
    }

    position_tray_popover(&window, app_handle, position, rect)?;
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
    .title(copy::copy().brand.name.as_str())
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
    rect: Rect,
) -> tauri::Result<()> {
    let monitors = app_handle.available_monitors()?;
    let Some(monitor) = tray_anchor_monitor(&monitors, rect, position) else {
        window.set_position(PhysicalPosition::new(
            position.x.round() as i32,
            position.y.round() as i32,
        ))?;
        return Ok(());
    };

    let scale = monitor.scale_factor();
    let work_area = monitor.work_area();
    let work_x = f64::from(work_area.position.x);
    let work_y = f64::from(work_area.position.y);
    let work_width = f64::from(work_area.size.width);
    let work_height = f64::from(work_area.size.height);
    let popover_width = TRAY_POPOVER_WIDTH * scale;
    let popover_height = TRAY_POPOVER_HEIGHT * scale;
    let margin = 8.0 * scale;
    let offset = 10.0 * scale;
    let anchor = tray_anchor_physical(rect, position, scale);
    let anchor = if monitor_contains(monitor, anchor) {
        anchor
    } else {
        position
    };

    let x = clamp(
        anchor.x - (popover_width / 2.0),
        work_x + margin,
        work_x + work_width - popover_width - margin,
    );
    let y = if anchor.y < work_y + (work_height / 2.0) {
        anchor.y + offset
    } else {
        anchor.y - popover_height - offset
    };
    let y = clamp(
        y,
        work_y + margin,
        work_y + work_height - popover_height - margin,
    );

    window.set_position(PhysicalPosition::new(x.round() as i32, y.round() as i32))?;
    Ok(())
}

fn tray_anchor_monitor(
    monitors: &[Monitor],
    rect: Rect,
    position: PhysicalPosition<f64>,
) -> Option<&Monitor> {
    monitors
        .iter()
        .find(|monitor| {
            let anchor = tray_anchor_physical(rect, position, monitor.scale_factor());
            monitor_contains(monitor, anchor)
        })
        .or_else(|| {
            monitors
                .iter()
                .find(|monitor| monitor_contains(monitor, position))
        })
        .or_else(|| closest_monitor(monitors, position))
}

fn tray_anchor_physical(
    rect: Rect,
    fallback: PhysicalPosition<f64>,
    scale_factor: f64,
) -> PhysicalPosition<f64> {
    let position = rect.position.to_physical::<f64>(scale_factor);
    let size = rect.size.to_physical::<f64>(scale_factor);
    if size.width <= 0.0 || size.height <= 0.0 {
        return fallback;
    }
    PhysicalPosition::new(
        position.x + (size.width / 2.0),
        position.y + (size.height / 2.0),
    )
}

fn monitor_contains(monitor: &Monitor, point: PhysicalPosition<f64>) -> bool {
    let work_area = monitor.work_area();
    let x = f64::from(work_area.position.x);
    let y = f64::from(work_area.position.y);
    let width = f64::from(work_area.size.width);
    let height = f64::from(work_area.size.height);
    point.x >= x && point.x <= x + width && point.y >= y && point.y <= y + height
}

fn closest_monitor(monitors: &[Monitor], point: PhysicalPosition<f64>) -> Option<&Monitor> {
    monitors.iter().min_by(|a, b| {
        monitor_distance_squared(a, point)
            .partial_cmp(&monitor_distance_squared(b, point))
            .unwrap_or(std::cmp::Ordering::Equal)
    })
}

fn monitor_distance_squared(monitor: &Monitor, point: PhysicalPosition<f64>) -> f64 {
    let work_area = monitor.work_area();
    let x = f64::from(work_area.position.x);
    let y = f64::from(work_area.position.y);
    let width = f64::from(work_area.size.width);
    let height = f64::from(work_area.size.height);
    let dx = if point.x < x {
        x - point.x
    } else if point.x > x + width {
        point.x - (x + width)
    } else {
        0.0
    };
    let dy = if point.y < y {
        y - point.y
    } else if point.y > y + height {
        point.y - (y + height)
    } else {
        0.0
    };
    dx.mul_add(dx, dy * dy)
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
            state.app.status = Some(AppStatus::new(notices.join(" · "), StatusTone::Warning));
        }
    }
}

pub(crate) fn sync_open_at_login<R: Runtime>(
    app_handle: &AppHandle<R>,
    enabled: bool,
) -> CommandResult<()> {
    let autostart = app_handle.autolaunch();
    let result = if enabled {
        autostart.enable()
    } else {
        autostart.disable()
    };
    result.map_err(|e| {
        CommandError::Tokenuse(copy::template(
            &copy::copy().status.open_at_login_failed,
            &[("error", e.to_string())],
        ))
    })
}

pub(crate) fn apply_dock_or_taskbar_icon<R: Runtime>(
    app_handle: &AppHandle<R>,
    visible: bool,
) -> CommandResult<()> {
    #[cfg(target_os = "macos")]
    {
        app_handle.set_dock_visibility(visible).map_err(|e| {
            CommandError::Tokenuse(copy::template(
                &copy::copy().status.dock_visibility_failed,
                &[("error", e.to_string())],
            ))
        })?;
    }

    #[cfg(not(target_os = "macos"))]
    if let Some(window) = app_handle.get_webview_window(MAIN_WINDOW_LABEL) {
        window.set_skip_taskbar(!visible).map_err(|e| {
            CommandError::Tokenuse(copy::template(
                &copy::copy().status.taskbar_visibility_failed,
                &[("error", e.to_string())],
            ))
        })?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::{export_format_id, parse_export_format, parse_tool, tool_id};
    use crate::snapshot::{snapshot, tray_snapshot};
    use tokenuse::{
        app::{Page, Period, ProjectFilter, SortMode, Tool},
        export::ExportFormat,
    };

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
    fn tool_helpers_roundtrip_gemini() {
        assert!(matches!(parse_tool("gemini").unwrap(), Tool::Gemini));
        assert_eq!(tool_id(Tool::Gemini), "gemini");
    }

    #[test]
    fn desktop_snapshot_includes_copy_and_config_row_ids() {
        let app = App::default();
        let snapshot = snapshot(&app);

        assert_eq!(snapshot.copy.brand.name, copy::copy().brand.name);
        assert_eq!(snapshot.config_rows[0].id, "currency_override");
        assert_eq!(snapshot.config_rows[1].id, "rates_json");
        assert_eq!(
            snapshot.shortcut_footer[0].label,
            copy::copy().footer("desktop")[0].label
        );
    }

    #[test]
    fn desktop_snapshot_uses_page_specific_filter_footers() {
        let mut app = App::default();

        app.set_page(Page::Usage);
        let usage = snapshot(&app);
        assert_eq!(
            usage.shortcut_footer[0].label,
            copy::copy().footer("desktop_usage")[0].label
        );

        app.set_page(Page::Config);
        let config = snapshot(&app);
        assert_eq!(
            config.shortcut_footer[0].label,
            copy::copy().footer("desktop_config")[0].label
        );
    }

    #[test]
    fn desktop_usage_snapshot_displays_fixed_disabled_filters() {
        let mut app = App::default();
        app.tool = Tool::Codex;
        app.sort = SortMode::Tokens;
        app.project_filter = ProjectFilter::Selected {
            identity: "project-id".into(),
            label: "Project".into(),
        };

        app.set_page(Page::Usage);
        let usage = snapshot(&app);

        assert_eq!(usage.tool, "all");
        assert_eq!(usage.sort, "spend");
        assert_eq!(usage.project.identity, None);
        assert_eq!(usage.project.label, copy::copy().tools.all.as_str());
        assert!(matches!(app.tool, Tool::Codex));
        assert!(matches!(app.sort, SortMode::Tokens));
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
            commands::get_snapshot,
            commands::get_tray_snapshot,
            commands::open_main_window,
            commands::hide_tray_popover,
            commands::set_page,
            commands::set_period,
            commands::set_tool,
            commands::set_sort,
            commands::set_project,
            commands::open_session,
            commands::close_session,
            commands::set_currency,
            commands::set_open_at_login,
            commands::set_show_dock_or_taskbar_icon,
            commands::refresh_archive,
            commands::clear_data,
            commands::refresh_currency_rates,
            commands::refresh_pricing_snapshot,
            commands::set_export_dir,
            commands::export_current,
            commands::handle_shortcut,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tokenuse desktop application")
        .run(handle_run_event);
}
