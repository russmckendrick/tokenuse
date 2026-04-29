use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use color_eyre::Result;
use rusqlite::{Connection, OpenFlags};

use crate::pricing;
use crate::providers::{ParsedCall, SessionSource, Speed};

use super::config;

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

const AGENT_KV_QUERY: &str = "
SELECT
  key,
  json_extract(value, '$.role')                              AS role,
  json_extract(value, '$.content')                           AS content,
  json_extract(value, '$.providerOptions.cursor.requestId')  AS request_id
FROM cursorDiskKV
WHERE key LIKE 'agentKv:blob:%'
  AND hex(substr(value, 1, 1)) = '7B'
ORDER BY ROWID ASC
";

pub fn parse_session(source: &SessionSource, seen: &mut HashSet<String>) -> Result<Vec<ParsedCall>> {
    let uri = format!("file:{}?immutable=1", source.path.display());
    let conn = match Connection::open_with_flags(
        &uri,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    ) {
        Ok(c) => c,
        Err(_) => return Ok(Vec::new()),
    };
    parse_with_conn(&conn, source, seen)
}

fn parse_with_conn(
    conn: &Connection,
    source: &SessionSource,
    seen: &mut HashSet<String>,
) -> Result<Vec<ParsedCall>> {
    if !validate_schema(conn) {
        return Ok(Vec::new());
    }

    let mut calls = parse_bubbles(conn, source, seen)?;
    calls.extend(parse_agent_kv(conn, source, seen)?);
    Ok(calls)
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
    source: &SessionSource,
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

        let conversation_id = row.conversation_id.clone().unwrap_or_else(|| "unknown".into());
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
        let user_message = row.user_text.unwrap_or_default();

        let mut call = ParsedCall {
            provider: config::PROVIDER_ID,
            model: display_model.clone(),
            input_tokens,
            output_tokens,
            speed: Speed::Standard,
            timestamp,
            dedup_key,
            user_message,
            session_id: conversation_id,
            project: source.project.clone(),
            ..ParsedCall::default()
        };
        call.cost_usd = pricing::cost(&display_model, &call, Speed::Standard);
        calls.push(call);
    }

    Ok(calls)
}

#[derive(Debug)]
struct AgentKvRow {
    role: Option<String>,
    content: Option<String>,
    request_id: Option<String>,
}

#[derive(Default)]
struct AgentKvSession {
    input_chars: usize,
    output_chars: usize,
    model: Option<String>,
    user_text: String,
}

fn parse_agent_kv(
    conn: &Connection,
    source: &SessionSource,
    seen: &mut HashSet<String>,
) -> Result<Vec<ParsedCall>> {
    let mut stmt = match conn.prepare(AGENT_KV_QUERY) {
        Ok(s) => s,
        Err(_) => return Ok(Vec::new()),
    };

    let rows = stmt.query_map([], |r| {
        Ok(AgentKvRow {
            role: r.get::<_, Option<String>>(1)?,
            content: r.get::<_, Option<String>>(2)?,
            request_id: r.get::<_, Option<String>>(3)?,
        })
    });

    let rows = match rows {
        Ok(rows) => rows,
        Err(_) => return Ok(Vec::new()),
    };

    let mut sessions: HashMap<String, AgentKvSession> = HashMap::new();
    let mut order: Vec<String> = Vec::new();
    let mut current_request_id = "unknown".to_string();

    for row in rows.flatten() {
        let Some(role) = row.role else { continue };
        let Some(content_str) = row.content else { continue };

        let request_id = row.request_id.unwrap_or_else(|| current_request_id.clone());
        if request_id != current_request_id {
            current_request_id = request_id.clone();
        }

        let (text_length, model_from_content, first_text) = analyze_content(&content_str);

        let entry = if let Some(s) = sessions.get_mut(&request_id) {
            s
        } else {
            order.push(request_id.clone());
            sessions.entry(request_id.clone()).or_default()
        };

        match role.as_str() {
            "user" => {
                entry.input_chars += text_length;
                if entry.user_text.is_empty() {
                    let candidate = first_text.unwrap_or_else(|| content_str.clone());
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
        let Some(session) = sessions.remove(&request_id) else { continue };
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
        let mut call = ParsedCall {
            provider: config::PROVIDER_ID,
            model: display_model.clone(),
            input_tokens,
            output_tokens,
            speed: Speed::Standard,
            timestamp: None,
            dedup_key,
            user_message: session.user_text,
            session_id: request_id,
            project: source.project.clone(),
            ..ParsedCall::default()
        };
        call.cost_usd = pricing::cost(&display_model, &call, Speed::Standard);
        calls.push(call);
    }

    Ok(calls)
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
            return truncate(&text[inner_start..inner_end].trim(), 500);
        }
    }
    truncate(text, 500)
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

fn column_as_string(
    row: &rusqlite::Row<'_>,
    idx: usize,
) -> rusqlite::Result<Option<String>> {
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
    if let Ok(ms) = s.parse::<i64>() {
        return DateTime::from_timestamp_millis(ms);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn source() -> SessionSource {
        SessionSource {
            path: std::path::PathBuf::from(":memory:"),
            project: "cursor-workspace".into(),
            provider: config::PROVIDER_ID,
        }
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
        insert(
            &conn,
            "agentKv:blob:r1:1",
            r#"{"role":"user","content":[{"type":"text","text":"<user_query>fix typo</user_query>"}],"providerOptions":{"cursor":{"requestId":"req-1"}}}"#,
        );
        insert(
            &conn,
            "agentKv:blob:r1:2",
            r#"{"role":"assistant","content":[{"type":"text","text":"sure here is the patch","providerOptions":{"cursor":{"modelName":"gpt-5"}}}],"providerOptions":{"cursor":{"requestId":"req-1"}}}"#,
        );

        let mut seen = HashSet::new();
        let calls = parse_with_conn(&conn, &source(), &mut seen).unwrap();
        assert_eq!(calls.len(), 4, "3 bubbles + 1 agentKv session, got {:?}", calls.iter().map(|c| &c.dedup_key).collect::<Vec<_>>());

        let assistant = &calls[0];
        assert_eq!(assistant.input_tokens, 120);
        assert_eq!(assistant.output_tokens, 80);
        assert_eq!(assistant.model, "claude-sonnet-4-5");
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
        let first = parse_with_conn(&conn, &source(), &mut seen).unwrap();
        assert_eq!(first.len(), 1);
        let second = parse_with_conn(&conn, &source(), &mut seen).unwrap();
        assert!(second.is_empty(), "second pass must dedup");
    }

    #[test]
    fn extract_user_query_finds_inner_text() {
        assert_eq!(
            extract_user_query("preface <user_query>real question</user_query> trailer"),
            "real question"
        );
        assert_eq!(extract_user_query("no tags here"), "no tags here");
    }
}
