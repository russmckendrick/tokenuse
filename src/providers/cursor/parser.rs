use std::collections::HashSet;

use color_eyre::Result;

use crate::providers::{ParsedCall, SessionSource};

pub fn parse_session(
    _source: &SessionSource,
    _seen: &mut HashSet<String>,
) -> Result<Vec<ParsedCall>> {
    // Scaffold. See docs/providers/cursor.md for the full SQLite parsing plan
    // (bubbleId:* and agentKv:blob:* paths in cursorDiskKV) and the field
    // mappings into ParsedCall.
    Ok(Vec::new())
}
