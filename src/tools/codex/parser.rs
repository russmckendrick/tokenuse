use std::collections::HashSet;
use std::mem;

use chrono::{DateTime, Utc};
use color_eyre::Result;
use serde::{Deserialize, Deserializer};
use serde_json::Value;

use crate::pricing;
use crate::tools::{
    jsonl, CodeBlock, LimitCredits, LimitSnapshot, LimitWindow, ParsedCall, SessionSource, Speed,
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
    id: Option<String>,
    #[serde(default)]
    cwd: Option<String>,
    #[serde(default)]
    originator: Option<String>,
    /// Present when this rollout was forked from another session; the file
    /// then replays the parent's history before any new work.
    #[serde(default)]
    forked_from_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ResponseItem {
    #[serde(default, rename = "type")]
    kind: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    namespace: Option<String>,
    #[serde(default, alias = "input")]
    arguments: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
struct NestedToolCall<'a> {
    name: &'a str,
    arguments: &'a str,
}

#[derive(Debug, Deserialize)]
struct EventMsg {
    #[serde(default, rename = "type")]
    kind: String,
    #[serde(default)]
    info: Option<TokenInfo>,
    /// `user_message` / `agent_message` text.
    #[serde(default)]
    message: Option<String>,
    /// `patch_apply_end` outcome.
    #[serde(default)]
    success: Option<bool>,
    /// `patch_apply_end` per-file changes keyed by absolute path.
    #[serde(default)]
    changes: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct RateLimitEvent {
    #[serde(default, rename = "type")]
    kind: String,
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
    #[serde(default, alias = "cache_read_input_tokens")]
    cached_input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
    #[serde(default)]
    reasoning_output_tokens: u64,
    #[serde(default)]
    total_tokens: u64,
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
    #[serde(default, deserialize_with = "deserialize_optional_f64")]
    balance: Option<f64>,
}

fn deserialize_optional_f64<'de, D>(deserializer: D) -> std::result::Result<Option<f64>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<Value>::deserialize(deserializer)?;
    Ok(value.and_then(|value| match value {
        Value::Number(number) => number.as_f64(),
        Value::String(number) => number.trim().parse::<f64>().ok(),
        _ => None,
    }))
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
    // Dedup keys are addressed by the session lineage, not the file path: a
    // forked rollout replays the parent's history verbatim, so keying the
    // fork by its parent id makes every replayed event collide with the
    // parent's rows regardless of which file carries it.
    let root_id = [meta.forked_from_id.as_deref(), meta.id.as_deref()]
        .into_iter()
        .flatten()
        .find(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| session_id.clone());
    // Usage stamped within 5s of a fork's creation is the replay being
    // written out, even when the parent's rows are no longer around to
    // collide with.
    let fork_cutoff = match meta.forked_from_id.as_deref() {
        Some(id) if !id.is_empty() => first
            .timestamp
            .as_deref()
            .and_then(parse_timestamp)
            .map(|ts| ts + chrono::Duration::seconds(5)),
        _ => None,
    };
    let project = meta
        .cwd
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| source.project.clone());

    let mut current_model = String::new();
    let mut pending_tools: Vec<String> = Vec::new();
    let mut pending_bash: Vec<String> = Vec::new();
    let mut pending_code_blocks: Vec<CodeBlock> = Vec::new();
    let mut pending_edited: Vec<String> = Vec::new();
    let mut pending_response_chars: Option<u64> = None;
    let mut last_user_text = String::new();
    let mut last_user_chars: Option<u64> = None;
    let mut last_user_ts: Option<DateTime<Utc>> = None;
    let mut previous_total_usage: Option<TokenUsage> = None;
    // Legacy path-based dedup keys of events that no longer emit (fork
    // replays, cross-file duplicates) plus each emitted call's own prior key;
    // the next emitted call carries them so the archive retires the rows the
    // pre-v6 parser wrote.
    let mut pending_superseded: Vec<String> = Vec::new();
    let mut calls: Vec<ParsedCall> = Vec::new();

    for line in iter {
        let Ok(entry) = serde_json::from_str::<Entry>(&line) else {
            continue;
        };
        let Some(payload) = entry.payload else {
            continue;
        };

        match entry.kind.as_str() {
            "turn_context" => {
                if let Some(model) = extract_model(&payload) {
                    current_model = model;
                }
            }
            "response_item" => {
                if let Ok(item) = serde_json::from_value::<ResponseItem>(payload) {
                    if !matches!(item.kind.as_str(), "function_call" | "custom_tool_call") {
                        continue;
                    }
                    let Some(raw_name) = item.name else { continue };
                    record_tool_activity(
                        &raw_name,
                        item.namespace.as_deref(),
                        item.arguments.as_deref(),
                        &mut pending_tools,
                        &mut pending_bash,
                    );
                }
            }
            "event_msg" => {
                let model_hint = extract_model(&payload);
                let Ok(event) = serde_json::from_value::<EventMsg>(payload) else {
                    continue;
                };
                match event.kind.as_str() {
                    "user_message" => {
                        let text = event.message.as_deref().map(str::trim).unwrap_or("");
                        if !text.is_empty() {
                            last_user_chars = Some(text.chars().count() as u64);
                            last_user_text = jsonl::truncate_chars(text, 500);
                            last_user_ts = entry.timestamp.as_deref().and_then(parse_timestamp);
                        }
                        continue;
                    }
                    // Reasoning text (`agent_reasoning`) is deliberately not
                    // counted, mirroring how Claude thinking blocks are
                    // excluded from response_chars.
                    "agent_message" => {
                        if let Some(message) = event.message.as_deref() {
                            *pending_response_chars.get_or_insert(0) +=
                                message.chars().count() as u64;
                            pending_code_blocks.extend(jsonl::extract_code_fences(message));
                        }
                        continue;
                    }
                    "turn_aborted" => {
                        if let Some(last) = calls.last_mut() {
                            last.is_canceled = true;
                        }
                        continue;
                    }
                    "patch_apply_end" => {
                        if event.success.unwrap_or(false) {
                            record_patch_changes(
                                event.changes.as_ref(),
                                &mut pending_edited,
                                &mut pending_code_blocks,
                            );
                        }
                        continue;
                    }
                    "token_count" => {}
                    _ => continue,
                }
                if let Some(model) = model_hint {
                    current_model = model;
                }
                let Some(info) = event.info else { continue };
                let usage = match (info.total_token_usage, info.last_token_usage) {
                    (Some(total), _) => total.saturating_delta(previous_total_usage),
                    (None, Some(last)) => last,
                    (None, None) => continue,
                };
                let total = info.total_token_usage.unwrap_or(usage);
                if let Some(total_usage) = info.total_token_usage {
                    previous_total_usage = Some(total_usage);
                }
                if usage.input_tokens == 0
                    && usage.cached_input_tokens == 0
                    && usage.output_tokens == 0
                    && usage.reasoning_output_tokens == 0
                {
                    pending_tools.clear();
                    pending_bash.clear();
                    pending_code_blocks.clear();
                    pending_edited.clear();
                    pending_response_chars = None;
                    continue;
                }

                let timestamp_str = entry.timestamp.clone().unwrap_or_default();
                // The key the pre-v6 parser gave this event; carried on the
                // next emitted call so the archive retires that row.
                let legacy_key = format!(
                    "codex:{}:{}:{}+{}",
                    source.path.display(),
                    timestamp_str,
                    total.input_tokens,
                    total.output_tokens
                );
                let timestamp = parse_timestamp(&timestamp_str);

                if let Some(cutoff) = fork_cutoff {
                    if timestamp.is_some_and(|ts| ts < cutoff) {
                        // Replayed parent history: the delta state above has
                        // already absorbed it, so post-fork deltas stay
                        // correct; nothing is emitted.
                        pending_superseded.push(legacy_key);
                        pending_tools.clear();
                        pending_bash.clear();
                        pending_code_blocks.clear();
                        pending_edited.clear();
                        pending_response_chars = None;
                        continue;
                    }
                }

                // Cumulative totals are copied verbatim into replays, so a
                // content-addressed key collides across files and rewritten
                // timestamps. Only the last-usage fallback, which has no
                // cumulative totals to address, still needs the timestamp.
                let dedup_key = if info.total_token_usage.is_some() {
                    format!(
                        "codex:{root_id}:{}:{}:{}:{}:{}",
                        total.input_tokens,
                        total.cached_input_tokens,
                        total.output_tokens,
                        total.reasoning_output_tokens,
                        total.total_tokens
                    )
                } else {
                    format!(
                        "codex:{root_id}:last:{timestamp_str}:{}:{}:{}:{}",
                        usage.input_tokens,
                        usage.cached_input_tokens,
                        usage.output_tokens,
                        usage.reasoning_output_tokens
                    )
                };
                if !seen.insert(dedup_key.clone()) {
                    pending_superseded.push(legacy_key);
                    pending_tools.clear();
                    pending_bash.clear();
                    pending_code_blocks.clear();
                    pending_edited.clear();
                    pending_response_chars = None;
                    continue;
                }

                let model = if current_model.is_empty() {
                    DEFAULT_MODEL.to_string()
                } else {
                    current_model.clone()
                };

                let input_tokens = usage.input_tokens.saturating_sub(usage.cached_input_tokens);
                let output_tokens = usage.output_tokens + usage.reasoning_output_tokens;

                let superseded_dedup_keys = {
                    let mut keys = mem::take(&mut pending_superseded);
                    keys.push(legacy_key);
                    keys
                };
                let mut call = ParsedCall {
                    tool: config::TOOL_ID,
                    model: model.clone(),
                    input_tokens,
                    output_tokens,
                    cache_read_input_tokens: usage.cached_input_tokens,
                    cached_input_tokens: usage.cached_input_tokens,
                    reasoning_tokens: usage.reasoning_output_tokens,
                    speed: Speed::Standard,
                    tools: mem::take(&mut pending_tools),
                    bash_commands: mem::take(&mut pending_bash),
                    timestamp,
                    dedup_key,
                    user_message: last_user_text.clone(),
                    session_id: session_id.clone(),
                    project: project.clone(),
                    prompt_chars: last_user_chars,
                    response_chars: pending_response_chars.take(),
                    elapsed_ms: jsonl::turn_elapsed_ms(timestamp, last_user_ts),
                    code_blocks: jsonl::merge_code_blocks(mem::take(&mut pending_code_blocks)),
                    edited_files: jsonl::dedup_files(mem::take(&mut pending_edited)),
                    superseded_dedup_keys,
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
        let Ok(event) = serde_json::from_value::<RateLimitEvent>(payload) else {
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
            total: None,
            additional_usage: None,
        }
    }
}

impl TokenUsage {
    fn saturating_delta(self, previous: Option<Self>) -> Self {
        let Some(previous) = previous else {
            return self;
        };
        Self {
            input_tokens: self.input_tokens.saturating_sub(previous.input_tokens),
            cached_input_tokens: self
                .cached_input_tokens
                .saturating_sub(previous.cached_input_tokens),
            output_tokens: self.output_tokens.saturating_sub(previous.output_tokens),
            reasoning_output_tokens: self
                .reasoning_output_tokens
                .saturating_sub(previous.reasoning_output_tokens),
            total_tokens: self.total_tokens.saturating_sub(previous.total_tokens),
        }
    }
}

/// Fold a successful `patch_apply_end` into pending enrichment: every touched
/// file becomes an edited-files entry and its unified diff's added lines count
/// as AI code output in the file's language.
fn record_patch_changes(
    changes: Option<&Value>,
    pending_edited: &mut Vec<String>,
    pending_code_blocks: &mut Vec<CodeBlock>,
) {
    let Some(changes) = changes.and_then(|c| c.as_object()) else {
        return;
    };
    for (path, change) in changes {
        pending_edited.push(path.clone());
        let added = change
            .get("unified_diff")
            .and_then(|d| d.as_str())
            .map(count_added_lines)
            .unwrap_or(0);
        if added > 0 {
            pending_code_blocks.push(CodeBlock {
                language: jsonl::extension_language(path),
                loc: added,
            });
        }
    }
}

fn count_added_lines(diff: &str) -> u64 {
    diff.lines()
        .filter(|line| line.starts_with('+') && !line.starts_with("+++"))
        .count() as u64
}

fn extract_model(payload: &Value) -> Option<String> {
    let obj = payload.as_object()?;
    obj.get("info")
        .and_then(|info| {
            let info = info.as_object()?;
            model_from_obj(info)
        })
        .or_else(|| model_from_obj(obj))
}

fn model_from_obj(obj: &serde_json::Map<String, Value>) -> Option<String> {
    non_empty_str(obj.get("model"))
        .or_else(|| non_empty_str(obj.get("model_name")))
        .or_else(|| {
            obj.get("metadata")
                .and_then(|m| m.as_object())
                .and_then(model_from_obj)
        })
}

fn non_empty_str(value: Option<&Value>) -> Option<String> {
    value
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string)
}

fn normalize_tool(name: &str, namespace: Option<&str>) -> String {
    if let Some(server) = namespace
        .and_then(|ns| ns.strip_prefix("mcp__"))
        .and_then(|rest| rest.strip_suffix("__"))
        .filter(|s| !s.is_empty())
    {
        return format!("mcp__{server}__{name}");
    }
    match name {
        "exec_command" => "Bash".to_string(),
        "read_file" => "Read".to_string(),
        "write_file" | "apply_patch" | "apply_diff" => "Edit".to_string(),
        "web_search" => "WebSearch".to_string(),
        other => other.to_string(),
    }
}

fn record_tool_activity(
    raw_name: &str,
    namespace: Option<&str>,
    arguments: Option<&str>,
    pending_tools: &mut Vec<String>,
    pending_bash: &mut Vec<String>,
) {
    if raw_name == "exec" {
        let nested = arguments.map(nested_tool_calls).unwrap_or_default();
        if !nested.is_empty() {
            for call in nested {
                if call.name == "exec_command" {
                    if let Some(cmd) = extract_js_cmd(call.arguments) {
                        pending_bash.extend(jsonl::split_bash_commands(&cmd));
                    }
                }
                pending_tools.push(normalize_tool(call.name, None));
            }
            return;
        }
    }

    if raw_name == "exec_command" {
        if let Some(args_str) = arguments {
            if let Ok(args) = serde_json::from_str::<ExecArgs>(args_str) {
                if let Some(cmd) = args.cmd {
                    pending_bash.extend(jsonl::split_bash_commands(&cmd));
                }
            }
        }
    }
    pending_tools.push(normalize_tool(raw_name, namespace));
}

fn nested_tool_calls(script: &str) -> Vec<NestedToolCall<'_>> {
    let bytes = script.as_bytes();
    let mut calls = Vec::new();
    let mut index = 0;

    while index < bytes.len() {
        if is_js_quote(bytes[index]) {
            index = skip_js_string(bytes, index);
            continue;
        }
        if let Some(next) = skip_js_comment(bytes, index) {
            index = next;
            continue;
        }
        if bytes[index..].starts_with(b"tools.")
            && (index == 0 || !is_js_identifier_byte(bytes[index - 1]))
        {
            let name_start = index + "tools.".len();
            let mut name_end = name_start;
            while name_end < bytes.len() && is_js_identifier_byte(bytes[name_end]) {
                name_end += 1;
            }
            let mut open = name_end;
            while open < bytes.len() && bytes[open].is_ascii_whitespace() {
                open += 1;
            }
            if name_end > name_start && bytes.get(open) == Some(&b'(') {
                if let Some(close) = matching_js_paren(bytes, open) {
                    calls.push(NestedToolCall {
                        name: &script[name_start..name_end],
                        arguments: &script[open + 1..close],
                    });
                }
            }
            index = name_end;
            continue;
        }
        index += 1;
    }

    calls
}

fn extract_js_cmd(arguments: &str) -> Option<String> {
    let bytes = arguments.as_bytes();
    let mut index = 0;

    while index < bytes.len() {
        if is_js_quote(bytes[index]) {
            index = skip_js_string(bytes, index);
            continue;
        }
        if let Some(next) = skip_js_comment(bytes, index) {
            index = next;
            continue;
        }
        if bytes[index..].starts_with(b"cmd")
            && (index == 0 || !is_js_identifier_byte(bytes[index - 1]))
            && bytes
                .get(index + 3)
                .is_none_or(|byte| !is_js_identifier_byte(*byte))
        {
            let mut value_start = index + 3;
            while bytes.get(value_start).is_some_and(u8::is_ascii_whitespace) {
                value_start += 1;
            }
            if bytes.get(value_start) != Some(&b':') {
                index += 3;
                continue;
            }
            value_start += 1;
            while bytes.get(value_start).is_some_and(u8::is_ascii_whitespace) {
                value_start += 1;
            }
            return parse_js_string(arguments, value_start).map(|(value, _)| value);
        }
        index += 1;
    }

    None
}

fn parse_js_string(input: &str, start: usize) -> Option<(String, usize)> {
    let bytes = input.as_bytes();
    let quote = *bytes.get(start)?;
    if !is_js_quote(quote) {
        return None;
    }
    let end = skip_js_string(bytes, start);
    if end > bytes.len() || bytes.get(end - 1) != Some(&quote) {
        return None;
    }
    let literal = &input[start..end];
    if quote == b'"' {
        return serde_json::from_str(literal).ok().map(|value| (value, end));
    }

    let mut value = String::new();
    let mut chars = input[start + 1..end - 1].chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            value.push(ch);
            continue;
        }
        let escaped = chars.next()?;
        value.push(match escaped {
            'n' => '\n',
            'r' => '\r',
            't' => '\t',
            other => other,
        });
    }
    Some((value, end))
}

fn matching_js_paren(bytes: &[u8], open: usize) -> Option<usize> {
    let mut depth = 0;
    let mut index = open;
    while index < bytes.len() {
        if is_js_quote(bytes[index]) {
            index = skip_js_string(bytes, index);
            continue;
        }
        if let Some(next) = skip_js_comment(bytes, index) {
            index = next;
            continue;
        }
        match bytes[index] {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
        index += 1;
    }
    None
}

fn skip_js_string(bytes: &[u8], start: usize) -> usize {
    let quote = bytes[start];
    let mut index = start + 1;
    while index < bytes.len() {
        if bytes[index] == b'\\' {
            index = (index + 2).min(bytes.len());
        } else if bytes[index] == quote {
            return index + 1;
        } else {
            index += 1;
        }
    }
    bytes.len()
}

fn skip_js_comment(bytes: &[u8], start: usize) -> Option<usize> {
    if bytes.get(start) != Some(&b'/') {
        return None;
    }
    match bytes.get(start + 1) {
        Some(b'/') => Some(
            bytes[start + 2..]
                .iter()
                .position(|byte| *byte == b'\n')
                .map_or(bytes.len(), |offset| start + 3 + offset),
        ),
        Some(b'*') => Some(
            bytes[start + 2..]
                .windows(2)
                .position(|window| window == b"*/")
                .map_or(bytes.len(), |offset| start + 4 + offset),
        ),
        _ => None,
    }
}

fn is_js_quote(byte: u8) -> bool {
    matches!(byte, b'\'' | b'"' | b'`')
}

fn is_js_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'$')
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
        SessionSource::session(path, "2026/03/29", config::TOOL_ID)
    }

    const META_OK: &str = r#"{"timestamp":"2026-03-29T15:04:01.475Z","type":"session_meta","payload":{"id":"sess-1","cwd":"/Users/me/proj","originator":"Codex Desktop"}}"#;
    const TURN_GPT5: &str = r#"{"timestamp":"2026-03-29T15:04:01.477Z","type":"turn_context","payload":{"model":"gpt-5"}}"#;
    const TURN_O3: &str = r#"{"timestamp":"2026-03-29T15:04:30.000Z","type":"turn_context","payload":{"model":"o3"}}"#;
    const EXEC_LS: &str = r#"{"timestamp":"2026-03-29T15:04:05.000Z","type":"response_item","payload":{"type":"function_call","name":"exec_command","arguments":"{\"cmd\":\"ls -la | grep foo\"}","call_id":"c1"}}"#;
    const TOKEN_NULL: &str = r#"{"timestamp":"2026-03-29T15:04:01.591Z","type":"event_msg","payload":{"type":"token_count","info":null}}"#;
    const TOKEN_FIRST: &str = r#"{"timestamp":"2026-03-29T15:04:10.090Z","type":"event_msg","payload":{"type":"token_count","info":{"last_token_usage":{"input_tokens":18193,"cached_input_tokens":10624,"output_tokens":371,"reasoning_output_tokens":38,"total_tokens":18564},"total_token_usage":{"input_tokens":18193,"cached_input_tokens":10624,"output_tokens":371,"reasoning_output_tokens":38,"total_tokens":18564}}}}"#;
    const TOKEN_FIRST_DUPLICATE: &str = r#"{"timestamp":"2026-03-29T15:04:11.000Z","type":"event_msg","payload":{"type":"token_count","info":{"last_token_usage":{"input_tokens":18193,"cached_input_tokens":10624,"output_tokens":371,"reasoning_output_tokens":38,"total_tokens":18564},"total_token_usage":{"input_tokens":18193,"cached_input_tokens":10624,"output_tokens":371,"reasoning_output_tokens":38,"total_tokens":18564}}}}"#;
    const TOKEN_SECOND: &str = r#"{"timestamp":"2026-03-29T15:05:00.000Z","type":"event_msg","payload":{"type":"token_count","info":{"last_token_usage":{"input_tokens":21590,"cached_input_tokens":10624,"output_tokens":375,"reasoning_output_tokens":12,"total_tokens":21965},"total_token_usage":{"input_tokens":39783,"cached_input_tokens":21248,"output_tokens":746,"reasoning_output_tokens":50,"total_tokens":40529}}}}"#;
    const TOKEN_TOTAL_ONLY_FIRST: &str = r#"{"timestamp":"2026-03-29T15:04:10.090Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":100,"cache_read_input_tokens":20,"output_tokens":10,"reasoning_output_tokens":1,"total_tokens":111}}}}"#;
    const TOKEN_TOTAL_ONLY_SECOND: &str = r#"{"timestamp":"2026-03-29T15:05:00.000Z","type":"event_msg","payload":{"type":"token_count","info":{"model":"gpt-5.4","total_token_usage":{"input_tokens":180,"cache_read_input_tokens":50,"output_tokens":30,"reasoning_output_tokens":4,"total_tokens":214}}}}"#;
    const USER_MSG: &str = r#"{"timestamp":"2026-03-29T15:04:02.000Z","type":"event_msg","payload":{"type":"user_message","message":"please fix the tests\n","images":[]}}"#;
    const AGENT_MSG: &str = r#"{"timestamp":"2026-03-29T15:04:08.000Z","type":"event_msg","payload":{"type":"agent_message","message":"Fixing:\n```rust\nassert!(ok);\n```","phase":"commentary"}}"#;
    const AGENT_REASONING: &str = r#"{"timestamp":"2026-03-29T15:04:07.000Z","type":"event_msg","payload":{"type":"agent_reasoning","text":"thinking about the failing assertion in depth"}}"#;
    const PATCH_END: &str = r#"{"timestamp":"2026-03-29T15:04:09.000Z","type":"event_msg","payload":{"type":"patch_apply_end","call_id":"c9","success":true,"stdout":"Success.","changes":{"/proj/src/lib.rs":{"type":"update","unified_diff":"+++ b/src/lib.rs\n@@\n+let a = 1;\n+let b = 2;\n-let old = 0;\n"}}}}"#;
    const PATCH_END_FAILED: &str = r#"{"timestamp":"2026-03-29T15:04:09.500Z","type":"event_msg","payload":{"type":"patch_apply_end","call_id":"c10","success":false,"changes":{"/proj/src/broken.rs":{"type":"update","unified_diff":"+nope\n"}}}}"#;
    const TURN_ABORTED: &str = r#"{"timestamp":"2026-03-29T15:04:20.000Z","type":"event_msg","payload":{"type":"turn_aborted","turn_id":"t1","reason":"interrupted","duration_ms":13921}}"#;
    const TOKEN_LIMIT_NULL: &str = r#"{"timestamp":"2026-04-29T07:59:08.887Z","type":"event_msg","payload":{"type":"token_count","info":null,"rate_limits":{"limit_id":"codex","limit_name":null,"primary":{"used_percent":17.0,"window_minutes":300,"resets_at":1777477636},"secondary":{"used_percent":6.0,"window_minutes":10080,"resets_at":1777960801},"credits":null,"plan_type":"prolite","rate_limit_reached_type":null}}}"#;
    const TOKEN_LIMIT_MODEL: &str = r#"{"timestamp":"2026-04-29T07:59:28.815Z","type":"event_msg","payload":{"type":"token_count","info":{"last_token_usage":{"input_tokens":18193,"cached_input_tokens":10624,"output_tokens":371,"reasoning_output_tokens":38,"total_tokens":18564},"total_token_usage":{"input_tokens":18193,"cached_input_tokens":10624,"output_tokens":371,"reasoning_output_tokens":38,"total_tokens":18564}},"rate_limits":{"limit_id":"codex_bengalfox","limit_name":"GPT-5.3-Codex-Spark","primary":{"used_percent":0.0,"window_minutes":300,"resets_at":1777487853},"secondary":{"used_percent":0.0,"window_minutes":10080,"resets_at":1778074653},"credits":{"has_credits":false,"unlimited":false,"balance":null},"plan_type":null,"rate_limit_reached_type":null}}}"#;
    const TOKEN_LIMIT_STRING_BALANCE: &str = r#"{"timestamp":"2026-07-16T16:05:42.263Z","type":"event_msg","payload":{"type":"token_count","info":{"last_token_usage":{"input_tokens":19737,"cached_input_tokens":3840,"output_tokens":179,"reasoning_output_tokens":57,"total_tokens":19916},"total_token_usage":{"input_tokens":19737,"cached_input_tokens":3840,"output_tokens":179,"reasoning_output_tokens":57,"total_tokens":19916}},"rate_limits":{"limit_id":"codex","limit_name":null,"primary":{"used_percent":10.0,"window_minutes":10080,"resets_at":1784793140},"secondary":null,"credits":{"has_credits":false,"unlimited":false,"balance":"0"},"individual_limit":null,"plan_type":"prolite","rate_limit_reached_type":null}}}"#;

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
    fn total_only_usage_deltas_cache_alias_and_info_model_are_supported() {
        let f = write_session(&[META_OK, TOKEN_TOTAL_ONLY_FIRST, TOKEN_TOTAL_ONLY_SECOND]);
        let mut seen = HashSet::new();

        let calls = parse_session(&source_for(f.path().to_path_buf()), &mut seen).unwrap();

        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].input_tokens, 80);
        assert_eq!(calls[0].cache_read_input_tokens, 20);
        assert_eq!(calls[0].output_tokens, 11);
        assert_eq!(calls[1].model, "gpt-5.4");
        assert_eq!(calls[1].input_tokens, 50);
        assert_eq!(calls[1].cache_read_input_tokens, 30);
        assert_eq!(calls[1].output_tokens, 23);
        assert_eq!(calls[1].reasoning_tokens, 3);
    }

    #[test]
    fn duplicate_cumulative_token_counts_do_not_emit_usage() {
        let f = write_session(&[
            META_OK,
            TURN_GPT5,
            TOKEN_FIRST,
            TOKEN_FIRST_DUPLICATE,
            TOKEN_SECOND,
        ]);
        let mut seen = HashSet::new();

        let calls = parse_session(&source_for(f.path().to_path_buf()), &mut seen).unwrap();

        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].input_tokens, 18193 - 10624);
        assert_eq!(calls[0].output_tokens, 371 + 38);
        assert_eq!(calls[1].input_tokens, 21590 - 10624);
        assert_eq!(calls[1].output_tokens, 375 + 12);
    }

    #[test]
    fn duplicate_cumulative_token_counts_clear_pending_tools() {
        let f = write_session(&[
            META_OK,
            TURN_GPT5,
            TOKEN_FIRST,
            EXEC_LS,
            TOKEN_FIRST_DUPLICATE,
            TOKEN_SECOND,
        ]);
        let mut seen = HashSet::new();

        let calls = parse_session(&source_for(f.path().to_path_buf()), &mut seen).unwrap();

        assert_eq!(calls.len(), 2);
        assert!(calls[1].tools.is_empty());
        assert!(calls[1].bash_commands.is_empty());
    }

    #[test]
    fn enrichment_fields_capture_turn_shape() {
        let f = write_session(&[
            META_OK,
            TURN_GPT5,
            USER_MSG,
            EXEC_LS,
            AGENT_REASONING,
            AGENT_MSG,
            PATCH_END,
            PATCH_END_FAILED,
            TOKEN_FIRST,
            TURN_ABORTED,
        ]);
        let mut seen = HashSet::new();
        let calls = parse_session(&source_for(f.path().to_path_buf()), &mut seen).unwrap();
        assert_eq!(calls.len(), 1);

        let call = &calls[0];
        assert_eq!(call.user_message, "please fix the tests");
        assert_eq!(call.prompt_chars, Some(20));
        assert_eq!(
            call.elapsed_ms,
            Some(8_090),
            "token_count at 15:04:10.090 minus user_message at 15:04:02.000"
        );
        assert_eq!(
            call.response_chars,
            Some("Fixing:\n```rust\nassert!(ok);\n```".chars().count() as u64),
            "agent_reasoning text must not count"
        );
        assert_eq!(
            call.code_blocks,
            vec![CodeBlock {
                language: "rust".into(),
                loc: 3,
            }],
            "one fence line plus two added diff lines; failed patches and +++ headers don't count"
        );
        assert_eq!(call.edited_files, vec!["/proj/src/lib.rs"]);
        assert!(
            call.is_canceled,
            "turn_aborted cancels the last emitted call"
        );
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
    fn mcp_namespace_yields_canonical_tool_name() {
        const MCP_CALL: &str = r#"{"timestamp":"2026-03-29T15:04:06.000Z","type":"response_item","payload":{"type":"function_call","name":"search_graph","namespace":"mcp__codebase_memory_mcp__","arguments":"{}","call_id":"c2"}}"#;
        let f = write_session(&[META_OK, TURN_GPT5, MCP_CALL, TOKEN_FIRST]);
        let mut seen = HashSet::new();
        let calls = parse_session(&source_for(f.path().to_path_buf()), &mut seen).unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(
            calls[0].tools,
            vec!["mcp__codebase_memory_mcp__search_graph"]
        );
    }

    #[test]
    fn exec_wrapper_exposes_nested_shell_and_mcp_activity() {
        const EXEC_WRAPPER: &str = r#"{"timestamp":"2026-03-29T15:04:06.000Z","type":"response_item","payload":{"type":"custom_tool_call","name":"exec","input":"const rs = await Promise.all([tools.exec_command({cmd:\"cargo fmt --check && cargo test\"}), tools.mcp__codebase_memory_mcp__search_graph({project:\"tokens\", query:\"Codex parser\"})]);","call_id":"c2"}}"#;
        let f = write_session(&[META_OK, TURN_GPT5, EXEC_WRAPPER, TOKEN_FIRST]);
        let mut seen = HashSet::new();
        let calls = parse_session(&source_for(f.path().to_path_buf()), &mut seen).unwrap();

        assert_eq!(
            (&calls[0].tools, &calls[0].bash_commands),
            (
                &vec![
                    "Bash".to_string(),
                    "mcp__codebase_memory_mcp__search_graph".to_string()
                ],
                &vec!["cargo fmt --check".to_string(), "cargo test".to_string()]
            )
        );
    }

    #[test]
    fn nested_tool_scanner_ignores_string_and_comment_examples() {
        let script = r#"
            const example = "tools.exec_command({cmd: 'not real'})";
            // tools.mcp__fake__search({});
            await tools.apply_patch("*** Begin Patch");
        "#;

        assert_eq!(
            nested_tool_calls(script),
            vec![NestedToolCall {
                name: "apply_patch",
                arguments: r#""*** Begin Patch""#,
            }]
        );
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
    fn string_credit_balance_is_parsed_from_new_desktop_shape() {
        let f = write_session(&[META_OK, TURN_GPT5, TOKEN_LIMIT_STRING_BALANCE]);

        let limits = parse_session_limits(&source_for(f.path().to_path_buf())).unwrap();
        let limit = &limits[0];

        assert_eq!(
            (
                limits.len(),
                limit.primary.as_ref().map(|window| window.window_minutes),
                limit.secondary.is_none(),
                limit.credits.as_ref().and_then(|credits| credits.balance),
            ),
            (1, Some(10_080), true, Some(0.0))
        );
    }

    #[test]
    fn numeric_credit_balance_remains_supported() {
        let credits: RateLimitCredits =
            serde_json::from_str(r#"{"has_credits":true,"balance":12.5}"#).unwrap();

        assert_eq!(credits.balance, Some(12.5));
    }

    #[test]
    fn null_credit_balance_remains_supported() {
        let credits: RateLimitCredits =
            serde_json::from_str(r#"{"has_credits":false,"balance":null}"#).unwrap();

        assert_eq!(credits.balance, None);
    }

    #[test]
    fn malformed_credit_balance_does_not_suppress_usage() {
        let malformed = TOKEN_LIMIT_STRING_BALANCE.replace("\"0\"", "\"not-a-number\"");
        let f = write_session(&[META_OK, TURN_GPT5, &malformed]);
        let mut seen = HashSet::new();

        let calls = parse_session(&source_for(f.path().to_path_buf()), &mut seen).unwrap();

        assert_eq!(calls.len(), 1);
    }

    #[test]
    fn malformed_credit_balance_is_ignored_in_limit_snapshot() {
        let malformed = TOKEN_LIMIT_STRING_BALANCE.replace("\"0\"", "\"not-a-number\"");
        let f = write_session(&[META_OK, TURN_GPT5, &malformed]);

        let limits = parse_session_limits(&source_for(f.path().to_path_buf())).unwrap();

        assert_eq!(
            limits[0]
                .credits
                .as_ref()
                .and_then(|credits| credits.balance),
            None
        );
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

    fn token_total_only(ts: &str, input: u64, output: u64, total: u64) -> String {
        format!(
            r#"{{"timestamp":"{ts}","type":"event_msg","payload":{{"type":"token_count","info":{{"total_token_usage":{{"input_tokens":{input},"output_tokens":{output},"total_tokens":{total}}}}}}}}}"#
        )
    }

    fn fork_meta(ts: &str, forked_from: &str) -> String {
        format!(
            r#"{{"timestamp":"{ts}","type":"session_meta","payload":{{"id":"sess-fork","cwd":"/Users/me/proj","originator":"Codex Desktop","forked_from_id":"{forked_from}"}}}}"#
        )
    }

    fn parent_meta(ts: &str) -> String {
        format!(
            r#"{{"timestamp":"{ts}","type":"session_meta","payload":{{"id":"sess-parent","cwd":"/Users/me/proj","originator":"Codex Desktop"}}}}"#
        )
    }

    fn write_session_owned(lines: &[String]) -> tempfile_lite::TempFile {
        let refs: Vec<&str> = lines.iter().map(|s| s.as_str()).collect();
        write_session(&refs)
    }

    #[test]
    fn fork_replay_within_cutoff_is_dropped_and_superseded() {
        let f = write_session_owned(&[
            fork_meta("2026-03-29T15:10:00Z", "sess-parent"),
            token_total_only("2026-03-29T15:10:01Z", 700, 0, 700),
            token_total_only("2026-03-29T15:10:03Z", 1100, 0, 1100),
            token_total_only("2026-03-29T15:10:30Z", 1500, 0, 1500),
        ]);
        let mut seen = HashSet::new();
        let calls = parse_session(&source_for(f.path().to_path_buf()), &mut seen).unwrap();

        assert_eq!(calls.len(), 1, "replayed history must not emit");
        assert_eq!(
            calls[0].input_tokens, 400,
            "delta state must advance through the dropped replay"
        );
        assert_eq!(
            calls[0].superseded_dedup_keys.len(),
            3,
            "two replay legacy keys plus the call's own legacy key"
        );
        let path = f.path().display().to_string();
        assert!(calls[0]
            .superseded_dedup_keys
            .contains(&format!("codex:{path}:2026-03-29T15:10:01Z:700+0")));
        assert!(calls[0]
            .superseded_dedup_keys
            .contains(&format!("codex:{path}:2026-03-29T15:10:30Z:1500+0")));
    }

    #[test]
    fn fork_replay_past_cutoff_collides_with_parent() {
        let parent = write_session_owned(&[
            parent_meta("2026-03-29T15:00:00Z"),
            token_total_only("2026-03-29T15:00:01Z", 700, 0, 700),
            token_total_only("2026-03-29T15:00:02Z", 1100, 0, 1100),
        ]);
        // The fork replays the parent's events with rewritten timestamps well
        // past the 5s cutoff, then adds genuine work.
        let fork = write_session_owned(&[
            fork_meta("2026-03-29T15:10:00Z", "sess-parent"),
            token_total_only("2026-03-29T15:20:00Z", 700, 0, 700),
            token_total_only("2026-03-29T15:20:01Z", 1100, 0, 1100),
            token_total_only("2026-03-29T15:20:30Z", 1500, 0, 1500),
        ]);

        let mut seen = HashSet::new();
        let parent_calls =
            parse_session(&source_for(parent.path().to_path_buf()), &mut seen).unwrap();
        let fork_calls = parse_session(&source_for(fork.path().to_path_buf()), &mut seen).unwrap();

        assert_eq!(parent_calls.len(), 2);
        assert_eq!(
            fork_calls.len(),
            1,
            "replays keyed by lineage collide with the parent's rows"
        );
        assert_eq!(fork_calls[0].input_tokens, 400);
    }

    #[test]
    fn divergent_fork_usage_sharing_a_cumulative_total_is_kept() {
        let parent = write_session_owned(&[
            parent_meta("2026-03-29T15:00:00Z"),
            token_total_only("2026-03-29T15:00:01Z", 700, 0, 700),
            token_total_only("2026-03-29T15:00:02Z", 1100, 0, 1100),
            token_total_only("2026-03-29T15:00:03Z", 1600, 0, 1600),
        ]);
        // Genuinely different post-fork work that happens to reach the same
        // cumulative total as the parent, but via output tokens.
        let fork = write_session_owned(&[
            fork_meta("2026-03-29T15:10:00Z", "sess-parent"),
            token_total_only("2026-03-29T15:20:00Z", 700, 0, 700),
            token_total_only("2026-03-29T15:20:01Z", 1100, 0, 1100),
            token_total_only("2026-03-29T15:20:30Z", 1100, 500, 1600),
        ]);

        let mut seen = HashSet::new();
        let parent_calls =
            parse_session(&source_for(parent.path().to_path_buf()), &mut seen).unwrap();
        let fork_calls = parse_session(&source_for(fork.path().to_path_buf()), &mut seen).unwrap();

        assert_eq!(parent_calls.len(), 3);
        assert_eq!(
            fork_calls.len(),
            1,
            "the key is the full cumulative breakdown, not the bare total"
        );
        assert_eq!(fork_calls[0].output_tokens, 500);
    }

    #[test]
    fn emitted_calls_carry_their_legacy_path_key() {
        let f = write_session(&[META_OK, TURN_GPT5, TOKEN_FIRST]);
        let mut seen = HashSet::new();
        let calls = parse_session(&source_for(f.path().to_path_buf()), &mut seen).unwrap();

        assert_eq!(calls.len(), 1);
        assert_eq!(
            calls[0].superseded_dedup_keys,
            vec![format!(
                "codex:{}:2026-03-29T15:04:10.090Z:18193+371",
                f.path().display()
            )],
            "the pre-v6 key rides along so the archive can retire that row"
        );
        assert!(
            calls[0].dedup_key.starts_with("codex:sess-1:"),
            "keys are addressed by session lineage, not file path"
        );
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
        use std::sync::atomic::{AtomicU64, Ordering};

        static SEQ: AtomicU64 = AtomicU64::new(0);

        pub struct TempFile(PathBuf);

        impl TempFile {
            pub fn new(name: &str) -> Self {
                let seq = SEQ.fetch_add(1, Ordering::Relaxed);
                let path = std::env::temp_dir().join(format!(
                    "tokenuse-codex-{}-{}-{}-{}",
                    std::process::id(),
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_nanos(),
                    seq,
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
