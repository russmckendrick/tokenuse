//! Groups per-round `ParsedCall` rows into user turns and sessions.
//!
//! The reference analyzers (Microsoft AI-Engineering-Coach, MIT, Copyright
//! (c) Microsoft Corporation) evaluate rules over one row per *user turn*.
//! tokens' parsers emit one row per assistant API round, so consecutive
//! calls of a session that share the same prompt state are collapsed into a
//! `Turn` before rules run. Two identical prompts sent back-to-back merge
//! into one turn - an accepted simplification, documented in
//! docs/development/coach.md.

use chrono::{DateTime, Utc};

use crate::tools::{CodeBlock, ParsedCall};

pub struct Turn<'a> {
    pub calls: Vec<&'a ParsedCall>,
    pub tool: &'static str,
    pub project: &'a str,
    pub model: &'a str,
    pub user_message: &'a str,
    pub prompt_chars: Option<u64>,
    /// Timestamp of the turn's first round.
    pub timestamp: Option<DateTime<Utc>>,
    /// Timestamp of the turn's last round.
    pub end_timestamp: Option<DateTime<Utc>>,
    /// Largest per-round latency - the last round's user-to-assistant gap
    /// approximates the full turn latency.
    pub elapsed_ms: Option<u64>,
    pub output_tokens: u64,
    /// Full input context: uncached input + cache reads + cache writes.
    pub full_prompt_tokens: u64,
    pub cache_read_tokens: u64,
    /// Sum of assistant text lengths; `None` when no round carried a signal.
    pub response_chars: Option<u64>,
    pub ai_loc: u64,
    pub code_blocks: Vec<CodeBlock>,
    pub tool_calls: u64,
    pub distinct_tools: Vec<&'a str>,
    pub edited_files: u64,
    pub referenced_files: u64,
    pub is_canceled: bool,
}

pub struct CoachSession<'a> {
    pub key: String,
    pub tool: &'static str,
    pub project: String,
    pub turns: Vec<Turn<'a>>,
}

impl CoachSession<'_> {
    pub fn ai_loc(&self) -> u64 {
        self.turns.iter().map(|t| t.ai_loc).sum()
    }

    pub fn distinct_tool_count(&self) -> usize {
        let mut names: Vec<&str> = self
            .turns
            .iter()
            .flat_map(|t| t.distinct_tools.iter().copied())
            .collect();
        names.sort_unstable();
        names.dedup();
        names.len()
    }
}

/// Group timestamp-sorted calls into sessions (by the shared session key)
/// and each session's calls into turns. Calls without a session id are
/// grouped per (tool, project) so they still participate in request-scoped
/// rules.
pub fn group_sessions<'a>(calls: &[&'a ParsedCall]) -> Vec<CoachSession<'a>> {
    let mut sessions: Vec<CoachSession<'a>> = Vec::new();
    let mut index: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

    for call in calls {
        let key = crate::ingest::session_key(call)
            .unwrap_or_else(|| format!("{}:{}", call.tool, call.project));
        let slot = *index.entry(key.clone()).or_insert_with(|| {
            sessions.push(CoachSession {
                key,
                tool: call.tool,
                project: call.project.clone(),
                turns: Vec::new(),
            });
            sessions.len() - 1
        });
        push_call(&mut sessions[slot], call);
    }

    sessions
}

fn push_call<'a>(session: &mut CoachSession<'a>, call: &'a ParsedCall) {
    let starts_new_turn = match session.turns.last() {
        None => true,
        Some(turn) => {
            turn.user_message != call.user_message || turn.prompt_chars != call.prompt_chars
        }
    };
    if starts_new_turn {
        session.turns.push(Turn {
            calls: Vec::new(),
            tool: call.tool,
            project: &call.project,
            model: &call.model,
            user_message: &call.user_message,
            prompt_chars: call.prompt_chars,
            timestamp: call.timestamp,
            end_timestamp: call.timestamp,
            elapsed_ms: None,
            output_tokens: 0,
            full_prompt_tokens: 0,
            cache_read_tokens: 0,
            response_chars: None,
            ai_loc: 0,
            code_blocks: Vec::new(),
            tool_calls: 0,
            distinct_tools: Vec::new(),
            edited_files: 0,
            referenced_files: 0,
            is_canceled: false,
        });
    }
    let turn = session.turns.last_mut().expect("turn just ensured");
    turn.calls.push(call);
    if turn.timestamp.is_none() {
        turn.timestamp = call.timestamp;
    }
    if call.timestamp.is_some() {
        turn.end_timestamp = call.timestamp;
    }
    if let Some(elapsed) = call.elapsed_ms {
        turn.elapsed_ms = Some(turn.elapsed_ms.unwrap_or(0).max(elapsed));
    }
    turn.output_tokens += call.output_tokens;
    turn.full_prompt_tokens +=
        call.input_tokens + call.cache_read_input_tokens + call.cache_creation_input_tokens;
    turn.cache_read_tokens += call.cache_read_input_tokens;
    if let Some(chars) = call.response_chars {
        *turn.response_chars.get_or_insert(0) += chars;
    }
    turn.ai_loc += call.code_blocks.iter().map(|b| b.loc).sum::<u64>();
    turn.code_blocks.extend(call.code_blocks.iter().cloned());
    turn.tool_calls += call.tools.len() as u64;
    for tool in &call.tools {
        if !turn.distinct_tools.contains(&tool.as_str()) {
            turn.distinct_tools.push(tool.as_str());
        }
    }
    turn.edited_files += call.edited_files.len() as u64;
    turn.referenced_files += call.referenced_files.len() as u64;
    turn.is_canceled |= call.is_canceled;
}

/// User think-time between two consecutive turns: from the end of `cur` to
/// the moment the user sent `next` (approximated by subtracting `next`'s
/// turn latency from its first round timestamp when known).
pub fn think_gap_ms(cur: &Turn<'_>, next: &Turn<'_>) -> Option<i64> {
    let cur_end = cur.end_timestamp?;
    let next_start = next.timestamp?;
    let mut gap = next_start.signed_duration_since(cur_end).num_milliseconds();
    if let Some(latency) = next.elapsed_ms {
        gap -= latency as i64;
    }
    Some(gap)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn call(session: &str, msg: &str, ts_min: u32, prompt_chars: Option<u64>) -> ParsedCall {
        ParsedCall {
            tool: crate::tools::claude_code::config::TOOL_ID,
            model: "claude-opus-4-7".into(),
            output_tokens: 10,
            input_tokens: 100,
            cache_read_input_tokens: 50,
            timestamp: Some(Utc.with_ymd_and_hms(2026, 6, 1, 10, ts_min, 0).unwrap()),
            session_id: session.into(),
            project: "/tmp/p".into(),
            user_message: msg.into(),
            prompt_chars,
            ..ParsedCall::default()
        }
    }

    #[test]
    fn consecutive_rounds_with_same_prompt_form_one_turn() {
        let a = call("s1", "fix the bug", 0, Some(11));
        let b = call("s1", "fix the bug", 1, Some(11));
        let c = call("s1", "now add tests", 2, Some(13));
        let refs: Vec<&ParsedCall> = vec![&a, &b, &c];
        let sessions = group_sessions(&refs);
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].turns.len(), 2);
        let first = &sessions[0].turns[0];
        assert_eq!(first.calls.len(), 2);
        assert_eq!(first.output_tokens, 20);
        assert_eq!(first.full_prompt_tokens, 300);
        assert_eq!(first.cache_read_tokens, 100);
        assert_eq!(
            first.end_timestamp,
            Some(Utc.with_ymd_and_hms(2026, 6, 1, 10, 1, 0).unwrap())
        );
    }

    #[test]
    fn sessions_split_by_key_and_missing_ids_group_by_project() {
        let a = call("s1", "one", 0, Some(3));
        let b = call("s2", "two", 1, Some(3));
        let mut c = call("", "three", 2, Some(5));
        c.project = "/tmp/other".into();
        let refs: Vec<&ParsedCall> = vec![&a, &b, &c];
        let sessions = group_sessions(&refs);
        assert_eq!(sessions.len(), 3);
    }

    #[test]
    fn think_gap_subtracts_next_turn_latency() {
        let a = call("s1", "one", 0, Some(3));
        let mut b = call("s1", "two", 5, Some(3));
        b.elapsed_ms = Some(60_000);
        let refs: Vec<&ParsedCall> = vec![&a, &b];
        let sessions = group_sessions(&refs);
        let turns = &sessions[0].turns;
        // 5 minutes between rounds minus 1 minute of turn latency = 4 minutes.
        assert_eq!(think_gap_ms(&turns[0], &turns[1]), Some(240_000));
    }
}
