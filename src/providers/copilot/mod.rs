use std::collections::HashSet;

use color_eyre::Result;

use super::{ParsedCall, Provider, SessionSource};

pub mod config;
pub mod discovery;
pub mod parser;

pub struct Copilot;

impl Provider for Copilot {
    fn id(&self) -> &'static str {
        config::PROVIDER_ID
    }

    fn display_name(&self) -> &'static str {
        config::DISPLAY_NAME
    }

    fn discover(&self) -> Result<Vec<SessionSource>> {
        discovery::discover()
    }

    fn parse(
        &self,
        source: &SessionSource,
        seen: &mut HashSet<String>,
    ) -> Result<Vec<ParsedCall>> {
        parser::parse_session(source, seen)
    }
}
