use std::collections::HashSet;
use std::fs;
use std::path::Path;

use chrono::{DateTime, Utc};
use color_eyre::Result;
use serde::Deserialize;
use serde_json::Value;

use crate::pricing;
use crate::tools::{jsonl, CodeBlock, ParsedCall, SessionSource, Speed};

use super::config;

/// User lines starting with this marker record an interrupted assistant turn.
/// Covers both "[Request interrupted by user]" and the "for tool use" variant.
const INTERRUPT_MARKER: &str = "[Request interrupted by user";

#[derive(Debug, Deserialize)]
struct JournalEntry {
    #[serde(default, rename = "type")]
    kind: String,
    #[serde(default)]
    timestamp: Option<String>,
    #[serde(default, rename = "sessionId")]
    session_id: Option<String>,
    #[serde(default)]
    cwd: Option<String>,
    #[serde(default)]
    message: Option<Message>,
}

#[derive(Debug, Deserialize)]
struct Message {
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    usage: Option<Usage>,
    #[serde(default)]
    content: Option<Value>,
}

#[derive(Debug, Deserialize, Default)]
struct Usage {
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
    #[serde(default)]
    cache_creation_input_tokens: u64,
    #[serde(default)]
    cache_read_input_tokens: u64,
    #[serde(default)]
    speed: Option<String>,
    #[serde(default)]
    server_tool_use: Option<ServerToolUse>,
}

#[derive(Debug, Deserialize, Default)]
struct ServerToolUse {
    #[serde(default)]
    web_search_requests: u64,
}

pub fn parse_session(
    source: &SessionSource,
    seen: &mut HashSet<String>,
) -> Result<Vec<ParsedCall>> {
    let mut calls: Vec<ParsedCall> = Vec::new();
    for path in collect_jsonl(&source.path) {
        let mut last_user_text = String::new();
        let mut last_user_chars: Option<u64> = None;
        let mut last_user_ts: Option<DateTime<Utc>> = None;
        let file_start_index = calls.len();
        let mut project = source.project.clone();
        let lines = match jsonl::read_lines(&path) {
            Ok(l) => l,
            Err(_) => continue,
        };
        let session_id = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();

        for line in lines {
            let entry: JournalEntry = match serde_json::from_str(&line) {
                Ok(e) => e,
                Err(_) => continue,
            };

            if let Some(cwd) = entry.cwd.as_ref().filter(|cwd| !cwd.trim().is_empty()) {
                project = cwd.clone();
            }

            match entry.kind.as_str() {
                "user" => {
                    if let Some(msg) = &entry.message {
                        if msg.role.as_deref() == Some("user") {
                            let text = extract_user_text(msg);
                            let trimmed = text.trim();
                            if trimmed.is_empty() {
                                continue;
                            }
                            if trimmed.starts_with(INTERRUPT_MARKER) {
                                // The interruption belongs to the previous
                                // call of this session file, if any.
                                if calls.len() > file_start_index {
                                    if let Some(last) = calls.last_mut() {
                                        last.is_canceled = true;
                                    }
                                }
                                continue;
                            }
                            if trimmed.starts_with("<command-")
                                || trimmed.starts_with("<local-command-")
                            {
                                // Slash-command wrappers are UI plumbing, not
                                // a prompt the user wrote.
                                continue;
                            }
                            last_user_chars = Some(text.chars().count() as u64);
                            last_user_text = jsonl::truncate_chars(&text, 500);
                            last_user_ts = entry.timestamp.as_deref().and_then(parse_timestamp);
                        }
                    }
                }
                "assistant" => {
                    let Some(msg) = entry.message.as_ref() else {
                        continue;
                    };
                    let Some(model) = msg.model.clone() else {
                        continue;
                    };
                    let Some(usage) = msg.usage.as_ref() else {
                        continue;
                    };

                    let dedup_key = msg.id.clone().unwrap_or_else(|| {
                        format!("claude:{}", entry.timestamp.clone().unwrap_or_default())
                    });

                    if !seen.insert(dedup_key.clone()) {
                        continue;
                    }

                    let speed = match usage.speed.as_deref() {
                        Some("fast") => Speed::Fast,
                        _ => Speed::Standard,
                    };

                    let activity = extract_activity(msg.content.as_ref());
                    let response_text = extract_response_text(msg.content.as_ref());
                    let mut code_blocks = jsonl::extract_code_fences(&response_text);
                    code_blocks.extend(activity.code_blocks);

                    let timestamp = entry.timestamp.as_deref().and_then(parse_timestamp);
                    let elapsed_ms = jsonl::turn_elapsed_ms(timestamp, last_user_ts);

                    let mut call = ParsedCall {
                        tool: config::TOOL_ID,
                        model: model.clone(),
                        input_tokens: usage.input_tokens,
                        output_tokens: usage.output_tokens,
                        cache_creation_input_tokens: usage.cache_creation_input_tokens,
                        cache_read_input_tokens: usage.cache_read_input_tokens,
                        web_search_requests: usage
                            .server_tool_use
                            .as_ref()
                            .map(|s| s.web_search_requests)
                            .unwrap_or(0),
                        speed,
                        tools: activity.tools,
                        bash_commands: activity.bash_commands,
                        timestamp,
                        dedup_key,
                        user_message: last_user_text.clone(),
                        session_id: entry
                            .session_id
                            .clone()
                            .unwrap_or_else(|| session_id.clone()),
                        project: project.clone(),
                        prompt_chars: last_user_chars,
                        response_chars: Some(response_text.chars().count() as u64),
                        elapsed_ms,
                        code_blocks: jsonl::merge_code_blocks(code_blocks),
                        edited_files: jsonl::dedup_files(activity.edited_files),
                        referenced_files: jsonl::dedup_files(activity.referenced_files),
                        ..ParsedCall::default()
                    };

                    call.cost_usd = pricing::cost(&model, &call, speed);
                    calls.push(call);
                }
                _ => {}
            }
        }
    }

    Ok(calls)
}

fn collect_jsonl(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return files,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file()
            && path.extension().and_then(|s| s.to_str()) == Some(config::SESSION_GLOB_EXT)
        {
            files.push(path);
        } else if path.is_dir() && entry.file_name() == config::SUBAGENTS_DIR {
            if let Ok(sub_entries) = fs::read_dir(&path) {
                for sub in sub_entries.flatten() {
                    let sub_path = sub.path();
                    if sub_path.is_file()
                        && sub_path.extension().and_then(|s| s.to_str())
                            == Some(config::SESSION_GLOB_EXT)
                    {
                        files.push(sub_path);
                    }
                }
            }
        }
    }
    files
}

fn parse_timestamp(s: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|d| d.with_timezone(&Utc))
}

fn extract_user_text(msg: &Message) -> String {
    let Some(content) = msg.content.as_ref() else {
        return String::new();
    };
    if let Some(s) = content.as_str() {
        return s.to_string();
    }
    if let Some(arr) = content.as_array() {
        let mut parts = Vec::new();
        for block in arr {
            if block.get("type").and_then(|t| t.as_str()) == Some("text") {
                if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                    parts.push(text.to_string());
                }
            }
        }
        return parts.join(" ");
    }
    String::new()
}

#[derive(Default)]
struct ToolActivity {
    tools: Vec<String>,
    bash_commands: Vec<String>,
    edited_files: Vec<String>,
    referenced_files: Vec<String>,
    code_blocks: Vec<CodeBlock>,
}

fn extract_activity(content: Option<&Value>) -> ToolActivity {
    let mut activity = ToolActivity::default();

    let Some(arr) = content.and_then(|v| v.as_array()) else {
        return activity;
    };

    for block in arr {
        if block.get("type").and_then(|t| t.as_str()) != Some("tool_use") {
            continue;
        }
        let name = block
            .get("name")
            .and_then(|n| n.as_str())
            .unwrap_or("")
            .to_string();
        if name.is_empty() {
            continue;
        }
        let input = block.get("input");
        let input_str = |key: &str| {
            input
                .and_then(|i| i.get(key))
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
        };

        match name.as_str() {
            "Bash" | "BashOutput" => {
                if let Some(cmd) = input_str("command") {
                    activity
                        .bash_commands
                        .extend(jsonl::split_bash_commands(cmd));
                }
            }
            "Write" => {
                record_edit(&mut activity, input_str("file_path"), input_str("content"));
            }
            "Edit" => {
                record_edit(
                    &mut activity,
                    input_str("file_path"),
                    input_str("new_string"),
                );
            }
            "MultiEdit" => {
                let path = input_str("file_path");
                if let Some(edits) = input
                    .and_then(|i| i.get("edits"))
                    .and_then(|e| e.as_array())
                {
                    for edit in edits {
                        let payload = edit.get("new_string").and_then(|v| v.as_str());
                        record_edit(&mut activity, path, payload.filter(|s| !s.is_empty()));
                    }
                } else {
                    record_edit(&mut activity, path, None);
                }
            }
            "NotebookEdit" => {
                record_edit(
                    &mut activity,
                    input_str("notebook_path"),
                    input_str("new_source"),
                );
            }
            "Read" => {
                if let Some(path) = input_str("file_path") {
                    activity.referenced_files.push(path.to_string());
                }
            }
            _ => {}
        }
        activity.tools.push(name);
    }

    activity
}

/// Record a Write/Edit-style payload: the touched file plus its new content
/// counted as AI code output, attributed to the file's language.
fn record_edit(activity: &mut ToolActivity, path: Option<&str>, payload: Option<&str>) {
    if let Some(path) = path {
        activity.edited_files.push(path.to_string());
    }
    if let Some(payload) = payload {
        let language = path
            .map(jsonl::extension_language)
            .unwrap_or_else(|| "unknown".to_string());
        activity.code_blocks.push(CodeBlock {
            language,
            loc: payload.lines().count() as u64,
        });
    }
}

fn extract_response_text(content: Option<&Value>) -> String {
    let Some(arr) = content.and_then(|v| v.as_array()) else {
        return String::new();
    };
    let mut parts = Vec::new();
    for block in arr {
        if block.get("type").and_then(|t| t.as_str()) == Some("text") {
            if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                parts.push(text);
            }
        }
    }
    parts.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn fixture() -> tempfile_lite::TempDir {
        let dir = tempfile_lite::TempDir::new();
        let path = dir.path().join("session.jsonl");
        let mut f = std::fs::File::create(&path).unwrap();
        for line in [
            r#"{"type":"user","timestamp":"2026-04-26T10:00:00Z","sessionId":"s1","message":{"role":"user","content":"refactor the parser"}}"#,
            r#"{"type":"assistant","timestamp":"2026-04-26T10:00:01Z","sessionId":"s1","message":{"role":"assistant","id":"msg_1","model":"claude-opus-4-7-20250514","usage":{"input_tokens":100,"output_tokens":50,"cache_creation_input_tokens":1000,"cache_read_input_tokens":5000,"speed":"fast"},"content":[{"type":"text","text":"Here:\n```rust\nfn a() {}\nfn b() {}\n```"},{"type":"tool_use","name":"Bash","input":{"command":"ls -la | grep foo"}},{"type":"tool_use","name":"Edit","input":{"file_path":"src/app.rs","new_string":"let a = 1;\nlet b = 2;"}},{"type":"tool_use","name":"Read","input":{"file_path":"docs/x.md"}}]}}"#,
            r#"{"type":"assistant","timestamp":"2026-04-26T10:00:02Z","sessionId":"s1","message":{"role":"assistant","id":"msg_1","model":"claude-opus-4-7","usage":{"input_tokens":999}}}"#,
            r#"{"type":"user","timestamp":"2026-04-26T10:00:30Z","sessionId":"s1","message":{"role":"user","content":"[Request interrupted by user]"}}"#,
            r#"{"type":"user","timestamp":"2026-04-26T10:04:00Z","sessionId":"s1","message":{"role":"user","content":"<command-name>/compact</command-name>"}}"#,
            r#"{"type":"user","timestamp":"2026-04-26T10:05:00Z","sessionId":"s1","message":{"role":"user","content":"try again with tests"}}"#,
            r#"{"type":"assistant","timestamp":"2026-04-26T10:05:20Z","sessionId":"s1","message":{"role":"assistant","id":"msg_2","model":"claude-opus-4-7-20250514","usage":{"input_tokens":10,"output_tokens":5},"content":[{"type":"text","text":"done"}]}}"#,
        ] {
            writeln!(f, "{}", line).unwrap();
        }
        dir
    }

    #[test]
    fn parses_assistant_entries_and_dedups() {
        let dir = fixture();
        let source =
            SessionSource::session(dir.path().to_path_buf(), "test/project", config::TOOL_ID);
        let mut seen = HashSet::new();
        let calls = parse_session(&source, &mut seen).unwrap();
        assert_eq!(calls.len(), 2, "duplicate msg.id should be dropped");
        let call = &calls[0];
        assert_eq!(call.input_tokens, 100);
        assert_eq!(call.output_tokens, 50);
        assert_eq!(call.cache_creation_input_tokens, 1000);
        assert_eq!(call.cache_read_input_tokens, 5000);
        assert_eq!(call.speed, Speed::Fast);
        assert_eq!(call.tools, vec!["Bash", "Edit", "Read"]);
        assert_eq!(call.bash_commands, vec!["ls -la", "grep foo"]);
        assert!(call.cost_usd > 0.0);
        assert_eq!(call.user_message, "refactor the parser");
    }

    #[test]
    fn enrichment_fields_capture_turn_shape() {
        let dir = fixture();
        let source =
            SessionSource::session(dir.path().to_path_buf(), "test/project", config::TOOL_ID);
        let mut seen = HashSet::new();
        let calls = parse_session(&source, &mut seen).unwrap();
        assert_eq!(calls.len(), 2);

        let first = &calls[0];
        assert_eq!(first.prompt_chars, Some(19));
        assert_eq!(first.elapsed_ms, Some(1_000));
        assert_eq!(
            first.response_chars,
            Some("Here:\n```rust\nfn a() {}\nfn b() {}\n```".chars().count() as u64)
        );
        assert_eq!(
            first.code_blocks,
            vec![CodeBlock {
                language: "rust".into(),
                loc: 4,
            }],
            "fence LoC and Edit payload LoC merge under one language"
        );
        assert_eq!(first.edited_files, vec!["src/app.rs"]);
        assert_eq!(first.referenced_files, vec!["docs/x.md"]);
        assert!(
            first.is_canceled,
            "interrupt marker cancels the previous call"
        );

        let second = &calls[1];
        assert_eq!(
            second.user_message, "try again with tests",
            "command wrappers and interrupt markers never become prompts"
        );
        assert_eq!(second.prompt_chars, Some(20));
        assert_eq!(second.elapsed_ms, Some(20_000));
        assert_eq!(second.response_chars, Some(4));
        assert!(!second.is_canceled);
        assert!(second.code_blocks.is_empty());
    }

    #[test]
    fn interrupt_before_any_call_is_ignored() {
        let dir = tempfile_lite::TempDir::new();
        let path = dir.path().join("session.jsonl");
        let mut f = std::fs::File::create(&path).unwrap();
        for line in [
            r#"{"type":"user","timestamp":"2026-04-26T10:00:00Z","sessionId":"s1","message":{"role":"user","content":"[Request interrupted by user]"}}"#,
            r#"{"type":"assistant","timestamp":"2026-04-26T10:00:01Z","sessionId":"s1","message":{"role":"assistant","id":"msg_9","model":"claude-opus-4-7","usage":{"input_tokens":1,"output_tokens":1}}}"#,
        ] {
            writeln!(f, "{}", line).unwrap();
        }

        let source = SessionSource::session(dir.path().to_path_buf(), "p", config::TOOL_ID);
        let mut seen = HashSet::new();
        let calls = parse_session(&source, &mut seen).unwrap();
        assert_eq!(calls.len(), 1);
        assert!(!calls[0].is_canceled);
        assert_eq!(calls[0].user_message, "");
        assert_eq!(calls[0].prompt_chars, None);
        assert_eq!(calls[0].elapsed_ms, None);
    }

    #[test]
    fn cwd_overrides_lossy_project_directory_fallback() {
        let dir = tempfile_lite::TempDir::new();
        let path = dir.path().join("session.jsonl");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"type":"assistant","timestamp":"2026-04-26T10:00:01Z","sessionId":"s1","cwd":"/Users/russ.mckendrick/Code/ai-commit-dev","message":{{"role":"assistant","id":"msg_1","model":"claude-opus-4-7","usage":{{"input_tokens":100,"output_tokens":50}}}}}}"#
        )
        .unwrap();

        let source = SessionSource::session(
            dir.path().to_path_buf(),
            "/Users/russ/mckendrick/Code/ai/commit/dev",
            config::TOOL_ID,
        );
        let mut seen = HashSet::new();
        let calls = parse_session(&source, &mut seen).unwrap();

        assert_eq!(calls.len(), 1);
        assert_eq!(
            calls[0].project,
            "/Users/russ.mckendrick/Code/ai-commit-dev"
        );
    }

    mod tempfile_lite {
        use std::path::{Path, PathBuf};
        use std::sync::atomic::{AtomicU64, Ordering};

        static SEQ: AtomicU64 = AtomicU64::new(0);

        pub struct TempDir(PathBuf);

        impl TempDir {
            pub fn new() -> Self {
                let seq = SEQ.fetch_add(1, Ordering::Relaxed);
                let base = std::env::temp_dir().join(format!(
                    "tokenuse-test-{}-{}-{}",
                    std::process::id(),
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_nanos(),
                    seq
                ));
                std::fs::create_dir_all(&base).unwrap();
                Self(base)
            }
            pub fn path(&self) -> &Path {
                &self.0
            }
        }

        impl Drop for TempDir {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
    }
}
