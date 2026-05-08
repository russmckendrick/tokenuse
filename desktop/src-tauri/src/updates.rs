use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DesktopUpdateMetadata {
    pub(crate) version: String,
    pub(crate) current_version: String,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(not(any(windows, target_os = "linux")), allow(dead_code))]
#[serde(tag = "event", content = "data", rename_all = "camelCase")]
pub(crate) enum DesktopUpdateDownloadEvent {
    #[serde(rename_all = "camelCase")]
    Started {
        content_length: Option<u64>,
    },
    #[serde(rename_all = "camelCase")]
    Progress {
        chunk_length: usize,
    },
    Finished,
}

#[derive(Debug, Error)]
pub(crate) enum DesktopUpdateError {
    #[cfg(any(windows, target_os = "linux"))]
    #[error(transparent)]
    Updater(#[from] tauri_plugin_updater::Error),
    #[cfg(any(windows, target_os = "linux"))]
    #[error("pending desktop update state is unavailable")]
    PendingUpdatePoisoned,
    #[cfg(any(windows, target_os = "linux"))]
    #[error("there is no pending desktop update")]
    NoPendingUpdate,
    #[cfg(not(any(windows, target_os = "linux")))]
    #[error("desktop updates are available on Windows and Linux only")]
    Unsupported,
}

impl Serialize for DesktopUpdateError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

pub(crate) type DesktopUpdateResult<T> = Result<T, DesktopUpdateError>;

#[cfg(any(windows, target_os = "linux"))]
mod supported {
    use std::sync::Mutex;

    use tauri_plugin_updater::{Update, UpdaterExt};

    use super::{
        DesktopUpdateDownloadEvent, DesktopUpdateError, DesktopUpdateMetadata, DesktopUpdateResult,
    };

    #[derive(Default)]
    pub(crate) struct PendingDesktopUpdate(pub(crate) Mutex<Option<Update>>);

    pub(crate) async fn check_desktop_update(
        app: tauri::AppHandle,
        pending_update: tauri::State<'_, PendingDesktopUpdate>,
    ) -> DesktopUpdateResult<Option<DesktopUpdateMetadata>> {
        let update = app.updater()?.check().await?;
        let metadata = update.as_ref().map(|update| DesktopUpdateMetadata {
            version: update.version.clone(),
            current_version: update.current_version.clone(),
        });

        *pending_update
            .0
            .lock()
            .map_err(|_| DesktopUpdateError::PendingUpdatePoisoned)? = update;

        Ok(metadata)
    }

    pub(crate) async fn install_desktop_update(
        app: tauri::AppHandle,
        pending_update: tauri::State<'_, PendingDesktopUpdate>,
        on_event: tauri::ipc::Channel<DesktopUpdateDownloadEvent>,
    ) -> DesktopUpdateResult<()> {
        let update = {
            let mut pending_update = pending_update
                .0
                .lock()
                .map_err(|_| DesktopUpdateError::PendingUpdatePoisoned)?;
            pending_update
                .take()
                .ok_or(DesktopUpdateError::NoPendingUpdate)?
        };

        let mut started = false;
        update
            .download_and_install(
                |chunk_length, content_length| {
                    if !started {
                        let _ =
                            on_event.send(DesktopUpdateDownloadEvent::Started { content_length });
                        started = true;
                    }
                    let _ = on_event.send(DesktopUpdateDownloadEvent::Progress { chunk_length });
                },
                || {
                    let _ = on_event.send(DesktopUpdateDownloadEvent::Finished);
                },
            )
            .await?;

        app.restart();
    }
}

#[cfg(any(windows, target_os = "linux"))]
pub(crate) use supported::PendingDesktopUpdate;

#[cfg(any(windows, target_os = "linux"))]
#[tauri::command]
pub(crate) async fn check_desktop_update(
    app: tauri::AppHandle,
    pending_update: tauri::State<'_, PendingDesktopUpdate>,
) -> DesktopUpdateResult<Option<DesktopUpdateMetadata>> {
    supported::check_desktop_update(app, pending_update).await
}

#[cfg(any(windows, target_os = "linux"))]
#[tauri::command]
pub(crate) async fn install_desktop_update(
    app: tauri::AppHandle,
    pending_update: tauri::State<'_, PendingDesktopUpdate>,
    on_event: tauri::ipc::Channel<DesktopUpdateDownloadEvent>,
) -> DesktopUpdateResult<()> {
    supported::install_desktop_update(app, pending_update, on_event).await
}

#[cfg(not(any(windows, target_os = "linux")))]
#[tauri::command]
pub(crate) async fn check_desktop_update() -> DesktopUpdateResult<Option<DesktopUpdateMetadata>> {
    Err(DesktopUpdateError::Unsupported)
}

#[cfg(not(any(windows, target_os = "linux")))]
#[tauri::command]
pub(crate) async fn install_desktop_update(
    _on_event: tauri::ipc::Channel<DesktopUpdateDownloadEvent>,
) -> DesktopUpdateResult<()> {
    Err(DesktopUpdateError::Unsupported)
}
