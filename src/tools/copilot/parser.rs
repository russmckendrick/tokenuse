use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, NaiveDateTime, Utc};
use color_eyre::Result;
use rusqlite::Connection;
use serde_json::Value;

use crate::pricing;
use crate::tools::{jsonl, ParsedCall, SessionSource, Speed};

use super::config;

const CHARS_PER_TOKEN: f64 = config::CHARS_PER_TOKEN;
const ANTHROPIC_AUTO: &str = "anthropic-auto";
const OPENAI_AUTO: &str = "openai-auto";
const COPILOT_AUTO: &str = "copilot-auto";

pub fn parse_session(
    source: &SessionSource,
    seen: &mut HashSet<String>,
) -> Result<Vec<ParsedCall>> {
    let path = &source.path;
    if path.is_file() {
        match path.file_name().and_then(|name| name.to_str()) {
            Some(config::CLI_SESSION_STORE_FILE) => {
                return Ok(parse_session_store(path, source, seen));
            }
            Some(config::CLI_DATA_STORE_FILE) => {
                return Ok(parse_data_store(path, source, seen));
            }
            Some(config::OTEL_TRACES_FILE) => {
                return Ok(parse_otel_traces(path, source, seen));
            }
            _ => {}
        }
    }
    if path.is_dir() {
        let is_chat_sessions =
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| {
                    name == config::CHAT_SESSIONS_DIR
                        || name == config::EMPTY_WINDOW_CHAT_SESSIONS_DIR
                });
        let mut calls = Vec::new();
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_file() && p.extension().and_then(|s| s.to_str()) == Some("jsonl") {
                    if is_chat_sessions {
                        calls.extend(parse_chat_session_file(&p, source, seen));
                    } else {
                        calls.extend(parse_file(&p, source, seen));
                    }
                }
            }
        }
        return Ok(calls);
    }

    Ok(parse_file(path, source, seen))
}

fn parse_file(path: &Path, source: &SessionSource, seen: &mut HashSet<String>) -> Vec<ParsedCall> {
    let lines: Vec<String> = match jsonl::read_lines(path) {
        Ok(it) => it.collect(),
        Err(_) => return Vec::new(),
    };
    if lines.is_empty() {
        return Vec::new();
    }

    if is_transcript(&lines[0]) {
        let session_id = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        let project = transcript_cwd(&lines)
            .or_else(|| workspace_cwd(path))
            .unwrap_or_else(|| source.project.clone());
        parse_transcript(&lines, &session_id, &project, seen)
    } else {
        let session_id = path
            .parent()
            .and_then(|p| p.file_name())
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        let project = workspace_cwd(path).unwrap_or_else(|| source.project.clone());
        parse_legacy(&lines, &session_id, &project, seen)
    }
}

fn is_transcript(first_line: &str) -> bool {
    let Ok(value) = serde_json::from_str::<Value>(first_line) else {
        return false;
    };
    value.get("type").and_then(|t| t.as_str()) == Some("session.start")
        && value.pointer("/data/producer").and_then(|p| p.as_str()) == Some("copilot-agent")
}

fn parse_legacy(
    lines: &[String],
    session_id: &str,
    project: &str,
    seen: &mut HashSet<String>,
) -> Vec<ParsedCall> {
    let mut current_model = String::new();
    let mut pending_user_message = String::new();
    let mut pending_user_chars: Option<u64> = None;
    let mut calls = Vec::new();

    for line in lines {
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let kind = event.get("type").and_then(|t| t.as_str()).unwrap_or("");

        match kind {
            "session.model_change" => {
                if let Some(m) = event.pointer("/data/newModel").and_then(|v| v.as_str()) {
                    if !m.is_empty() {
                        current_model = m.to_string();
                    }
                }
            }
            "user.message" => {
                if let Some(c) = event.pointer("/data/content").and_then(|v| v.as_str()) {
                    pending_user_chars = Some(c.chars().count() as u64);
                    pending_user_message = truncate(c, 500);
                }
            }
            "assistant.message" => {
                let output_tokens = event
                    .pointer("/data/outputTokens")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                if output_tokens == 0 || current_model.is_empty() {
                    continue;
                }
                let Some(message_id) = event.pointer("/data/messageId").and_then(|v| v.as_str())
                else {
                    continue;
                };

                let dedup_key = format!("copilot:{}:{}", session_id, message_id);
                if !seen.insert(dedup_key.clone()) {
                    continue;
                }

                let (tools, bash_commands) = extract_tools(
                    event
                        .pointer("/data/toolRequests")
                        .and_then(|v| v.as_array()),
                );
                let timestamp = event
                    .get("timestamp")
                    .and_then(|v| v.as_str())
                    .and_then(parse_timestamp);

                let mut call = ParsedCall {
                    tool: config::TOOL_ID,
                    model: current_model.clone(),
                    input_tokens: 0,
                    output_tokens,
                    speed: Speed::Standard,
                    tools,
                    bash_commands,
                    timestamp,
                    dedup_key,
                    user_message: std::mem::take(&mut pending_user_message),
                    session_id: session_id.to_string(),
                    project: project.to_string(),
                    prompt_chars: pending_user_chars.take(),
                    ..ParsedCall::default()
                };
                call.cost_usd = pricing::cost(&current_model, &call, Speed::Standard);
                calls.push(call);
            }
            "session.shutdown" => {
                calls.extend(shutdown_rollup_calls(&event, session_id, project, seen));
            }
            _ => {}
        }
    }

    calls
}

/// `session.shutdown` carries the only real input/cache token counts a legacy
/// CLI session records, as per-model rollups. `usage.inputTokens` is written
/// cache-inclusive (input + cache reads + cache writes), so the pure input is
/// recovered by subtraction. One supplementary input-only call is emitted per
/// model; output stays with the per-turn `assistant.message` records so
/// nothing double counts.
fn shutdown_rollup_calls(
    event: &Value,
    session_id: &str,
    project: &str,
    seen: &mut HashSet<String>,
) -> Vec<ParsedCall> {
    let Some(metrics) = event
        .pointer("/data/modelMetrics")
        .and_then(|v| v.as_object())
    else {
        return Vec::new();
    };
    let timestamp = event
        .get("timestamp")
        .and_then(|v| v.as_str())
        .and_then(parse_timestamp)
        .or_else(|| {
            event
                .pointer("/data/sessionStartTime")
                .and_then(|v| v.as_i64())
                .and_then(epoch_to_datetime)
        });

    let mut calls = Vec::new();
    for (model, entry) in metrics {
        let usage = entry.get("usage");
        let field = |key: &str| {
            usage
                .and_then(|u| u.get(key))
                .and_then(|v| v.as_u64())
                .unwrap_or(0)
        };
        let cache_read = field("cacheReadTokens");
        let cache_write = field("cacheWriteTokens");
        let input = field("inputTokens").saturating_sub(cache_read + cache_write);
        if input == 0 && cache_read == 0 && cache_write == 0 {
            continue;
        }

        let dedup_key = format!("copilot:{session_id}:shutdown:{model}");
        if !seen.insert(dedup_key.clone()) {
            continue;
        }

        let mut call = ParsedCall {
            tool: config::TOOL_ID,
            model: model.clone(),
            input_tokens: input,
            output_tokens: 0,
            cache_creation_input_tokens: cache_write,
            cache_read_input_tokens: cache_read,
            speed: Speed::Standard,
            timestamp,
            dedup_key,
            session_id: session_id.to_string(),
            project: project.to_string(),
            token_quality: crate::tools::TokenQuality::Exact,
            ..ParsedCall::default()
        };
        call.cost_usd = pricing::cost(model, &call, Speed::Standard);
        calls.push(call);
    }
    calls
}

fn epoch_to_datetime(raw: i64) -> Option<DateTime<Utc>> {
    // Accept seconds, milliseconds, or nanoseconds since the epoch.
    let millis = if raw > 1_000_000_000_000_000 {
        raw / 1_000_000
    } else if raw > 100_000_000_000 {
        raw
    } else {
        raw.checked_mul(1000)?
    };
    DateTime::from_timestamp_millis(millis)
}

fn workspace_cwd(path: &Path) -> Option<String> {
    let dir = if path.is_dir() { path } else { path.parent()? };
    let raw = fs::read_to_string(dir.join(config::WORKSPACE_FILE)).ok()?;
    raw.lines().find_map(parse_workspace_cwd_line)
}

fn parse_workspace_cwd_line(line: &str) -> Option<String> {
    let value = line.trim().strip_prefix("cwd:")?.trim();
    let value = value.trim_matches('"').trim_matches('\'').trim();
    if value.is_empty() || value == "null" {
        None
    } else {
        Some(value.to_string())
    }
}

fn parse_transcript(
    lines: &[String],
    session_id: &str,
    project: &str,
    seen: &mut HashSet<String>,
) -> Vec<ParsedCall> {
    let events: Vec<Value> = lines
        .iter()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .collect();

    // Tool-call id prefixes reflect the backend that actually served the
    // session and win over `session.start data.model`, which has been observed
    // to disagree (e.g. declaring gpt-5 while the tool ids are Bedrock
    // Anthropic). The declared model is only trusted when no known prefix
    // appears, where it beats the generic copilot-auto bucket.
    let inferred = infer_model_from_tool_ids(&events);
    let model = if inferred == COPILOT_AUTO {
        transcript_declared_model(&events).unwrap_or(inferred)
    } else {
        inferred
    };

    let mut pending_user_message = String::new();
    let mut pending_user_chars: Option<u64> = None;
    let mut calls = Vec::new();

    for event in &events {
        let kind = event.get("type").and_then(|t| t.as_str()).unwrap_or("");

        if kind == "user.message" {
            if let Some(c) = event.pointer("/data/content").and_then(|v| v.as_str()) {
                pending_user_chars = Some(c.chars().count() as u64);
                pending_user_message = truncate(c, 500);
            }
            continue;
        }

        if kind != "assistant.message" {
            continue;
        }

        let content_text = event
            .pointer("/data/content")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let reasoning_text = event
            .pointer("/data/reasoningText")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let tool_array = event
            .pointer("/data/toolRequests")
            .and_then(|v| v.as_array());
        let has_tools = tool_array.map(|a| !a.is_empty()).unwrap_or(false);

        if content_text.is_empty() && reasoning_text.is_empty() && !has_tools {
            continue;
        }

        let Some(message_id) = event.pointer("/data/messageId").and_then(|v| v.as_str()) else {
            continue;
        };
        let dedup_key = format!("copilot:{}:{}", session_id, message_id);
        if !seen.insert(dedup_key.clone()) {
            continue;
        }

        let explicit_output = event
            .pointer("/data/outputTokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let (output_tokens, reasoning_tokens) = if explicit_output > 0 {
            (explicit_output, 0)
        } else {
            let visible_output = ((content_text.len() as f64) / CHARS_PER_TOKEN).ceil() as u64;
            let reasoning = ((reasoning_text.len() as f64) / CHARS_PER_TOKEN).ceil() as u64;
            (visible_output.saturating_add(reasoning), reasoning)
        };
        let input_tokens = ((pending_user_message.len() as f64) / CHARS_PER_TOKEN).ceil() as u64;

        let (tools, bash_commands) = extract_tools(tool_array);
        let timestamp = event
            .get("timestamp")
            .and_then(|v| v.as_str())
            .and_then(parse_timestamp);

        let mut call = ParsedCall {
            tool: config::TOOL_ID,
            model: model.clone(),
            input_tokens,
            output_tokens,
            reasoning_tokens,
            speed: Speed::Standard,
            tools,
            bash_commands,
            timestamp,
            dedup_key,
            user_message: std::mem::take(&mut pending_user_message),
            session_id: session_id.to_string(),
            project: project.to_string(),
            prompt_chars: pending_user_chars.take(),
            // Reasoning text is deliberately excluded, mirroring how Claude
            // thinking blocks stay out of response_chars.
            response_chars: Some(content_text.chars().count() as u64),
            code_blocks: jsonl::merge_code_blocks(jsonl::extract_code_fences(content_text)),
            ..ParsedCall::default()
        };
        call.cost_usd = pricing::cost(&model, &call, Speed::Standard);
        calls.push(call);
    }

    calls
}

/// Private working copy of a Copilot CLI SQLite store. The CLI keeps these
/// databases in WAL mode and may hold un-checkpointed rows in the -wal file,
/// which `immutable=1` reads would miss; copying db + sidecars and opening the
/// copy lets SQLite recover the WAL without touching the live files.
struct StoreCopy {
    dir: PathBuf,
    db: PathBuf,
}

impl StoreCopy {
    fn new(path: &Path) -> Option<Self> {
        // The counter keeps concurrent copies (parallel tests, several stores
        // in one sync) from landing in the same nanosecond-named directory.
        static COPY_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let file_name = path.file_name()?.to_string_lossy().to_string();
        let dir = std::env::temp_dir().join(format!(
            "tokenuse-copilot-store-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .ok()?
                .as_nanos(),
            COPY_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::create_dir_all(&dir).ok()?;
        let db = dir.join(&file_name);
        if fs::copy(path, &db).is_err() {
            let _ = fs::remove_dir_all(&dir);
            return None;
        }
        for suffix in ["-wal", "-shm"] {
            let sidecar = PathBuf::from(format!("{}{suffix}", path.display()));
            if sidecar.is_file() {
                let _ = fs::copy(&sidecar, dir.join(format!("{file_name}{suffix}")));
            }
        }
        Some(Self { dir, db })
    }

    fn open(&self) -> rusqlite::Result<Connection> {
        Connection::open(&self.db)
    }
}

impl Drop for StoreCopy {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

/// Copilot Chat's OpenTelemetry span store (`agent-traces.db`): `chat` spans
/// carry real GenAI-convention token counts including both cache buckets;
/// `execute_tool` spans in the same trace carry the turn's tool calls;
/// `invoke_agent` spans with a parent chat session mark subagent delegation.
fn parse_otel_traces(
    path: &Path,
    source: &SessionSource,
    seen: &mut HashSet<String>,
) -> Vec<ParsedCall> {
    let Some(copy) = StoreCopy::new(path) else {
        return Vec::new();
    };
    let Ok(conn) = copy.open() else {
        return Vec::new();
    };
    otel_calls(&conn, source, seen).unwrap_or_default()
}

struct OtelSpan {
    span_id: String,
    trace_id: String,
    operation: String,
    start_ms: i64,
    response_model: Option<String>,
    attrs: HashMap<String, String>,
}

fn otel_calls(
    conn: &Connection,
    source: &SessionSource,
    seen: &mut HashSet<String>,
) -> rusqlite::Result<Vec<ParsedCall>> {
    let mut attrs: HashMap<String, HashMap<String, String>> = HashMap::new();
    {
        let mut stmt = conn.prepare("SELECT span_id, key, value FROM span_attributes")?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
            ))
        })?;
        for row in rows.flatten() {
            let (span_id, key, value) = row;
            attrs
                .entry(span_id)
                .or_default()
                .insert(key, value.unwrap_or_default());
        }
    }

    let mut spans: Vec<OtelSpan> = Vec::new();
    {
        let mut stmt = conn.prepare(
            "SELECT span_id, trace_id, operation_name, start_time_ms, response_model
             FROM spans ORDER BY start_time_ms, span_id",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, Option<String>>(4)?,
            ))
        })?;
        for row in rows.flatten() {
            let (span_id, trace_id, operation, start_ms, response_model) = row;
            let span_attrs = attrs.remove(&span_id).unwrap_or_default();
            spans.push(OtelSpan {
                span_id,
                trace_id,
                operation: operation.unwrap_or_default(),
                start_ms,
                response_model,
                attrs: span_attrs,
            });
        }
    }

    // A conversation's project comes from whichever of its spans carries the
    // repository attribute.
    let mut conversation_project: HashMap<String, String> = HashMap::new();
    for span in &spans {
        if let (Some(conv), Some(repo)) = (
            span.attrs.get("gen_ai.conversation.id"),
            span.attrs.get("github.copilot.git.repository"),
        ) {
            if !repo.is_empty() {
                conversation_project
                    .entry(conv.clone())
                    .or_insert_with(|| repository_label(repo));
            }
        }
    }

    // Tool activity per trace, attached to the trace's chat span.
    let mut trace_tools: HashMap<String, (Vec<String>, Vec<String>)> = HashMap::new();
    for span in &spans {
        let entry = trace_tools.entry(span.trace_id.clone()).or_default();
        match span.operation.as_str() {
            "execute_tool" => {
                let Some(raw_name) = span.attrs.get("gen_ai.tool.name").filter(|n| !n.is_empty())
                else {
                    continue;
                };
                let name = normalize_tool(raw_name);
                if name == "Bash" {
                    if let Some(args) = span.attrs.get("gen_ai.tool.call.arguments") {
                        entry.1.extend(otel_bash_commands(args));
                    }
                }
                entry.0.push(name);
            }
            // The root agent has no parent chat session; only genuine
            // subagent delegation counts.
            "invoke_agent"
                if span
                    .attrs
                    .contains_key("copilot_chat.parent_chat_session_id")
                    && span
                        .attrs
                        .get("gen_ai.agent.name")
                        .is_some_and(|n| !n.is_empty()) =>
            {
                entry.0.push("Agent".to_string());
            }
            _ => {}
        }
    }

    let mut calls = Vec::new();
    for span in &spans {
        if span.operation != "chat" {
            continue;
        }
        let attr_u64 = |key: &str| {
            span.attrs
                .get(key)
                .and_then(|v| v.trim().parse::<u64>().ok())
                .unwrap_or(0)
        };
        let input = attr_u64("gen_ai.usage.input_tokens");
        let output = attr_u64("gen_ai.usage.output_tokens");
        if input == 0 && output == 0 {
            continue;
        }
        let cache_read = attr_u64("gen_ai.usage.cache_read.input_tokens");
        let cache_write = attr_u64("gen_ai.usage.cache_creation.input_tokens");

        let model = span
            .attrs
            .get("gen_ai.response.model")
            .or_else(|| span.attrs.get("gen_ai.request.model"))
            .filter(|m| !m.is_empty())
            .cloned()
            .or_else(|| span.response_model.clone().filter(|m| !m.is_empty()))
            .unwrap_or_else(|| COPILOT_AUTO.to_string());

        let conversation = span
            .attrs
            .get("gen_ai.conversation.id")
            .filter(|c| !c.is_empty())
            .cloned()
            .unwrap_or_else(|| span.trace_id.clone());

        let dedup_key = format!("{}{}", config::OTEL_DEDUP_PREFIX, span.span_id);
        if !seen.insert(dedup_key.clone()) {
            continue;
        }
        // The same turn may also exist in transcript/journal sources (written
        // by the same extension). Claim its key so later sources skip it, and
        // carry it as a superseded hint so previously archived estimate rows
        // are zeroed.
        let mut superseded = Vec::new();
        if let Some(turn_id) = span
            .attrs
            .get("github.copilot.chat.turn.id")
            .filter(|t| !t.is_empty())
        {
            let turn_key = format!("copilot:{conversation}:{turn_id}");
            seen.insert(turn_key.clone());
            superseded.push(turn_key);
        }

        let (tools, bash_commands) = trace_tools.get(&span.trace_id).cloned().unwrap_or_default();

        let mut call = ParsedCall {
            tool: config::TOOL_ID,
            model: model.clone(),
            input_tokens: input,
            output_tokens: output,
            cache_creation_input_tokens: cache_write,
            cache_read_input_tokens: cache_read,
            speed: Speed::Standard,
            tools,
            bash_commands,
            timestamp: epoch_to_datetime(span.start_ms),
            dedup_key,
            session_id: conversation.clone(),
            project: conversation_project
                .get(&conversation)
                .cloned()
                .unwrap_or_else(|| source.project.clone()),
            token_quality: crate::tools::TokenQuality::Exact,
            superseded_dedup_keys: superseded,
            ..ParsedCall::default()
        };
        call.cost_usd = pricing::cost(&model, &call, Speed::Standard);
        calls.push(call);
    }

    Ok(calls)
}

/// OTel records full multi-line scripts; flatten newlines into separators so
/// the shared splitter sees one command list.
fn otel_bash_commands(raw_args: &str) -> Vec<String> {
    let Ok(args) = serde_json::from_str::<Value>(raw_args) else {
        return Vec::new();
    };
    let Some(command) = args
        .get("command")
        .or_else(|| args.get("cmd"))
        .and_then(|v| v.as_str())
    else {
        return Vec::new();
    };
    jsonl::split_bash_commands(&command.replace('\n', ";"))
}

/// `github.copilot.git.repository` values look like `owner/repo` or a URL;
/// keep the repository name.
fn repository_label(repo: &str) -> String {
    let trimmed = repo.trim().trim_end_matches('/');
    let base = trimmed.rsplit('/').next().unwrap_or(trimmed);
    base.trim_end_matches(".git").to_string()
}

/// VS Code core chat-session journal: a delta log where kind 0 sets the root
/// object, kind 1 sets a value at a path, and kind 2 appends items to an
/// array at a path (default `requests`). Requests carry real prompt/output
/// token counts in `result.metadata`.
fn parse_chat_session_file(
    path: &Path,
    source: &SessionSource,
    seen: &mut HashSet<String>,
) -> Vec<ParsedCall> {
    let lines: Vec<String> = match jsonl::read_lines(path) {
        Ok(it) => it.collect(),
        Err(_) => return Vec::new(),
    };

    let mut root = Value::Null;
    for line in &lines {
        let Ok(entry) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        match entry.get("kind").and_then(|v| v.as_u64()) {
            Some(0) => {
                if let Some(v) = entry.get("v") {
                    root = v.clone();
                }
            }
            Some(1) => apply_journal_set(&mut root, entry.get("k"), entry.get("v")),
            Some(2) => apply_journal_append(&mut root, entry.get("k"), entry.get("v")),
            _ => {}
        }
    }

    let session_id = root
        .get("sessionId")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| {
            path.file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default()
        });
    let creation_ts = root
        .get("creationDate")
        .and_then(|v| v.as_i64())
        .and_then(epoch_to_datetime);

    let Some(requests) = root.get("requests").and_then(|v| v.as_array()) else {
        return Vec::new();
    };

    let mut calls = Vec::new();
    for (idx, req) in requests.iter().enumerate() {
        let prompt = req
            .pointer("/result/metadata/promptTokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let output = req
            .pointer("/result/metadata/outputTokens")
            .and_then(|v| v.as_u64())
            .filter(|n| *n > 0)
            .or_else(|| req.get("completionTokens").and_then(|v| v.as_u64()))
            .unwrap_or(0);
        if prompt == 0 && output == 0 {
            continue;
        }

        let model = req
            .pointer("/result/metadata/resolvedModel")
            .and_then(|v| v.as_str())
            .filter(|m| !m.is_empty())
            .map(str::to_string)
            .or_else(|| {
                req.get("modelId")
                    .and_then(|v| v.as_str())
                    .map(|m| m.strip_prefix("copilot/").unwrap_or(m))
                    .filter(|m| !m.is_empty() && *m != "auto")
                    .map(str::to_string)
            })
            .unwrap_or_else(|| COPILOT_AUTO.to_string());

        let request_id = req
            .get("requestId")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| format!("request-{idx}"));
        let dedup_key = format!(
            "{}{}:{}",
            config::CHAT_SESSION_DEDUP_PREFIX,
            session_id,
            request_id
        );
        if !seen.insert(dedup_key.clone()) {
            continue;
        }

        let tools = chat_session_tools(req);
        let timestamp = req
            .get("timestamp")
            .and_then(|v| v.as_i64())
            .and_then(epoch_to_datetime)
            .or_else(|| {
                req.get("timestamp")
                    .and_then(|v| v.as_str())
                    .and_then(parse_timestamp)
            })
            .or(creation_ts);
        let user_message = req
            .get("message")
            .and_then(|m| m.get("text"))
            .and_then(|v| v.as_str())
            .map(|t| truncate(t, 500))
            .unwrap_or_default();

        let mut call = ParsedCall {
            tool: config::TOOL_ID,
            model: model.clone(),
            input_tokens: prompt,
            output_tokens: output,
            speed: Speed::Standard,
            tools,
            timestamp,
            dedup_key,
            user_message,
            session_id: session_id.clone(),
            project: source.project.clone(),
            token_quality: crate::tools::TokenQuality::Exact,
            ..ParsedCall::default()
        };
        call.cost_usd = pricing::cost(&model, &call, Speed::Standard);
        calls.push(call);
    }
    calls
}

fn apply_journal_set(root: &mut Value, path: Option<&Value>, value: Option<&Value>) {
    let (Some(path), Some(value)) = (path, value) else {
        return;
    };
    let Some(segments) = journal_path_segments(path) else {
        return;
    };
    if let Some(target) = journal_walk(root, &segments[..segments.len().saturating_sub(1)]) {
        if let (Some(last), Value::Object(map)) = (segments.last(), target) {
            map.insert(last.clone(), value.clone());
        }
    }
}

fn apply_journal_append(root: &mut Value, path: Option<&Value>, value: Option<&Value>) {
    let Some(value) = value else {
        return;
    };
    let segments = path
        .and_then(journal_path_segments_opt)
        .unwrap_or_else(|| vec!["requests".to_string()]);
    let Some(target) = journal_walk(root, &segments) else {
        return;
    };
    if target.is_null() {
        *target = Value::Array(Vec::new());
    }
    let Value::Array(items) = target else {
        return;
    };
    match value {
        Value::Array(new_items) => items.extend(new_items.iter().cloned()),
        other => items.push(other.clone()),
    }
}

fn journal_path_segments(path: &Value) -> Option<Vec<String>> {
    journal_path_segments_opt(path)
}

fn journal_path_segments_opt(path: &Value) -> Option<Vec<String>> {
    let arr = path.as_array()?;
    let mut segments = Vec::with_capacity(arr.len());
    for seg in arr {
        match seg {
            Value::String(s) => segments.push(s.clone()),
            Value::Number(n) => segments.push(n.to_string()),
            _ => return None,
        }
    }
    Some(segments)
}

/// Walk (and create) the object path; numeric segments index into arrays.
fn journal_walk<'a>(root: &'a mut Value, segments: &[String]) -> Option<&'a mut Value> {
    let mut current = root;
    for seg in segments {
        if current.is_null() {
            *current = Value::Object(serde_json::Map::new());
        }
        current = match current {
            Value::Object(map) => map.entry(seg.clone()).or_insert(Value::Null),
            Value::Array(items) => {
                let idx: usize = seg.parse().ok()?;
                items.get_mut(idx)?
            }
            _ => return None,
        };
    }
    Some(current)
}

fn chat_session_tools(req: &Value) -> Vec<String> {
    let Some(rounds) = req
        .pointer("/result/metadata/toolCallRounds")
        .and_then(|v| v.as_array())
    else {
        return Vec::new();
    };
    let mut tools = Vec::new();
    for round in rounds {
        for key in ["toolName", "name", "tool"] {
            if let Some(name) = round.get(key).and_then(|v| v.as_str()) {
                if !name.is_empty() {
                    tools.push(normalize_tool(name));
                    break;
                }
            }
        }
        for nested_key in ["tools", "toolCalls", "toolRequests"] {
            if let Some(nested) = round.get(nested_key).and_then(|v| v.as_array()) {
                for item in nested {
                    for key in ["toolName", "name", "tool"] {
                        if let Some(name) = item.get(key).and_then(|v| v.as_str()) {
                            if !name.is_empty() {
                                tools.push(normalize_tool(name));
                                break;
                            }
                        }
                    }
                }
            }
        }
    }
    tools
}

struct UsageEvent {
    id: i64,
    turn_index: Option<i64>,
    model: String,
    input_tokens: u64,
    output_tokens: u64,
    cache_read_tokens: u64,
    cache_write_tokens: u64,
    reasoning_tokens: u64,
    timestamp: Option<DateTime<Utc>>,
    project: String,
}

/// Per-request usage rows written by newer CLI builds. Older stores have no
/// `assistant_usage_events` table, which surfaces as a prepare error and an
/// empty map — the caller then keeps the chars/4 estimate path.
fn load_usage_events(
    conn: &Connection,
    fallback_project: &str,
) -> HashMap<String, Vec<UsageEvent>> {
    let Ok(mut stmt) = conn.prepare(config::CLI_USAGE_EVENTS_SQL) else {
        return HashMap::new();
    };
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<i64>>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, Option<i64>>(4)?,
            row.get::<_, Option<i64>>(5)?,
            row.get::<_, Option<i64>>(6)?,
            row.get::<_, Option<i64>>(7)?,
            row.get::<_, Option<i64>>(8)?,
            row.get::<_, Option<String>>(9)?,
            row.get::<_, Option<String>>(10)?,
            row.get::<_, Option<String>>(11)?,
        ))
    });
    let Ok(rows) = rows else {
        return HashMap::new();
    };

    let mut events: HashMap<String, Vec<UsageEvent>> = HashMap::new();
    for row in rows.flatten() {
        let (
            id,
            session_id,
            turn_index,
            model,
            input,
            output,
            cache_read,
            cache_write,
            reasoning,
            created_at,
            cwd,
            repository,
        ) = row;
        let project = store_project(cwd.as_deref(), repository.as_deref(), fallback_project);
        events.entry(session_id).or_default().push(UsageEvent {
            id,
            turn_index,
            model: model.unwrap_or_default(),
            input_tokens: input.unwrap_or(0).max(0) as u64,
            output_tokens: output.unwrap_or(0).max(0) as u64,
            cache_read_tokens: cache_read.unwrap_or(0).max(0) as u64,
            cache_write_tokens: cache_write.unwrap_or(0).max(0) as u64,
            reasoning_tokens: reasoning.unwrap_or(0).max(0) as u64,
            timestamp: created_at.as_deref().and_then(parse_store_timestamp),
            project,
        });
    }
    events
}

fn usage_dedup_key(session_id: &str, event: &UsageEvent) -> String {
    // Encoding the covered turn in the key lets the archive zero exactly the
    // superseded chars/4 estimate row (`copilot:<sid>:turn-<idx>`).
    match event.turn_index {
        Some(turn) => format!(
            "copilot:{session_id}{}{turn}{}{}",
            config::CLI_TURN_DEDUP_MARKER,
            config::CLI_USAGE_DEDUP_MARKER,
            event.id
        ),
        None => format!(
            "copilot:{session_id}{}{}",
            config::CLI_USAGE_DEDUP_MARKER,
            event.id
        ),
    }
}

fn usage_event_calls(
    events_by_session: &HashMap<String, Vec<UsageEvent>>,
    turn_messages: &HashMap<(String, i64), String>,
    seen: &mut HashSet<String>,
) -> Vec<ParsedCall> {
    let mut sessions: Vec<_> = events_by_session.iter().collect();
    sessions.sort_by(|a, b| a.0.cmp(b.0));

    let mut calls = Vec::new();
    for (session_id, events) in sessions {
        let mut message_taken: HashSet<i64> = HashSet::new();
        for event in events {
            let dedup_key = usage_dedup_key(session_id, event);
            if !seen.insert(dedup_key.clone()) {
                continue;
            }

            let user_message = event
                .turn_index
                .filter(|turn| message_taken.insert(*turn))
                .and_then(|turn| turn_messages.get(&(session_id.clone(), turn)))
                .map(|message| truncate(message, 500))
                .unwrap_or_default();
            let model = if event.model.trim().is_empty() {
                COPILOT_AUTO.to_string()
            } else {
                event.model.clone()
            };
            // The store's input_tokens column includes cache reads (verified
            // against token_details_json); subtract before pricing, the same
            // convention as the data.db aggregates.
            let cache_read = event.cache_read_tokens.min(event.input_tokens);
            let input_tokens = event.input_tokens - cache_read;

            let mut call = ParsedCall {
                tool: config::TOOL_ID,
                model: model.clone(),
                input_tokens,
                output_tokens: event.output_tokens,
                cache_creation_input_tokens: event.cache_write_tokens,
                cache_read_input_tokens: cache_read,
                cached_input_tokens: cache_read,
                reasoning_tokens: event.reasoning_tokens,
                speed: Speed::Standard,
                timestamp: event.timestamp,
                dedup_key,
                user_message,
                session_id: session_id.clone(),
                project: event.project.clone(),
                ..ParsedCall::default()
            };
            call.cost_usd = pricing::cost(&model, &call, Speed::Standard);
            calls.push(call);
        }
    }
    calls
}

fn parse_session_store(
    path: &Path,
    source: &SessionSource,
    seen: &mut HashSet<String>,
) -> Vec<ParsedCall> {
    let Some(store) = StoreCopy::new(path) else {
        return Vec::new();
    };
    let Ok(conn) = store.open() else {
        return Vec::new();
    };

    let usage_events = load_usage_events(&conn, &source.project);

    let Ok(mut stmt) = conn.prepare(config::CLI_TURNS_SQL) else {
        return usage_event_calls(&usage_events, &HashMap::new(), seen);
    };
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, Option<String>>(4)?,
            row.get::<_, Option<String>>(5)?,
            row.get::<_, Option<String>>(6)?,
        ))
    });
    let Ok(rows) = rows else {
        return usage_event_calls(&usage_events, &HashMap::new(), seen);
    };
    let turn_rows: Vec<_> = rows.flatten().collect();

    let mut turn_messages: HashMap<(String, i64), String> = HashMap::new();
    for (session_id, turn_index, user_message, ..) in &turn_rows {
        if let Some(message) = user_message {
            if !message.trim().is_empty() {
                turn_messages
                    .entry((session_id.clone(), *turn_index))
                    .or_insert_with(|| message.clone());
            }
        }
    }
    let evented_turns: HashSet<(String, i64)> = usage_events
        .iter()
        .flat_map(|(session_id, events)| {
            events
                .iter()
                .filter_map(|event| event.turn_index.map(|turn| (session_id.clone(), turn)))
        })
        .collect();

    let mut calls = usage_event_calls(&usage_events, &turn_messages, seen);

    for row in turn_rows {
        let (session_id, turn_index, user_message, assistant_response, timestamp, cwd, repository) =
            row;
        // Turns covered by real usage rows would double count as estimates.
        if evented_turns.contains(&(session_id.clone(), turn_index)) {
            continue;
        }
        let user_message = user_message.unwrap_or_default();
        let assistant_response = assistant_response.unwrap_or_default();
        if user_message.trim().is_empty() && assistant_response.trim().is_empty() {
            continue;
        }

        let dedup_key = format!("copilot:{session_id}:turn-{turn_index}");
        if !seen.insert(dedup_key.clone()) {
            continue;
        }

        let input_tokens = ((user_message.len() as f64) / CHARS_PER_TOKEN).ceil() as u64;
        let output_tokens = ((assistant_response.len() as f64) / CHARS_PER_TOKEN).ceil() as u64;
        let project = store_project(cwd.as_deref(), repository.as_deref(), &source.project);
        let model = COPILOT_AUTO.to_string();

        let mut call = ParsedCall {
            tool: config::TOOL_ID,
            model: model.clone(),
            input_tokens,
            output_tokens,
            speed: Speed::Standard,
            timestamp: timestamp.as_deref().and_then(parse_store_timestamp),
            dedup_key,
            user_message: truncate(&user_message, 500),
            session_id,
            project,
            ..ParsedCall::default()
        };
        call.cost_usd = pricing::cost(&model, &call, Speed::Standard);
        calls.push(call);
    }
    calls
}

/// A signed-in account from the Copilot CLI's data.db `accounts` table. The
/// CLI itself stores the access token in plaintext; tokenuse only reads it in
/// memory for the explicit quota sync and never rewrites it anywhere except
/// the request Authorization header.
#[cfg(feature = "quota-sync")]
pub(crate) struct CliAccount {
    pub login: String,
    pub host: String,
    pub token: String,
}

#[cfg(feature = "quota-sync")]
pub(crate) fn cli_accounts(data_db_path: &Path) -> Vec<CliAccount> {
    let Some(store) = StoreCopy::new(data_db_path) else {
        return Vec::new();
    };
    let Ok(conn) = store.open() else {
        return Vec::new();
    };
    let Ok(mut stmt) = conn.prepare(config::CLI_ACCOUNTS_SQL) else {
        return Vec::new();
    };
    let Ok(rows) = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    }) else {
        return Vec::new();
    };
    rows.flatten()
        .map(|(login, host, token)| CliAccount { login, host, token })
        .collect()
}

/// Session ids with per-request usage rows in the sibling session-store.db.
/// Their data.db aggregates describe the same requests and would double count
/// (verified: both stores share session ids and identical token totals).
fn sibling_usage_session_ids(data_db_path: &Path) -> HashSet<String> {
    let Some(dir) = data_db_path.parent() else {
        return HashSet::new();
    };
    let store_path = dir.join(config::CLI_SESSION_STORE_FILE);
    if !store_path.is_file() {
        return HashSet::new();
    }
    let Some(store) = StoreCopy::new(&store_path) else {
        return HashSet::new();
    };
    let Ok(conn) = store.open() else {
        return HashSet::new();
    };
    let Ok(mut stmt) = conn.prepare(config::CLI_USAGE_SESSIONS_SQL) else {
        return HashSet::new();
    };
    let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(0)) else {
        return HashSet::new();
    };
    rows.flatten().collect()
}

fn parse_data_store(
    path: &Path,
    source: &SessionSource,
    seen: &mut HashSet<String>,
) -> Vec<ParsedCall> {
    let covered = sibling_usage_session_ids(path);
    let Some(store) = StoreCopy::new(path) else {
        return Vec::new();
    };
    let Ok(conn) = store.open() else {
        return Vec::new();
    };
    let Ok(mut stmt) = conn.prepare(config::CLI_APP_SESSIONS_SQL) else {
        return Vec::new();
    };
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, Option<String>>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, i64>(3)?,
            row.get::<_, i64>(4)?,
            row.get::<_, i64>(5)?,
            row.get::<_, Option<String>>(6)?,
            row.get::<_, Option<String>>(7)?,
        ))
    });
    let Ok(rows) = rows else {
        return Vec::new();
    };

    let mut calls = Vec::new();
    for row in rows.flatten() {
        let (id, model, input, output, cached, reasoning, created_at, updated_at) = row;
        if covered.contains(&id) {
            continue;
        }
        let total_input = input.max(0) as u64;
        let output_tokens = output.max(0) as u64;
        let cached_tokens = (cached.max(0) as u64).min(total_input);
        if total_input == 0 && output_tokens == 0 {
            continue;
        }

        // One aggregate call per session: data.db keeps running totals, not a
        // turn log. The dedup key stays stable so re-syncs update in place.
        let dedup_key = format!("{}{id}", config::CLI_APP_DEDUP_PREFIX);
        if !seen.insert(dedup_key.clone()) {
            continue;
        }

        let model = model
            .filter(|m| !m.trim().is_empty())
            .unwrap_or_else(|| COPILOT_AUTO.to_string());
        // Cached tokens are assumed to be reported inside the input total, the
        // same subtract-then-price convention the Codex parser uses.
        let input_tokens = total_input - cached_tokens;

        let mut call = ParsedCall {
            tool: config::TOOL_ID,
            model: model.clone(),
            input_tokens,
            output_tokens,
            cache_read_input_tokens: cached_tokens,
            cached_input_tokens: cached_tokens,
            reasoning_tokens: reasoning.max(0) as u64,
            speed: Speed::Standard,
            timestamp: updated_at
                .as_deref()
                .and_then(parse_store_timestamp)
                .or_else(|| created_at.as_deref().and_then(parse_store_timestamp)),
            dedup_key,
            session_id: id,
            project: source.project.clone(),
            ..ParsedCall::default()
        };
        call.cost_usd = pricing::cost(&model, &call, Speed::Standard);
        calls.push(call);
    }
    calls
}

fn store_project(cwd: Option<&str>, repository: Option<&str>, fallback: &str) -> String {
    if let Some(cwd) = cwd.map(str::trim) {
        if cwd.len() > 1 && cwd != "null" {
            return cwd.to_string();
        }
    }
    if let Some(repo) = repository.map(str::trim) {
        if !repo.is_empty() && repo != "null" {
            return repo.to_string();
        }
    }
    fallback.to_string()
}

fn parse_store_timestamp(s: &str) -> Option<DateTime<Utc>> {
    parse_timestamp(s).or_else(|| {
        NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
            .ok()
            .map(|dt| DateTime::from_naive_utc_and_offset(dt, Utc))
    })
}

fn transcript_cwd(lines: &[String]) -> Option<String> {
    lines.iter().find_map(|line| {
        let value = serde_json::from_str::<Value>(line).ok()?;
        if value.get("type").and_then(|t| t.as_str()) != Some("session.start") {
            return None;
        }
        value
            .pointer("/data/context/cwd")
            .and_then(|cwd| cwd.as_str())
            .filter(|cwd| !cwd.trim().is_empty())
            .map(|cwd| cwd.to_string())
    })
}

fn transcript_declared_model(events: &[Value]) -> Option<String> {
    events.iter().find_map(|event| {
        if event.get("type").and_then(|t| t.as_str()) != Some("session.start") {
            return None;
        }
        event
            .pointer("/data/model")
            .and_then(|model| model.as_str())
            .map(str::trim)
            .filter(|model| !model.is_empty() && !model.eq_ignore_ascii_case("auto"))
            .map(str::to_string)
    })
}

fn infer_model_from_tool_ids(events: &[Value]) -> String {
    let mut counts: HashMap<&'static str, u64> = HashMap::new();
    for event in events {
        if event.get("type").and_then(|v| v.as_str()) != Some("assistant.message") {
            continue;
        }
        let Some(tools) = event
            .pointer("/data/toolRequests")
            .and_then(|v| v.as_array())
        else {
            continue;
        };
        for tool in tools {
            let id = tool
                .get("toolCallId")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let model = match () {
                _ if id.starts_with("toolu_bdrk_")
                    || id.starts_with("toolu_vrtx_")
                    || id.starts_with("tooluse_") =>
                {
                    Some(ANTHROPIC_AUTO)
                }
                _ if id.starts_with("call_") => Some(OPENAI_AUTO),
                _ => None,
            };
            if let Some(m) = model {
                *counts.entry(m).or_insert(0) += 1;
            }
        }
    }
    counts
        .into_iter()
        .max_by_key(|(_, n)| *n)
        .map(|(k, _)| k.to_string())
        .unwrap_or_else(|| COPILOT_AUTO.to_string())
}

fn extract_tools(tool_requests: Option<&Vec<Value>>) -> (Vec<String>, Vec<String>) {
    let mut tools = Vec::new();
    let mut bash_commands = Vec::new();
    let Some(arr) = tool_requests else {
        return (tools, bash_commands);
    };
    for tool in arr {
        let raw_name = tool.get("name").and_then(|v| v.as_str()).unwrap_or("");
        if raw_name.is_empty() {
            continue;
        }
        if matches!(raw_name, "bash" | "run_in_terminal" | "kill_terminal") {
            if let Some(args_str) = tool.get("arguments").and_then(|v| v.as_str()) {
                if let Ok(args) = serde_json::from_str::<Value>(args_str) {
                    let cmd = args
                        .get("command")
                        .and_then(|v| v.as_str())
                        .or_else(|| args.get("cmd").and_then(|v| v.as_str()));
                    if let Some(c) = cmd {
                        bash_commands.extend(jsonl::split_bash_commands(c));
                    }
                }
            }
        }
        tools.push(normalize_tool(raw_name));
    }
    (tools, bash_commands)
}

fn normalize_tool(name: &str) -> String {
    let mapped = match name {
        "bash" | "run_in_terminal" | "kill_terminal" => "Bash",
        "read_file" => "Read",
        "write_file" | "edit_file" | "replace_string_in_file" | "apply_patch" => "Edit",
        "create_file" => "Write",
        "delete_file" => "Delete",
        "search_files" | "file_search" => "Grep",
        "find_files" => "Glob",
        "list_directory" | "list_dir" => "LS",
        "web_search" => "WebSearch",
        "fetch_webpage" => "WebFetch",
        "github_repo" => "GitHub",
        "memory" => "Memory",
        other => return other.to_string(),
    };
    mapped.to_string()
}

fn parse_timestamp(s: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|d| d.with_timezone(&Utc))
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    s.chars().take(max).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::CodeBlock;
    use std::io::Write;
    use std::path::{Path, PathBuf};

    struct TempDir(PathBuf);
    impl TempDir {
        fn new() -> Self {
            let p = std::env::temp_dir().join(format!(
                "tokenuse-copilot-{}",
                crate::tools::paths::test_run_id()
            ));
            std::fs::create_dir_all(&p).unwrap();
            Self(p)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn write_lines(path: &Path, lines: &[&str]) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        let mut f = std::fs::File::create(path).unwrap();
        for line in lines {
            writeln!(f, "{}", line).unwrap();
        }
    }

    #[test]
    fn parses_legacy_events() {
        let dir = TempDir::new();
        let session_id = "sess-abc";
        let session_dir = dir.path().join(session_id);
        std::fs::create_dir_all(&session_dir).unwrap();
        let events = session_dir.join("events.jsonl");
        std::fs::write(
            session_dir.join(config::WORKSPACE_FILE),
            "cwd: /Users/me/Code/aicommit\n",
        )
        .unwrap();
        write_lines(
            &events,
            &[
                r#"{"type":"session.model_change","timestamp":"2026-04-26T10:00:00Z","data":{"newModel":"claude-sonnet-4-5"}}"#,
                r#"{"type":"user.message","timestamp":"2026-04-26T10:00:01Z","data":{"content":"fix the typo"}}"#,
                r#"{"type":"assistant.message","timestamp":"2026-04-26T10:00:02Z","data":{"messageId":"m1","outputTokens":220,"toolRequests":[{"toolCallId":"tooluse_xyz","name":"bash","arguments":"{\"command\":\"ls -la | wc -l\"}"},{"toolCallId":"tooluse_yyy","name":"edit_file"}]}}"#,
            ],
        );

        let source = SessionSource::session(events.clone(), "demo", config::TOOL_ID);
        let mut seen = HashSet::new();
        let calls = parse_session(&source, &mut seen).unwrap();
        assert_eq!(calls.len(), 1);
        let call = &calls[0];
        assert_eq!(call.model, "claude-sonnet-4-5");
        assert_eq!(call.output_tokens, 220);
        assert_eq!(call.session_id, session_id);
        assert_eq!(call.tools, vec!["Bash", "Edit"]);
        assert_eq!(call.bash_commands, vec!["ls -la", "wc -l"]);
        assert_eq!(call.user_message, "fix the typo");
        assert_eq!(call.project, "/Users/me/Code/aicommit");
        assert!(call.cost_usd > 0.0);
        assert_eq!(call.dedup_key, format!("copilot:{}:m1", session_id));
        assert_eq!(call.prompt_chars, Some(12), "full user text length");
        assert_eq!(
            call.response_chars, None,
            "legacy events never read assistant content"
        );
        assert!(call.code_blocks.is_empty());
        assert!(!call.is_canceled);
        assert_eq!(call.elapsed_ms, None);
        assert!(call.edited_files.is_empty());
        assert!(call.referenced_files.is_empty());

        // dedup
        let again = parse_session(&source, &mut seen).unwrap();
        assert!(again.is_empty());
    }

    fn create_otel_db(path: &Path) -> Connection {
        let conn = Connection::open(path).unwrap();
        conn.execute_batch(
            "
            CREATE TABLE spans (
                span_id        TEXT    PRIMARY KEY NOT NULL,
                trace_id       TEXT    NOT NULL,
                operation_name TEXT,
                start_time_ms  INTEGER NOT NULL DEFAULT 0,
                response_model TEXT
            );
            CREATE TABLE span_attributes (
                id      INTEGER PRIMARY KEY AUTOINCREMENT,
                span_id TEXT    NOT NULL,
                key     TEXT    NOT NULL,
                value   TEXT
            );
            ",
        )
        .unwrap();
        conn
    }

    fn insert_span(
        conn: &Connection,
        span_id: &str,
        trace_id: &str,
        operation: &str,
        start_ms: i64,
        attrs: &[(&str, &str)],
    ) {
        conn.execute(
            "INSERT INTO spans (span_id, trace_id, operation_name, start_time_ms, response_model)
             VALUES (?1, ?2, ?3, ?4, NULL)",
            rusqlite::params![span_id, trace_id, operation, start_ms],
        )
        .unwrap();
        for (key, value) in attrs {
            conn.execute(
                "INSERT INTO span_attributes (span_id, key, value) VALUES (?1, ?2, ?3)",
                rusqlite::params![span_id, key, value],
            )
            .unwrap();
        }
    }

    #[test]
    fn parses_otel_traces_with_real_token_counts() {
        let dir = TempDir::new();
        let db = dir.path().join(config::OTEL_TRACES_FILE);
        {
            let conn = create_otel_db(&db);
            insert_span(
                &conn,
                "span-1",
                "trace-1",
                "chat",
                1_780_157_113_020,
                &[
                    ("gen_ai.conversation.id", "conv-1"),
                    ("gen_ai.response.model", "claude-sonnet-4-6"),
                    ("gen_ai.usage.input_tokens", "1200"),
                    ("gen_ai.usage.output_tokens", "340"),
                    ("gen_ai.usage.cache_read.input_tokens", "5000"),
                    ("gen_ai.usage.cache_creation.input_tokens", "900"),
                    ("github.copilot.git.repository", "russmckendrick/tokens.git"),
                    ("github.copilot.chat.turn.id", "turn-1"),
                ],
            );
            insert_span(
                &conn,
                "span-2",
                "trace-1",
                "execute_tool",
                1_780_157_114_000,
                &[
                    ("gen_ai.tool.name", "run_in_terminal"),
                    (
                        "gen_ai.tool.call.arguments",
                        r#"{"command":"cargo fmt\ncargo test"}"#,
                    ),
                ],
            );
            insert_span(
                &conn,
                "span-3",
                "trace-1",
                "invoke_agent",
                1_780_157_114_500,
                &[
                    ("gen_ai.agent.name", "test-writer"),
                    ("copilot_chat.parent_chat_session_id", "conv-1"),
                ],
            );
            // Zero-usage chat spans carry no billing.
            insert_span(
                &conn,
                "span-4",
                "trace-2",
                "chat",
                1_780_157_115_000,
                &[
                    ("gen_ai.conversation.id", "conv-1"),
                    ("gen_ai.usage.input_tokens", "0"),
                    ("gen_ai.usage.output_tokens", "0"),
                ],
            );
        }

        let source = SessionSource::session(db, config::OTEL_PROJECT_LABEL, config::TOOL_ID);
        let mut seen = HashSet::new();
        let calls = parse_session(&source, &mut seen).unwrap();

        assert_eq!(calls.len(), 1, "zero-usage chat spans are skipped");
        let call = &calls[0];
        assert_eq!(call.input_tokens, 1200);
        assert_eq!(call.output_tokens, 340);
        assert_eq!(call.cache_read_input_tokens, 5000);
        assert_eq!(call.cache_creation_input_tokens, 900);
        assert_eq!(call.model, "claude-sonnet-4-6");
        assert_eq!(call.session_id, "conv-1");
        assert_eq!(call.project, "tokens", "repository attr, .git stripped");
        assert_eq!(call.dedup_key, "copilot-otel:span-1");
        assert_eq!(call.tools, vec!["Bash", "Agent"]);
        assert_eq!(call.bash_commands, vec!["cargo fmt", "cargo test"]);
        assert_eq!(
            call.superseded_dedup_keys,
            vec!["copilot:conv-1:turn-1".to_string()],
            "the transcript-style key rides along for archive zeroing"
        );
        assert!(call.cost_usd > 0.0);
        assert!(
            seen.contains("copilot:conv-1:turn-1"),
            "the matching transcript turn is claimed so later sources skip it"
        );
    }

    #[test]
    fn otel_suppresses_the_same_turn_arriving_via_transcripts() {
        let dir = TempDir::new();
        let db = dir.path().join(config::OTEL_TRACES_FILE);
        {
            let conn = create_otel_db(&db);
            insert_span(
                &conn,
                "span-1",
                "trace-1",
                "chat",
                1_780_157_113_020,
                &[
                    ("gen_ai.conversation.id", "conv-1"),
                    ("gen_ai.response.model", "gpt-5"),
                    ("gen_ai.usage.input_tokens", "100"),
                    ("gen_ai.usage.output_tokens", "50"),
                    ("github.copilot.chat.turn.id", "m1"),
                ],
            );
        }
        let transcript = dir.path().join("conv-1.jsonl");
        write_lines(
            &transcript,
            &[
                r#"{"type":"session.start","data":{"sessionId":"conv-1","producer":"copilot-agent","model":"gpt-5"}}"#,
                r#"{"type":"user.message","data":{"content":"hello"}}"#,
                r#"{"type":"assistant.message","data":{"messageId":"m1","content":"hi there","toolRequests":[{"toolCallId":"call_1","name":"read_file"}]}}"#,
            ],
        );

        let mut seen = HashSet::new();
        let otel_calls = parse_session(
            &SessionSource::session(db, config::OTEL_PROJECT_LABEL, config::TOOL_ID),
            &mut seen,
        )
        .unwrap();
        let transcript_calls = parse_session(
            &SessionSource::session(transcript, "demo", config::TOOL_ID),
            &mut seen,
        )
        .unwrap();

        assert_eq!(otel_calls.len(), 1);
        assert!(
            transcript_calls.is_empty(),
            "the transcript's estimate of the same turn must not double count"
        );
    }

    #[test]
    fn parses_chat_session_journals() {
        let dir = TempDir::new();
        let chat_dir = dir.path().join(config::CHAT_SESSIONS_DIR);
        let journal = chat_dir.join("chat-session-1.jsonl");
        write_lines(
            &journal,
            &[
                r#"{"kind":0,"v":{"version":3,"creationDate":1780157113020,"sessionId":"chat-session-1","requests":[]}}"#,
                r#"{"kind":1,"k":["customTitle"],"v":"Fix the parser"}"#,
                r#"{"kind":2,"k":["requests"],"v":[{"requestId":"request_8c8ce017","modelId":"copilot/claude-sonnet-4.6","completionTokens":490,"message":{"text":"fix the failing test"},"result":{"metadata":{"promptTokens":32543,"outputTokens":60,"resolvedModel":"claude-sonnet-4-6","toolCallRounds":[{"toolName":"read_file"}]}}}]}"#,
                r#"{"kind":2,"v":[{"requestId":"request_zero","result":{"metadata":{"promptTokens":0,"outputTokens":0}}}]}"#,
            ],
        );

        let source = SessionSource::session(chat_dir, "tokens", config::TOOL_ID);
        let mut seen = HashSet::new();
        let calls = parse_session(&source, &mut seen).unwrap();

        assert_eq!(calls.len(), 1, "zero-token requests are skipped");
        let call = &calls[0];
        assert_eq!(call.input_tokens, 32543);
        assert_eq!(
            call.output_tokens, 60,
            "metadata outputTokens wins over completionTokens"
        );
        assert_eq!(call.model, "claude-sonnet-4-6");
        assert_eq!(
            call.dedup_key,
            "copilot-chatsession:chat-session-1:request_8c8ce017"
        );
        assert_eq!(call.session_id, "chat-session-1");
        assert_eq!(call.project, "tokens");
        assert_eq!(call.tools, vec!["Read"]);
        assert_eq!(call.user_message, "fix the failing test");
        assert!(call.timestamp.is_some(), "falls back to root creationDate");
        assert!(call.cost_usd > 0.0);

        let again = parse_session(&source, &mut seen).unwrap();
        assert!(again.is_empty(), "request ids dedup across reparses");
    }

    #[test]
    fn legacy_shutdown_rollup_emits_input_only_calls() {
        let dir = TempDir::new();
        let session_dir = dir.path().join("sess-roll");
        let events = session_dir.join("events.jsonl");
        write_lines(
            &events,
            &[
                r#"{"type":"session.model_change","timestamp":"2026-04-15T10:00:00Z","data":{"newModel":"gpt-5"}}"#,
                r#"{"type":"assistant.message","timestamp":"2026-04-15T10:00:10Z","data":{"messageId":"m1","outputTokens":220,"toolRequests":[]}}"#,
                r#"{"type":"session.shutdown","timestamp":"2026-04-15T10:05:00Z","data":{"shutdownType":"routine","sessionStartTime":1784102040274,"modelMetrics":{"gpt-5":{"requests":{"count":1,"cost":1},"usage":{"inputTokens":1000,"outputTokens":999,"cacheReadTokens":300,"cacheWriteTokens":200,"reasoningTokens":0}},"idle-model":{"usage":{"inputTokens":0,"outputTokens":0,"cacheReadTokens":0,"cacheWriteTokens":0}}}}}"#,
            ],
        );

        let source = SessionSource::session(events, "demo", config::TOOL_ID);
        let mut seen = HashSet::new();
        let calls = parse_session(&source, &mut seen).unwrap();

        assert_eq!(calls.len(), 2, "per-turn output call plus one rollup");
        let rollup = calls
            .iter()
            .find(|c| c.dedup_key == "copilot:sess-roll:shutdown:gpt-5")
            .expect("shutdown rollup call");
        assert_eq!(
            rollup.input_tokens, 500,
            "inputTokens is cache-inclusive: 1000 - 300 - 200"
        );
        assert_eq!(
            rollup.output_tokens, 0,
            "output stays with the per-turn call"
        );
        assert_eq!(rollup.cache_read_input_tokens, 300);
        assert_eq!(rollup.cache_creation_input_tokens, 200);
        assert!(rollup.cost_usd > 0.0);
        assert!(
            !calls.iter().any(|c| c.dedup_key.contains("idle-model")),
            "all-zero model rollups are skipped"
        );
    }

    #[test]
    fn legacy_skips_zero_output_tokens() {
        let dir = TempDir::new();
        let events = dir.path().join("sess-zero").join("events.jsonl");
        write_lines(
            &events,
            &[
                r#"{"type":"session.model_change","data":{"newModel":"claude-sonnet-4-5"}}"#,
                r#"{"type":"assistant.message","data":{"messageId":"m1","outputTokens":0}}"#,
            ],
        );
        let source = SessionSource::session(events, "demo", config::TOOL_ID);
        let mut seen = HashSet::new();
        let calls = parse_session(&source, &mut seen).unwrap();
        assert!(calls.is_empty());
    }

    #[test]
    fn parses_workspace_cwd_from_yaml_line() {
        assert_eq!(
            parse_workspace_cwd_line(r#"cwd: "/Users/me/Code/tokens""#).as_deref(),
            Some("/Users/me/Code/tokens")
        );
        assert_eq!(parse_workspace_cwd_line("cwd: null"), None);
    }

    #[test]
    fn parses_transcript_with_anthropic_inference() {
        let dir = TempDir::new();
        let uuid = "11111111-2222-3333-4444-555555555555";
        let transcript = dir.path().join(format!("{}.jsonl", uuid));
        write_lines(
            &transcript,
            &[
                r#"{"type":"session.start","data":{"sessionId":"x","producer":"copilot-agent","model":"gpt-5","context":{"cwd":"/Users/me/Code/tokens"}}}"#,
                r#"{"type":"user.message","data":{"content":"hello world"}}"#,
                r#"{"type":"assistant.message","data":{"messageId":"abc","content":"sure thing","reasoningText":"let me think","toolRequests":[{"toolCallId":"toolu_bdrk_01ZZ","name":"read_file"},{"toolCallId":"toolu_bdrk_02YY","name":"edit_file"}]}}"#,
                r#"{"type":"assistant.message","data":{"messageId":"def","content":"ok","toolRequests":[{"toolCallId":"call_999","name":"web_search"}]}}"#,
            ],
        );

        let source = SessionSource::session(transcript.clone(), "vscode-ws", config::TOOL_ID);
        let mut seen = HashSet::new();
        let calls = parse_session(&source, &mut seen).unwrap();
        assert_eq!(calls.len(), 2);

        // Anthropic prefixes outnumber OpenAI 2:1, so model inference picks anthropic-auto.
        assert_eq!(calls[0].model, "anthropic-auto");
        assert_eq!(calls[1].model, "anthropic-auto");
        assert_eq!(calls[0].session_id, uuid);
        assert_eq!(calls[0].dedup_key, format!("copilot:{}:abc", uuid));
        assert_eq!(calls[0].project, "/Users/me/Code/tokens");
        assert_eq!(calls[0].tools, vec!["Read", "Edit"]);
        assert_eq!(calls[1].tools, vec!["WebSearch"]);
        assert_eq!(calls[0].output_tokens, 6);
        assert_eq!(calls[0].reasoning_tokens, 3);
        assert!(
            calls[0].input_tokens > 0,
            "input estimated from user message length"
        );
        assert!(calls[0].output_tokens > 0);
        assert!(calls[0].reasoning_tokens > 0);
        assert!(calls[0].cost_usd > 0.0);
        assert_eq!(calls[0].prompt_chars, Some(11), "\"hello world\" length");
        assert_eq!(
            calls[0].response_chars,
            Some(10),
            "content only; reasoningText excluded"
        );
        assert_eq!(
            calls[1].prompt_chars, None,
            "the user message attaches only to the first assistant message"
        );
        assert_eq!(calls[1].response_chars, Some(2));
    }

    #[test]
    fn transcript_enrichment_measures_full_prompt_and_fences() {
        let dir = TempDir::new();
        let transcript = dir
            .path()
            .join("22222222-3333-4444-5555-666666666666.jsonl");
        let long_prompt = "p".repeat(600);
        let user_line =
            format!(r#"{{"type":"user.message","data":{{"content":"{long_prompt}"}}}}"#);
        write_lines(
            &transcript,
            &[
                r#"{"type":"session.start","data":{"producer":"copilot-agent","context":{"cwd":"/Users/me/Code/tokens"}}}"#,
                &user_line,
                r#"{"type":"assistant.message","data":{"messageId":"m1","content":"Sure:\n```rust\nfn a() {}\nfn b() {}\n```"}}"#,
            ],
        );

        let source = SessionSource::session(transcript, "ws", config::TOOL_ID);
        let mut seen = HashSet::new();
        let calls = parse_session(&source, &mut seen).unwrap();

        assert_eq!(calls.len(), 1);
        let call = &calls[0];
        assert_eq!(
            call.user_message.chars().count(),
            500,
            "display text stays truncated"
        );
        assert_eq!(call.prompt_chars, Some(600), "measured before truncation");
        assert_eq!(
            call.response_chars,
            Some("Sure:\n```rust\nfn a() {}\nfn b() {}\n```".chars().count() as u64)
        );
        assert_eq!(
            call.code_blocks,
            vec![CodeBlock {
                language: "rust".into(),
                loc: 2,
            }]
        );
        assert!(!call.is_canceled);
        assert_eq!(call.elapsed_ms, None, "turn latency is not derived");
        assert!(call.edited_files.is_empty());
        assert!(call.referenced_files.is_empty());
    }

    #[test]
    fn declared_model_used_when_no_tool_id_prefix_is_recognized() {
        let dir = TempDir::new();
        let transcript = dir
            .path()
            .join("99999999-8888-7777-6666-555555555555.jsonl");
        write_lines(
            &transcript,
            &[
                r#"{"type":"session.start","data":{"producer":"copilot-agent","model":"Claude Sonnet 4.5","context":{"cwd":"/Users/me/Code/tokens"}}}"#,
                r#"{"type":"user.message","data":{"content":"hello"}}"#,
                r#"{"type":"assistant.message","data":{"messageId":"m1","content":"hi there","toolRequests":[{"toolCallId":"mystery_123","name":"read_file"}]}}"#,
            ],
        );

        let source = SessionSource::session(transcript, "vscode-ws", config::TOOL_ID);
        let mut seen = HashSet::new();
        let calls = parse_session(&source, &mut seen).unwrap();

        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].model, "Claude Sonnet 4.5");
        assert!(
            calls[0].cost_usd > 0.0,
            "display name resolves via copilot tool aliases"
        );
    }

    #[test]
    fn parses_cli_session_store_turns_including_wal_rows() {
        let dir = TempDir::new();
        let db_path = dir.path().join(config::CLI_SESSION_STORE_FILE);
        let conn = Connection::open(&db_path).unwrap();
        conn.pragma_update(None, "journal_mode", "WAL").unwrap();
        conn.execute_batch(
            "CREATE TABLE sessions (id TEXT PRIMARY KEY, cwd TEXT, repository TEXT, host_type TEXT, branch TEXT, summary TEXT, created_at TEXT, updated_at TEXT);
             CREATE TABLE turns (id INTEGER PRIMARY KEY AUTOINCREMENT, session_id TEXT NOT NULL, turn_index INTEGER NOT NULL, user_message TEXT, assistant_response TEXT, timestamp TEXT);
             INSERT INTO sessions (id, cwd) VALUES ('sess-1', '/Users/me/Code/tokens');
             INSERT INTO sessions (id, cwd, repository) VALUES ('sess-2', '/', 'russmckendrick/tokens');
             INSERT INTO turns (session_id, turn_index, user_message, assistant_response, timestamp) VALUES ('sess-1', 0, 'hello there', 'general kenobi, nice to see you', '2026-07-04T07:20:25.254Z');
             INSERT INTO turns (session_id, turn_index, user_message, assistant_response, timestamp) VALUES ('sess-2', 0, 'hi', 'ok', '2026-07-04 07:21:00');
             INSERT INTO turns (session_id, turn_index, user_message, assistant_response) VALUES ('sess-2', 1, '', '');",
        )
        .unwrap();

        // Keep the writer connection open so the rows stay in the -wal sidecar;
        // the parser must surface them through its private store copy.
        let source = SessionSource::session(db_path.clone(), "copilot-cli", config::TOOL_ID);
        let mut seen = HashSet::new();
        let calls = parse_session(&source, &mut seen).unwrap();
        drop(conn);

        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].model, "copilot-auto");
        assert_eq!(calls[0].project, "/Users/me/Code/tokens");
        assert_eq!(calls[0].dedup_key, "copilot:sess-1:turn-0");
        assert_eq!(calls[0].session_id, "sess-1");
        assert!(calls[0].input_tokens > 0);
        assert!(calls[0].output_tokens > 0);
        assert!(calls[0].cost_usd > 0.0);
        assert!(calls[0].timestamp.is_some());
        assert_eq!(
            calls[0].prompt_chars, None,
            "chars/4 estimate rows carry no enrichment"
        );
        assert_eq!(calls[0].response_chars, None);
        assert!(calls[0].code_blocks.is_empty());
        assert_eq!(calls[1].project, "russmckendrick/tokens");
        assert!(
            calls[1].timestamp.is_some(),
            "sqlite datetime format parses"
        );

        let again = parse_session(&source, &mut seen).unwrap();
        assert!(again.is_empty());
    }

    #[test]
    fn parses_cli_data_store_session_totals() {
        let dir = TempDir::new();
        let db_path = dir.path().join(config::CLI_DATA_STORE_FILE);
        let conn = Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE sessions (id TEXT PRIMARY KEY, model TEXT, total_input_tokens INTEGER NOT NULL DEFAULT 0, total_output_tokens INTEGER NOT NULL DEFAULT 0, total_cached_tokens INTEGER NOT NULL DEFAULT 0, total_reasoning_tokens INTEGER NOT NULL DEFAULT 0, created_at TEXT, updated_at TEXT);
             INSERT INTO sessions VALUES ('app-1', 'claude-sonnet-4.5', 12000, 3000, 10000, 500, '2026-07-01T10:00:00.000Z', '2026-07-01T11:00:00.000Z');
             INSERT INTO sessions VALUES ('app-empty', 'gpt-5', 0, 0, 0, 0, NULL, NULL);",
        )
        .unwrap();
        drop(conn);

        let source = SessionSource::session(db_path, "copilot-cli", config::TOOL_ID);
        let mut seen = HashSet::new();
        let calls = parse_session(&source, &mut seen).unwrap();

        assert_eq!(calls.len(), 1);
        let call = &calls[0];
        assert_eq!(call.model, "claude-sonnet-4.5");
        assert_eq!(
            call.input_tokens, 2_000,
            "cached tokens subtracted from input"
        );
        assert_eq!(call.cache_read_input_tokens, 10_000);
        assert_eq!(call.cached_input_tokens, 10_000);
        assert_eq!(call.output_tokens, 3_000);
        assert_eq!(call.reasoning_tokens, 500);
        assert_eq!(call.dedup_key, "copilot:cli:app-1");
        assert_eq!(call.project, "copilot-cli");
        assert!(call.cost_usd > 0.0);
        assert_eq!(
            call.timestamp.map(|ts| ts.to_rfc3339()),
            Some("2026-07-01T11:00:00+00:00".to_string())
        );
        assert_eq!(
            call.prompt_chars, None,
            "data.db aggregate rows carry no enrichment"
        );
        assert_eq!(call.response_chars, None);
    }

    #[test]
    fn usage_events_supersede_turn_estimates() {
        let dir = TempDir::new();
        let db_path = dir.path().join(config::CLI_SESSION_STORE_FILE);
        let conn = Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE sessions (id TEXT PRIMARY KEY, cwd TEXT, repository TEXT, host_type TEXT, branch TEXT, summary TEXT, created_at TEXT, updated_at TEXT);
             CREATE TABLE turns (id INTEGER PRIMARY KEY AUTOINCREMENT, session_id TEXT NOT NULL, turn_index INTEGER NOT NULL, user_message TEXT, assistant_response TEXT, timestamp TEXT);
             CREATE TABLE assistant_usage_events (id INTEGER PRIMARY KEY AUTOINCREMENT, session_id TEXT NOT NULL, turn_index INTEGER, agent_id TEXT, parent_tool_call_id TEXT, model TEXT NOT NULL, input_tokens INTEGER, output_tokens INTEGER, cache_read_tokens INTEGER, cache_write_tokens INTEGER, reasoning_tokens INTEGER, total_nano_aiu INTEGER, request_multiplier REAL, duration_ms INTEGER, time_to_first_token_ms INTEGER, inter_token_latency_ms INTEGER, initiator TEXT, api_endpoint TEXT, reasoning_effort TEXT, finish_reason TEXT, content_filter_triggered INTEGER, token_details_json TEXT, created_at TEXT);
             INSERT INTO sessions (id, cwd) VALUES ('sess-ev', '/Users/me/Code/tokens');
             INSERT INTO turns (session_id, turn_index, user_message, assistant_response, timestamp) VALUES ('sess-ev', 0, 'fix the bug', 'done', '2026-07-11T13:43:23.000Z');
             INSERT INTO turns (session_id, turn_index, user_message, assistant_response, timestamp) VALUES ('sess-ev', 1, 'turn without events', 'estimated reply', '2026-07-11T13:50:00.000Z');
             INSERT INTO assistant_usage_events (session_id, turn_index, model, input_tokens, output_tokens, cache_read_tokens, cache_write_tokens, reasoning_tokens, created_at) VALUES ('sess-ev', 0, 'gpt-5.3-codex', 27803, 122, 27392, 0, 44, '2026-07-11T13:43:25.000Z');
             INSERT INTO assistant_usage_events (session_id, turn_index, model, input_tokens, output_tokens, cache_read_tokens, cache_write_tokens, reasoning_tokens, created_at) VALUES ('sess-ev', 0, 'gpt-5.3-codex', 28034, 233, 27776, 128, 168, '2026-07-11T13:44:00.000Z');",
        )
        .unwrap();
        drop(conn);

        let source = SessionSource::session(db_path, "copilot-cli", config::TOOL_ID);
        let mut seen = HashSet::new();
        let calls = parse_session(&source, &mut seen).unwrap();

        assert_eq!(
            calls.len(),
            3,
            "two usage rows plus one uncovered estimate turn"
        );

        let first = &calls[0];
        assert_eq!(first.dedup_key, "copilot:sess-ev:turn-0:usage-1");
        assert_eq!(first.model, "gpt-5.3-codex");
        assert_eq!(first.input_tokens, 411, "cache reads subtracted from input");
        assert_eq!(first.cache_read_input_tokens, 27_392);
        assert_eq!(first.cached_input_tokens, 27_392);
        assert_eq!(first.output_tokens, 122);
        assert_eq!(first.reasoning_tokens, 44);
        assert_eq!(first.user_message, "fix the bug");
        assert_eq!(first.project, "/Users/me/Code/tokens");
        assert!(first.cost_usd > 0.0);
        assert!(first.timestamp.is_some());
        assert_eq!(
            first.prompt_chars, None,
            "usage-event rows carry no enrichment"
        );
        assert_eq!(first.response_chars, None);

        let second = &calls[1];
        assert_eq!(second.dedup_key, "copilot:sess-ev:turn-0:usage-2");
        assert_eq!(second.cache_creation_input_tokens, 128);
        assert_eq!(
            second.user_message, "",
            "the turn message attaches only to the first usage row"
        );

        let estimate = &calls[2];
        assert_eq!(estimate.dedup_key, "copilot:sess-ev:turn-1");
        assert_eq!(estimate.model, "copilot-auto");
    }

    #[cfg(feature = "quota-sync")]
    #[test]
    fn cli_accounts_reads_signed_in_hosts() {
        let dir = TempDir::new();
        let db_path = dir.path().join(config::CLI_DATA_STORE_FILE);
        let conn = Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE accounts (id TEXT PRIMARY KEY, login TEXT NOT NULL, host TEXT NOT NULL DEFAULT 'github.com', is_default INTEGER NOT NULL DEFAULT 0, access_token TEXT);
             INSERT INTO accounts VALUES ('a', 'octocat', 'github.com', 1, 'gho_personal');
             INSERT INTO accounts VALUES ('b', 'worky', 'octocorp.ghe.com', 0, 'gho_work');
             INSERT INTO accounts VALUES ('c', 'tokenless', 'github.com', 0, NULL);",
        )
        .unwrap();
        drop(conn);

        let accounts = cli_accounts(&db_path);

        assert_eq!(accounts.len(), 2);
        assert_eq!(accounts[0].login, "octocat");
        assert_eq!(accounts[0].token, "gho_personal");
        assert_eq!(accounts[1].host, "octocorp.ghe.com");
    }

    #[test]
    fn data_store_skips_sessions_covered_by_usage_events() {
        let dir = TempDir::new();
        let store_path = dir.path().join(config::CLI_SESSION_STORE_FILE);
        let conn = Connection::open(&store_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE assistant_usage_events (id INTEGER PRIMARY KEY AUTOINCREMENT, session_id TEXT NOT NULL, turn_index INTEGER, model TEXT NOT NULL, input_tokens INTEGER, output_tokens INTEGER, cache_read_tokens INTEGER, cache_write_tokens INTEGER, reasoning_tokens INTEGER, created_at TEXT);
             INSERT INTO assistant_usage_events (session_id, turn_index, model, input_tokens, output_tokens) VALUES ('app-1', 0, 'gpt-5.3-codex', 100, 10);",
        )
        .unwrap();
        drop(conn);

        let data_path = dir.path().join(config::CLI_DATA_STORE_FILE);
        let conn = Connection::open(&data_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE sessions (id TEXT PRIMARY KEY, model TEXT, total_input_tokens INTEGER NOT NULL DEFAULT 0, total_output_tokens INTEGER NOT NULL DEFAULT 0, total_cached_tokens INTEGER NOT NULL DEFAULT 0, total_reasoning_tokens INTEGER NOT NULL DEFAULT 0, created_at TEXT, updated_at TEXT);
             INSERT INTO sessions VALUES ('app-1', 'auto', 100, 10, 0, 0, '2026-07-11T13:43:15.000Z', '2026-07-11T14:00:00.000Z');
             INSERT INTO sessions VALUES ('app-2', 'gpt-5', 500, 50, 0, 0, '2026-07-01T10:00:00.000Z', '2026-07-01T11:00:00.000Z');",
        )
        .unwrap();
        drop(conn);

        let source = SessionSource::session(data_path, "copilot-cli", config::TOOL_ID);
        let mut seen = HashSet::new();
        let calls = parse_session(&source, &mut seen).unwrap();

        assert_eq!(
            calls.len(),
            1,
            "the session covered by usage events is skipped"
        );
        assert_eq!(calls[0].dedup_key, "copilot:cli:app-2");
    }

    #[test]
    fn directory_source_iterates_transcripts() {
        let dir = TempDir::new();
        let t1 = dir
            .path()
            .join("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee.jsonl");
        write_lines(
            &t1,
            &[
                r#"{"type":"session.start","data":{"producer":"copilot-agent"}}"#,
                r#"{"type":"assistant.message","data":{"messageId":"m1","content":"hi","toolRequests":[{"toolCallId":"call_1","name":"web_search"}]}}"#,
            ],
        );
        let source = SessionSource::session(dir.path().to_path_buf(), "ws", config::TOOL_ID);
        let mut seen = HashSet::new();
        let calls = parse_session(&source, &mut seen).unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].model, "openai-auto");
    }
}
