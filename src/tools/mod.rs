use std::collections::HashSet;

use color_eyre::Result;

pub mod claude_code;
pub mod codex;
pub mod copilot;
pub mod cursor;
pub mod jsonl;
pub mod paths;
pub mod types;

pub use types::{LimitCredits, LimitSnapshot, LimitWindow, ParsedCall, SessionSource, Speed};

pub trait ToolAdapter: Send + Sync {
    fn id(&self) -> &'static str;
    fn display_name(&self) -> &'static str;

    fn discover(&self) -> Result<Vec<SessionSource>>;

    fn parse(&self, source: &SessionSource, seen: &mut HashSet<String>) -> Result<Vec<ParsedCall>>;

    fn parse_limits(&self, _source: &SessionSource) -> Result<Vec<LimitSnapshot>> {
        Ok(Vec::new())
    }

    fn model_display(&self, model: &str) -> String {
        model.to_string()
    }

    fn tool_display(&self, tool: &str) -> String {
        tool.to_string()
    }
}

pub fn registry() -> Vec<Box<dyn ToolAdapter>> {
    vec![
        Box::new(claude_code::ClaudeCode),
        Box::new(cursor::Cursor),
        Box::new(codex::Codex),
        Box::new(copilot::Copilot),
    ]
}
