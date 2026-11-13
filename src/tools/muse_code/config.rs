use std::path::PathBuf;

use crate::tools::paths;

pub const TOOL_ID: &str = "muse-code";
pub const DISPLAY_NAME: &str = "Muse Code";
/// Points directly at the sessions directory. This is first-party: the
/// shipped session tooling resolves the same variable, defaulting to
/// `${XDG_DATA_HOME:-$HOME/.local/share}/muse/sessions`.
pub const ENV_OVERRIDE: &str = "MUSE_SESSIONS";
pub const XDG_DATA_HOME_ENV: &str = "XDG_DATA_HOME";
pub const DATA_SUBDIR: &str = "muse";
pub const LOCAL_SHARE_SUBDIR: &str = ".local/share";
pub const SESSIONS_SUBDIR: &str = "sessions";
pub const SESSION_FILE: &str = "session.jsonl";
pub const SUBAGENT_DIR: &str = "subagent";
/// Date-sharded layout is `sessions/YYYY/MM/DD/<session-id>/session.jsonl`
/// with subagent logs nested deeper; 8 covers the nesting with headroom.
pub const WALK_MAX_DEPTH: usize = 8;

/// Bump when the parser learns to extract new fields so archived sessions
/// re-parse through it on the next sync.
pub const SOURCE_FINGERPRINT_VERSION: &str = "muse-code-v3-cache-subset";

pub fn sessions_root() -> Option<PathBuf> {
    if let Some(sessions) = paths::env_path(ENV_OVERRIDE) {
        return Some(sessions);
    }
    if let Some(xdg) = paths::env_path(XDG_DATA_HOME_ENV) {
        return Some(xdg.join(DATA_SUBDIR).join(SESSIONS_SUBDIR));
    }
    // Muse uses the same XDG-style path on every platform, including
    // Windows, so this is not `dirs::data_dir()`.
    paths::home().map(|h| {
        h.join(LOCAL_SHARE_SUBDIR)
            .join(DATA_SUBDIR)
            .join(SESSIONS_SUBDIR)
    })
}
