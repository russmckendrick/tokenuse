use std::collections::HashSet;

use color_eyre::Result;

use super::{LimitSnapshot, ParsedCall, SessionSource, ToolAdapter};

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

    fn parse_limits(&self, source: &SessionSource) -> Result<Vec<LimitSnapshot>> {
        parser::parse_session_limits(source)
    }

    fn model_display(&self, model: &str) -> String {
        let lower = model.trim().to_lowercase();
        if lower.starts_with("gpt-") {
            return format_gpt_model(&lower);
        }
        for (key, name) in SHORT_NAMES {
            if lower.starts_with(key) {
                return (*name).to_string();
            }
        }
        model.to_string()
    }
}

fn format_gpt_model(model: &str) -> String {
    let mut parts = model.split('-');
    let _ = parts.next();
    let Some(base) = parts.next() else {
        return "GPT".to_string();
    };

    let mut label = format!("GPT-{base}");
    for suffix in parts {
        if suffix.is_empty() {
            continue;
        }
        label.push(' ');
        label.push_str(&title_case_word(suffix));
    }
    label
}

fn title_case_word(word: &str) -> String {
    let mut chars = word.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    let mut out = String::new();
    out.extend(first.to_uppercase());
    out.push_str(chars.as_str());
    out
}

const SHORT_NAMES: &[(&str, &str)] = &[("o3", "o3")];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_display_preserves_full_gpt_model_names() {
        assert_eq!(Codex.model_display("gpt-5.6-sol"), "GPT-5.6 Sol");
        assert_eq!(Codex.model_display("gpt-5.6-terra"), "GPT-5.6 Terra");
        assert_eq!(Codex.model_display("gpt-5.6-luna"), "GPT-5.6 Luna");
        assert_eq!(Codex.model_display("gpt-5.6"), "GPT-5.6");
        assert_eq!(Codex.model_display("gpt-5.3-codex"), "GPT-5.3 Codex");
        assert_eq!(
            Codex.model_display("gpt-5.3-codex-spark"),
            "GPT-5.3 Codex Spark"
        );
        assert_eq!(Codex.model_display("gpt-4o-mini"), "GPT-4o Mini");
        assert_eq!(Codex.model_display("gpt-5"), "GPT-5");
    }
}
