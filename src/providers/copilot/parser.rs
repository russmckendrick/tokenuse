use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use chrono::{DateTime, Utc};
use color_eyre::Result;
use serde_json::Value;

use crate::pricing;
use crate::providers::{jsonl, ParsedCall, SessionSource, Speed};

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
    if path.is_dir() {
        let mut calls = Vec::new();
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_file() && p.extension().and_then(|s| s.to_str()) == Some("jsonl") {
                    calls.extend(parse_file(&p, source, seen));
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
                    provider: config::PROVIDER_ID,
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
                    ..ParsedCall::default()
                };
                call.cost_usd = pricing::cost(&current_model, &call, Speed::Standard);
                calls.push(call);
            }
            _ => {}
        }
    }

    calls
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

    let model = infer_model_from_tool_ids(&events);

    let mut pending_user_message = String::new();
    let mut calls = Vec::new();

    for event in &events {
        let kind = event.get("type").and_then(|t| t.as_str()).unwrap_or("");

        if kind == "user.message" {
            if let Some(c) = event.pointer("/data/content").and_then(|v| v.as_str()) {
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
            (
                ((content_text.len() as f64) / CHARS_PER_TOKEN).ceil() as u64,
                ((reasoning_text.len() as f64) / CHARS_PER_TOKEN).ceil() as u64,
            )
        };
        let input_tokens = ((pending_user_message.len() as f64) / CHARS_PER_TOKEN).ceil() as u64;

        let (tools, bash_commands) = extract_tools(tool_array);
        let timestamp = event
            .get("timestamp")
            .and_then(|v| v.as_str())
            .and_then(parse_timestamp);

        let mut call = ParsedCall {
            provider: config::PROVIDER_ID,
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
            ..ParsedCall::default()
        };
        call.cost_usd = pricing::cost(&model, &call, Speed::Standard);
        calls.push(call);
    }

    calls
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
    use std::io::Write;
    use std::path::{Path, PathBuf};

    struct TempDir(PathBuf);
    impl TempDir {
        fn new() -> Self {
            let p = std::env::temp_dir().join(format!(
                "tokenuse-copilot-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
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

        let source = SessionSource {
            path: events.clone(),
            project: "demo".into(),
            provider: config::PROVIDER_ID,
        };
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

        // dedup
        let again = parse_session(&source, &mut seen).unwrap();
        assert!(again.is_empty());
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
        let source = SessionSource {
            path: events,
            project: "demo".into(),
            provider: config::PROVIDER_ID,
        };
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

        let source = SessionSource {
            path: transcript.clone(),
            project: "vscode-ws".into(),
            provider: config::PROVIDER_ID,
        };
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
        assert!(
            calls[0].input_tokens > 0,
            "input estimated from user message length"
        );
        assert!(calls[0].output_tokens > 0);
        assert!(calls[0].reasoning_tokens > 0);
        assert!(calls[0].cost_usd > 0.0);
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
        let source = SessionSource {
            path: dir.path().to_path_buf(),
            project: "ws".into(),
            provider: config::PROVIDER_ID,
        };
        let mut seen = HashSet::new();
        let calls = parse_session(&source, &mut seen).unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].model, "openai-auto");
    }
}
