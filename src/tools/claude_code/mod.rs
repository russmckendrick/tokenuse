use std::collections::HashSet;

use color_eyre::Result;

use super::{ParsedCall, SessionSource, ToolAdapter};

pub mod config;
pub mod discovery;
pub mod parser;

pub struct ClaudeCode;

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
        parser::parse_session(source, seen)
    }

    fn model_display(&self, model: &str) -> String {
        let canonical = canonicalize(model);
        for (key, name) in SHORT_NAMES {
            if canonical.starts_with(key) {
                return (*name).to_string();
            }
        }
        canonical
    }
}

fn canonicalize(model: &str) -> String {
    let mut s = model.to_string();
    if let Some(idx) = s.find('@') {
        s.truncate(idx);
    }
    let bytes = s.as_bytes();
    if bytes.len() >= 9 {
        let tail = &bytes[bytes.len() - 9..];
        if tail[0] == b'-' && tail[1..].iter().all(|b| b.is_ascii_digit()) {
            s.truncate(s.len() - 9);
        }
    }
    s
}

const SHORT_NAMES: &[(&str, &str)] = &[
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
    ("claude-haiku-4-5", "Haiku 4.5"),
    ("claude-3-5-haiku", "Haiku 3.5"),
];
