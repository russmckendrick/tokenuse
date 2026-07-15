//! AI code output analysis.
//!
//! Ported from Microsoft's AI-Engineering-Coach `analyzer-production.ts`
//! (MIT, Copyright (c) Microsoft Corporation): fold per-call code blocks
//! into LoC totals by language, day, project, and model, plus a coverage
//! note naming tools that contribute calls but no code-output signal.

use chrono::{Local, NaiveDate};

use super::CoachContext;

pub struct OutputStats {
    pub total_loc: u64,
    /// (language, loc), descending.
    pub by_language: Vec<(String, u64)>,
    /// (local day, loc), ascending by day.
    pub by_day: Vec<(NaiveDate, u64)>,
    /// (project identity, loc), descending.
    pub by_project: Vec<(String, u64)>,
    /// (model, loc), descending.
    pub by_model: Vec<(String, u64)>,
    /// Tools present in the data that never produced a code-output signal.
    pub uncovered_tools: Vec<&'static str>,
}

pub fn output_stats(ctx: &CoachContext) -> OutputStats {
    let mut total = 0u64;
    let mut by_language: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
    let mut by_day: std::collections::BTreeMap<NaiveDate, u64> = std::collections::BTreeMap::new();
    let mut by_project: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
    let mut by_model: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
    let mut tools_with_loc: std::collections::HashSet<&'static str> =
        std::collections::HashSet::new();
    let mut tools_seen: std::collections::HashSet<&'static str> = std::collections::HashSet::new();

    for session in &ctx.sessions {
        for turn in &session.turns {
            tools_seen.insert(turn.tool);
            if turn.ai_loc == 0 {
                continue;
            }
            tools_with_loc.insert(turn.tool);
            total += turn.ai_loc;
            for block in &turn.code_blocks {
                *by_language.entry(block.language.clone()).or_insert(0) += block.loc;
            }
            if let Some(ts) = turn.timestamp {
                *by_day
                    .entry(ts.with_timezone(&Local).date_naive())
                    .or_insert(0) += turn.ai_loc;
            }
            *by_project
                .entry(crate::ingest::projects::project_identity(&session.project))
                .or_insert(0) += turn.ai_loc;
            *by_model.entry(turn.model.to_string()).or_insert(0) += turn.ai_loc;
        }
    }

    let mut uncovered_tools: Vec<&'static str> =
        tools_seen.difference(&tools_with_loc).copied().collect();
    uncovered_tools.sort_unstable();

    OutputStats {
        total_loc: total,
        by_language: sorted_desc(by_language),
        by_day: by_day.into_iter().collect(),
        by_project: sorted_desc(by_project),
        by_model: sorted_desc(by_model),
        uncovered_tools,
    }
}

fn sorted_desc(map: std::collections::HashMap<String, u64>) -> Vec<(String, u64)> {
    let mut rows: Vec<(String, u64)> = map.into_iter().collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coach::testutil::{call, ctx_calls, with_code};

    #[test]
    fn folds_loc_by_language_and_flags_uncovered_tools() {
        let mut calls = vec![
            with_code(call("s1", 0, "write the parser in rust"), "rust", 100),
            with_code(call("s1", 1, "and helper scripts"), "shell", 20),
            with_code(call("s2", 2, "more rust please"), "rust", 30),
        ];
        let mut bare = call("s3", 3, "cursor style row without code signal");
        bare.tool = crate::tools::cursor::config::TOOL_ID;
        calls.push(bare);

        let refs = ctx_calls(&calls);
        let ctx = crate::coach::CoachContext::new(&refs);
        let stats = output_stats(&ctx);
        assert_eq!(stats.total_loc, 150);
        assert_eq!(stats.by_language[0], ("rust".to_string(), 130));
        assert_eq!(stats.by_language[1], ("shell".to_string(), 20));
        assert_eq!(stats.by_day.len(), 1);
        assert_eq!(stats.uncovered_tools, vec!["cursor"]);
    }
}
