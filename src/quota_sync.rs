#![cfg(feature = "quota-sync")]

use std::time::Duration;

use crate::config::ConfigPaths;
use crate::tools::{claude_subscription, codex_subscription, copilot};

/// Budget for each quota-sync HTTP request. ureq has no default read/overall
/// timeout, and these calls run on the shared background Refresher thread —
/// an unbounded hang would stall every automatic and manual reload.
pub const HTTP_TIMEOUT: Duration = Duration::from_secs(20);

/// Best-effort subscription-quota refresh. Called from the periodic refresher
/// (and at the start of each manual reload). For each provider with a session
/// cookie in the OS keychain, fetch the live quota payload and overwrite the
/// local sidecar. Copilot needs no cookie: its sidecar's existence is the
/// opt-in (the first write only ever happens through the confirmed Config-page
/// sync; deleting the file opts back out). Errors are swallowed silently — the
/// next ingest pass will use whatever sidecar(s) made it to disk, and the
/// manual "Sync" Config-page action remains the source of truth for surfacing
/// failures to the user.
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
    if copilot_auto_refresh_enabled(paths) {
        let _ = copilot::limits::refresh_sidecar(&paths.copilot_limits_file);
    }
}

/// Copilot auto-refresh is consented by sidecar existence: the legacy
/// `copilot.json` or any per-account `copilot-<host>-<login>.json` is only
/// ever created by the explicit, confirmed Config-page sync action.
fn copilot_auto_refresh_enabled(paths: &ConfigPaths) -> bool {
    copilot::limits::any_sidecar_present(&paths.copilot_limits_file)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copilot_auto_refresh_gates_on_sidecar_presence() {
        let dir = std::env::temp_dir().join(format!(
            "tokenuse-quota-sync-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let paths = ConfigPaths::new(dir.clone());

        assert!(!copilot_auto_refresh_enabled(&paths));

        std::fs::create_dir_all(paths.copilot_limits_file.parent().unwrap()).unwrap();
        std::fs::write(&paths.copilot_limits_file, "{}").unwrap();
        assert!(copilot_auto_refresh_enabled(&paths));

        // Per-account sidecars keep auto-refresh enabled after the legacy
        // file is removed by a multi-account sync.
        std::fs::remove_file(&paths.copilot_limits_file).unwrap();
        assert!(!copilot_auto_refresh_enabled(&paths));
        std::fs::write(
            paths.limits_dir.join("copilot-github.com-octocat.json"),
            "{}",
        )
        .unwrap();
        assert!(copilot_auto_refresh_enabled(&paths));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
