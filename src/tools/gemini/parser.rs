use std::collections::HashSet;
use std::fs;

use chrono::{DateTime, Utc};
use color_eyre::Result;
use serde::Deserialize;
use serde_json::Value;

use crate::pricing;
use crate::tools::{jsonl, ParsedCall, SessionSource, Speed};

use super::config;

#[derive(Debug, Deserialize, Default)]
struct GeminiSession {
    #[serde(default, rename = "sessionId")]
    session_id: String,
    #[serde(default, rename = "projectHash")]
    project_hash: Option<String>,
    #[serde(default, rename = "startTime")]
    start_time: String,
    #[serde(default, rename = "lastUpdated")]
    last_updated: Option<String>,
    #[serde(default)]
    messages: Vec<GeminiMessage>,
    #[serde(default)]
    kind: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct GeminiMessage {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    timestamp: Option<String>,
    #[serde(default, rename = "type")]
    kind: String,
    #[serde(default)]
    content: Option<Value>,
    #[serde(default)]
    tokens: Option<GeminiTokens>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default, rename = "toolCalls")]
    tool_calls: Option<Vec<GeminiToolCall>>,
}

#[derive(Debug, Deserialize, Default, Clone, Copy)]
struct GeminiTokens {
    #[serde(default)]
    input: u64,
    #[serde(default)]
    output: u64,
    #[serde(default)]
    cached: u64,
    #[serde(default)]
    thoughts: u64,
}

#[derive(Debug, Deserialize, Default)]
struct GeminiToolCall {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    args: Option<Value>,
    #[serde(default, rename = "displayName")]
    display_name: Option<String>,
}

pub fn parse_session(
    source: &SessionSource,
    seen: &mut HashSet<String>,
) -> Result<Vec<ParsedCall>> {
    let raw = match fs::read_to_string(&source.path) {
        Ok(raw) => raw,
        Err(_) => return Ok(Vec::new()),
    };
    let Some(session) = parse_raw_session(&raw) else {
        return Ok(Vec::new());
    };
    Ok(parse_session_data(session, source, seen))
}

fn parse_raw_session(raw: &str) -> Option<GeminiSession> {
    if raw.trim().is_empty() {
        return None;
    }

    if let Ok(session) = serde_json::from_str::<GeminiSession>(raw) {
        if !session.session_id.is_empty() && !session.messages.is_empty() {
            return Some(session);
        }
    }

    parse_jsonl(raw)
}

fn parse_jsonl(raw: &str) -> Option<GeminiSession> {
    let mut session = GeminiSession::default();

    for line in raw.lines().filter(|line| !line.trim().is_empty()) {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if value.get("$set").is_some() {
            continue;
        }

        if value.get("sessionId").is_some() && value.get("startTime").is_some() {
            if session.session_id.is_empty() {
                if let Ok(meta) = serde_json::from_value::<GeminiSession>(value) {
                    session.session_id = meta.session_id;
                    session.project_hash = meta.project_hash;
                    session.start_time = meta.start_time;
                    session.last_updated = meta.last_updated;
                    session.kind = meta.kind;
                }
            }
        } else if value.get("id").is_some() && value.get("type").is_some() {
            if let Ok(message) = serde_json::from_value::<GeminiMessage>(value) {
                session.messages.push(message);
            }
        }
    }

    (!session.session_id.is_empty()).then_some(session)
}

fn parse_session_data(
    session: GeminiSession,
    source: &SessionSource,
    seen: &mut HashSet<String>,
) -> Vec<ParsedCall> {
    let session_start = parse_timestamp(&session.start_time);
    let mut last_user_text = String::new();
    let mut calls = Vec::new();

    for message in session.messages {
        if is_user_message(&message) {
            let text = extract_content_text(message.content.as_ref());
            if !text.trim().is_empty() {
                last_user_text = truncate(&text, 500);
            }
            continue;
        }

        if !is_gemini_message(&message) {
            continue;
        }

        let Some(tokens) = message.tokens else {
            continue;
        };
        let Some(model) = message.model.clone().filter(|model| !model.is_empty()) else {
            continue;
        };
        if tokens.input == 0 && tokens.output == 0 && tokens.cached == 0 && tokens.thoughts == 0 {
            continue;
        }

        let dedup_key = gemini_dedup_key(&session.session_id, &message, &model, tokens);
        if !seen.insert(dedup_key.clone()) {
            continue;
        }

        let input_tokens = tokens.input.saturating_sub(tokens.cached);
        let output_tokens = tokens.output.saturating_add(tokens.thoughts);
        let (tools, bash_commands) = extract_tools(message.tool_calls.as_deref());
        let timestamp = message
            .timestamp
            .as_deref()
            .and_then(parse_timestamp)
            .or(session_start);

        let mut call = ParsedCall {
            tool: config::TOOL_ID,
            model: model.clone(),
            input_tokens,
            output_tokens,
            cache_read_input_tokens: tokens.cached,
            cached_input_tokens: tokens.cached,
            reasoning_tokens: tokens.thoughts,
            speed: Speed::Standard,
            tools,
            bash_commands,
            timestamp,
            dedup_key,
            user_message: last_user_text.clone(),
            session_id: session.session_id.clone(),
            project: source.project.clone(),
            ..ParsedCall::default()
        };
        call.cost_usd = pricing::cost(&model, &call, Speed::Standard);
        calls.push(call);
    }

    calls
}

fn is_user_message(message: &GeminiMessage) -> bool {
    message.kind == "user"
}

fn is_gemini_message(message: &GeminiMessage) -> bool {
    matches!(message.kind.as_str(), "gemini" | "model" | "assistant")
}

fn gemini_dedup_key(
    session_id: &str,
    message: &GeminiMessage,
    model: &str,
    tokens: GeminiTokens,
) -> String {
    if let Some(id) = message.id.as_ref().filter(|id| !id.is_empty()) {
        return format!("gemini:{session_id}:{id}");
    }
    format!(
        "gemini:{}:{}:{}:{}+{}+{}",
        session_id,
        message.timestamp.as_deref().unwrap_or_default(),
        model,
        tokens.input,
        tokens.output,
        tokens.cached
    )
}

fn extract_content_text(content: Option<&Value>) -> String {
    let Some(content) = content else {
        return String::new();
    };
    if let Some(s) = content.as_str() {
        return s.to_string();
    }
    if let Some(arr) = content.as_array() {
        let parts: Vec<&str> = arr
            .iter()
            .filter_map(|part| part.get("text").and_then(|text| text.as_str()))
            .collect();
        return parts.join(" ");
    }
    String::new()
}

fn extract_tools(tool_calls: Option<&[GeminiToolCall]>) -> (Vec<String>, Vec<String>) {
    let mut tools = Vec::new();
    let mut bash_commands = Vec::new();
    let Some(tool_calls) = tool_calls else {
        return (tools, bash_commands);
    };

    for tool_call in tool_calls {
        let raw_name = tool_call.name.as_deref().unwrap_or("");
        let display_name = tool_call.display_name.as_deref().unwrap_or("");
        let normalized = normalize_tool(display_name)
            .or_else(|| normalize_tool(raw_name))
            .unwrap_or_else(|| {
                if !display_name.is_empty() {
                    display_name.to_string()
                } else {
                    raw_name.to_string()
                }
            });
        if normalized.is_empty() {
            continue;
        }

        if normalized == "Bash" {
            if let Some(command) = command_arg(tool_call.args.as_ref()) {
                bash_commands.extend(jsonl::split_bash_commands(&command));
            }
        }
        tools.push(normalized);
    }

    (tools, bash_commands)
}

fn normalize_tool(name: &str) -> Option<String> {
    let mapped = match name {
        "read_file" | "ReadFile" => "Read",
        "write_file" | "create_file" | "WriteFile" => "Write",
        "edit_file" | "EditFile" | "replace" => "Edit",
        "delete_file" => "Delete",
        "list_dir" | "ListDir" => "LS",
        "grep_search" | "search_files" | "SearchText" => "Grep",
        "find_files" => "Glob",
        "run_command" | "Shell" => "Bash",
        "web_search" => "WebSearch",
        "" => return None,
        _ => return None,
    };
    Some(mapped.to_string())
}

fn command_arg(args: Option<&Value>) -> Option<String> {
    let args = args?;
    if let Some(obj) = args.as_object() {
        return obj
            .get("command")
            .and_then(|v| v.as_str())
            .or_else(|| obj.get("cmd").and_then(|v| v.as_str()))
            .map(ToString::to_string);
    }
    let raw = args.as_str()?;
    if let Ok(parsed) = serde_json::from_str::<Value>(raw) {
        return command_arg(Some(&parsed));
    }
    None
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
    use std::io::Write;
    use std::path::{Path, PathBuf};

    fn write_session(raw: &str, ext: &str) -> TempFile {
        let file = TempFile::new(&format!("session-test.{ext}"));
        let mut handle = std::fs::File::create(file.path()).unwrap();
        handle.write_all(raw.as_bytes()).unwrap();
        file
    }

    fn source_for(path: PathBuf) -> SessionSource {
        SessionSource {
            path,
            project: "project-hash".into(),
            tool: config::TOOL_ID,
        }
    }

    #[test]
    fn parses_json_session_per_gemini_message() {
        let raw = r#"{
          "sessionId": "s1",
          "projectHash": "project-hash",
          "startTime": "2026-05-01T18:34:30.869Z",
          "messages": [
            { "id": "u1", "timestamp": "2026-05-01T18:34:31Z", "type": "user", "content": [{ "text": "run the build" }] },
            { "id": "g1", "timestamp": "2026-05-01T18:34:32Z", "type": "gemini", "content": "done", "model": "gemini-2.5-pro",
              "tokens": { "input": 120, "output": 30, "cached": 20, "thoughts": 5 },
              "toolCalls": [{ "id": "t1", "name": "run_command", "args": { "command": "cargo check | tee out.txt" } }]
            },
            { "id": "g2", "timestamp": "2026-05-01T18:34:33Z", "type": "gemini", "content": "again", "model": "gemini-2.5-flash",
              "tokens": { "input": 80, "output": 12, "cached": 0, "thoughts": 0 },
              "toolCalls": [{ "id": "t2", "name": "read_file", "args": {} }]
            }
          ]
        }"#;
        let file = write_session(raw, "json");
        let mut seen = HashSet::new();

        let calls = parse_session(&source_for(file.path().to_path_buf()), &mut seen).unwrap();

        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].dedup_key, "gemini:s1:g1");
        assert_eq!(calls[0].model, "gemini-2.5-pro");
        assert_eq!(calls[0].input_tokens, 100);
        assert_eq!(calls[0].output_tokens, 35);
        assert_eq!(calls[0].cache_read_input_tokens, 20);
        assert_eq!(calls[0].cached_input_tokens, 20);
        assert_eq!(calls[0].reasoning_tokens, 5);
        assert_eq!(calls[0].tools, vec!["Bash"]);
        assert_eq!(calls[0].bash_commands, vec!["cargo check", "tee out.txt"]);
        assert_eq!(calls[0].user_message, "run the build");
        assert_eq!(calls[0].session_id, "s1");
        assert_eq!(calls[0].project, "project-hash");
        assert!(calls[0].timestamp.is_some());
        assert!(calls[0].cost_usd > 0.0);

        assert_eq!(calls[1].dedup_key, "gemini:s1:g2");
        assert_eq!(calls[1].tools, vec!["Read"]);
    }

    #[test]
    fn parses_jsonl_session_and_falls_back_to_start_time() {
        let raw = [
            r#"{"sessionId":"s2","projectHash":"hash","startTime":"2026-05-01T18:34:30.869Z","lastUpdated":"2026-05-01T18:35:00Z","kind":"chat"}"#,
            r#"{"id":"u1","type":"user","content":"explain this"}"#,
            r#"{"id":"g1","type":"gemini","model":"gemini-2.5-pro","tokens":{"input":42,"output":9,"cached":50,"thoughts":1},"toolCalls":[{"name":"Shell","args":"{\"cmd\":\"ls -la; pwd\"}"}]}"#,
        ]
        .join("\n");
        let file = write_session(&raw, "jsonl");
        let mut seen = HashSet::new();

        let calls = parse_session(&source_for(file.path().to_path_buf()), &mut seen).unwrap();

        assert_eq!(calls.len(), 1);
        let call = &calls[0];
        assert_eq!(call.input_tokens, 0, "cached input subtraction saturates");
        assert_eq!(call.output_tokens, 10);
        assert_eq!(call.cache_read_input_tokens, 50);
        assert_eq!(call.tools, vec!["Bash"]);
        assert_eq!(call.bash_commands, vec!["ls -la", "pwd"]);
        assert_eq!(
            call.timestamp.unwrap(),
            parse_timestamp("2026-05-01T18:34:30.869Z").unwrap()
        );
    }

    #[test]
    fn skips_invalid_empty_and_metadata_only_sessions() {
        for raw in [
            "",
            "not json",
            r#"{"sessionId":"s1","startTime":"2026-05-01T00:00:00Z"}"#,
        ] {
            let file = write_session(raw, "jsonl");
            let mut seen = HashSet::new();
            let calls = parse_session(&source_for(file.path().to_path_buf()), &mut seen).unwrap();
            assert!(calls.is_empty());
        }
    }

    #[test]
    fn deduplicates_by_message_id() {
        let raw = [
            r#"{"sessionId":"s2","projectHash":"hash","startTime":"2026-05-01T18:34:30.869Z"}"#,
            r#"{"id":"g1","type":"gemini","model":"gemini-2.5-pro","tokens":{"input":42,"output":9}}"#,
        ]
        .join("\n");
        let file = write_session(&raw, "jsonl");
        let source = source_for(file.path().to_path_buf());
        let mut seen = HashSet::new();

        let first = parse_session(&source, &mut seen).unwrap();
        let second = parse_session(&source, &mut seen).unwrap();

        assert_eq!(first.len(), 1);
        assert!(second.is_empty());
    }

    #[test]
    fn normalizes_gemini_tool_names() {
        let calls = vec![
            GeminiToolCall {
                name: Some("write_file".into()),
                args: None,
                display_name: None,
            },
            GeminiToolCall {
                name: Some("ignored".into()),
                args: None,
                display_name: Some("SearchText".into()),
            },
            GeminiToolCall {
                name: Some("find_files".into()),
                args: None,
                display_name: None,
            },
            GeminiToolCall {
                name: Some("custom_tool".into()),
                args: None,
                display_name: None,
            },
        ];

        let (tools, bash) = extract_tools(Some(&calls));

        assert_eq!(tools, vec!["Write", "Grep", "Glob", "custom_tool"]);
        assert!(bash.is_empty());
    }

    #[test]
    fn invalid_message_timestamp_can_fall_back_to_none() {
        let raw = [
            r#"{"sessionId":"s2","projectHash":"hash","startTime":"not-a-date"}"#,
            r#"{"id":"g1","timestamp":"also-bad","type":"gemini","model":"gemini-2.5-pro","tokens":{"input":42,"output":9}}"#,
        ]
        .join("\n");
        let file = write_session(&raw, "jsonl");
        let mut seen = HashSet::new();

        let calls = parse_session(&source_for(file.path().to_path_buf()), &mut seen).unwrap();

        assert_eq!(calls.len(), 1);
        assert!(calls[0].timestamp.is_none());
    }

    struct TempFile(PathBuf);

    impl TempFile {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "tokenuse-gemini-parser-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir.join(name))
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempFile {
        fn drop(&mut self) {
            if let Some(parent) = self.0.parent() {
                let _ = std::fs::remove_dir_all(parent);
            }
        }
    }
}
