use std::path::PathBuf;

use tauri::{AppHandle, State};
use tokenuse::{
    advice::{AdviceDataScope, AdviceItemStatus, AdviceTool},
    app::{AppStatus, Page, StatusTone},
    copy,
    data::ProjectOption,
    keymap::{self, KeyInput},
    reports::{ReportRequest, ReportScope},
};

use crate::{
    apply_dock_or_taskbar_icon, hide_tray_popover_window,
    ids::{parse_page, parse_period, parse_report_format, parse_sort, parse_tool},
    restore_main_window,
    snapshot::{
        snapshot, tray_snapshot, DesktopSnapshot, ReportResponse, ShortcutResponse, TraySnapshot,
    },
    state::{save_user_settings, with_app, CommandError, CommandResult, SharedState},
    sync_open_at_login,
};

#[tauri::command]
pub(crate) async fn get_snapshot(state: State<'_, SharedState>) -> CommandResult<DesktopSnapshot> {
    with_app(state, |app| Ok(snapshot(app))).await
}

#[tauri::command]
pub(crate) async fn get_tray_snapshot(
    state: State<'_, SharedState>,
) -> CommandResult<TraySnapshot> {
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
pub(crate) async fn set_page(
    page: String,
    state: State<'_, SharedState>,
) -> CommandResult<DesktopSnapshot> {
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
pub(crate) async fn set_tool(
    tool: String,
    state: State<'_, SharedState>,
) -> CommandResult<DesktopSnapshot> {
    with_app(state, move |app| {
        app.set_tool(parse_tool(&tool)?);
        Ok(snapshot(app))
    })
    .await
}

#[tauri::command]
pub(crate) async fn set_sort(
    sort: String,
    state: State<'_, SharedState>,
) -> CommandResult<DesktopSnapshot> {
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
pub(crate) async fn set_advice_tool(
    tool: String,
    state: State<'_, SharedState>,
) -> CommandResult<DesktopSnapshot> {
    with_app(state, move |app| {
        let tool = AdviceTool::from_id(&tool)
            .ok_or_else(|| CommandError::Unknown {
                kind: "advice tool",
                value: tool,
            })?;
        app.set_advice_tool(tool);
        Ok(snapshot(app))
    })
    .await
}

#[tauri::command]
pub(crate) async fn prepare_advice_prompts(
    state: State<'_, SharedState>,
) -> CommandResult<DesktopSnapshot> {
    with_app(state, |app| {
        app.prepare_advice_prompts();
        Ok(snapshot(app))
    })
    .await
}

#[tauri::command]
pub(crate) async fn generate_advice(
    data_scope: String,
    state: State<'_, SharedState>,
) -> CommandResult<DesktopSnapshot> {
    with_app(state, move |app| {
        let data_scope = AdviceDataScope::from_id(&data_scope)
            .ok_or_else(|| CommandError::Unknown {
                kind: "advice data scope",
                value: data_scope,
            })?;
        app.generate_advice(data_scope)
            .map_err(CommandError::Tokenuse)?;
        Ok(snapshot(app))
    })
    .await
}

#[tauri::command]
pub(crate) async fn update_advice_item_status(
    item_id: i64,
    status: String,
    notes: Option<String>,
    state: State<'_, SharedState>,
) -> CommandResult<DesktopSnapshot> {
    with_app(state, move |app| {
        let status = AdviceItemStatus::from_id(&status)
            .ok_or_else(|| CommandError::Unknown {
                kind: "advice item status",
                value: status,
            })?;
        app.update_advice_item_status(item_id, status, notes)
            .map_err(CommandError::Tokenuse)?;
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
pub(crate) async fn refresh_archive(
    state: State<'_, SharedState>,
) -> CommandResult<DesktopSnapshot> {
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
pub(crate) async fn refresh_currency_rates(
    state: State<'_, SharedState>,
) -> CommandResult<DesktopSnapshot> {
    with_app(state, |app| {
        app.refresh_currency_rates();
        Ok(snapshot(app))
    })
    .await
}

#[tauri::command]
pub(crate) async fn refresh_pricing_snapshot(
    state: State<'_, SharedState>,
) -> CommandResult<DesktopSnapshot> {
    with_app(state, |app| {
        app.refresh_pricing_snapshot();
        Ok(snapshot(app))
    })
    .await
}

#[tauri::command]
pub(crate) async fn sync_claude_limits(
    state: State<'_, SharedState>,
) -> CommandResult<DesktopSnapshot> {
    with_app(state, |app| {
        app.sync_claude_limits();
        Ok(snapshot(app))
    })
    .await
}

#[tauri::command]
pub(crate) async fn install_claude_statusline(
    state: State<'_, SharedState>,
) -> CommandResult<DesktopSnapshot> {
    with_app(state, |app| {
        app.install_claude_statusline();
        Ok(snapshot(app))
    })
    .await
}

#[tauri::command]
pub(crate) async fn install_claude_statusline_manual(
    state: State<'_, SharedState>,
) -> CommandResult<DesktopSnapshot> {
    with_app(state, |app| {
        app.install_claude_statusline_manual();
        Ok(snapshot(app))
    })
    .await
}

#[tauri::command]
pub(crate) async fn uninstall_claude_statusline(
    state: State<'_, SharedState>,
) -> CommandResult<DesktopSnapshot> {
    with_app(state, |app| {
        app.uninstall_claude_statusline();
        Ok(snapshot(app))
    })
    .await
}

#[tauri::command]
pub(crate) async fn sync_copilot_limits(
    state: State<'_, SharedState>,
) -> CommandResult<DesktopSnapshot> {
    with_app(state, |app| {
        app.sync_copilot_limits();
        Ok(snapshot(app))
    })
    .await
}

#[tauri::command]
pub(crate) async fn sync_claude_subscription_limits(
    state: State<'_, SharedState>,
) -> CommandResult<DesktopSnapshot> {
    with_app(state, |app| {
        app.sync_claude_subscription_limits();
        Ok(snapshot(app))
    })
    .await
}

#[tauri::command]
pub(crate) async fn sync_codex_subscription_limits(
    state: State<'_, SharedState>,
) -> CommandResult<DesktopSnapshot> {
    with_app(state, |app| {
        app.sync_codex_subscription_limits();
        Ok(snapshot(app))
    })
    .await
}

#[cfg(feature = "quota-sync")]
#[tauri::command]
pub(crate) async fn set_claude_session_cookie(
    value: String,
    state: State<'_, SharedState>,
) -> CommandResult<DesktopSnapshot> {
    with_app(state, move |app| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(CommandError::Tokenuse(
                "Claude session cookie value is empty".into(),
            ));
        }
        tokenuse::secrets::store(
            tokenuse::tools::claude_subscription::config::KEYRING_ACCOUNT,
            trimmed,
        )
        .map_err(|e| CommandError::Tokenuse(e.to_string()))?;
        Ok(snapshot(app))
    })
    .await
}

#[cfg(feature = "quota-sync")]
#[tauri::command]
pub(crate) async fn clear_claude_session_cookie(
    state: State<'_, SharedState>,
) -> CommandResult<DesktopSnapshot> {
    with_app(state, |app| {
        tokenuse::secrets::delete(
            tokenuse::tools::claude_subscription::config::KEYRING_ACCOUNT,
        )
        .map_err(|e| CommandError::Tokenuse(e.to_string()))?;
        Ok(snapshot(app))
    })
    .await
}

#[cfg(feature = "quota-sync")]
#[tauri::command]
pub(crate) async fn set_codex_session_cookie(
    value: String,
    state: State<'_, SharedState>,
) -> CommandResult<DesktopSnapshot> {
    with_app(state, move |app| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(CommandError::Tokenuse(
                "Codex session-token cookie value is empty".into(),
            ));
        }
        tokenuse::secrets::store(
            tokenuse::tools::codex_subscription::config::KEYRING_ACCOUNT,
            trimmed,
        )
        .map_err(|e| CommandError::Tokenuse(e.to_string()))?;
        Ok(snapshot(app))
    })
    .await
}

#[cfg(feature = "quota-sync")]
#[tauri::command]
pub(crate) async fn clear_codex_session_cookie(
    state: State<'_, SharedState>,
) -> CommandResult<DesktopSnapshot> {
    with_app(state, |app| {
        tokenuse::secrets::delete(
            tokenuse::tools::codex_subscription::config::KEYRING_ACCOUNT,
        )
        .map_err(|e| CommandError::Tokenuse(e.to_string()))?;
        Ok(snapshot(app))
    })
    .await
}

#[cfg(not(feature = "quota-sync"))]
#[tauri::command]
pub(crate) async fn set_claude_session_cookie(
    _value: String,
    _state: State<'_, SharedState>,
) -> CommandResult<DesktopSnapshot> {
    Err(CommandError::Tokenuse(
        "Subscription quota sync unavailable in this build".into(),
    ))
}

#[cfg(not(feature = "quota-sync"))]
#[tauri::command]
pub(crate) async fn clear_claude_session_cookie(
    _state: State<'_, SharedState>,
) -> CommandResult<DesktopSnapshot> {
    Err(CommandError::Tokenuse(
        "Subscription quota sync unavailable in this build".into(),
    ))
}

#[cfg(not(feature = "quota-sync"))]
#[tauri::command]
pub(crate) async fn set_codex_session_cookie(
    _value: String,
    _state: State<'_, SharedState>,
) -> CommandResult<DesktopSnapshot> {
    Err(CommandError::Tokenuse(
        "Subscription quota sync unavailable in this build".into(),
    ))
}

#[cfg(not(feature = "quota-sync"))]
#[tauri::command]
pub(crate) async fn clear_codex_session_cookie(
    _state: State<'_, SharedState>,
) -> CommandResult<DesktopSnapshot> {
    Err(CommandError::Tokenuse(
        "Subscription quota sync unavailable in this build".into(),
    ))
}

#[tauri::command]
pub(crate) async fn set_report_dir(
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
pub(crate) async fn report_projects(
    period: String,
    state: State<'_, SharedState>,
) -> CommandResult<Vec<ProjectOption>> {
    with_app(state, move |app| {
        let period = parse_period(&period)?;
        Ok(app.report_project_options(period))
    })
    .await
}

#[tauri::command]
pub(crate) async fn generate_report(
    format: String,
    period: String,
    project_identity: Option<String>,
    redacted: bool,
    state: State<'_, SharedState>,
) -> CommandResult<ReportResponse> {
    with_app(state, move |app| {
        let format = parse_report_format(&format)?;
        let period = parse_period(&period)?;
        let scope = match project_identity {
            Some(identity) => app
                .report_project_options(period)
                .into_iter()
                .find(|option| option.identity.as_deref() == Some(identity.as_str()))
                .map(|option| ReportScope::Project {
                    identity: identity.clone(),
                    label: option.label,
                })
                .ok_or_else(|| {
                    CommandError::Tokenuse(copy::template(
                        &copy::copy().status.project_not_found,
                        &[("identity", identity.clone())],
                    ))
                })?,
            None => ReportScope::AllProjects,
        };
        let path = app
            .generate_report(ReportRequest {
                format,
                period,
                scope,
                redacted,
            })
            .map_err(|e| CommandError::Tokenuse(e.to_string()))?;
        Ok(ReportResponse {
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
            Some(keymap::ACTION_GENERATE_ADVICE_SELECTED) => {
                effect = Some("generate_advice_selected");
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
