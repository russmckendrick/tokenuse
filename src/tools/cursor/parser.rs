use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use chrono::{DateTime, Utc};
use color_eyre::Result;
use rusqlite::{Connection, OpenFlags};
use serde_json::Value;

use crate::pricing;
use crate::tools::{ParsedCall, SessionSource, Speed};

use super::{config, discovery};

const CHARS_PER_TOKEN: f64 = config::CHARS_PER_TOKEN;
const COST_MODEL_FALLBACK: &str = "cursor-auto";

const BUBBLE_QUERY: &str = "
SELECT
  json_extract(value, '$.tokenCount.inputTokens')  AS input_tokens,
  json_extract(value, '$.tokenCount.outputTokens') AS output_tokens,
  json_extract(value, '$.modelInfo.modelName')     AS model,
  json_extract(value, '$.createdAt')               AS created_at,
  json_extract(value, '$.conversationId')          AS conversation_id,
  substr(json_extract(value, '$.text'), 1, 500)    AS user_text,
  length(json_extract(value, '$.text'))            AS text_length,
  json_extract(value, '$.type')                    AS bubble_type,
  json_extract(value, '$.codeBlocks')              AS code_blocks
FROM cursorDiskKV
WHERE key LIKE 'bubbleId:%'
ORDER BY ROWID ASC
";

pub fn parse_session(
    source: &SessionSource,
    seen: &mut HashSet<String>,
) -> Result<Vec<ParsedCall>> {
    if !is_state_db_source(&source.path) {
        let tracking_db = config::agent_tracking_db_path();
        return parse_transcript_file(source, seen, tracking_db.as_deref());
    }

    parse_state_db_session(source, seen)
}

fn parse_state_db_session(
    source: &SessionSource,
    seen: &mut HashSet<String>,
) -> Result<Vec<ParsedCall>> {
    let uri = format!("file:{}?immutable=1", source.path.display());
    let conn = match Connection::open_with_flags(
        &uri,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    ) {
        Ok(c) => c,
        Err(_) => return Ok(Vec::new()),
    };
    let transcript_projects = transcript_project_lookup();
    parse_with_conn(&conn, source, seen, &transcript_projects)
}

fn is_state_db_source(path: &Path) -> bool {
    path.file_name().and_then(|n| n.to_str()) == Some(config::STATE_DB)
}

fn parse_with_conn(
    conn: &Connection,
    source: &SessionSource,
    seen: &mut HashSet<String>,
    transcript_projects: &HashMap<String, String>,
) -> Result<Vec<ParsedCall>> {
    if !validate_schema(conn) {
        return Ok(Vec::new());
    }

    let fallback_project =
        single_agent_workspace_path(conn).unwrap_or_else(|| source.project.clone());

    let mut calls = parse_bubbles(conn, &fallback_project, transcript_projects, seen)?;
    calls.extend(parse_agent_kv(conn, &fallback_project, seen)?);
    Ok(calls)
}

#[derive(Debug, Default)]
struct TranscriptTurn {
    user_message: String,
    input_chars: usize,
    output_chars: usize,
    tools: Vec<String>,
    bash_commands: Vec<String>,
    project: Option<String>,
}

#[derive(Debug, Default)]
struct TranscriptMetadata {
    model: Option<String>,
    timestamp: Option<DateTime<Utc>>,
    project: Option<String>,
}

fn parse_transcript_file(
    source: &SessionSource,
    seen: &mut HashSet<String>,
    tracking_db: Option<&Path>,
) -> Result<Vec<ParsedCall>> {
    let raw = match fs::read_to_string(&source.path) {
        Ok(raw) => raw,
        Err(_) => return Ok(Vec::new()),
    };

    let source_project_id = transcript_project_id(&source.path);
    let turns = match source.path.extension().and_then(|ext| ext.to_str()) {
        Some("jsonl") => parse_jsonl_transcript_with_project_id(&raw, source_project_id.as_deref()),
        Some("txt") => parse_legacy_txt_transcript(&raw),
        _ => Vec::new(),
    };
    if turns.is_empty() {
        return Ok(Vec::new());
    }

    let conversation_id = transcript_conversation_id(&source.path);
    let metadata = tracking_db
        .map(|db| transcript_metadata(db, &conversation_id, source_project_id.as_deref()))
        .unwrap_or_default();
    let timestamp = metadata
        .timestamp
        .or_else(|| file_modified_timestamp(&source.path));
    let display_model = display_model_for(metadata.model.as_deref());
    let metadata_project = metadata.project.clone();

    let mut calls = Vec::new();
    for (idx, turn) in turns.into_iter().enumerate() {
        if turn.input_chars == 0 && turn.output_chars == 0 && turn.tools.is_empty() {
            continue;
        }

        let input_tokens = estimate_tokens(turn.input_chars);
        let output_tokens = estimate_tokens(turn.output_chars);
        let dedup_key = format!(
            "cursor:transcript:{}:{}:{}",
            source.path.display(),
            conversation_id,
            idx
        );
        if !seen.insert(dedup_key.clone()) {
            continue;
        }

        let mut call = ParsedCall {
            tool: config::TOOL_ID,
            model: display_model.clone(),
            input_tokens,
            output_tokens,
            speed: Speed::Standard,
            timestamp,
            dedup_key,
            user_message: turn.user_message,
            session_id: conversation_id.clone(),
            project: choose_project(
                turn.project,
                metadata_project.clone(),
                source.project.as_str(),
            ),
            tools: turn.tools,
            bash_commands: turn.bash_commands,
            ..ParsedCall::default()
        };
        call.cost_usd = pricing::cost(&display_model, &call, Speed::Standard);
        calls.push(call);
    }

    Ok(calls)
}

fn parse_jsonl_transcript_with_project_id(
    raw: &str,
    source_project_id: Option<&str>,
) -> Vec<TranscriptTurn> {
    let mut turns = Vec::new();
    let mut current: Option<TranscriptTurn> = None;

    for line in raw.lines().filter(|l| !l.trim().is_empty()) {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let Some(role) = value.get("role").and_then(|v| v.as_str()) else {
            continue;
        };

        match role {
            "user" => {
                flush_transcript_turn(&mut current, &mut turns);
                let text = message_text(&value);
                let user_message = extract_user_query(&text);
                current = Some(TranscriptTurn {
                    input_chars: user_message.len(),
                    project: extract_workspace_path(&text),
                    user_message,
                    ..TranscriptTurn::default()
                });
            }
            "assistant" => {
                let Some(turn) = current.as_mut() else {
                    continue;
                };
                analyze_jsonl_assistant(&value, turn, source_project_id);
            }
            "tool" | "system" => {
                if let Some(turn) = current.as_mut() {
                    let text = message_text(&value);
                    turn.input_chars = turn.input_chars.saturating_add(text.len());
                    if turn.project.is_none() {
                        turn.project = extract_workspace_path(&text);
                    }
                }
            }
            _ => {}
        }
    }

    flush_transcript_turn(&mut current, &mut turns);
    turns
}

fn parse_legacy_txt_transcript(raw: &str) -> Vec<TranscriptTurn> {
    let mut turns = Vec::new();
    let mut pending_users = Vec::new();
    let mut active = TranscriptBlock::None;
    let mut user_lines: Vec<String> = Vec::new();
    let mut assistant_lines: Vec<String> = Vec::new();

    for line in raw.lines() {
        if let Some(user) = strip_marker_case_insensitive(line, "user:") {
            flush_legacy_user(&mut user_lines, &mut pending_users);
            flush_legacy_assistant(&mut assistant_lines, &mut pending_users, &mut turns);
            active = TranscriptBlock::User;
            user_lines.push(user.to_string());
            continue;
        }

        if let Some(assistant) = strip_assistant_marker(line) {
            flush_legacy_user(&mut user_lines, &mut pending_users);
            flush_legacy_assistant(&mut assistant_lines, &mut pending_users, &mut turns);
            active = TranscriptBlock::Assistant;
            assistant_lines.push(assistant.to_string());
            continue;
        }

        match active {
            TranscriptBlock::User => user_lines.push(line.to_string()),
            TranscriptBlock::Assistant => assistant_lines.push(line.to_string()),
            TranscriptBlock::None => {}
        }
    }

    flush_legacy_user(&mut user_lines, &mut pending_users);
    flush_legacy_assistant(&mut assistant_lines, &mut pending_users, &mut turns);
    turns
}

#[derive(Clone, Copy)]
enum TranscriptBlock {
    None,
    User,
    Assistant,
}

fn flush_legacy_user(user_lines: &mut Vec<String>, pending_users: &mut Vec<String>) {
    if user_lines.is_empty() {
        return;
    }
    let raw = user_lines.join("\n");
    let user_message = extract_user_query(&raw);
    if !user_message.is_empty() {
        pending_users.push(user_message);
    }
    user_lines.clear();
}

fn flush_legacy_assistant(
    assistant_lines: &mut Vec<String>,
    pending_users: &mut Vec<String>,
    turns: &mut Vec<TranscriptTurn>,
) {
    if assistant_lines.is_empty() || pending_users.is_empty() {
        assistant_lines.clear();
        return;
    }

    let user_message = pending_users.remove(0);
    let mut turn = TranscriptTurn {
        input_chars: user_message.len(),
        user_message,
        ..TranscriptTurn::default()
    };

    for line in assistant_lines.drain(..) {
        let trimmed = line.trim_start();
        if trimmed.to_ascii_lowercase().starts_with("[tool result]") {
            continue;
        }
        if let Some(body) = strip_marker_case_insensitive(trimmed, "[thinking]") {
            turn.output_chars = turn.output_chars.saturating_add(body.trim().len());
            continue;
        }
        if let Some(tool) = strip_marker_case_insensitive(trimmed, "[tool call]") {
            add_tool(&mut turn.tools, &normalize_tool_name(tool.trim()));
            continue;
        }
        turn.output_chars = turn.output_chars.saturating_add(line.len());
    }

    flush_completed_turn(turn, turns);
}

fn strip_marker_case_insensitive<'a>(line: &'a str, marker: &str) -> Option<&'a str> {
    let trimmed = line.trim_start();
    if trimmed
        .to_ascii_lowercase()
        .starts_with(&marker.to_ascii_lowercase())
    {
        return Some(&trimmed[marker.len()..]);
    }
    None
}

fn strip_assistant_marker(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    trimmed.strip_prefix("A:")
}

fn analyze_jsonl_assistant(
    value: &Value,
    turn: &mut TranscriptTurn,
    source_project_id: Option<&str>,
) {
    let Some(content) = value.pointer("/message/content") else {
        turn.output_chars = turn.output_chars.saturating_add(message_text(value).len());
        return;
    };

    match content {
        Value::Array(blocks) => {
            for block in blocks {
                let block_type = block.get("type").and_then(|v| v.as_str());
                match block_type {
                    Some("text") => {
                        if let Some(text) = block.get("text").and_then(|v| v.as_str()) {
                            turn.output_chars = turn.output_chars.saturating_add(text.len());
                            if turn.project.is_none() {
                                turn.project = extract_workspace_path(text);
                            }
                        }
                    }
                    Some("tool_use") => {
                        let name = block.get("name").and_then(|v| v.as_str()).unwrap_or("");
                        let normalized = normalize_tool_name(name);
                        add_tool(&mut turn.tools, &normalized);
                        if let Some(input) = block.get("input") {
                            if let Ok(raw_input) = serde_json::to_string(input) {
                                turn.output_chars =
                                    turn.output_chars.saturating_add(raw_input.len());
                            }
                            if normalized == "Bash" {
                                if let Some(command) = input.get("command").and_then(|v| v.as_str())
                                {
                                    turn.bash_commands.push(command.to_string());
                                }
                            }
                            if turn.project.is_none() {
                                turn.project = project_from_tool_input(input, source_project_id);
                            }
                        }
                        turn.output_chars = turn.output_chars.saturating_add(name.len());
                    }
                    _ => {
                        if let Ok(serialized) = serde_json::to_string(block) {
                            turn.output_chars = turn.output_chars.saturating_add(serialized.len());
                        }
                    }
                }
            }
        }
        Value::String(text) => {
            turn.output_chars = turn.output_chars.saturating_add(text.len());
        }
        _ => {
            if let Ok(serialized) = serde_json::to_string(content) {
                turn.output_chars = turn.output_chars.saturating_add(serialized.len());
            }
        }
    }
}

fn message_text(value: &Value) -> String {
    let content = value
        .pointer("/message/content")
        .or_else(|| value.get("content"));
    let Some(content) = content else {
        return String::new();
    };
    content_text(content)
}

fn content_text(content: &Value) -> String {
    match content {
        Value::String(text) => text.clone(),
        Value::Array(blocks) => blocks
            .iter()
            .filter_map(|block| block.get("text").and_then(|v| v.as_str()))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

fn flush_transcript_turn(current: &mut Option<TranscriptTurn>, turns: &mut Vec<TranscriptTurn>) {
    let Some(turn) = current.take() else {
        return;
    };
    flush_completed_turn(turn, turns);
}

fn flush_completed_turn(turn: TranscriptTurn, turns: &mut Vec<TranscriptTurn>) {
    if turn.user_message.is_empty() {
        return;
    }
    if turn.output_chars == 0 && turn.tools.is_empty() {
        return;
    }
    turns.push(turn);
}

fn add_tool(tools: &mut Vec<String>, tool: &str) {
    if !tool.is_empty() && !tools.iter().any(|existing| existing == tool) {
        tools.push(tool.to_string());
    }
}

fn normalize_tool_name(raw: &str) -> String {
    let clean = raw.trim();
    let lower = clean.to_ascii_lowercase().replace(['_', ' '], "-");
    match lower.as_str() {
        "bash" | "shell" => "Bash".into(),
        "read" => "Read".into(),
        "grep" => "Grep".into(),
        "glob" => "Glob".into(),
        "write" => "Write".into(),
        "strreplace" | "str-replace" => "StrReplace".into(),
        "createplan" | "create-plan" => "CreatePlan".into(),
        "readlints" | "read-lints" => "ReadLints".into(),
        _ if clean.is_empty() => "unknown".into(),
        _ => clean.to_string(),
    }
}

fn project_from_tool_input(input: &Value, source_project_id: Option<&str>) -> Option<String> {
    match input {
        Value::Object(map) => {
            for key in ["target_directory", "working_directory", "cwd"] {
                if let Some(path) = map.get(key).and_then(|v| v.as_str()) {
                    if let Some(project) = clean_absolute_project_path(path) {
                        return Some(project);
                    }
                }
            }
            if let Some(path) = map.get("path").and_then(|v| v.as_str()) {
                if let Some(project) =
                    project_from_path_hint_with_project_id(path, source_project_id)
                {
                    return Some(project);
                }
            }
            if let Some(command) = map.get("command").and_then(|v| v.as_str()) {
                if let Some(project) = project_from_command(command, source_project_id) {
                    return Some(project);
                }
            }
            for value in map.values() {
                if let Some(project) = project_from_tool_input(value, source_project_id) {
                    return Some(project);
                }
            }
            None
        }
        Value::Array(items) => items
            .iter()
            .find_map(|item| project_from_tool_input(item, source_project_id)),
        _ => None,
    }
}

fn project_from_command(command: &str, source_project_id: Option<&str>) -> Option<String> {
    let tokens = command
        .split(|c: char| c.is_whitespace() || matches!(c, ';' | '&' | '|' | '(' | ')'))
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();

    for window in tokens.windows(2) {
        if window[0] == "cd" {
            if let Some(project) = clean_absolute_project_path(window[1]) {
                return Some(project);
            }
        }
    }

    tokens
        .iter()
        .find_map(|token| project_from_path_hint_with_project_id(token, source_project_id))
}

fn clean_absolute_project_path(raw: &str) -> Option<String> {
    let value = clean_path_token(raw);
    if !looks_absolute_path(&value) || value.contains("/.cursor/") {
        return None;
    }
    Some(value)
}

fn project_from_path_hint(raw: &str) -> Option<String> {
    project_from_path_hint_with_project_id(raw, None)
}

fn project_from_path_hint_with_project_id(raw: &str, project_id: Option<&str>) -> Option<String> {
    let value = clean_path_token(raw);
    if !looks_absolute_path(&value) || value.contains("/.cursor/") {
        return None;
    }
    if let Some(project_id) = project_id {
        if let Some(project) = project_root_matching_folder_id(&value, project_id) {
            return Some(project);
        }
    }
    if let Some(project) = project_root_after_anchor(&value, "/Code/") {
        return Some(project);
    }
    if let Some(project) = project_root_after_anchor(&value, "/Desktop/") {
        return Some(project);
    }
    Some(value)
}

fn project_root_matching_folder_id(path: &str, project_id: &str) -> Option<String> {
    let (base, encoded_rest) = folder_id_base_and_rest(project_id)?;
    let anchor = format!("/{base}/");
    let rest_start = path.find(&anchor)? + anchor.len();
    let prefix = &path[..rest_start];
    let parts = path[rest_start..]
        .split('/')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();

    for end in 1..=parts.len() {
        let encoded = parts[..end]
            .iter()
            .map(|part| encode_cursor_path_component(part))
            .collect::<Vec<_>>()
            .join("-");
        if encoded == encoded_rest {
            return Some(format!("{prefix}{}", parts[..end].join("/")));
        }
    }
    None
}

fn folder_id_base_and_rest(project_id: &str) -> Option<(&'static str, &str)> {
    for (base, anchor) in [
        ("Code", "-Code-"),
        ("Desktop", "-Desktop-"),
        ("Documents", "-Documents-"),
    ] {
        if let Some(idx) = project_id.find(anchor) {
            let rest = &project_id[idx + anchor.len()..];
            if !rest.is_empty() {
                return Some((base, rest));
            }
        }
    }
    None
}

fn encode_cursor_path_component(raw: &str) -> String {
    raw.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

fn choose_project(
    primary: Option<String>,
    secondary: Option<String>,
    fallback_project: &str,
) -> String {
    match (primary, secondary) {
        (Some(primary), Some(secondary)) if is_more_specific_project(&primary, &secondary) => {
            secondary
        }
        (Some(primary), _) => primary,
        (None, Some(secondary)) => secondary,
        (None, None) => fallback_project.to_string(),
    }
}

fn is_more_specific_project(parent: &str, child: &str) -> bool {
    let parent = parent.trim_end_matches('/');
    child
        .trim_end_matches('/')
        .strip_prefix(parent)
        .is_some_and(|rest| rest.starts_with('/'))
}

fn clean_path_token(raw: &str) -> String {
    raw.trim_matches(|c: char| {
        c == '"'
            || c == '\''
            || c == '`'
            || c == ','
            || c == ';'
            || c == ')'
            || c == '('
            || c == '['
            || c == ']'
            || c == '{'
            || c == '}'
    })
    .to_string()
}

fn looks_absolute_path(value: &str) -> bool {
    value.starts_with('/') || value.starts_with('~') || value.contains(":\\")
}

fn project_root_after_anchor(path: &str, anchor: &str) -> Option<String> {
    let idx = path.find(anchor)?;
    let rest_start = idx + anchor.len();
    let rest = &path[rest_start..];
    let first = rest.split('/').next().filter(|s| !s.is_empty())?;
    Some(path[..rest_start + first.len()].to_string())
}

fn estimate_tokens(chars: usize) -> u64 {
    if chars == 0 {
        0
    } else {
        ((chars as f64) / CHARS_PER_TOKEN).ceil() as u64
    }
}

fn transcript_conversation_id(path: &Path) -> String {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .unwrap_or("unknown")
        .to_string()
}

fn transcript_metadata(
    db_path: &Path,
    conversation_id: &str,
    source_project_id: Option<&str>,
) -> TranscriptMetadata {
    let uri = format!("file:{}?immutable=1", db_path.display());
    let Ok(conn) = Connection::open_with_flags(
        &uri,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    ) else {
        return TranscriptMetadata::default();
    };

    query_conversation_summary(&conn, conversation_id)
        .or_else(|| query_ai_code_hashes(&conn, conversation_id, source_project_id))
        .unwrap_or_default()
}

fn query_conversation_summary(
    conn: &Connection,
    conversation_id: &str,
) -> Option<TranscriptMetadata> {
    let mut stmt = conn
        .prepare(
            "SELECT model, updatedAt
             FROM conversation_summaries
             WHERE conversationId = ?1
             LIMIT 1",
        )
        .ok()?;
    stmt.query_row([conversation_id], |row| {
        let model = row.get::<_, Option<String>>(0)?;
        let updated_at = column_as_string(row, 1)?;
        Ok(TranscriptMetadata {
            model,
            timestamp: updated_at.as_deref().and_then(parse_timestamp),
            project: None,
        })
    })
    .ok()
}

fn query_ai_code_hashes(
    conn: &Connection,
    conversation_id: &str,
    source_project_id: Option<&str>,
) -> Option<TranscriptMetadata> {
    let mut stmt = conn
        .prepare(
            "SELECT model, COALESCE(timestamp, createdAt) AS observed_at, fileName
             FROM ai_code_hashes
             WHERE conversationId = ?1
             ORDER BY CASE
                    WHEN fileName LIKE '/%' OR fileName LIKE '~/%' OR fileName LIKE '%:\\%' THEN 0
                    ELSE 1
                 END,
                 observed_at DESC
             LIMIT 1",
        )
        .ok()?;
    stmt.query_row([conversation_id], |row| {
        let model = row.get::<_, Option<String>>(0)?;
        let observed_at = column_as_string(row, 1)?;
        let file_name = row.get::<_, Option<String>>(2)?;
        Ok(TranscriptMetadata {
            model,
            timestamp: observed_at.as_deref().and_then(parse_timestamp),
            project: file_name.as_deref().and_then(|file_name| {
                project_from_path_hint_with_project_id(file_name, source_project_id)
            }),
        })
    })
    .ok()
}

fn transcript_project_id(path: &Path) -> Option<String> {
    let mut dir = path.parent();
    while let Some(current) = dir {
        if current.file_name().and_then(|name| name.to_str()) == Some(config::AGENT_TRANSCRIPTS_DIR)
        {
            return current
                .parent()
                .and_then(|project| project.file_name())
                .and_then(|name| name.to_str())
                .map(ToString::to_string);
        }
        dir = current.parent();
    }
    None
}

fn file_modified_timestamp(path: &Path) -> Option<DateTime<Utc>> {
    fs::metadata(path)
        .ok()
        .and_then(|metadata| metadata.modified().ok())
        .map(DateTime::<Utc>::from)
}

fn validate_schema(conn: &Connection) -> bool {
    conn.query_row::<i64, _, _>(
        "SELECT COUNT(*) FROM cursorDiskKV WHERE key LIKE 'bubbleId:%' LIMIT 1",
        [],
        |r| r.get(0),
    )
    .is_ok()
}

#[derive(Debug)]
struct BubbleRow {
    input_tokens: u64,
    output_tokens: u64,
    model: Option<String>,
    created_at: Option<String>,
    conversation_id: Option<String>,
    user_text: Option<String>,
    text_length: u64,
    bubble_type: i64,
}

fn parse_bubbles(
    conn: &Connection,
    fallback_project: &str,
    transcript_projects: &HashMap<String, String>,
    seen: &mut HashSet<String>,
) -> Result<Vec<ParsedCall>> {
    let mut stmt = match conn.prepare(BUBBLE_QUERY) {
        Ok(s) => s,
        Err(_) => return Ok(Vec::new()),
    };

    let rows = stmt.query_map([], |r| {
        Ok(BubbleRow {
            input_tokens: r.get::<_, Option<i64>>(0)?.unwrap_or(0).max(0) as u64,
            output_tokens: r.get::<_, Option<i64>>(1)?.unwrap_or(0).max(0) as u64,
            model: r.get::<_, Option<String>>(2)?,
            created_at: column_as_string(r, 3)?,
            conversation_id: r.get::<_, Option<String>>(4)?,
            user_text: r.get::<_, Option<String>>(5)?,
            text_length: r.get::<_, Option<i64>>(6)?.unwrap_or(0).max(0) as u64,
            bubble_type: r.get::<_, Option<i64>>(7)?.unwrap_or(-1),
        })
    });

    let rows = match rows {
        Ok(rows) => rows,
        Err(_) => return Ok(Vec::new()),
    };

    let mut calls = Vec::new();
    for row in rows.flatten() {
        let mut input_tokens = row.input_tokens;
        let mut output_tokens = row.output_tokens;

        if input_tokens == 0 && output_tokens == 0 {
            if row.text_length == 0 {
                continue;
            }
            let estimate = ((row.text_length as f64) / CHARS_PER_TOKEN).ceil() as u64;
            if row.bubble_type == 1 {
                input_tokens = estimate;
            } else {
                output_tokens = estimate;
            }
        }

        let conversation_id = row
            .conversation_id
            .clone()
            .unwrap_or_else(|| "unknown".into());
        let created_at = row.created_at.clone().unwrap_or_default();
        let dedup_key = format!(
            "cursor:{}:{}:{}:{}",
            conversation_id, created_at, input_tokens, output_tokens
        );
        if !seen.insert(dedup_key.clone()) {
            continue;
        }

        let display_model = display_model_for(row.model.as_deref());
        let timestamp = parse_timestamp(&created_at);
        let project = row
            .user_text
            .as_deref()
            .and_then(extract_workspace_path)
            .or_else(|| transcript_projects.get(&conversation_id).cloned())
            .unwrap_or_else(|| fallback_project.to_string());
        let user_message = row.user_text.unwrap_or_default();

        let mut call = ParsedCall {
            tool: config::TOOL_ID,
            model: display_model.clone(),
            input_tokens,
            output_tokens,
            speed: Speed::Standard,
            timestamp,
            dedup_key,
            user_message,
            session_id: conversation_id,
            project,
            ..ParsedCall::default()
        };
        call.cost_usd = pricing::cost(&display_model, &call, Speed::Standard);
        calls.push(call);
    }

    Ok(calls)
}

fn transcript_project_lookup() -> HashMap<String, String> {
    let tracking_files = config::agent_tracking_db_path()
        .as_deref()
        .map(tracking_db_file_lookup)
        .unwrap_or_default();
    let mut lookup = tracking_files
        .iter()
        .filter_map(|(conversation_id, files)| {
            files
                .iter()
                .find_map(|file_name| project_from_path_hint(file_name))
                .map(|project| (conversation_id.clone(), project))
        })
        .collect::<HashMap<_, _>>();

    let Some(projects_dir) = config::agent_projects_dir() else {
        return lookup;
    };
    let Ok(project_dirs) = fs::read_dir(projects_dir) else {
        return lookup;
    };

    for entry in project_dirs.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_dir() {
            continue;
        }

        let project_id = entry.file_name().to_string_lossy().to_string();
        let fallback_project = discovery::project_from_folder_id(&project_id);
        let transcript_dir = entry.path().join(config::AGENT_TRANSCRIPTS_DIR);
        collect_transcript_project_lookup(
            &transcript_dir,
            &project_id,
            &fallback_project,
            &tracking_files,
            &mut lookup,
        );
    }
    lookup
}

fn tracking_db_file_lookup(db_path: &Path) -> HashMap<String, Vec<String>> {
    let uri = format!("file:{}?immutable=1", db_path.display());
    let Ok(conn) = Connection::open_with_flags(
        &uri,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    ) else {
        return HashMap::new();
    };

    let mut stmt = match conn.prepare(
        "SELECT conversationId, fileName, COALESCE(timestamp, createdAt) AS observed_at
         FROM ai_code_hashes
         WHERE conversationId IS NOT NULL
           AND conversationId != ''
           AND fileName IS NOT NULL
           AND fileName != ''
         ORDER BY observed_at DESC",
    ) {
        Ok(stmt) => stmt,
        Err(_) => return HashMap::new(),
    };

    let rows = match stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, Option<String>>(1)?.unwrap_or_default(),
        ))
    }) {
        Ok(rows) => rows,
        Err(_) => return HashMap::new(),
    };

    let mut lookup: HashMap<String, Vec<String>> = HashMap::new();
    for row in rows.flatten() {
        let (conversation_id, file_name) = row;
        lookup.entry(conversation_id).or_default().push(file_name);
    }
    lookup
}

fn collect_transcript_project_lookup(
    dir: &Path,
    project_id: &str,
    fallback_project: &str,
    tracking_files: &HashMap<String, Vec<String>>,
    lookup: &mut HashMap<String, String>,
) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };

        if file_type.is_dir() {
            collect_transcript_project_lookup(
                &path,
                project_id,
                fallback_project,
                tracking_files,
                lookup,
            );
            continue;
        }

        if !file_type.is_file() || !is_transcript_file(&path) {
            continue;
        }

        let conversation_id = transcript_conversation_id(&path);
        if let Some(project) = transcript_project_hint(&path) {
            lookup.insert(conversation_id, project);
        } else if let Some(project) = tracking_files.get(&conversation_id).and_then(|files| {
            files.iter().find_map(|file_name| {
                project_from_path_hint_with_project_id(file_name, Some(project_id))
            })
        }) {
            lookup.insert(conversation_id, project);
        } else {
            lookup
                .entry(conversation_id)
                .or_insert_with(|| fallback_project.to_string());
        }
    }
}

fn is_transcript_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some("jsonl" | "txt")
    )
}

fn transcript_project_hint(path: &Path) -> Option<String> {
    let raw = fs::read_to_string(path).ok()?;
    let source_project_id = transcript_project_id(path);
    let turns = match path.extension().and_then(|ext| ext.to_str()) {
        Some("jsonl") => parse_jsonl_transcript_with_project_id(&raw, source_project_id.as_deref()),
        Some("txt") => parse_legacy_txt_transcript(&raw),
        _ => Vec::new(),
    };
    turns.into_iter().find_map(|turn| turn.project)
}

#[derive(Debug)]
struct AgentKvRow {
    role: String,
    content: String,
    request_id: Option<String>,
}

#[derive(Default)]
struct AgentKvSession {
    input_chars: usize,
    output_chars: usize,
    model: Option<String>,
    user_text: String,
    project: Option<String>,
}

fn parse_agent_kv(
    conn: &Connection,
    fallback_project: &str,
    seen: &mut HashSet<String>,
) -> Result<Vec<ParsedCall>> {
    let mut stmt = match conn.prepare(config::AGENT_KV_QUERY) {
        Ok(s) => s,
        Err(_) => return Ok(Vec::new()),
    };

    let rows = stmt.query_map([], |r| column_as_string(r, 1));

    let rows = match rows {
        Ok(rows) => rows,
        Err(_) => return Ok(Vec::new()),
    };

    let mut sessions: HashMap<String, AgentKvSession> = HashMap::new();
    let mut order: Vec<String> = Vec::new();
    let mut current_request_id = "unknown".to_string();

    for row in rows.flatten() {
        let Some(row) = row else {
            continue;
        };
        let Some(row) = parse_agent_kv_row(&row) else {
            continue;
        };

        let request_id = row.request_id.unwrap_or_else(|| current_request_id.clone());
        if request_id != current_request_id {
            current_request_id = request_id.clone();
        }

        let (text_length, model_from_content, first_text) = analyze_content(&row.content);

        let entry = if let Some(s) = sessions.get_mut(&request_id) {
            s
        } else {
            order.push(request_id.clone());
            sessions.entry(request_id.clone()).or_default()
        };

        if entry.project.is_none() {
            let context_text = first_text.as_deref().unwrap_or(&row.content);
            entry.project = extract_workspace_path(context_text);
        }

        match row.role.as_str() {
            "user" => {
                entry.input_chars += text_length;
                if entry.user_text.is_empty() {
                    let candidate = first_text.unwrap_or_else(|| row.content.clone());
                    entry.user_text = extract_user_query(&candidate);
                }
            }
            "assistant" => {
                entry.output_chars += text_length;
                if let Some(m) = model_from_content {
                    entry.model = Some(m);
                }
            }
            "tool" | "system" => {
                entry.input_chars += text_length;
            }
            _ => {}
        }
    }

    let mut calls = Vec::new();
    for request_id in order {
        let Some(session) = sessions.remove(&request_id) else {
            continue;
        };
        if session.input_chars == 0 && session.output_chars == 0 {
            continue;
        }
        let input_tokens = ((session.input_chars as f64) / CHARS_PER_TOKEN).ceil() as u64;
        let output_tokens = ((session.output_chars as f64) / CHARS_PER_TOKEN).ceil() as u64;
        let dedup_key = format!("cursor:agentKv:{}", request_id);
        if !seen.insert(dedup_key.clone()) {
            continue;
        }

        let display_model = display_model_for(session.model.as_deref());
        let project = session
            .project
            .unwrap_or_else(|| fallback_project.to_string());
        let mut call = ParsedCall {
            tool: config::TOOL_ID,
            model: display_model.clone(),
            input_tokens,
            output_tokens,
            speed: Speed::Standard,
            timestamp: None,
            dedup_key,
            user_message: session.user_text,
            session_id: request_id,
            project,
            ..ParsedCall::default()
        };
        call.cost_usd = pricing::cost(&display_model, &call, Speed::Standard);
        calls.push(call);
    }

    Ok(calls)
}

fn parse_agent_kv_row(raw: &str) -> Option<AgentKvRow> {
    let value = serde_json::from_str::<serde_json::Value>(raw).ok()?;
    let role = value.get("role").and_then(|v| v.as_str())?.to_string();
    let content_value = value.get("content")?;
    let content = content_value
        .as_str()
        .map(ToString::to_string)
        .unwrap_or_else(|| content_value.to_string());
    let request_id = value
        .pointer("/providerOptions/cursor/requestId")
        .and_then(|v| v.as_str())
        .map(ToString::to_string);
    Some(AgentKvRow {
        role,
        content,
        request_id,
    })
}

fn single_agent_workspace_path(conn: &Connection) -> Option<String> {
    let mut stmt = conn.prepare(config::AGENT_KV_QUERY).ok()?;
    let rows = stmt.query_map([], |r| column_as_string(r, 1)).ok()?;

    let mut found: Option<String> = None;
    for row in rows.flatten() {
        let Some(row) = row else {
            continue;
        };
        let Some(row) = parse_agent_kv_row(&row) else {
            continue;
        };
        if !matches!(row.role.as_str(), "user" | "system") {
            continue;
        }
        let (_, _, first_text) = analyze_content(&row.content);
        let context_text = first_text.as_deref().unwrap_or(&row.content);
        let Some(path) = extract_workspace_path(context_text) else {
            continue;
        };
        match &found {
            None => found = Some(path),
            Some(existing) if existing == &path => {}
            Some(_) => return None,
        }
    }
    found
}

fn analyze_content(raw: &str) -> (usize, Option<String>, Option<String>) {
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) {
        if let Some(arr) = value.as_array() {
            let mut total = 0usize;
            let mut model: Option<String> = None;
            let mut first_text: Option<String> = None;
            for item in arr {
                if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                    total += text.len();
                    if first_text.is_none() {
                        first_text = Some(text.to_string());
                    }
                }
                if model.is_none() {
                    if let Some(m) = item
                        .pointer("/providerOptions/cursor/modelName")
                        .and_then(|v| v.as_str())
                    {
                        model = Some(m.to_string());
                    }
                }
            }
            return (total, model, first_text);
        }
    }
    (raw.len(), None, None)
}

fn extract_user_query(text: &str) -> String {
    if let Some(start) = text.find("<user_query>") {
        if let Some(end) = text[start..].find("</user_query>") {
            let inner_start = start + "<user_query>".len();
            let inner_end = start + end;
            return truncate(text[inner_start..inner_end].trim(), 500);
        }
    }
    truncate(text, 500)
}

fn extract_workspace_path(text: &str) -> Option<String> {
    for line in text.lines() {
        let trimmed = line.trim();
        let lower = trimmed.to_ascii_lowercase();
        let Some(rest) = lower
            .strip_prefix("workspace path:")
            .map(|_| &trimmed["Workspace Path:".len()..])
        else {
            continue;
        };
        let value = rest.trim();
        if value.starts_with('/') || value.starts_with('~') || value.contains(":\\") {
            return Some(value.to_string());
        }
    }
    None
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    s.chars().take(max).collect()
}

fn display_model_for(raw: Option<&str>) -> String {
    match raw {
        None => COST_MODEL_FALLBACK.to_string(),
        Some(s) if s.is_empty() || s == "default" => COST_MODEL_FALLBACK.to_string(),
        Some(s) => s.to_string(),
    }
}

fn column_as_string(row: &rusqlite::Row<'_>, idx: usize) -> rusqlite::Result<Option<String>> {
    use rusqlite::types::ValueRef;
    match row.get_ref(idx)? {
        ValueRef::Null => Ok(None),
        ValueRef::Text(b) => Ok(Some(String::from_utf8_lossy(b).into_owned())),
        ValueRef::Integer(i) => Ok(Some(i.to_string())),
        ValueRef::Real(f) => Ok(Some(f.to_string())),
        ValueRef::Blob(b) => Ok(Some(String::from_utf8_lossy(b).into_owned())),
    }
}

fn parse_timestamp(s: &str) -> Option<DateTime<Utc>> {
    if s.is_empty() {
        return None;
    }
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.with_timezone(&Utc));
    }
    if let Ok(raw) = s.parse::<i64>() {
        if raw.abs() < 1_000_000_000_000 {
            return DateTime::from_timestamp(raw, 0);
        }
        return DateTime::from_timestamp_millis(raw);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn seed_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE cursorDiskKV (key TEXT PRIMARY KEY, value TEXT)",
            [],
        )
        .unwrap();
        conn
    }

    fn insert(conn: &Connection, key: &str, value: &str) {
        conn.execute(
            "INSERT INTO cursorDiskKV(key, value) VALUES (?1, ?2)",
            [key, value],
        )
        .unwrap();
    }

    fn insert_blob(conn: &Connection, key: &str, value: &str) {
        conn.execute(
            "INSERT INTO cursorDiskKV(key, value) VALUES (?1, ?2)",
            rusqlite::params![key, value.as_bytes()],
        )
        .unwrap();
    }

    fn source() -> SessionSource {
        SessionSource {
            path: std::path::PathBuf::from(":memory:"),
            project: "cursor-workspace".into(),
            tool: config::TOOL_ID,
        }
    }

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let suffix = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let counter = TEMP_COUNTER.fetch_add(1, Ordering::SeqCst);
            let path = std::env::temp_dir().join(format!(
                "tokenuse-cursor-parser-{}-{suffix}-{counter}",
                std::process::id()
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn transcript_source(path: PathBuf) -> SessionSource {
        SessionSource {
            path,
            project: "/Users/me/Code/fallback".into(),
            tool: config::TOOL_ID,
        }
    }

    fn tracking_db(path: &Path, conversation_id: &str, model: &str, observed_at: i64) -> PathBuf {
        let db = path.join(config::AGENT_TRACKING_DB);
        let conn = Connection::open(&db).unwrap();
        conn.execute(
            "CREATE TABLE ai_code_hashes (
                hash TEXT PRIMARY KEY,
                source TEXT NOT NULL,
                fileExtension TEXT,
                fileName TEXT,
                requestId TEXT,
                conversationId TEXT,
                timestamp INTEGER,
                createdAt INTEGER NOT NULL,
                model TEXT
            )",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO ai_code_hashes(hash, source, fileName, conversationId, timestamp, createdAt, model)
             VALUES ('h1', 'composer', '/Users/me/Code/tracked/src/lib.rs', ?1, ?2, ?2, ?3)",
            rusqlite::params![conversation_id, observed_at, model],
        )
        .unwrap();
        db
    }

    #[test]
    fn validates_missing_table_silently() {
        let conn = Connection::open_in_memory().unwrap();
        assert!(!validate_schema(&conn));
    }

    #[test]
    fn parses_assistant_user_zero_token_and_agentkv() {
        let conn = seed_db();
        // Assistant bubble with explicit token counts and a model.
        insert(
            &conn,
            "bubbleId:c1:a",
            r#"{"type":0,"createdAt":"2026-04-26T10:00:00Z","conversationId":"conv-1","tokenCount":{"inputTokens":120,"outputTokens":80},"modelInfo":{"modelName":"claude-sonnet-4-5"},"text":"hello assistant"}"#,
        );
        // User bubble with explicit token counts.
        insert(
            &conn,
            "bubbleId:c1:b",
            r#"{"type":1,"createdAt":"2026-04-26T10:00:01Z","conversationId":"conv-1","tokenCount":{"inputTokens":40,"outputTokens":0},"text":"a user message"}"#,
        );
        // Zero-token assistant bubble with text — must use char/4 fallback.
        insert(
            &conn,
            "bubbleId:c1:c",
            r#"{"type":0,"createdAt":"2026-04-26T10:00:02Z","conversationId":"conv-1","tokenCount":{"inputTokens":0,"outputTokens":0},"modelInfo":{"modelName":"default"},"text":"abcdefghij"}"#,
        );
        // AgentKv conversation: user + assistant.
        insert_blob(
            &conn,
            "agentKv:blob:r1:1",
            r#"{"role":"user","content":[{"type":"text","text":"<user_info>\nWorkspace Path: /Users/me/Code/blog\n</user_info>\n<user_query>fix typo</user_query>"}],"providerOptions":{"cursor":{"requestId":"req-1"}}}"#,
        );
        insert_blob(
            &conn,
            "agentKv:blob:r1:2",
            r#"{"role":"assistant","content":[{"type":"text","text":"sure here is the patch","providerOptions":{"cursor":{"modelName":"gpt-5"}}}],"providerOptions":{"cursor":{"requestId":"req-1"}}}"#,
        );

        let mut seen = HashSet::new();
        let calls = parse_with_conn(&conn, &source(), &mut seen, &HashMap::new()).unwrap();
        assert_eq!(
            calls.len(),
            4,
            "3 bubbles + 1 agentKv session, got {:?}",
            calls.iter().map(|c| &c.dedup_key).collect::<Vec<_>>()
        );

        let assistant = &calls[0];
        assert_eq!(assistant.input_tokens, 120);
        assert_eq!(assistant.output_tokens, 80);
        assert_eq!(assistant.model, "claude-sonnet-4-5");
        assert_eq!(assistant.project, "/Users/me/Code/blog");
        assert!(assistant.cost_usd > 0.0);

        let user = &calls[1];
        assert_eq!(user.input_tokens, 40);
        assert_eq!(user.output_tokens, 0);
        assert_eq!(user.model, "cursor-auto");

        let zero = &calls[2];
        assert_eq!(zero.input_tokens, 0);
        assert_eq!(zero.output_tokens, 3, "ceil(10/4) = 3");
        assert_eq!(zero.model, "cursor-auto", "default → cursor-auto");

        let agent = &calls[3];
        assert_eq!(agent.session_id, "req-1");
        assert!(agent.dedup_key.starts_with("cursor:agentKv:"));
        assert_eq!(agent.user_message, "fix typo");
        assert_eq!(agent.project, "/Users/me/Code/blog");
        assert_eq!(agent.model, "gpt-5");
        assert!(agent.input_tokens > 0);
        assert!(agent.output_tokens > 0);
    }

    #[test]
    fn dedup_blocks_second_pass() {
        let conn = seed_db();
        insert(
            &conn,
            "bubbleId:x:1",
            r#"{"type":0,"createdAt":"2026-04-26T11:00:00Z","conversationId":"conv-x","tokenCount":{"inputTokens":10,"outputTokens":5},"text":"hi"}"#,
        );
        let mut seen = HashSet::new();
        let first = parse_with_conn(&conn, &source(), &mut seen, &HashMap::new()).unwrap();
        assert_eq!(first.len(), 1);
        let second = parse_with_conn(&conn, &source(), &mut seen, &HashMap::new()).unwrap();
        assert!(second.is_empty(), "second pass must dedup");
    }

    #[test]
    fn bubbles_use_transcript_project_lookup_by_conversation_id() {
        let conn = seed_db();
        insert(
            &conn,
            "bubbleId:conv-mapped:1",
            r#"{"type":0,"createdAt":"2026-04-26T11:00:00Z","conversationId":"conv-mapped","tokenCount":{"inputTokens":10,"outputTokens":5},"text":"hi"}"#,
        );

        let mut projects = HashMap::new();
        projects.insert(
            "conv-mapped".to_string(),
            "/Users/me/Code/mapped".to_string(),
        );
        let mut seen = HashSet::new();
        let calls = parse_with_conn(&conn, &source(), &mut seen, &projects).unwrap();

        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].project, "/Users/me/Code/mapped");
    }

    #[test]
    fn extract_user_query_finds_inner_text() {
        assert_eq!(
            extract_user_query("preface <user_query>real question</user_query> trailer"),
            "real question"
        );
        assert_eq!(extract_user_query("no tags here"), "no tags here");
    }

    #[test]
    fn extract_workspace_path_finds_user_info_value() {
        assert_eq!(
            extract_workspace_path(
                "<user_info>\nWorkspace Path: /Users/me/Code/blog\n</user_info>"
            )
            .as_deref(),
            Some("/Users/me/Code/blog")
        );
        assert_eq!(extract_workspace_path("Terminals folder: /tmp"), None);
    }

    #[test]
    fn path_hint_uses_cursor_project_id_to_restore_nested_project_root() {
        assert_eq!(
            project_from_path_hint_with_project_id(
                "/Users/me/Code/Octo/Documents/pipelines/build.yml",
                Some("Users-me-Code-Octo-Documents")
            )
            .as_deref(),
            Some("/Users/me/Code/Octo/Documents")
        );
        assert_eq!(
            project_from_path_hint_with_project_id(
                "/Users/me/Code/Octo-Bot-Two-Point-Oh/src/main.rs",
                Some("Users-me-Code-Octo-Bot-Two-Point-Oh")
            )
            .as_deref(),
            Some("/Users/me/Code/Octo-Bot-Two-Point-Oh")
        );
    }

    #[test]
    fn parses_jsonl_transcript_with_tools_commands_project_and_tracking_metadata() {
        let dir = TempDir::new();
        let conversation_id = "84ba1021-c047-4841-a76e-e4332adba063";
        let transcript = dir.path().join(format!("{conversation_id}.jsonl"));
        fs::write(
            &transcript,
            r#"{"role":"user","message":{"content":[{"type":"text","text":"<user_query>build report</user_query>"}]}}"#
                .to_string()
                + "\n"
                + r#"{"role":"assistant","message":{"content":[{"type":"text","text":"I will inspect it."},{"type":"tool_use","name":"Read","input":{"path":"/Users/me/Code/app/src/main.rs"}},{"type":"tool_use","name":"Shell","input":{"command":"cd /Users/me/Code/app && cargo test"}}]}}"#
                + "\n",
        )
        .unwrap();
        let observed_at = DateTime::parse_from_rfc3339("2026-04-26T12:00:00Z")
            .unwrap()
            .timestamp_millis();
        let db = tracking_db(dir.path(), conversation_id, "gpt-5", observed_at);

        let source = transcript_source(transcript);
        let mut seen = HashSet::new();
        let calls = parse_transcript_file(&source, &mut seen, Some(&db)).unwrap();
        assert_eq!(calls.len(), 1);
        let call = &calls[0];
        assert_eq!(call.tool, config::TOOL_ID);
        assert_eq!(call.model, "gpt-5");
        assert_eq!(call.timestamp.unwrap().timestamp_millis(), observed_at);
        assert_eq!(call.user_message, "build report");
        assert_eq!(call.project, "/Users/me/Code/app");
        assert_eq!(call.tools, vec!["Read", "Bash"]);
        assert_eq!(
            call.bash_commands,
            vec!["cd /Users/me/Code/app && cargo test"]
        );
        assert!(call.input_tokens > 0);
        assert!(call.output_tokens > 0);

        let second = parse_transcript_file(&source, &mut seen, Some(&db)).unwrap();
        assert!(second.is_empty());
    }

    #[test]
    fn transcript_uses_tracking_db_project_when_turn_has_no_path_hint() {
        let dir = TempDir::new();
        let conversation_id = "53e222bf-5cc6-49a0-9972-f3cf1f20b121";
        let transcript = dir.path().join(format!("{conversation_id}.jsonl"));
        fs::write(
            &transcript,
            r#"{"role":"user","message":{"content":[{"type":"text","text":"summarize"}]}}"#
                .to_string()
                + "\n"
                + r#"{"role":"assistant","message":{"content":[{"type":"text","text":"done"}]}}"#
                + "\n",
        )
        .unwrap();
        let observed_at = DateTime::parse_from_rfc3339("2026-04-26T12:00:00Z")
            .unwrap()
            .timestamp_millis();
        let db = tracking_db(dir.path(), conversation_id, "gpt-5", observed_at);

        let mut seen = HashSet::new();
        let calls =
            parse_transcript_file(&transcript_source(transcript), &mut seen, Some(&db)).unwrap();

        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].project, "/Users/me/Code/tracked");
    }

    #[test]
    fn parses_legacy_txt_transcript() {
        let turns = parse_legacy_txt_transcript(
            "user: <user_query>fix bug</user_query>\nA: I will check.\n[Thinking] tracing\n[Tool call] Shell\n[Tool result] ok\n",
        );
        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0].user_message, "fix bug");
        assert_eq!(turns[0].tools, vec!["Bash"]);
        assert!(turns[0].output_chars > 0);
    }

    #[test]
    fn transcript_uses_file_mtime_without_tracking_metadata() {
        let dir = TempDir::new();
        let transcript = dir.path().join("conversation.jsonl");
        fs::write(
            &transcript,
            r#"{"role":"user","message":{"content":[{"type":"text","text":"hello"}]}}"#.to_string()
                + "\n"
                + r#"{"role":"assistant","message":{"content":[{"type":"text","text":"hi"}]}}"#
                + "\n",
        )
        .unwrap();

        let mut seen = HashSet::new();
        let calls = parse_transcript_file(&transcript_source(transcript), &mut seen, None).unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].model, "cursor-auto");
        assert!(calls[0].timestamp.is_some());
    }
}
