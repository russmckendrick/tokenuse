use std::collections::HashSet;

use color_eyre::Result;

use crate::providers::{ParsedCall, SessionSource};

pub fn parse_session(
    _source: &SessionSource,
    _seen: &mut HashSet<String>,
) -> Result<Vec<ParsedCall>> {
    // Scaffold. See docs/providers/copilot.md for the dual-source schema
    // (legacy events.jsonl and VS Code transcripts) and the tool-call-ID
    // model inference rules used when token counts are absent.
    Ok(Vec::new())
}
