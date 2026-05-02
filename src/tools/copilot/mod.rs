use std::collections::HashSet;

use color_eyre::Result;

use super::{ParsedCall, SessionSource, ToolAdapter};

pub mod config;
pub mod discovery;
pub mod parser;

pub struct Copilot;

impl ToolAdapter for Copilot {
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
        match lower.as_str() {
            "copilot-auto" => return "Copilot (auto)".into(),
            "openai-auto" | "copilot-openai-auto" => {
                return "Copilot (OpenAI auto)".into();
            }
            "anthropic-auto" | "copilot-anthropic-auto" => {
                return "Copilot (Anthropic auto)".into();
            }
            _ => {}
        }
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
    ("gpt-5.3-codex", "GPT-5.3 Codex"),
    ("gpt-5-mini", "GPT-5 Mini"),
    ("gpt-5", "GPT-5"),
    ("gpt-4.1-nano", "GPT-4.1 Nano"),
    ("gpt-4.1-mini", "GPT-4.1 Mini"),
    ("gpt-4.1", "GPT-4.1"),
    ("gpt-4o-mini", "GPT-4o Mini"),
    ("gpt-4o", "GPT-4o"),
    ("claude-opus-4-7", "Opus 4.7"),
    ("claude-opus-4-6", "Opus 4.6"),
    ("claude-opus-4-5", "Opus 4.5"),
    ("claude-opus-4-1", "Opus 4.1"),
    ("claude-opus-4", "Opus 4"),
    ("claude-sonnet-4-6", "Sonnet 4.6"),
    ("claude-sonnet-4-5", "Sonnet 4.5"),
    ("claude-sonnet-4", "Sonnet 4"),
    ("claude-3-7-sonnet", "Sonnet 3.7"),
    ("claude-3-5-sonnet", "Sonnet 3.5"),
    ("o4-mini", "o4-mini"),
    ("o3", "o3"),
];
