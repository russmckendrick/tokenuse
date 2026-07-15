use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Speed {
    #[default]
    Standard,
    Fast,
}

#[derive(Debug, Clone)]
pub struct SessionSource {
    pub path: PathBuf,
    pub project: String,
    pub tool: &'static str,
    pub kind: SessionSourceKind,
}

impl SessionSource {
    pub fn session(path: PathBuf, project: impl Into<String>, tool: &'static str) -> Self {
        Self {
            path,
            project: project.into(),
            tool,
            kind: SessionSourceKind::Session,
        }
    }

    pub fn limit(path: PathBuf, project: impl Into<String>, tool: &'static str) -> Self {
        Self {
            path,
            project: project.into(),
            tool,
            kind: SessionSourceKind::Limit,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionSourceKind {
    Session,
    Limit,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LimitSnapshot {
    pub tool: &'static str,
    pub limit_id: String,
    pub limit_name: Option<String>,
    pub plan_type: Option<String>,
    pub observed_at: Option<DateTime<Utc>>,
    pub primary: Option<LimitWindow>,
    pub secondary: Option<LimitWindow>,
    pub credits: Option<LimitCredits>,
    pub rate_limit_reached_type: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LimitWindow {
    pub used_percent: f64,
    pub window_minutes: u64,
    pub resets_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LimitCredits {
    pub has_credits: bool,
    pub unlimited: bool,
    pub balance: Option<f64>,
    pub total: Option<f64>,
    pub additional_usage: Option<bool>,
}

/// One chunk of AI-generated code attributed to a call: a fenced block in
/// assistant text, or a Write/Edit-style tool payload counted as code output.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CodeBlock {
    /// Lowercase fence tag or file extension; "unknown" when neither exists.
    pub language: String,
    pub loc: u64,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ParsedCall {
    pub tool: &'static str,
    pub model: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_creation_input_tokens: u64,
    pub cache_read_input_tokens: u64,
    pub cached_input_tokens: u64,
    pub reasoning_tokens: u64,
    pub web_search_requests: u64,
    pub cost_usd: f64,
    pub tools: Vec<String>,
    pub bash_commands: Vec<String>,
    pub timestamp: Option<DateTime<Utc>>,
    pub speed: Speed,
    pub dedup_key: String,
    pub user_message: String,
    pub session_id: String,
    pub project: String,
    /// `false` also when the source has no cancellation signal; the coach's
    /// per-tool capability table decides which tools enter cancellation rates.
    pub is_canceled: bool,
    /// Full prompt length in chars, measured before `user_message` truncation.
    /// `None` = source lacks the signal (excluded from ratio denominators).
    pub prompt_chars: Option<u64>,
    pub response_chars: Option<u64>,
    /// User-message to final-assistant-message turn latency.
    pub elapsed_ms: Option<u64>,
    pub code_blocks: Vec<CodeBlock>,
    pub edited_files: Vec<String>,
    pub referenced_files: Vec<String>,
}
