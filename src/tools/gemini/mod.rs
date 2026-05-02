use std::collections::HashSet;

use color_eyre::Result;

use super::{ParsedCall, SessionSource, ToolAdapter};

pub mod config;
pub mod discovery;
pub mod parser;

pub struct Gemini;

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

    fn model_display(&self, model: &str) -> String {
        let canonical = canonicalize(model);
        for (key, name) in SHORT_NAMES {
            if canonical == *key {
                return (*name).to_string();
            }
        }
        model.to_string()
    }
}

fn canonicalize(model: &str) -> String {
    let mut s = model.trim().to_lowercase();
    if let Some(idx) = s.find('@') {
        s.truncate(idx);
    }
    if let Some(idx) = s.rfind('/') {
        s = s[idx + 1..].to_string();
    }
    s
}

const SHORT_NAMES: &[(&str, &str)] = &[
    ("gemini-3-flash-preview", "Gemini 3 Flash"),
    ("gemini-3.1-pro-preview", "Gemini 3.1 Pro"),
    ("gemini-2.5-pro", "Gemini 2.5 Pro"),
    ("gemini-2.5-flash", "Gemini 2.5 Flash"),
    ("gemini-2.0-flash", "Gemini 2.0 Flash"),
    ("gemini-1.5-pro", "Gemini 1.5 Pro"),
    ("gemini-1.5-flash", "Gemini 1.5 Flash"),
    ("gemini-auto", "Gemini (auto)"),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::ToolAdapter;

    #[test]
    fn model_display_shortens_common_gemini_models() {
        assert_eq!(Gemini.model_display("gemini-2.5-pro"), "Gemini 2.5 Pro");
        assert_eq!(
            Gemini.model_display("google/gemini-2.5-flash@latest"),
            "Gemini 2.5 Flash"
        );
        assert_eq!(Gemini.model_display("gemini-future"), "gemini-future");
    }
}
