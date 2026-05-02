use std::path::PathBuf;

use crate::tools::paths;

pub const TOOL_ID: &str = "cursor";
pub const DISPLAY_NAME: &str = "Cursor";
pub const STATE_DB: &str = "state.vscdb";
pub const CACHE_FILE: &str = "cursor-results.json";
pub const CHARS_PER_TOKEN: f64 = 4.0;
pub const AGENT_HOME_ENV: &str = "CURSOR_AGENT_HOME";
pub const AGENT_PROJECTS_DIR: &str = "projects";
pub const AGENT_TRANSCRIPTS_DIR: &str = "agent-transcripts";
pub const AGENT_SUBAGENTS_DIR: &str = "subagents";
pub const AGENT_TRACKING_DB: &str = "ai-code-tracking.db";
pub const AGENT_TRACKING_DIR: &str = "ai-tracking";

pub fn state_db_path() -> Option<PathBuf> {
    let home = paths::home()?;
    let base = if cfg!(target_os = "macos") {
        home.join("Library/Application Support/Cursor/User/globalStorage")
    } else if cfg!(target_os = "windows") {
        home.join("AppData/Roaming/Cursor/User/globalStorage")
    } else {
        home.join(".config/Cursor/User/globalStorage")
    };
    Some(base.join(STATE_DB))
}

pub fn cache_path() -> Option<PathBuf> {
    paths::cache_dir().map(|c| c.join(CACHE_FILE))
}

pub fn agent_home() -> Option<PathBuf> {
    paths::env_path(AGENT_HOME_ENV).or_else(|| paths::home().map(|h| h.join(".cursor")))
}

pub fn agent_projects_dir() -> Option<PathBuf> {
    agent_home().map(|h| h.join(AGENT_PROJECTS_DIR))
}

pub fn agent_tracking_db_path() -> Option<PathBuf> {
    agent_home().map(|h| h.join(AGENT_TRACKING_DIR).join(AGENT_TRACKING_DB))
}

pub const BUBBLE_QUERY: &str = "SELECT key, value FROM cursorDiskKV WHERE key LIKE 'bubbleId:%'";

pub const AGENT_KV_QUERY: &str =
    "SELECT key, value FROM cursorDiskKV WHERE key LIKE 'agentKv:blob:%'";
