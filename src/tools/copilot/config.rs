use std::path::PathBuf;

use crate::tools::paths;

pub const TOOL_ID: &str = "copilot";
pub const DISPLAY_NAME: &str = "Copilot";
pub const CLI_DIR: &str = ".copilot";
pub const LEGACY_DIR: &str = ".copilot/session-state";
pub const LEGACY_EVENTS: &str = "events.jsonl";
pub const WORKSPACE_FILE: &str = "workspace.yaml";
pub const VSCODE_EXTENSION_DIR: &str = "GitHub.copilot-chat/transcripts";
pub const VSCODE_PRODUCER: &str = "copilot-agent";
pub const CHARS_PER_TOKEN: f64 = 4.0;
pub const LIMIT_SIDECAR_FILE: &str = "copilot.json";
pub const COPILOT_INTERNAL_USER_URL: &str = "https://api.github.com/copilot_internal/user";
pub const GITHUB_COPILOT_CONFIG_DIR: &str = "github-copilot";

// The Copilot CLI stopped writing per-session events.jsonl around May 2026.
// Newer builds keep turn history in a central session-store.db and, in the
// workspace app, per-session token totals in data.db.
pub const CLI_SESSION_STORE_FILE: &str = "session-store.db";
pub const CLI_DATA_STORE_FILE: &str = "data.db";
pub const CLI_STORE_PROJECT_LABEL: &str = "copilot-cli";
pub const CLI_APP_DEDUP_PREFIX: &str = "copilot:cli:";

pub const CLI_TURNS_SQL: &str = "
    SELECT t.session_id, t.turn_index, t.user_message, t.assistant_response, t.timestamp,
           s.cwd, s.repository
    FROM turns t
    LEFT JOIN sessions s ON s.id = t.session_id
    ORDER BY t.session_id, t.turn_index";

pub const CLI_APP_SESSIONS_SQL: &str = "
    SELECT id, model, total_input_tokens, total_output_tokens, total_cached_tokens,
           total_reasoning_tokens, created_at, updated_at
    FROM sessions";

pub fn legacy_root() -> Option<PathBuf> {
    paths::home().map(|h| h.join(LEGACY_DIR))
}

pub fn cli_root() -> Option<PathBuf> {
    paths::home().map(|h| h.join(CLI_DIR))
}

pub fn vscode_workspace_storage_dirs() -> Vec<PathBuf> {
    let Some(home) = paths::home() else {
        return Vec::new();
    };
    if cfg!(target_os = "macos") {
        return vec![
            home.join("Library/Application Support/Code/User/workspaceStorage"),
            home.join("Library/Application Support/Code - Insiders/User/workspaceStorage"),
        ];
    }
    if cfg!(target_os = "windows") {
        return vec![
            home.join("AppData/Roaming/Code/User/workspaceStorage"),
            home.join("AppData/Roaming/Code - Insiders/User/workspaceStorage"),
        ];
    }
    vec![
        home.join(".config/Code/User/workspaceStorage"),
        home.join(".config/Code - Insiders/User/workspaceStorage"),
        home.join(".vscode-server/data/User/workspaceStorage"),
    ]
}

pub fn limit_sidecar() -> Option<PathBuf> {
    paths::config_dir().map(|dir| dir.join("limits").join(LIMIT_SIDECAR_FILE))
}

pub fn credential_files() -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Some(config_dir) = dirs::config_dir() {
        let dir = config_dir.join(GITHUB_COPILOT_CONFIG_DIR);
        files.push(dir.join("hosts.json"));
        files.push(dir.join("apps.json"));
    }
    if let Some(home) = paths::home() {
        let dir = home.join(".config").join(GITHUB_COPILOT_CONFIG_DIR);
        files.push(dir.join("hosts.json"));
        files.push(dir.join("apps.json"));
    }
    files.sort();
    files.dedup();
    files
}
