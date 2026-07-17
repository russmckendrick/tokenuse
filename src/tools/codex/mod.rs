use std::collections::HashSet;

use color_eyre::Result;

use super::{fingerprint_source, LimitSnapshot, ParsedCall, SessionSource, ToolAdapter};

pub mod config;
pub mod discovery;
pub mod parser;

pub struct Codex;

/// Bump when the parser changes what it extracts so archived rollouts
/// re-parse on the next sync. Version 6 re-keys every call onto
/// lineage-addressed dedup keys (fork-aware) and retires the legacy
/// path-based rows via supersession.
const SOURCE_FINGERPRINT_VERSION: &str = "codex-v6-fork-aware-dedup";

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

    fn probe_roots(&self) -> Vec<super::ProbeRoot> {
        let mut roots = Vec::new();
        if let Some(sessions) = config::sessions_root() {
            roots.push(super::ProbeRoot::new("sessions", sessions));
        }
        if let Some(archived) = config::archived_sessions_root() {
            roots.push(super::ProbeRoot::new("archived sessions", archived));
        }
        roots
    }

    fn env_overrides(&self) -> &'static [&'static str] {
        &[config::ENV_OVERRIDE]
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
