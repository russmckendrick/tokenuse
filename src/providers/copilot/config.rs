use std::path::PathBuf;

use crate::providers::paths;

pub const PROVIDER_ID: &str = "copilot";
pub const DISPLAY_NAME: &str = "Copilot";
pub const LEGACY_DIR: &str = ".copilot/session-state";
pub const LEGACY_EVENTS: &str = "events.jsonl";
pub const VSCODE_EXTENSION_DIR: &str = "GitHub.copilot-chat/transcripts";
pub const VSCODE_PRODUCER: &str = "copilot-agent";
pub const CHARS_PER_TOKEN: f64 = 4.0;

pub fn legacy_root() -> Option<PathBuf> {
    paths::home().map(|h| h.join(LEGACY_DIR))
}

pub fn vscode_workspace_storage() -> Option<PathBuf> {
    let home = paths::home()?;
    let base = if cfg!(target_os = "macos") {
        home.join("Library/Application Support/Code/User/workspaceStorage")
    } else if cfg!(target_os = "windows") {
        home.join("AppData/Roaming/Code/User/workspaceStorage")
    } else {
        home.join(".config/Code/User/workspaceStorage")
    };
    Some(base)
}
