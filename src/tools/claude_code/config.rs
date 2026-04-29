use std::path::PathBuf;

use crate::tools::paths;

pub const TOOL_ID: &str = "claude-code";
pub const DISPLAY_NAME: &str = "Claude Code";
pub const SESSION_GLOB_EXT: &str = "jsonl";
pub const SUBAGENTS_DIR: &str = "subagents";
pub const DESKTOP_WALK_DEPTH: usize = 8;
pub const ENV_OVERRIDE: &str = "CLAUDE_CONFIG_DIR";

pub fn claude_dir() -> Option<PathBuf> {
    if let Some(p) = paths::env_path(ENV_OVERRIDE) {
        return Some(p);
    }
    paths::home().map(|h| h.join(".claude"))
}

pub fn projects_dir() -> Option<PathBuf> {
    claude_dir().map(|d| d.join("projects"))
}

pub fn desktop_sessions_dir() -> Option<PathBuf> {
    let home = paths::home()?;
    if cfg!(target_os = "macos") {
        Some(home.join("Library/Application Support/Claude/local-agent-mode-sessions"))
    } else if cfg!(target_os = "windows") {
        Some(home.join("AppData/Roaming/Claude/local-agent-mode-sessions"))
    } else {
        Some(home.join(".config/Claude/local-agent-mode-sessions"))
    }
}

pub fn unsanitize_project(dir_name: &str) -> String {
    dir_name.replace('-', "/")
}
