use std::path::PathBuf;

use crate::tools::paths;

pub const TOOL_ID: &str = "gemini";
pub const DISPLAY_NAME: &str = "Gemini";
pub const DEFAULT_DIR: &str = ".gemini";
pub const ENV_OVERRIDE: &str = "GEMINI_DIR";
pub const TMP_DIR: &str = "tmp";
pub const CHATS_DIR: &str = "chats";
pub const SESSION_PREFIX: &str = "session-";
pub const JSON_EXT: &str = "json";
pub const JSONL_EXT: &str = "jsonl";

pub fn gemini_tmp_root() -> Option<PathBuf> {
    if let Some(path) = paths::env_path(ENV_OVERRIDE) {
        return Some(path.join(TMP_DIR));
    }
    paths::home().map(|home| home.join(DEFAULT_DIR).join(TMP_DIR))
}
