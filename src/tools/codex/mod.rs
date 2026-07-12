use std::collections::HashSet;

use color_eyre::Result;

use super::{fingerprint_source, LimitSnapshot, ParsedCall, SessionSource, ToolAdapter};

pub mod config;
pub mod discovery;
pub mod parser;

pub struct Codex;

const SOURCE_FINGERPRINT_VERSION: &str = "codex-v2-nested-exec-tools";

impl ToolAdapter for Codex {
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
        parser::parse_session(source, seen)
    }

    fn parse_limits(&self, source: &SessionSource) -> Result<Vec<LimitSnapshot>> {
        parser::parse_session_limits(source)
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
            "/tokenuse-codex-fingerprint-test-missing".into(),
            "Codex",
            config::TOOL_ID,
        );
        let legacy = fingerprint_source(&source).unwrap();

        assert_eq!(
            Codex.source_fingerprint(&source).unwrap(),
            format!("{SOURCE_FINGERPRINT_VERSION}:{legacy}")
        );
    }
}
