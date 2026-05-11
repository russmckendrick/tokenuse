use std::path::PathBuf;

use crate::tools::paths;

pub const TOOL_ID: &str = "codex_subscription";
pub const DISPLAY_NAME: &str = "Codex Subscription";
pub const DISPLAY_TOOL: &str = "codex";

pub const LIMIT_SIDECAR_FILE: &str = "codex_subscription.json";
pub const KEYRING_ACCOUNT: &str = "codex_subscription.session";

pub const BASE_URL: &str = "https://chatgpt.com";
pub const AUTH_SESSION_PATH: &str = "/api/auth/session";
pub const USAGE_PATH: &str = "/backend-api/wham/usage";
pub const REFERER: &str = "https://chatgpt.com/";
pub const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

pub fn limit_sidecar() -> Option<PathBuf> {
    paths::config_dir().map(|dir| dir.join("limits").join(LIMIT_SIDECAR_FILE))
}

pub fn auth_session_url() -> String {
    format!("{BASE_URL}{AUTH_SESSION_PATH}")
}

pub fn usage_url() -> String {
    format!("{BASE_URL}{USAGE_PATH}")
}
