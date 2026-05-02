use std::path::PathBuf;

use crate::tools::paths;

pub const TOOL_ID: &str = "copilot";
pub const DISPLAY_NAME: &str = "Copilot";
pub const LEGACY_DIR: &str = ".copilot/session-state";
pub const LEGACY_EVENTS: &str = "events.jsonl";
pub const WORKSPACE_FILE: &str = "workspace.yaml";
pub const VSCODE_EXTENSION_DIR: &str = "GitHub.copilot-chat/transcripts";
pub const VSCODE_PRODUCER: &str = "copilot-agent";
pub const CHARS_PER_TOKEN: f64 = 4.0;

pub fn legacy_root() -> Option<PathBuf> {
    paths::home().map(|h| h.join(LEGACY_DIR))
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
