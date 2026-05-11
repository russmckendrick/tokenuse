use std::path::PathBuf;

use crate::tools::paths;

pub const TOOL_ID: &str = "claude_subscription";
pub const DISPLAY_NAME: &str = "Claude Subscription";
pub const DISPLAY_TOOL: &str = "claude-code";

pub const LIMIT_SIDECAR_FILE: &str = "claude_subscription.json";
pub const KEYRING_ACCOUNT: &str = "claude_subscription.session";

pub const BASE_URL: &str = "https://claude.ai";
pub const ORGANIZATIONS_PATH: &str = "/api/organizations";
pub const USAGE_PATH_TEMPLATE: &str = "/api/organizations/{org}/usage";
pub const OVERAGE_PATH_TEMPLATE: &str = "/api/organizations/{org}/overage_spend_limit";
pub const REFERER: &str = "https://claude.ai/settings/usage";
pub const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";
pub const ANTHROPIC_CLIENT_PLATFORM: &str = "web_claude_ai";
pub const ANTHROPIC_CLIENT_VERSION: &str = "1.0.0";

pub fn limit_sidecar() -> Option<PathBuf> {
    paths::config_dir().map(|dir| dir.join("limits").join(LIMIT_SIDECAR_FILE))
}

pub fn usage_url(organization_uuid: &str) -> String {
    format!(
        "{BASE_URL}{}",
        USAGE_PATH_TEMPLATE.replace("{org}", organization_uuid)
    )
}

pub fn overage_url(organization_uuid: &str) -> String {
    format!(
        "{BASE_URL}{}",
        OVERAGE_PATH_TEMPLATE.replace("{org}", organization_uuid)
    )
}

pub fn organizations_url() -> String {
    format!("{BASE_URL}{ORGANIZATIONS_PATH}")
}
