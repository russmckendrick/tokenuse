use std::collections::HashSet;

use color_eyre::Result;

use super::{fingerprint_source, ParsedCall, SessionSource, ToolAdapter};

pub mod config;
pub mod discovery;
pub mod parser;

pub struct Gemini;

/// Bump when the parser learns to extract new fields so archived sessions
/// re-parse through it on the next sync.
const SOURCE_FINGERPRINT_VERSION: &str = "gemini-v2-coach-enrichment";

impl ToolAdapter for Gemini {
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
            "/tokenuse-gemini-fingerprint-test-missing".into(),
            "Gemini",
            config::TOOL_ID,
        );
        let legacy = fingerprint_source(&source).unwrap();

        assert_eq!(
            Gemini.source_fingerprint(&source).unwrap(),
            format!("{SOURCE_FINGERPRINT_VERSION}:{legacy}")
        );
    }
}
