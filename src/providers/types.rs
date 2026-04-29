use std::path::PathBuf;

use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Speed {
    Standard,
    Fast,
}

impl Default for Speed {
    fn default() -> Self {
        Self::Standard
    }
}

#[derive(Debug, Clone)]
pub struct SessionSource {
    pub path: PathBuf,
    pub project: String,
    pub provider: &'static str,
}

#[derive(Debug, Clone, Default)]
pub struct ParsedCall {
    pub provider: &'static str,
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
}
