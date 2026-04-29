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
    pub tool: &'static str,
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LimitWindow {
    pub used_percent: f64,
    pub window_minutes: u64,
    pub resets_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LimitCredits {
    pub has_credits: bool,
    pub unlimited: bool,
    pub balance: Option<f64>,
}

#[derive(Debug, Clone, Default)]
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
}
