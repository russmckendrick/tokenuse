use std::collections::HashSet;

use color_eyre::Result;

use super::{fingerprint_source, ParsedCall, SessionSource, ToolAdapter};

pub mod config;
pub mod discovery;
pub mod parser;

pub struct MuseCode;

impl ToolAdapter for MuseCode {
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
            "{}:{}",
            config::SOURCE_FINGERPRINT_VERSION,
            fingerprint_source(source)?
        ))
    }

    fn probe_roots(&self) -> Vec<super::ProbeRoot> {
        config::sessions_root()
            .map(|root| vec![super::ProbeRoot::new("sessions", root)])
            .unwrap_or_default()
    }

    fn env_overrides(&self) -> &'static [&'static str] {
        &[config::ENV_OVERRIDE, config::XDG_DATA_HOME_ENV]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_version_forces_archived_sessions_through_new_parser() {
        let source = SessionSource::session(
            "/tokenuse-muse-fingerprint-test-missing".into(),
            "Muse",
            config::TOOL_ID,
        );
        let fingerprint = MuseCode.source_fingerprint(&source).unwrap();
        assert!(
            fingerprint.starts_with(config::SOURCE_FINGERPRINT_VERSION),
            "unexpected fingerprint: {fingerprint}"
        );
    }
}
