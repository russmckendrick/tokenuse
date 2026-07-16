//! Per-tool capability tables.
//!
//! Enrichment fields degrade to defaults where a source lacks the signal, so
//! rules that build denominators from a signal must only count tools that can
//! actually produce it - otherwise "no data" reads as "clean behaviour"
//! (or worse, as a finding). The matrices live in
//! docs/development/tools/<name>.md.

fn is_claude(tool: &str) -> bool {
    tool == crate::tools::claude_code::config::TOOL_ID
}

fn is_codex(tool: &str) -> bool {
    tool == crate::tools::codex::config::TOOL_ID
}

fn is_cursor(tool: &str) -> bool {
    tool == crate::tools::cursor::config::TOOL_ID
}

/// `is_canceled` is a plain bool: false can also mean "source has no
/// cancellation signal".
pub fn supports_cancel(tool: &str) -> bool {
    is_claude(tool) || is_codex(tool)
}

/// Only these parsers populate `referenced_files`/`edited_files`.
pub fn supports_file_refs(tool: &str) -> bool {
    is_claude(tool) || is_codex(tool) || is_cursor(tool)
}

/// Tools whose cache-read tokens are genuinely reported; elsewhere a zero
/// cache column would read as "starved cache" when it just means "no data".
pub fn supports_cache_read(tool: &str) -> bool {
    is_claude(tool) || is_codex(tool)
}

/// Tools whose session ids map to real conversations. Session-shape rules
/// (abandoned/mega/drift) misfire on tools that archive aggregate rows.
pub fn has_conversational_sessions(tool: &str) -> bool {
    is_claude(tool) || is_codex(tool) || is_cursor(tool)
}

#[cfg(test)]
mod tests {
    #[test]
    fn capabilities_are_scoped_to_enriched_tools() {
        assert!(super::supports_file_refs("cursor"));
        assert!(super::has_conversational_sessions("cursor"));
        assert!(!super::supports_cancel("cursor"));
        assert!(!super::supports_cache_read("cursor"));
        assert!(!super::supports_file_refs("copilot"));
        assert!(!super::has_conversational_sessions("gemini"));
    }
}
