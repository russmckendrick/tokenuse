use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::{Manager, Runtime, State};
use thiserror::Error;
use tokenuse::app::App;

pub(crate) type SharedState = Arc<Mutex<DesktopState>>;
pub(crate) type CommandResult<T> = Result<T, CommandError>;

pub(crate) struct DesktopState {
    pub(crate) app: App,
    pub(crate) quitting: bool,
}

#[derive(Debug, Error)]
pub(crate) enum CommandError {
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

pub(crate) async fn with_app<T, F>(state: State<'_, SharedState>, f: F) -> CommandResult<T>
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

pub(crate) fn save_user_settings(app: &App) -> CommandResult<()> {
    app.settings
        .save(&app.paths)
        .map_err(|e| CommandError::Tokenuse(e.to_string()))
}

pub(crate) fn unknown(kind: &'static str, value: &str) -> CommandError {
    CommandError::Unknown {
        kind,
        value: value.into(),
    }
}

pub(crate) fn mark_quitting<R: Runtime>(app_handle: &tauri::AppHandle<R>) {
    let state = app_handle.state::<SharedState>();
    if let Ok(mut state) = state.inner().lock() {
        state.quitting = true;
    }
}

pub(crate) fn is_quitting<R: Runtime>(app_handle: &tauri::AppHandle<R>) -> bool {
    let state = app_handle.state::<SharedState>();
    state
        .inner()
        .lock()
        .map(|state| state.quitting)
        .unwrap_or(true)
}
