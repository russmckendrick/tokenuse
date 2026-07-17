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
pub const CHATS_DIR: &str = "chats";
pub const STORE_DB: &str = "store.db";

pub fn state_db_path() -> Option<PathBuf> {
    if let Some(root) = paths::env_path(AGENT_HOME_ENV) {
        return Some(root.join(STATE_DB));
    }
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

pub fn chats_dir() -> Option<PathBuf> {
    agent_home().map(|h| h.join(CHATS_DIR))
}

pub const VALIDATE_STATE_QUERY: &str = "SELECT COUNT(*) FROM cursorDiskKV LIMIT 1";

pub const STATE_BUBBLE_QUERY: &str = r#"
SELECT key,
       json_extract(value, '$.bubbleId'), json_extract(value, '$.requestId'),
       json_extract(value, '$.type'), json_extract(value, '$.text'),
       json_extract(value, '$.createdAt'),
       json_extract(value, '$.tokenCount.inputTokens'),
       json_extract(value, '$.tokenCount.outputTokens'),
       json_extract(value, '$.modelInfo.modelName'),
       json_extract(value, '$.turnDurationMs'), json_extract(value, '$.unifiedMode'),
       json_extract(value, '$.isAgentic'), json_extract(value, '$.isPlanExecution'),
       json_extract(value, '$.conversationId'),
       json_extract(value, '$.toolFormerData.name'),
       json_extract(value, '$.toolFormerData.status'),
       json_extract(value, '$.toolFormerData.params'),
       json_extract(value, '$.toolFormerData.rawArgs'),
       json_extract(value, '$.attachedFileCodeChunksUris'),
       json_extract(value, '$.attachedFileCodeChunksMetadataOnly'),
       json_extract(value, '$.attachedCodeChunks'), json_extract(value, '$.deletedFiles'),
       json_extract(value, '$.diffsSinceLastApply'),
       json_extract(value, '$.fileDiffTrajectories')
FROM cursorDiskKV WHERE key LIKE 'bubbleId:%' ORDER BY rowid
"#;

pub const STATE_COMPOSER_QUERY: &str = r#"
SELECT key, json_extract(value, '$.composerId'),
       json_extract(value, '$.fullConversationHeadersOnly'),
       json_extract(value, '$.modelConfig.modelName'),
       json_extract(value, '$.forceMode'), json_extract(value, '$.unifiedMode'),
       json_extract(value, '$.isAgentic'), json_extract(value, '$.status'),
       json_extract(value, '$.createdAt'),
       json_extract(value, '$.promptTokenBreakdown.totalUsedTokens'),
       json_extract(value, '$.contextTokensUsed')
FROM cursorDiskKV WHERE key LIKE 'composerData:%' ORDER BY rowid
"#;

/// Dedup key prefix for the once-per-conversation input credit taken from
/// Cursor's own context meter.
pub const COMPOSER_INPUT_DEDUP_PREFIX: &str = "cursor:composer-input:";

/// Per-workspace composer inventory. Cursor renamed `composer.composerData`
/// to `composer.composerHeaders` in newer builds, so both keys are read.
pub const WORKSPACE_COMPOSER_QUERY: &str = "SELECT value FROM ItemTable WHERE key IN ('composer.composerData', 'composer.composerHeaders')";

/// The per-workspace storage tree sibling to the global `state.vscdb`:
/// `<User>/workspaceStorage/<hash>/{workspace.json,state.vscdb}`.
pub fn workspace_storage_root() -> Option<PathBuf> {
    let global_storage = state_db_path()?.parent()?.to_path_buf();
    Some(global_storage.parent()?.join("workspaceStorage"))
}

pub const STATE_CONTEXT_QUERY: &str = r#"
SELECT key, json_extract(value, '$.attachedFileCodeChunksMetadataOnly'),
       json_extract(value, '$.deletedFiles'), json_extract(value, '$.diffsSinceLastApply'),
       json_extract(value, '$.currentFileLocationData')
FROM cursorDiskKV WHERE key LIKE 'messageRequestContext:%' ORDER BY rowid
"#;

pub const STATE_AGENT_QUERY: &str = r#"
SELECT json_extract(value, '$.role'),
       CASE WHEN json_extract(value, '$.role') IN ('user', 'assistant')
            THEN json_extract(value, '$.content') END,
       json_extract(value, '$.providerOptions.cursor.requestId'),
       json_extract(value, '$.providerOptions.cursor.highLevelToolCallResult.output.success.executionTime'),
       json_extract(value, '$.providerOptions.cursor.highLevelToolCallResult.output.success.localExecutionTimeMs')
FROM cursorDiskKV
WHERE key LIKE 'agentKv:blob:%' AND json_valid(value)
  AND json_extract(value, '$.role') IN ('user', 'assistant', 'tool')
ORDER BY rowid
"#;

pub const TRACKING_SUMMARY_QUERY: &str =
    "SELECT conversationId, model, mode, updatedAt FROM conversation_summaries";
pub const TRACKING_HASH_QUERY: &str = r#"
SELECT conversationId, model, COALESCE(timestamp, createdAt), fileName
FROM ai_code_hashes
WHERE conversationId IS NOT NULL AND conversationId != ''
ORDER BY COALESCE(timestamp, createdAt)
"#;
pub const STORE_META_QUERY: &str = "SELECT value FROM meta WHERE key = '0'";
pub const STORE_BLOB_QUERY: &str = "SELECT data FROM blobs WHERE id = ?1";
