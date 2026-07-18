use std::path::PathBuf;

use tauri::{AppHandle, State};
use tokenuse::{
    app::{AppStatus, StatusTone},
    copy,
    data::ProjectOption,
    reports::{ReportRequest, ReportScope},
};

use crate::{
    apply_dock_or_taskbar_icon, hide_tray_popover_window,
    ids::{parse_graph_metric, parse_period, parse_report_format, parse_sort, parse_tool},
    restore_main_window,
    snapshot::{
        model_page, project_page, snapshot, tray_snapshot, DesktopSnapshot, ModelPageData,
        ProjectPageData, ReportResponse, ToolPageData, TraySnapshot,
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
pub(crate) async fn set_plan_price(
    tool: String,
    price: Option<f64>,
    state: State<'_, SharedState>,
) -> CommandResult<DesktopSnapshot> {
    with_app(state, move |app| {
        app.set_plan_price(&tool, price);
        Ok(snapshot(app))
    })
    .await
}

#[tauri::command]
pub(crate) async fn get_model_catalog(
    period: String,
    state: State<'_, SharedState>,
) -> CommandResult<Vec<tokenuse::data::ModelCatalogEntry>> {
    with_app(state, move |app| {
        let period = parse_period(&period)?;
        Ok(app.model_catalog(period))
    })
    .await
}

#[tauri::command]
pub(crate) async fn get_tool_page(
    tool: String,
    state: State<'_, SharedState>,
) -> CommandResult<ToolPageData> {
    with_app(state, move |app| {
        let tool = parse_tool(&tool)?;
        Ok(ToolPageData {
            dashboard: app.dashboard_for(
                app.period,
                tool,
                &tokenuse::app::ProjectFilter::All,
                &tokenuse::app::ModelFilter::All,
                app.sort,
            ),
            usage: app.usage_for(tool, app.sort),
        })
    })
    .await
}

#[tauri::command]
pub(crate) async fn get_project_index(
    state: State<'_, SharedState>,
) -> CommandResult<Vec<tokenuse::data::ProjectIndexRow>> {
    with_app(state, |app| Ok(app.project_index())).await
}

#[tauri::command]
pub(crate) async fn get_project_page(
    identity: String,
    state: State<'_, SharedState>,
) -> CommandResult<ProjectPageData> {
    with_app(state, move |app| Ok(project_page(app, &identity))).await
}

#[tauri::command]
pub(crate) async fn get_model_page(
    canonical_id: String,
    state: State<'_, SharedState>,
) -> CommandResult<ModelPageData> {
    with_app(state, move |app| Ok(model_page(app, &canonical_id))).await
}

#[tauri::command]
pub(crate) async fn get_analytics(
    period: String,
    state: State<'_, SharedState>,
) -> CommandResult<tokenuse::data::AnalyticsData> {
    with_app(state, move |app| {
        let period = parse_period(&period)?;
        Ok(app.analytics_for(period, app.tool, &app.project_filter.clone()))
    })
    .await
}

#[tauri::command]
pub(crate) async fn get_graph(
    metric: String,
    state: State<'_, SharedState>,
) -> CommandResult<tokenuse::graph::GraphData> {
    with_app(state, move |app| Ok(app.graph_for(parse_graph_metric(&metric)?))).await
}

#[tauri::command]
pub(crate) async fn get_session_detail(
    key: String,
    state: State<'_, SharedState>,
) -> CommandResult<Option<tokenuse::data::SessionDetailView>> {
    with_app(state, move |app| Ok(app.session_detail(&key))).await
}

#[tauri::command]
/// Return the period-aware Coach payload, including the output trend at the
/// resolution selected by the core for the requested range.
pub(crate) async fn get_coach(
    period: String,
    state: State<'_, SharedState>,
) -> CommandResult<tokenuse::data::CoachData> {
    with_app(state, move |app| {
        let period = parse_period(&period)?;
        Ok(app.coach_for(period, app.tool, &app.project_filter.clone()))
    })
    .await
}

#[tauri::command]
pub(crate) async fn get_coach_timeline(
    day: String,
    state: State<'_, SharedState>,
) -> CommandResult<Option<tokenuse::data::CoachTimelineDay>> {
    with_app(state, move |app| {
        let day = tokenuse::coach::parse_day(&day).ok_or(CommandError::Unknown {
            kind: "day",
            value: day.clone(),
        })?;
        Ok(app.coach_timeline_for(day, app.tool, &app.project_filter.clone()))
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
pub(crate) async fn set_mcp_http_enabled(
    enabled: bool,
    state: State<'_, SharedState>,
) -> CommandResult<DesktopSnapshot> {
    with_app(state, move |app| {
        if enabled {
            // Bind before persisting: a port conflict leaves the saved
            // config disabled so the toggle snaps back on the next poll.
            let bound = crate::mcp_http::start(app.settings.mcp.http_port)?;
            app.settings.mcp.http_enabled = true;
            save_user_settings(app)?;
            app.status = Some(AppStatus::new(
                copy::template(
                    &copy::copy().status.mcp_http_started,
                    &[("port", bound.to_string())],
                ),
                StatusTone::Success,
            ));
        } else {
            crate::mcp_http::stop();
            app.settings.mcp.http_enabled = false;
            save_user_settings(app)?;
            app.status = Some(AppStatus::new(
                copy::copy().status.mcp_http_stopped.clone(),
                StatusTone::Success,
            ));
        }
        Ok(snapshot(app))
    })
    .await
}

#[tauri::command]
pub(crate) async fn set_mcp_http_port(
    port: u16,
    state: State<'_, SharedState>,
) -> CommandResult<DesktopSnapshot> {
    with_app(state, move |app| {
        if port == 0 {
            return Err(CommandError::Tokenuse(
                copy::copy().status.mcp_http_port_invalid.clone(),
            ));
        }
        app.settings.mcp.http_port = port;
        save_user_settings(app)?;
        if app.settings.mcp.http_enabled {
            crate::mcp_http::start(port)?;
        }
        app.status = Some(AppStatus::new(
            copy::template(
                &copy::copy().status.mcp_http_port_set,
                &[("port", port.to_string())],
            ),
            StatusTone::Success,
        ));
        Ok(snapshot(app))
    })
    .await
}

#[tauri::command]
/// The bearer token never rides in the snapshot poll; the Config page asks
/// for it on demand (reveal / copy buttons). Reads only the token file, so
/// it skips the shared App lock like `get_doctor`.
pub(crate) async fn reveal_mcp_token() -> CommandResult<String> {
    tauri::async_runtime::spawn_blocking(|| {
        tokenuse::mcp::http::load_or_create_token(&tokenuse::config::ConfigPaths::default())
            .map_err(|e| CommandError::Tokenuse(e.to_string()))
    })
    .await
    .map_err(|e| CommandError::Join(e.to_string()))?
}

#[tauri::command]
pub(crate) async fn toggle_data_source(
    state: State<'_, SharedState>,
) -> CommandResult<DesktopSnapshot> {
    with_app(state, |app| {
        app.toggle_data_source();
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
        tokenuse::secrets::delete(tokenuse::tools::claude_subscription::config::KEYRING_ACCOUNT)
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
        tokenuse::secrets::delete(tokenuse::tools::codex_subscription::config::KEYRING_ACCOUNT)
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
/// Run the read-only per-tool doctor diagnostics. This re-walks every
/// adapter's probe roots and parses a bounded sample, so it runs on demand
/// from the Config page button — never from the snapshot poll — and on a
/// blocking task so the UI thread is not stalled. It needs no app state.
pub(crate) async fn get_doctor() -> CommandResult<tokenuse::doctor::DoctorReport> {
    tauri::async_runtime::spawn_blocking(tokenuse::doctor::report)
        .await
        .map_err(|e| CommandError::Join(e.to_string()))
}

#[tauri::command]
/// Scrollback transcript search. Reads the archive over its own read-only
/// connection, so it deliberately skips the shared App lock and runs on the
/// blocking pool like `get_doctor`.
pub(crate) async fn search_transcripts(
    query: String,
    project: Option<String>,
    tool: Option<String>,
    limit: Option<u32>,
) -> CommandResult<tokenuse::search::SearchResults> {
    tauri::async_runtime::spawn_blocking(move || {
        let paths = tokenuse::config::ConfigPaths::default();
        let filters = tokenuse::search::SearchFilters {
            project,
            tool,
            session_limit: limit.map(|l| l as usize).unwrap_or(0),
        };
        let mut results = tokenuse::search::search_transcripts(&paths, &query, &filters)
            .map_err(|e| CommandError::Tokenuse(e.to_string()))?;
        let config = tokenuse::config::UserConfig::load(&paths).unwrap_or_default();
        let currency_table = tokenuse::currency::CurrencyTable::load(&paths).unwrap_or_else(|_| {
            tokenuse::currency::CurrencyTable::embedded()
                .expect("embedded currency rates must be valid JSON")
        });
        results.format_costs(&currency_table.formatter(&config.currency));
        Ok(results)
    })
    .await
    .map_err(|e| CommandError::Join(e.to_string()))?
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
