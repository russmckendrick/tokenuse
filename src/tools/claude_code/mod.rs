use std::collections::HashSet;

use color_eyre::Result;

use super::{
    fingerprint_source, LimitSnapshot, ParsedCall, SessionSource, SessionSourceKind, ToolAdapter,
};

pub mod config;
pub mod discovery;
pub mod limits;
pub mod parser;
pub mod statusline;

pub struct ClaudeCode;

/// Bump when the parser learns to extract new fields so archived sessions
/// re-parse through it on the next sync.
const SOURCE_FINGERPRINT_VERSION: &str = "claude-code-v3-streamed-blocks";

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

    fn source_fingerprint(&self, source: &SessionSource) -> Result<String> {
        Ok(format!(
            "{SOURCE_FINGERPRINT_VERSION}:{}",
            fingerprint_source(source)?
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_version_forces_archived_sessions_through_new_parser() {
        let source = SessionSource::session(
            "/tokenuse-claude-fingerprint-test-missing".into(),
            "Claude",
            config::TOOL_ID,
        );
        let legacy = fingerprint_source(&source).unwrap();

        assert_eq!(
            ClaudeCode.source_fingerprint(&source).unwrap(),
            format!("{SOURCE_FINGERPRINT_VERSION}:{legacy}")
        );
    }
}
