use std::collections::HashSet;

use color_eyre::Result;

use super::{LimitSnapshot, ParsedCall, SessionSource, SessionSourceKind, ToolAdapter};

pub mod config;
pub mod limits;

pub struct CodexSubscription;

impl ToolAdapter for CodexSubscription {
    fn id(&self) -> &'static str {
        config::TOOL_ID
    }

    fn display_name(&self) -> &'static str {
        config::DISPLAY_NAME
    }

    fn discover(&self) -> Result<Vec<SessionSource>> {
        let Some(sidecar) = config::limit_sidecar() else {
            return Ok(Vec::new());
        };
        if !sidecar.is_file() {
            return Ok(Vec::new());
        }
        Ok(vec![SessionSource::limit(
            sidecar,
            "codex-subscription-limits",
            config::TOOL_ID,
        )])
    }

    fn parse(
        &self,
        _source: &SessionSource,
        _seen: &mut HashSet<String>,
    ) -> Result<Vec<ParsedCall>> {
        Ok(Vec::new())
    }

    fn parse_limits(&self, source: &SessionSource) -> Result<Vec<LimitSnapshot>> {
        if source.kind == SessionSourceKind::Limit {
            return limits::parse_sidecar(source);
        }
        Ok(Vec::new())
    }
}
