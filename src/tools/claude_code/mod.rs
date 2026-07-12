use std::collections::HashSet;

use color_eyre::Result;

use super::{LimitSnapshot, ParsedCall, SessionSource, SessionSourceKind, ToolAdapter};

pub mod config;
pub mod discovery;
pub mod limits;
pub mod parser;
pub mod statusline;

pub struct ClaudeCode;

impl ToolAdapter for ClaudeCode {
    fn id(&self) -> &'static str {
        config::TOOL_ID
    }

    fn display_name(&self) -> &'static str {
        config::DISPLAY_NAME
    }

    fn discover(&self) -> Result<Vec<SessionSource>> {
        discovery::discover()
    }

    fn parse(&self, source: &SessionSource, seen: &mut HashSet<String>) -> Result<Vec<ParsedCall>> {
        if source.kind == SessionSourceKind::Limit {
            return Ok(Vec::new());
        }
        parser::parse_session(source, seen)
    }

    fn parse_limits(&self, source: &SessionSource) -> Result<Vec<LimitSnapshot>> {
        if source.kind == SessionSourceKind::Limit {
            return limits::parse_sidecar(source);
        }
        Ok(Vec::new())
    }
}
