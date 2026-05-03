use std::path::PathBuf;

use tauri::{AppHandle, State};
use tokenuse::{
    app::{AppStatus, Page, StatusTone},
    copy,
    keymap::{self, KeyInput},
};

use crate::{
    apply_dock_or_taskbar_icon, hide_tray_popover_window,
    ids::{parse_export_format, parse_page, parse_period, parse_sort, parse_tool},
    restore_main_window,
    snapshot::{snapshot, tray_snapshot, DesktopSnapshot, ExportResponse, ShortcutResponse, TraySnapshot},
    state::{save_user_settings, with_app, CommandError, CommandResult, SharedState},
    sync_open_at_login,
};

#[tauri::command]
pub(crate) async fn get_snapshot(state: State<'_, SharedState>) -> CommandResult<DesktopSnapshot> {
    with_app(state, |app| Ok(snapshot(app))).await
}

#[tauri::command]
pub(crate) async fn get_tray_snapshot(state: State<'_, SharedState>) -> CommandResult<TraySnapshot> {
    with_app(state, |app| Ok(tray_snapshot(app))).await
}

#[tauri::command]
pub(crate) fn open_main_window(app_handle: AppHandle) -> CommandResult<()> {
    hide_tray_popover_window(&app_handle)?;
    restore_main_window(&app_handle);
    Ok(())
}

#[tauri::command]
pub(crate) fn hide_tray_popover(app_handle: AppHandle) -> CommandResult<()> {
    hide_tray_popover_window(&app_handle)
}

#[tauri::command]
pub(crate) async fn set_page(page: String, state: State<'_, SharedState>) -> CommandResult<DesktopSnapshot> {
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
pub(crate) async fn set_period(
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
pub(crate) async fn set_tool(tool: String, state: State<'_, SharedState>) -> CommandResult<DesktopSnapshot> {
    with_app(state, move |app| {
        app.set_tool(parse_tool(&tool)?);
        Ok(snapshot(app))
    })
    .await
}

#[tauri::command]
pub(crate) async fn set_sort(sort: String, state: State<'_, SharedState>) -> CommandResult<DesktopSnapshot> {
    with_app(state, move |app| {
        app.set_sort(parse_sort(&sort)?);
        Ok(snapshot(app))
    })
    .await
}

#[tauri::command]
pub(crate) async fn set_project(
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
pub(crate) async fn open_session(
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
pub(crate) async fn close_session(state: State<'_, SharedState>) -> CommandResult<DesktopSnapshot> {
    with_app(state, |app| {
        app.leave_session();
        Ok(snapshot(app))
    })
    .await
}

#[tauri::command]
pub(crate) async fn set_currency(
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
pub(crate) async fn set_open_at_login(
    enabled: bool,
    app_handle: AppHandle,
    state: State<'_, SharedState>,
) -> CommandResult<DesktopSnapshot> {
    sync_open_at_login(&app_handle, enabled)?;
    with_app(state, move |app| {
        app.settings.desktop.open_at_login = enabled;
        save_user_settings(app)?;
        let state = if enabled {
            copy::copy().desktop.enabled.as_str()
        } else {
            copy::copy().desktop.disabled.as_str()
        };
        app.status = Some(AppStatus::new(
            copy::template(
                &copy::copy().status.open_at_login_state,
                &[("state", state.to_string())],
            ),
            StatusTone::Success,
        ));
        Ok(snapshot(app))
    })
    .await
}

#[tauri::command]
pub(crate) async fn set_show_dock_or_taskbar_icon(
    enabled: bool,
    app_handle: AppHandle,
    state: State<'_, SharedState>,
) -> CommandResult<DesktopSnapshot> {
    apply_dock_or_taskbar_icon(&app_handle, enabled)?;
    with_app(state, move |app| {
        app.settings.desktop.show_dock_or_taskbar_icon = enabled;
        save_user_settings(app)?;
        let state = if enabled {
            copy::copy().desktop.shown.as_str()
        } else {
            copy::copy().desktop.hidden.as_str()
        };
        app.status = Some(AppStatus::new(
            copy::template(
                &copy::copy().status.dock_taskbar_icon_state,
                &[("state", state.to_string())],
            ),
            StatusTone::Success,
        ));
        Ok(snapshot(app))
    })
    .await
}

#[tauri::command]
pub(crate) async fn refresh_archive(state: State<'_, SharedState>) -> CommandResult<DesktopSnapshot> {
    with_app(state, |app| {
        app.reload();
        Ok(snapshot(app))
    })
    .await
}

#[tauri::command]
pub(crate) async fn clear_data(state: State<'_, SharedState>) -> CommandResult<DesktopSnapshot> {
    with_app(state, |app| {
        app.clear_data();
        Ok(snapshot(app))
    })
    .await
}

#[tauri::command]
pub(crate) async fn refresh_currency_rates(state: State<'_, SharedState>) -> CommandResult<DesktopSnapshot> {
    with_app(state, |app| {
        app.refresh_currency_rates();
        Ok(snapshot(app))
    })
    .await
}

#[tauri::command]
pub(crate) async fn refresh_pricing_snapshot(state: State<'_, SharedState>) -> CommandResult<DesktopSnapshot> {
    with_app(state, |app| {
        app.refresh_pricing_snapshot();
        Ok(snapshot(app))
    })
    .await
}

#[tauri::command]
pub(crate) async fn set_export_dir(
    path: String,
    state: State<'_, SharedState>,
) -> CommandResult<DesktopSnapshot> {
    with_app(state, move |app| {
        if path.trim().is_empty() {
            return Err(CommandError::Tokenuse(
                copy::copy().status.export_folder_path_empty.clone(),
            ));
        }
        app.set_export_dir(PathBuf::from(path));
        Ok(snapshot(app))
    })
    .await
}

#[tauri::command]
pub(crate) async fn export_current(
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
pub(crate) async fn handle_shortcut(
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
