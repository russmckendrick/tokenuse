use std::path::PathBuf;

use crate::tools::paths;

pub const TOOL_ID: &str = "cursor";
pub const DISPLAY_NAME: &str = "Cursor";
pub const STATE_DB: &str = "state.vscdb";
pub const CACHE_FILE: &str = "cursor-results.json";
pub const CHARS_PER_TOKEN: f64 = 4.0;

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

pub const BUBBLE_QUERY: &str = "SELECT key, value FROM cursorDiskKV WHERE key LIKE 'bubbleId:%'";

pub const AGENT_KV_QUERY: &str =
    "SELECT key, value FROM cursorDiskKV WHERE key LIKE 'agentKv:blob:%'";
