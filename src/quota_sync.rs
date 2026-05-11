#![cfg(feature = "quota-sync")]

use crate::config::ConfigPaths;
use crate::tools::{claude_subscription, codex_subscription};

/// Best-effort subscription-quota refresh. Called from the periodic refresher
/// (and at the start of each manual reload). For each provider with a session
/// cookie in the OS keychain, fetch the live quota payload and overwrite the
/// local sidecar. Errors are swallowed silently — the next ingest pass will
/// use whatever sidecar(s) made it to disk, and the manual "Sync" Config-page
/// action remains the source of truth for surfacing failures to the user.
pub fn auto_refresh(paths: &ConfigPaths) {
    if let Ok(Some(cookie)) = crate::secrets::read(claude_subscription::config::KEYRING_ACCOUNT) {
        let _ = claude_subscription::limits::refresh_sidecar(
            &paths.claude_subscription_limits_file,
            &cookie,
        );
    }
    if let Ok(Some(cookie)) = crate::secrets::read(codex_subscription::config::KEYRING_ACCOUNT) {
        let _ = codex_subscription::limits::refresh_sidecar(
            &paths.codex_subscription_limits_file,
            &cookie,
        );
    }
}
