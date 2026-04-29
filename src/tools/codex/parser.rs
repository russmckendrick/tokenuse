use std::collections::HashSet;
use std::mem;

use chrono::{DateTime, Utc};
use color_eyre::Result;
use serde::Deserialize;

use crate::pricing;
use crate::tools::{
    jsonl, LimitCredits, LimitSnapshot, LimitWindow, ParsedCall, SessionSource, Speed,
};

use super::config;

const DEFAULT_MODEL: &str = "gpt-5";

#[derive(Debug, Deserialize)]
struct Entry {
    #[serde(default, rename = "type")]
    kind: String,
    #[serde(default)]
    timestamp: Option<String>,
    #[serde(default)]
    payload: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, Default)]
struct SessionMeta {
    #[serde(default)]
    cwd: Option<String>,
    #[serde(default)]
    originator: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct TurnContext {
    #[serde(default)]
    model: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ResponseItem {
    #[serde(default, rename = "type")]
    kind: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    arguments: Option<String>,
}

#[derive(Debug, Deserialize)]
struct EventMsg {
    #[serde(default, rename = "type")]
    kind: String,
    #[serde(default)]
    info: Option<TokenInfo>,
    #[serde(default)]
    rate_limits: Option<RateLimits>,
}

#[derive(Debug, Deserialize)]
struct TokenInfo {
    #[serde(default)]
    last_token_usage: Option<TokenUsage>,
    #[serde(default)]
    total_token_usage: Option<TokenUsage>,
}

#[derive(Debug, Deserialize, Default, Clone, Copy)]
struct TokenUsage {
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    cached_input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
    #[serde(default)]
    reasoning_output_tokens: u64,
}

#[derive(Debug, Deserialize)]
struct RateLimits {
    #[serde(default)]
    limit_id: String,
    #[serde(default)]
    limit_name: Option<String>,
    #[serde(default)]
    primary: Option<RateLimitWindow>,
    #[serde(default)]
    secondary: Option<RateLimitWindow>,
    #[serde(default)]
    credits: Option<RateLimitCredits>,
    #[serde(default)]
    plan_type: Option<String>,
    #[serde(default)]
    rate_limit_reached_type: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RateLimitWindow {
    #[serde(default)]
    used_percent: f64,
    #[serde(default)]
    window_minutes: u64,
    #[serde(default)]
    resets_at: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct RateLimitCredits {
    #[serde(default)]
    has_credits: bool,
    #[serde(default)]
    unlimited: bool,
    #[serde(default)]
    balance: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct ExecArgs {
    #[serde(default)]
    cmd: Option<String>,
}

pub fn parse_session(
    source: &SessionSource,
    seen: &mut HashSet<String>,
) -> Result<Vec<ParsedCall>> {
    let lines = match jsonl::read_lines(&source.path) {
        Ok(l) => l,
        Err(_) => return Ok(Vec::new()),
    };

    let mut iter = lines.peekable();

    let Some(first_raw) = iter.next() else {
        return Ok(Vec::new());
    };
    let Ok(first) = serde_json::from_str::<Entry>(&first_raw) else {
        return Ok(Vec::new());
    };
    if first.kind != "session_meta" {
        return Ok(Vec::new());
    }
    let meta: SessionMeta = first
        .payload
        .as_ref()
        .and_then(|p| serde_json::from_value(p.clone()).ok())
        .unwrap_or_default();
    if !meta
        .originator
        .as_deref()
        .map(|o| o.to_lowercase().contains("codex"))
        .unwrap_or(false)
    {
        return Ok(Vec::new());
    }

    let session_id = source
        .path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let project = meta
        .cwd
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| source.project.clone());

    let mut current_model = String::new();
    let mut pending_tools: Vec<String> = Vec::new();
    let mut pending_bash: Vec<String> = Vec::new();
    let mut calls = Vec::new();

    for line in iter {
        let Ok(entry) = serde_json::from_str::<Entry>(&line) else {
            continue;
        };
        let Some(payload) = entry.payload else {
            continue;
        };

        match entry.kind.as_str() {
            "turn_context" => {
                if let Ok(ctx) = serde_json::from_value::<TurnContext>(payload) {
                    if let Some(m) = ctx.model {
                        if !m.is_empty() {
                            current_model = m;
                        }
                    }
                }
            }
            "response_item" => {
                if let Ok(item) = serde_json::from_value::<ResponseItem>(payload) {
                    if !matches!(item.kind.as_str(), "function_call" | "custom_tool_call") {
                        continue;
                    }
                    let Some(raw_name) = item.name else { continue };
                    let normalized = normalize_tool(&raw_name);
                    if raw_name == "exec_command" {
                        if let Some(args_str) = item.arguments.as_deref() {
                            if let Ok(args) = serde_json::from_str::<ExecArgs>(args_str) {
                                if let Some(cmd) = args.cmd {
                                    pending_bash.extend(jsonl::split_bash_commands(&cmd));
                                }
                            }
                        }
                    }
                    pending_tools.push(normalized);
                }
            }
            "event_msg" => {
                let Ok(event) = serde_json::from_value::<EventMsg>(payload) else {
                    continue;
                };
                if event.kind != "token_count" {
                    continue;
                }
                let Some(info) = event.info else { continue };
                let Some(last) = info.last_token_usage else {
                    continue;
                };
                let total = info.total_token_usage.unwrap_or(last);

                let timestamp_str = entry.timestamp.clone().unwrap_or_default();
                let dedup_key = format!(
                    "codex:{}:{}:{}+{}",
                    source.path.display(),
                    timestamp_str,
                    total.input_tokens,
                    total.output_tokens
                );
                if !seen.insert(dedup_key.clone()) {
                    pending_tools.clear();
                    pending_bash.clear();
                    continue;
                }

                let model = if current_model.is_empty() {
                    DEFAULT_MODEL.to_string()
                } else {
                    current_model.clone()
                };

                let input_tokens = last.input_tokens.saturating_sub(last.cached_input_tokens);
                let output_tokens = last.output_tokens + last.reasoning_output_tokens;

                let mut call = ParsedCall {
                    tool: config::TOOL_ID,
                    model: model.clone(),
                    input_tokens,
                    output_tokens,
                    cache_read_input_tokens: last.cached_input_tokens,
                    cached_input_tokens: last.cached_input_tokens,
                    reasoning_tokens: last.reasoning_output_tokens,
                    speed: Speed::Standard,
                    tools: mem::take(&mut pending_tools),
                    bash_commands: mem::take(&mut pending_bash),
                    timestamp: parse_timestamp(&timestamp_str),
                    dedup_key,
                    session_id: session_id.clone(),
                    project: project.clone(),
                    ..ParsedCall::default()
                };

                call.cost_usd = pricing::cost(&model, &call, Speed::Standard);
                calls.push(call);
            }
            _ => {}
        }
    }

    Ok(calls)
}

pub fn parse_session_limits(source: &SessionSource) -> Result<Vec<LimitSnapshot>> {
    let lines = match jsonl::read_lines(&source.path) {
        Ok(l) => l,
        Err(_) => return Ok(Vec::new()),
    };

    let mut iter = lines.into_iter();

    let Some(first_raw) = iter.next() else {
        return Ok(Vec::new());
    };
    let Ok(first) = serde_json::from_str::<Entry>(&first_raw) else {
        return Ok(Vec::new());
    };
    if first.kind != "session_meta" {
        return Ok(Vec::new());
    }
    let meta: SessionMeta = first
        .payload
        .as_ref()
        .and_then(|p| serde_json::from_value(p.clone()).ok())
        .unwrap_or_default();
    if !meta
        .originator
        .as_deref()
        .map(|o| o.to_lowercase().contains("codex"))
        .unwrap_or(false)
    {
        return Ok(Vec::new());
    }

    let mut snapshots = Vec::new();
    for line in iter {
        let Ok(entry) = serde_json::from_str::<Entry>(&line) else {
            continue;
        };
        if entry.kind != "event_msg" {
            continue;
        }
        let Some(payload) = entry.payload else {
            continue;
        };
        let Ok(event) = serde_json::from_value::<EventMsg>(payload) else {
            continue;
        };
        if event.kind != "token_count" {
            continue;
        }
        let Some(rate_limits) = event.rate_limits else {
            continue;
        };
        if rate_limits.limit_id.is_empty() {
            continue;
        }

        snapshots.push(rate_limits.into_snapshot(
            config::TOOL_ID,
            entry.timestamp.as_deref().and_then(parse_timestamp),
        ));
    }

    Ok(snapshots)
}

impl RateLimits {
    fn into_snapshot(
        self,
        tool: &'static str,
        observed_at: Option<DateTime<Utc>>,
    ) -> LimitSnapshot {
        LimitSnapshot {
            tool,
            limit_id: self.limit_id,
            limit_name: self.limit_name.filter(|s| !s.is_empty()),
            plan_type: self.plan_type.filter(|s| !s.is_empty()),
            observed_at,
            primary: self.primary.map(Into::into),
            secondary: self.secondary.map(Into::into),
            credits: self.credits.map(Into::into),
            rate_limit_reached_type: self.rate_limit_reached_type.filter(|s| !s.is_empty()),
        }
    }
}

impl From<RateLimitWindow> for LimitWindow {
    fn from(value: RateLimitWindow) -> Self {
        Self {
            used_percent: value.used_percent,
            window_minutes: value.window_minutes,
            resets_at: value
                .resets_at
                .and_then(|seconds| DateTime::from_timestamp(seconds, 0)),
        }
    }
}

impl From<RateLimitCredits> for LimitCredits {
    fn from(value: RateLimitCredits) -> Self {
        Self {
            has_credits: value.has_credits,
            unlimited: value.unlimited,
            balance: value.balance,
        }
    }
}

fn normalize_tool(name: &str) -> String {
    match name {
        "exec_command" => "Bash".to_string(),
        "read_file" => "Read".to_string(),
        "write_file" | "apply_patch" | "apply_diff" => "Edit".to_string(),
        "web_search" => "WebSearch".to_string(),
        other => other.to_string(),
    }
}

fn parse_timestamp(s: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|d| d.with_timezone(&Utc))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_session(lines: &[&str]) -> tempfile_lite::TempFile {
        let f = tempfile_lite::TempFile::new("rollout-test.jsonl");
        let mut h = std::fs::File::create(f.path()).unwrap();
        for line in lines {
            writeln!(h, "{}", line).unwrap();
        }
        f
    }

    fn source_for(path: std::path::PathBuf) -> SessionSource {
        SessionSource {
            path,
            project: "2026/03/29".into(),
            tool: config::TOOL_ID,
        }
    }

    const META_OK: &str = r#"{"timestamp":"2026-03-29T15:04:01.475Z","type":"session_meta","payload":{"id":"sess-1","cwd":"/Users/me/proj","originator":"Codex Desktop"}}"#;
    const TURN_GPT5: &str = r#"{"timestamp":"2026-03-29T15:04:01.477Z","type":"turn_context","payload":{"model":"gpt-5"}}"#;
    const TURN_O3: &str = r#"{"timestamp":"2026-03-29T15:04:30.000Z","type":"turn_context","payload":{"model":"o3"}}"#;
    const EXEC_LS: &str = r#"{"timestamp":"2026-03-29T15:04:05.000Z","type":"response_item","payload":{"type":"function_call","name":"exec_command","arguments":"{\"cmd\":\"ls -la | grep foo\"}","call_id":"c1"}}"#;
    const TOKEN_NULL: &str = r#"{"timestamp":"2026-03-29T15:04:01.591Z","type":"event_msg","payload":{"type":"token_count","info":null}}"#;
    const TOKEN_FIRST: &str = r#"{"timestamp":"2026-03-29T15:04:10.090Z","type":"event_msg","payload":{"type":"token_count","info":{"last_token_usage":{"input_tokens":18193,"cached_input_tokens":10624,"output_tokens":371,"reasoning_output_tokens":38,"total_tokens":18564},"total_token_usage":{"input_tokens":18193,"cached_input_tokens":10624,"output_tokens":371,"reasoning_output_tokens":38,"total_tokens":18564}}}}"#;
    const TOKEN_SECOND: &str = r#"{"timestamp":"2026-03-29T15:05:00.000Z","type":"event_msg","payload":{"type":"token_count","info":{"last_token_usage":{"input_tokens":21590,"cached_input_tokens":10624,"output_tokens":375,"reasoning_output_tokens":12,"total_tokens":21965},"total_token_usage":{"input_tokens":39783,"cached_input_tokens":21248,"output_tokens":746,"reasoning_output_tokens":50,"total_tokens":40529}}}}"#;
    const TOKEN_LIMIT_NULL: &str = r#"{"timestamp":"2026-04-29T07:59:08.887Z","type":"event_msg","payload":{"type":"token_count","info":null,"rate_limits":{"limit_id":"codex","limit_name":null,"primary":{"used_percent":17.0,"window_minutes":300,"resets_at":1777477636},"secondary":{"used_percent":6.0,"window_minutes":10080,"resets_at":1777960801},"credits":null,"plan_type":"prolite","rate_limit_reached_type":null}}}"#;
    const TOKEN_LIMIT_MODEL: &str = r#"{"timestamp":"2026-04-29T07:59:28.815Z","type":"event_msg","payload":{"type":"token_count","info":{"last_token_usage":{"input_tokens":18193,"cached_input_tokens":10624,"output_tokens":371,"reasoning_output_tokens":38,"total_tokens":18564},"total_token_usage":{"input_tokens":18193,"cached_input_tokens":10624,"output_tokens":371,"reasoning_output_tokens":38,"total_tokens":18564}},"rate_limits":{"limit_id":"codex_bengalfox","limit_name":"GPT-5.3-Codex-Spark","primary":{"used_percent":0.0,"window_minutes":300,"resets_at":1777487853},"secondary":{"used_percent":0.0,"window_minutes":10080,"resets_at":1778074653},"credits":{"has_credits":false,"unlimited":false,"balance":null},"plan_type":null,"rate_limit_reached_type":null}}}"#;

    #[test]
    fn parses_basic_session() {
        let f = write_session(&[META_OK, TURN_GPT5, TOKEN_NULL, TOKEN_FIRST]);
        let mut seen = HashSet::new();
        let calls = parse_session(&source_for(f.path().to_path_buf()), &mut seen).unwrap();

        assert_eq!(calls.len(), 1, "null-info token_count must be skipped");
        let c = &calls[0];
        assert_eq!(c.model, "gpt-5");
        assert_eq!(c.input_tokens, 18193 - 10624, "cached must be subtracted");
        assert_eq!(c.output_tokens, 371 + 38, "reasoning folded into output");
        assert_eq!(c.cache_read_input_tokens, 10624);
        assert_eq!(c.reasoning_tokens, 38);
        assert_eq!(c.cache_creation_input_tokens, 0);
        assert_eq!(c.project, "/Users/me/proj");
        assert!(c.cost_usd > 0.0);
        assert_eq!(c.speed, Speed::Standard);
    }

    #[test]
    fn turn_context_switches_model_mid_stream() {
        let f = write_session(&[META_OK, TURN_GPT5, TOKEN_FIRST, TURN_O3, TOKEN_SECOND]);
        let mut seen = HashSet::new();
        let calls = parse_session(&source_for(f.path().to_path_buf()), &mut seen).unwrap();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].model, "gpt-5");
        assert_eq!(calls[1].model, "o3");
    }

    #[test]
    fn exec_command_populates_tools_and_bash() {
        let f = write_session(&[META_OK, TURN_GPT5, EXEC_LS, TOKEN_FIRST]);
        let mut seen = HashSet::new();
        let calls = parse_session(&source_for(f.path().to_path_buf()), &mut seen).unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tools, vec!["Bash"]);
        assert_eq!(calls[0].bash_commands, vec!["ls -la", "grep foo"]);
    }

    #[test]
    fn extracts_rate_limits_from_null_and_normal_token_counts() {
        let f = write_session(&[META_OK, TURN_GPT5, TOKEN_LIMIT_NULL, TOKEN_LIMIT_MODEL]);

        let limits = parse_session_limits(&source_for(f.path().to_path_buf())).unwrap();

        assert_eq!(limits.len(), 2);
        assert_eq!(limits[0].limit_id, "codex");
        assert_eq!(limits[0].limit_name, None);
        assert_eq!(limits[0].plan_type.as_deref(), Some("prolite"));
        assert_eq!(limits[0].primary.unwrap().used_percent, 17.0);
        assert_eq!(limits[0].primary.unwrap().window_minutes, 300);
        assert!(limits[0].primary.unwrap().resets_at.is_some());
        assert_eq!(limits[0].secondary.unwrap().used_percent, 6.0);
        assert_eq!(limits[1].limit_id, "codex_bengalfox");
        assert_eq!(limits[1].limit_name.as_deref(), Some("GPT-5.3-Codex-Spark"));
        assert!(limits[1]
            .credits
            .as_ref()
            .is_some_and(|credits| !credits.has_credits));

        let mut seen = HashSet::new();
        let calls = parse_session(&source_for(f.path().to_path_buf()), &mut seen).unwrap();
        assert_eq!(calls.len(), 1, "normal call parsing must stay unchanged");
    }

    #[test]
    fn dedup_key_drops_repeats() {
        let f = write_session(&[META_OK, TURN_GPT5, TOKEN_FIRST]);
        let mut seen = HashSet::new();
        let first = parse_session(&source_for(f.path().to_path_buf()), &mut seen).unwrap();
        assert_eq!(first.len(), 1);
        let second = parse_session(&source_for(f.path().to_path_buf()), &mut seen).unwrap();
        assert!(second.is_empty(), "second pass must dedup against `seen`");
    }

    #[test]
    fn rejects_non_codex_first_line() {
        let bogus = r#"{"timestamp":"2026-03-29T15:04:01.475Z","type":"session_meta","payload":{"originator":"someone-else"}}"#;
        let f = write_session(&[bogus, TURN_GPT5, TOKEN_FIRST]);
        let mut seen = HashSet::new();
        let calls = parse_session(&source_for(f.path().to_path_buf()), &mut seen).unwrap();
        assert!(calls.is_empty());
    }

    #[test]
    fn rejects_missing_session_meta() {
        let f = write_session(&[TURN_GPT5, TOKEN_FIRST]);
        let mut seen = HashSet::new();
        let calls = parse_session(&source_for(f.path().to_path_buf()), &mut seen).unwrap();
        assert!(calls.is_empty());
    }

    mod tempfile_lite {
        use std::path::{Path, PathBuf};

        pub struct TempFile(PathBuf);

        impl TempFile {
            pub fn new(name: &str) -> Self {
                let path = std::env::temp_dir().join(format!(
                    "tokenuse-codex-{}-{}-{}",
                    std::process::id(),
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_nanos(),
                    name
                ));
                Self(path)
            }
            pub fn path(&self) -> &Path {
                &self.0
            }
        }

        impl Drop for TempFile {
            fn drop(&mut self) {
                let _ = std::fs::remove_file(&self.0);
            }
        }
    }
}
