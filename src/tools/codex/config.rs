use std::path::PathBuf;

use crate::tools::paths;

pub const TOOL_ID: &str = "codex";
pub const DISPLAY_NAME: &str = "Codex";
pub const ENV_OVERRIDE: &str = "CODEX_HOME";
pub const SESSIONS_SUBDIR: &str = "sessions";
pub const ROLLOUT_PREFIX: &str = "rollout-";
pub const SESSION_GLOB_EXT: &str = "jsonl";

pub fn codex_home() -> Option<PathBuf> {
    if let Some(p) = paths::env_path(ENV_OVERRIDE) {
        return Some(p);
    }
    paths::home().map(|h| h.join(".codex"))
}

pub fn sessions_root() -> Option<PathBuf> {
    codex_home().map(|d| d.join(SESSIONS_SUBDIR))
}
