//! Process-global MCP HTTP listener. The desktop app hosts at most one
//! loopback endpoint (started from the Config-page toggle, or at launch when
//! the saved setting is enabled), so the handle lives in module statics
//! rather than `DesktopState` — that keeps `snapshot(app: &App)` and the ~50
//! commands that call it untouched. Always serves pseudonymised project
//! names; `--real-names` stays a CLI-only escape hatch.

use std::sync::{Mutex, MutexGuard, PoisonError};

use tokenuse::config::ConfigPaths;
use tokenuse::copy;
use tokenuse::mcp::http::{serve_http, HttpOptions, HttpServerHandle};

use crate::state::{CommandError, CommandResult};

static SERVER: Mutex<Option<HttpServerHandle>> = Mutex::new(None);
static LAST_ERROR: Mutex<Option<String>> = Mutex::new(None);

/// Start (or move to a new port by replacing) the listener. Returns the
/// bound port on success; on failure the formatted error is retained for the
/// snapshot's `last_error` and returned as the command error.
pub(crate) fn start(port: u16) -> CommandResult<u16> {
    let mut server = lock(&SERVER);
    if let Some(existing) = server.take() {
        existing.shutdown();
    }
    let options = HttpOptions {
        port,
        real_names: false,
    };
    match serve_http(&ConfigPaths::default(), &options) {
        Ok(handle) => {
            let bound = handle.port();
            *server = Some(handle);
            *lock(&LAST_ERROR) = None;
            Ok(bound)
        }
        Err(error) => {
            let message = copy::template(
                &copy::copy().status.mcp_http_start_failed,
                &[("error", error.to_string())],
            );
            *lock(&LAST_ERROR) = Some(message.clone());
            Err(CommandError::Tokenuse(message))
        }
    }
}

pub(crate) fn stop() {
    if let Some(handle) = lock(&SERVER).take() {
        handle.shutdown();
    }
    *lock(&LAST_ERROR) = None;
}

/// (`running`, `last_error`) for the snapshot poll.
pub(crate) fn status() -> (bool, Option<String>) {
    (lock(&SERVER).is_some(), lock(&LAST_ERROR).clone())
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
