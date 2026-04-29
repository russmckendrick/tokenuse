use std::collections::HashSet;

use color_eyre::Result;

use super::{ParsedCall, SessionSource, ToolAdapter};

pub mod config;
pub mod discovery;
pub mod parser;

pub struct Codex;

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

    fn model_display(&self, model: &str) -> String {
        let lower = model.trim().to_lowercase();
        for (key, name) in SHORT_NAMES {
            if lower.starts_with(key) {
                return (*name).to_string();
            }
        }
        model.to_string()
    }
}

const SHORT_NAMES: &[(&str, &str)] = &[
    ("gpt-5.4", "GPT-5.4"),
    ("gpt-5-mini", "GPT-5 Mini"),
    ("gpt-5", "GPT-5"),
    ("gpt-4o-mini", "GPT-4o Mini"),
    ("gpt-4o", "GPT-4o"),
    ("o3", "o3"),
];
