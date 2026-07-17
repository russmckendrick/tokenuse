use std::collections::hash_map::DefaultHasher;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::time::UNIX_EPOCH;

use color_eyre::Result;

use super::{ParsedCall, SessionSource, ToolAdapter};

pub mod config;
pub mod discovery;
pub mod parser;

pub struct Cursor;

/// v4: composer context-meter input credits, workspace project mapping.
const SOURCE_FINGERPRINT_VERSION: &str = "cursor-v4-context-meter";

impl ToolAdapter for Cursor {
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
        let mut hasher = DefaultHasher::new();
        SOURCE_FINGERPRINT_VERSION.hash(&mut hasher);
        source.path.hash(&mut hasher);
        for path in discovery::aggregate_files() {
            path.hash(&mut hasher);
            if let Ok(metadata) = path.metadata() {
                metadata.len().hash(&mut hasher);
                metadata
                    .modified()
                    .ok()
                    .and_then(|mtime| mtime.duration_since(UNIX_EPOCH).ok())
                    .map(|duration| duration.as_nanos())
                    .hash(&mut hasher);
            }
        }
        Ok(format!(
            "{SOURCE_FINGERPRINT_VERSION}:{:016x}",
            hasher.finish()
        ))
    }

    fn probe_roots(&self) -> Vec<super::ProbeRoot> {
        let mut roots = Vec::new();
        if let Some(state) = config::state_db_path() {
            roots.push(super::ProbeRoot::new("state database", state));
        }
        if let Some(agent) = config::agent_home() {
            roots.push(super::ProbeRoot::new("agent home", agent));
        }
        if let Some(tracking) = config::agent_tracking_db_path() {
            roots.push(super::ProbeRoot::new("ai tracking database", tracking));
        }
        if let Some(chats) = config::chats_dir() {
            roots.push(super::ProbeRoot::new("chat stores", chats));
        }
        if let Some(workspaces) = config::workspace_storage_root() {
            roots.push(super::ProbeRoot::new("workspace storage", workspaces));
        }
        roots
    }

    fn env_overrides(&self) -> &'static [&'static str] {
        &[config::AGENT_HOME_ENV]
    }
}
