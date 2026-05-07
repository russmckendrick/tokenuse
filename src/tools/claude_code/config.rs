use std::path::PathBuf;

use crate::tools::paths;

pub const TOOL_ID: &str = "claude-code";
pub const DISPLAY_NAME: &str = "Claude Code";
pub const SESSION_GLOB_EXT: &str = "jsonl";
pub const SUBAGENTS_DIR: &str = "subagents";
pub const DESKTOP_WALK_DEPTH: usize = 8;
pub const ENV_OVERRIDE: &str = "CLAUDE_CONFIG_DIR";
pub const XDG_CONFIG_OVERRIDE: &str = "XDG_CONFIG_HOME";
pub const LIMIT_SIDECAR_FILE: &str = "claude-code.json";
pub const STATUSLINE_DIR: &str = "statusline";
pub const WRAPPER_FILE_UNIX: &str = "claude-code.sh";
pub const WRAPPER_FILE_WINDOWS: &str = "claude-code.ps1";
pub const WRAPPER_MARKER: &str = "tokenuse:claude-code-statusline:v1";
pub const SETTINGS_FILE: &str = "settings.json";

pub fn claude_dirs() -> Vec<PathBuf> {
    if let Some(raw) = std::env::var_os(ENV_OVERRIDE) {
        let dirs = raw
            .to_string_lossy()
            .split(',')
            .map(str::trim)
            .filter(|p| !p.is_empty())
            .map(PathBuf::from)
            .collect::<Vec<_>>();
        if !dirs.is_empty() {
            return dirs;
        }
    }

    let mut dirs = Vec::new();
    if let Some(xdg) = paths::env_path(XDG_CONFIG_OVERRIDE) {
        dirs.push(xdg.join("claude"));
    } else if let Some(home) = paths::home() {
        dirs.push(home.join(".config").join("claude"));
    }
    if let Some(home) = paths::home() {
        dirs.push(home.join(".claude"));
    }
    dirs
}

pub fn projects_dirs() -> Vec<PathBuf> {
    claude_dirs()
        .into_iter()
        .map(|d| d.join("projects"))
        .collect()
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

pub fn limit_sidecar() -> Option<PathBuf> {
    paths::config_dir().map(|dir| dir.join("limits").join(LIMIT_SIDECAR_FILE))
}

pub fn wrapper_path() -> Option<PathBuf> {
    let dir = paths::config_dir()?.join(STATUSLINE_DIR);
    let file = if cfg!(target_os = "windows") {
        WRAPPER_FILE_WINDOWS
    } else {
        WRAPPER_FILE_UNIX
    };
    Some(dir.join(file))
}

pub fn settings_path() -> Option<PathBuf> {
    paths::home().map(|home| home.join(".claude").join(SETTINGS_FILE))
}

pub fn unsanitize_project(dir_name: &str) -> String {
    dir_name.replace('-', "/")
}
