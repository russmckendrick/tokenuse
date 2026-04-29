use std::collections::HashSet;

use color_eyre::Result;

use crate::providers::{ParsedCall, SessionSource};

pub fn parse_session(
    _source: &SessionSource,
    _seen: &mut HashSet<String>,
) -> Result<Vec<ParsedCall>> {
    // Scaffold. See docs/providers/codex.md for the entry-type schema
    // (session_meta, turn_context, response_item, event_msg/token_count) and
    // the cumulative-vs-last token-usage diffing rule.
    Ok(Vec::new())
}
