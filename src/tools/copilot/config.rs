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
// The Copilot Chat extension's OpenTelemetry span store: the one VS Code
// source with real token counts (input, output, cache read, cache write).
pub const OTEL_EXTENSION_DIR: &str = "github.copilot-chat";
pub const OTEL_TRACES_FILE: &str = "agent-traces.db";
pub const OTEL_DEDUP_PREFIX: &str = "copilot-otel:";
pub const OTEL_PROJECT_LABEL: &str = "copilot-chat";
// VS Code core chat-session journals: real prompt/output token counts kept
// as a delta journal per workspace, plus a global "empty window" folder.
pub const CHAT_SESSIONS_DIR: &str = "chatSessions";
pub const EMPTY_WINDOW_CHAT_SESSIONS_DIR: &str = "emptyWindowChatSessions";
pub const CHAT_SESSION_DEDUP_PREFIX: &str = "copilot-chatsession:";
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

// Newer CLI builds (~July 2026) also write per-request usage rows with real
// token counts, cache buckets, and the actual serving model. When present
// they supersede both the chars/4 turn estimates and the data.db aggregates.
pub const CLI_USAGE_EVENTS_SQL: &str = "
    SELECT e.id, e.session_id, e.turn_index, e.model,
           e.input_tokens, e.output_tokens, e.cache_read_tokens,
           e.cache_write_tokens, e.reasoning_tokens, e.created_at,
           s.cwd, s.repository
    FROM assistant_usage_events e
    LEFT JOIN sessions s ON s.id = e.session_id
    ORDER BY e.session_id, e.id";

pub const CLI_USAGE_SESSIONS_SQL: &str = "SELECT DISTINCT session_id FROM assistant_usage_events";

pub const CLI_USAGE_DEDUP_MARKER: &str = ":usage-";
pub const CLI_TURN_DEDUP_MARKER: &str = ":turn-";

// The Copilot CLI keeps signed-in accounts (including non-github.com hosts)
// in data.db; access_token is stored in plaintext by the CLI itself.
pub const CLI_ACCOUNTS_SQL: &str = "
    SELECT login, host, access_token
    FROM accounts
    WHERE access_token IS NOT NULL AND length(access_token) > 0
    ORDER BY is_default DESC, login";

pub const ENV_TOKEN_VARS: [&str; 3] = ["COPILOT_GITHUB_TOKEN", "GH_TOKEN", "GITHUB_TOKEN"];
pub const ENV_HOST_VARS: [&str; 2] = ["COPILOT_GH_HOST", "GH_HOST"];
pub const DEFAULT_HOST: &str = "github.com";

/// Copilot's user-quota endpoint for a host. github.com and GitHub Enterprise
/// Cloud data-residency tenants (`*.ghe.com`, endpoint `api.<host>`) are
/// supported; other hosts are skipped. Tokens are region-locked, so the host
/// that supplied a token must also serve its quota request.
pub fn copilot_user_url(host: &str) -> Option<String> {
    let host = host.trim().trim_matches('/').to_ascii_lowercase();
    if host.is_empty() {
        return None;
    }
    if host == DEFAULT_HOST {
        return Some(COPILOT_INTERNAL_USER_URL.to_string());
    }
    if host.ends_with(".ghe.com") {
        return Some(format!("https://api.{host}/copilot_internal/user"));
    }
    None
}

/// File name for a per-account limit sidecar, e.g.
/// `copilot-github.com-octocat.json`. The account-less name stays
/// `copilot.json` (the legacy single-account sidecar).
pub fn account_sidecar_file_name(host: &str, login: Option<&str>) -> String {
    let mut label = sanitize_file_component(host);
    if let Some(login) = login.map(str::trim).filter(|login| !login.is_empty()) {
        label.push('-');
        label.push_str(&sanitize_file_component(login));
    }
    format!("copilot-{label}.json")
}

fn sanitize_file_component(raw: &str) -> String {
    raw.trim()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '-'
            }
        })
        .collect()
}

pub fn is_limit_sidecar_name(name: &str) -> bool {
    name == LIMIT_SIDECAR_FILE || (name.starts_with("copilot-") && name.ends_with(".json"))
}

/// gh CLI config directory (`hosts.yml` lives here). gh uses `~/.config/gh`
/// on every platform, overridable via GH_CONFIG_DIR.
pub fn gh_config_dir() -> Option<PathBuf> {
    if let Some(dir) = paths::env_path("GH_CONFIG_DIR") {
        return Some(dir);
    }
    paths::home().map(|home| home.join(".config").join("gh"))
}

pub fn legacy_root() -> Option<PathBuf> {
    paths::home().map(|h| h.join(LEGACY_DIR))
}

pub fn cli_root() -> Option<PathBuf> {
    paths::home().map(|h| h.join(CLI_DIR))
}

/// One VS Code variant's storage pair. `workspace_storage` holds per-project
/// hash dirs (transcripts, chatSessions); `global_storage` holds the Copilot
/// Chat extension's OTel span store and the empty-window chat journals.
#[derive(Debug, Clone)]
pub struct VsCodeStorage {
    pub workspace_storage: PathBuf,
    pub global_storage: PathBuf,
}

pub fn vscode_storage_roots() -> Vec<VsCodeStorage> {
    vscode_user_dirs()
        .into_iter()
        .map(|user| VsCodeStorage {
            workspace_storage: user.join("workspaceStorage"),
            global_storage: user.join("globalStorage"),
        })
        .collect()
}

pub fn otel_trace_db(storage: &VsCodeStorage) -> PathBuf {
    storage
        .global_storage
        .join(OTEL_EXTENSION_DIR)
        .join(OTEL_TRACES_FILE)
}

fn vscode_user_dirs() -> Vec<PathBuf> {
    let Some(home) = paths::home() else {
        return Vec::new();
    };
    if cfg!(target_os = "macos") {
        return vec![
            home.join("Library/Application Support/Code/User"),
            home.join("Library/Application Support/Code - Insiders/User"),
            home.join("Library/Application Support/VSCodium/User"),
        ];
    }
    if cfg!(target_os = "windows") {
        return vec![
            home.join("AppData/Roaming/Code/User"),
            home.join("AppData/Roaming/Code - Insiders/User"),
            home.join("AppData/Roaming/VSCodium/User"),
        ];
    }
    vec![
        home.join(".config/Code/User"),
        home.join(".config/Code - Insiders/User"),
        home.join(".config/VSCodium/User"),
        home.join(".vscode-server/data/User"),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copilot_user_url_routes_supported_hosts() {
        assert_eq!(
            copilot_user_url("github.com").as_deref(),
            Some("https://api.github.com/copilot_internal/user")
        );
        assert_eq!(
            copilot_user_url("OctoCorp.ghe.com").as_deref(),
            Some("https://api.octocorp.ghe.com/copilot_internal/user")
        );
        assert_eq!(copilot_user_url("github.example.com"), None);
        assert_eq!(copilot_user_url(""), None);
    }

    #[test]
    fn account_sidecar_names_are_sanitized() {
        assert_eq!(
            account_sidecar_file_name("github.com", Some("R-McKendrick_Node4")),
            "copilot-github.com-R-McKendrick_Node4.json"
        );
        assert_eq!(
            account_sidecar_file_name("octo corp/ghe", None),
            "copilot-octo-corp-ghe.json"
        );
    }

    #[test]
    fn limit_sidecar_names_cover_legacy_and_account_files() {
        assert!(is_limit_sidecar_name("copilot.json"));
        assert!(is_limit_sidecar_name("copilot-github.com-octocat.json"));
        assert!(!is_limit_sidecar_name("copilot_subscription.json"));
        assert!(!is_limit_sidecar_name("claude-code.json"));
        assert!(!is_limit_sidecar_name(
            "copilot-github.com-octocat.json.bak"
        ));
    }
}
