use std::collections::HashSet;
use std::fs;
use std::path::Path;

use chrono::{DateTime, Utc};
use color_eyre::Result;
use serde::Deserialize;
use serde_json::Value;

use crate::pricing;
use crate::tools::{jsonl, ParsedCall, SessionSource, Speed};

use super::config;

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
    let mut calls = Vec::new();
    for path in collect_jsonl(&source.path) {
        let mut last_user_text = String::new();
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
                            if !text.trim().is_empty() {
                                last_user_text = truncate(&text, 500);
                            }
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

                    let (tools, bash_commands) = extract_tools(msg.content.as_ref());

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
                        tools,
                        bash_commands,
                        timestamp: entry.timestamp.as_deref().and_then(parse_timestamp),
                        dedup_key,
                        user_message: last_user_text.clone(),
                        session_id: entry
                            .session_id
                            .clone()
                            .unwrap_or_else(|| session_id.clone()),
                        project: project.clone(),
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

fn extract_tools(content: Option<&Value>) -> (Vec<String>, Vec<String>) {
    let mut tools = Vec::new();
    let mut bash = Vec::new();

    let Some(arr) = content.and_then(|v| v.as_array()) else {
        return (tools, bash);
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
        if matches!(name.as_str(), "Bash" | "BashOutput") {
            if let Some(cmd) = block
                .get("input")
                .and_then(|i| i.get("command"))
                .and_then(|c| c.as_str())
            {
                bash.extend(jsonl::split_bash_commands(cmd));
            }
        }
        tools.push(name);
    }

    (tools, bash)
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

    fn fixture() -> tempfile_lite::TempDir {
        let dir = tempfile_lite::TempDir::new();
        let path = dir.path().join("session.jsonl");
        let mut f = std::fs::File::create(&path).unwrap();
        for line in [
            r#"{"type":"user","timestamp":"2026-04-26T10:00:00Z","sessionId":"s1","message":{"role":"user","content":"refactor the parser"}}"#,
            r#"{"type":"assistant","timestamp":"2026-04-26T10:00:01Z","sessionId":"s1","message":{"role":"assistant","id":"msg_1","model":"claude-opus-4-7-20250514","usage":{"input_tokens":100,"output_tokens":50,"cache_creation_input_tokens":1000,"cache_read_input_tokens":5000,"speed":"fast"},"content":[{"type":"tool_use","name":"Bash","input":{"command":"ls -la | grep foo"}},{"type":"tool_use","name":"Edit","input":{}}]}}"#,
            r#"{"type":"assistant","timestamp":"2026-04-26T10:00:02Z","sessionId":"s1","message":{"role":"assistant","id":"msg_1","model":"claude-opus-4-7","usage":{"input_tokens":999}}}"#,
        ] {
            writeln!(f, "{}", line).unwrap();
        }
        dir
    }

    #[test]
    fn parses_assistant_entries_and_dedups() {
        let dir = fixture();
        let source = SessionSource {
            path: dir.path().to_path_buf(),
            project: "test/project".into(),
            tool: config::TOOL_ID,
        };
        let mut seen = HashSet::new();
        let calls = parse_session(&source, &mut seen).unwrap();
        assert_eq!(calls.len(), 1, "duplicate msg.id should be dropped");
        let call = &calls[0];
        assert_eq!(call.input_tokens, 100);
        assert_eq!(call.output_tokens, 50);
        assert_eq!(call.cache_creation_input_tokens, 1000);
        assert_eq!(call.cache_read_input_tokens, 5000);
        assert_eq!(call.speed, Speed::Fast);
        assert_eq!(call.tools, vec!["Bash", "Edit"]);
        assert_eq!(call.bash_commands, vec!["ls -la", "grep foo"]);
        assert!(call.cost_usd > 0.0);
        assert_eq!(call.user_message, "refactor the parser");
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

        let source = SessionSource {
            path: dir.path().to_path_buf(),
            project: "/Users/russ/mckendrick/Code/ai/commit/dev".into(),
            tool: config::TOOL_ID,
        };
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
