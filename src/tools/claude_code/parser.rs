use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::Path;

use chrono::{DateTime, Utc};
use color_eyre::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use walkdir::WalkDir;

use crate::pricing;
use crate::tools::{jsonl, AdapterParse, CodeBlock, ParsedCall, SessionSource, Speed};

use super::config;

/// Bump to invalidate persisted tail cursors when parsing semantics change;
/// a mismatch falls back to a full re-parse of every file in the source.
const PARSE_VERSION: &str = "1";
/// Newest-modified session files that keep a resume cursor. Older files
/// change rarely enough that a full re-parse on touch is fine, and the cap
/// bounds the cursor JSON stored per source.
const MAX_CURSOR_FILES: usize = 64;
/// Bytes hashed immediately before a stored offset so a rewritten prefix
/// (compaction, truncation, editor save) is detected even when the file is
/// at least as long as it was.
const PROBE_BYTES: u64 = 256;

/// Persisted per-source resume cursor: one entry per session file, keyed by
/// absolute path, plus the parse version that wrote it.
#[derive(Debug, Default, Serialize, Deserialize)]
struct SourceCursor {
    v: String,
    files: BTreeMap<String, FileCursor>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct FileCursor {
    /// Byte offset just past the last fully parsed (newline-terminated) line.
    offset: u64,
    /// `probe_hash` of the bytes ending at `offset`.
    probe: String,
    /// Carried cross-line parser state: the last user prompt seen before the
    /// offset, so tail assistant rounds keep their turn context.
    #[serde(default)]
    user_text: String,
    #[serde(default)]
    user_chars: Option<u64>,
    #[serde(default)]
    user_ts: Option<String>,
    #[serde(default)]
    project: Option<String>,
}

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
    cache_creation: Option<CacheCreation>,
    #[serde(default)]
    speed: Option<String>,
    #[serde(default)]
    server_tool_use: Option<ServerToolUse>,
    /// Advisor runs bill separately: each `advisor_message` iteration is its
    /// own API call with its own model and usage. A `message` iteration
    /// mirrors the top-level usage and must not be double counted.
    #[serde(default)]
    iterations: Option<Vec<AdvisorIteration>>,
}

/// Per-TTL cache-write split; the flat `cache_creation_input_tokens` field
/// remains as the total.
#[derive(Debug, Deserialize, Default)]
struct CacheCreation {
    #[serde(default)]
    ephemeral_5m_input_tokens: u64,
    #[serde(default)]
    ephemeral_1h_input_tokens: u64,
}

#[derive(Debug, Deserialize, Default)]
struct AdvisorIteration {
    #[serde(default, rename = "type")]
    kind: String,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
    #[serde(default)]
    cache_creation_input_tokens: u64,
    #[serde(default)]
    cache_read_input_tokens: u64,
    #[serde(default)]
    cache_creation: Option<CacheCreation>,
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

/// `total = max(flat legacy field, 5m + 1h)` guards against either field
/// lagging the other; the 1h share is capped by the total.
fn cache_creation_buckets(legacy: u64, per_ttl: Option<&CacheCreation>) -> (u64, u64) {
    let (five_m, one_h) = per_ttl
        .map(|c| (c.ephemeral_5m_input_tokens, c.ephemeral_1h_input_tokens))
        .unwrap_or((0, 0));
    let total = legacy.max(five_m + one_h);
    (total, one_h.min(total))
}

pub fn parse_session(
    source: &SessionSource,
    seen: &mut HashSet<String>,
) -> Result<Vec<ParsedCall>> {
    let mut calls: Vec<ParsedCall> = Vec::new();
    for path in collect_jsonl(&source.path) {
        parse_file(&path, &source.project, seen, &mut calls, None);
    }
    Ok(calls)
}

/// Cursor-aware parse: session files whose stored offset still matches skip
/// their already-parsed prefix; everything else (new files, rewritten files,
/// stale parse versions) parses fully. Returns the cursor to persist for
/// the next sync.
pub fn parse_session_with_cursor(
    source: &SessionSource,
    seen: &mut HashSet<String>,
    prior: Option<&str>,
) -> Result<AdapterParse> {
    let prior: Option<SourceCursor> = prior
        .and_then(|raw| serde_json::from_str(raw).ok())
        .filter(|cursor: &SourceCursor| cursor.v == PARSE_VERSION);

    let mut calls: Vec<ParsedCall> = Vec::new();
    let mut resumed_files = 0usize;
    let mut entries: Vec<(String, FileCursor, Option<std::time::SystemTime>)> = Vec::new();
    for path in collect_jsonl(&source.path) {
        let key = path.to_string_lossy().to_string();
        let resume = prior.as_ref().and_then(|cursor| cursor.files.get(&key));
        let Some(parsed) = parse_file(&path, &source.project, seen, &mut calls, resume) else {
            continue;
        };
        if parsed.resumed {
            resumed_files += 1;
        }
        let mtime = fs::metadata(&path).ok().and_then(|m| m.modified().ok());
        entries.push((key, parsed.cursor, mtime));
    }

    entries.sort_by_key(|entry| std::cmp::Reverse(entry.2));
    entries.truncate(MAX_CURSOR_FILES);
    let cursor = SourceCursor {
        v: PARSE_VERSION.to_string(),
        files: entries
            .into_iter()
            .map(|(key, cursor, _)| (key, cursor))
            .collect(),
    };
    Ok(AdapterParse {
        calls,
        cursor: serde_json::to_string(&cursor).ok(),
        resumed_files,
    })
}

struct FileParse {
    cursor: FileCursor,
    resumed: bool,
}

/// Parses one session file into `calls`, resuming from `resume` when its
/// offset and prefix probe still match the file on disk. Returns `None`
/// when the file cannot be opened.
///
/// A resumed parse cannot see prefix lines, so two signals degrade at the
/// boundary by design: an interrupt marker whose call was parsed in an
/// earlier sync no longer flags that call, and a message whose streamed
/// lines span the boundary re-emits with tail-only activity — the archive
/// merges it into the existing row via the `merge_activity` hint.
fn parse_file(
    path: &Path,
    source_project: &str,
    seen: &mut HashSet<String>,
    calls: &mut Vec<ParsedCall>,
    resume: Option<&FileCursor>,
) -> Option<FileParse> {
    let file_len = fs::metadata(path).ok()?.len();
    let resume = resume.filter(|cursor| {
        cursor.offset > 0
            && cursor.offset <= file_len
            && probe_hash(path, cursor.offset).as_deref() == Some(cursor.probe.as_str())
    });
    let resumed = resume.is_some();
    let start_offset = resume.map(|cursor| cursor.offset).unwrap_or(0);

    let mut last_user_text = resume.map(|c| c.user_text.clone()).unwrap_or_default();
    let mut last_user_chars: Option<u64> = resume.and_then(|c| c.user_chars);
    let mut last_user_ts: Option<DateTime<Utc>> = resume
        .and_then(|c| c.user_ts.as_deref())
        .and_then(parse_timestamp);
    let mut project = resume
        .and_then(|c| c.project.clone())
        .unwrap_or_else(|| source_project.to_string());

    let file_start_index = calls.len();
    // Claude Code streams one JSONL line per content block; every line of
    // a message repeats the message id and the final usage payload. Later
    // lines of an id already seen in this file merge their activity into
    // the call the first line created (tool_result lines interleave, so
    // same-id lines are not always adjacent).
    let mut file_calls_by_id: HashMap<String, usize> = HashMap::new();
    let session_id = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();

    let mut file = fs::File::open(path).ok()?;
    if start_offset > 0 {
        file.seek(SeekFrom::Start(start_offset)).ok()?;
    }
    let mut reader = BufReader::new(file);
    let mut consumed = start_offset;
    let mut buf: Vec<u8> = Vec::new();
    loop {
        buf.clear();
        let read = match reader.read_until(b'\n', &mut buf) {
            Ok(read) => read,
            Err(_) => break,
        };
        if read == 0 {
            break;
        }
        if !buf.ends_with(b"\n") {
            // Mid-append partial line: leave it (and the offset) for the
            // next sync so the completed line parses exactly once.
            break;
        }
        consumed += buf.len() as u64;
        let raw = String::from_utf8_lossy(&buf);
        let line = raw.trim_end_matches(['\n', '\r']);
        process_line(
            line,
            calls,
            seen,
            &mut file_calls_by_id,
            file_start_index,
            &session_id,
            &mut project,
            &mut last_user_text,
            &mut last_user_chars,
            &mut last_user_ts,
        );
    }

    if resumed {
        for call in &mut calls[file_start_index..] {
            call.merge_activity = true;
        }
    }

    Some(FileParse {
        cursor: FileCursor {
            offset: consumed,
            probe: probe_hash(path, consumed).unwrap_or_default(),
            user_text: last_user_text,
            user_chars: last_user_chars,
            user_ts: last_user_ts.map(|ts| ts.to_rfc3339()),
            project: Some(project),
        },
        resumed,
    })
}

/// FNV-1a of the up-to-`PROBE_BYTES` bytes ending at `offset`. An offset of
/// zero hashes nothing and yields the FNV basis, which is fine: offset zero
/// never resumes.
fn probe_hash(path: &Path, offset: u64) -> Option<String> {
    let mut file = fs::File::open(path).ok()?;
    let take = offset.min(PROBE_BYTES);
    file.seek(SeekFrom::Start(offset - take)).ok()?;
    let mut buf = vec![0u8; take as usize];
    file.read_exact(&mut buf).ok()?;
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in buf {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    Some(format!("{hash:016x}"))
}

#[allow(clippy::too_many_arguments)]
fn process_line(
    line: &str,
    calls: &mut Vec<ParsedCall>,
    seen: &mut HashSet<String>,
    file_calls_by_id: &mut HashMap<String, usize>,
    file_start_index: usize,
    session_id: &str,
    project: &mut String,
    last_user_text: &mut String,
    last_user_chars: &mut Option<u64>,
    last_user_ts: &mut Option<DateTime<Utc>>,
) {
    let entry: JournalEntry = match serde_json::from_str(line) {
        Ok(e) => e,
        Err(_) => return,
    };

    if let Some(cwd) = entry.cwd.as_ref().filter(|cwd| !cwd.trim().is_empty()) {
        *project = cwd.clone();
    }

    match entry.kind.as_str() {
        "user" => {
            if let Some(msg) = &entry.message {
                if msg.role.as_deref() == Some("user") {
                    let text = extract_user_text(msg);
                    let trimmed = text.trim();
                    if trimmed.is_empty() {
                        return;
                    }
                    if trimmed.starts_with(INTERRUPT_MARKER) {
                        // The interruption belongs to the previous
                        // call of this session file, if any.
                        if calls.len() > file_start_index {
                            if let Some(last) = calls.last_mut() {
                                last.is_canceled = true;
                            }
                        }
                        return;
                    }
                    if trimmed.starts_with("<command-") || trimmed.starts_with("<local-command-") {
                        // Slash-command wrappers are UI plumbing, not
                        // a prompt the user wrote.
                        return;
                    }
                    *last_user_chars = Some(text.chars().count() as u64);
                    *last_user_text = jsonl::truncate_chars(&text, 500);
                    *last_user_ts = entry.timestamp.as_deref().and_then(parse_timestamp);
                }
            }
        }
        "assistant" => {
            let Some(msg) = entry.message.as_ref() else {
                return;
            };

            let dedup_key = msg.id.clone().unwrap_or_else(|| {
                format!("claude:{}", entry.timestamp.clone().unwrap_or_default())
            });

            if let Some(&idx) = file_calls_by_id.get(&dedup_key) {
                merge_streamed_line(&mut calls[idx], msg.content.as_ref());
                return;
            }

            let Some(model) = msg.model.clone() else {
                return;
            };
            let Some(usage) = msg.usage.as_ref() else {
                return;
            };

            if !seen.insert(dedup_key.clone()) {
                return;
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
            let elapsed_ms = jsonl::turn_elapsed_ms(timestamp, *last_user_ts);
            let (cache_creation_total, cache_creation_1h) = cache_creation_buckets(
                usage.cache_creation_input_tokens,
                usage.cache_creation.as_ref(),
            );

            // Advisor iterations are separately billed API calls that
            // ride inside this message's usage. Emit them first so the
            // interrupt marker's "previous call" stays the main call.
            calls.extend(advisor_calls(
                usage,
                &dedup_key,
                &model,
                timestamp,
                entry.session_id.as_deref().unwrap_or(session_id),
                project,
                seen,
            ));

            let mut call = ParsedCall {
                tool: config::TOOL_ID,
                model: model.clone(),
                input_tokens: usage.input_tokens,
                output_tokens: usage.output_tokens,
                cache_creation_input_tokens: cache_creation_total,
                cache_creation_1h_input_tokens: cache_creation_1h,
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
                    .unwrap_or_else(|| session_id.to_string()),
                project: project.clone(),
                prompt_chars: *last_user_chars,
                response_chars: Some(response_text.chars().count() as u64),
                elapsed_ms,
                code_blocks: jsonl::merge_code_blocks(code_blocks),
                edited_files: jsonl::dedup_files(activity.edited_files),
                referenced_files: jsonl::dedup_files(activity.referenced_files),
                ..ParsedCall::default()
            };

            call.cost_usd = pricing::cost(&model, &call, speed);
            file_calls_by_id.insert(call.dedup_key.clone(), calls.len());
            calls.push(call);
        }
        _ => {}
    }
}

/// Later streamed lines of a message repeat its id and usage but carry the
/// next content block, so only activity accumulates; token counts and cost
/// stay from the first line (observed usage is identical across rewrites).
fn merge_streamed_line(call: &mut ParsedCall, content: Option<&Value>) {
    let activity = extract_activity(content);
    let response_text = extract_response_text(content);

    call.tools.extend(activity.tools);
    call.bash_commands.extend(activity.bash_commands);

    if !response_text.is_empty() || !activity.code_blocks.is_empty() {
        let mut blocks = std::mem::take(&mut call.code_blocks);
        blocks.extend(jsonl::extract_code_fences(&response_text));
        blocks.extend(activity.code_blocks);
        call.code_blocks = jsonl::merge_code_blocks(blocks);
    }
    if !response_text.is_empty() {
        call.response_chars =
            Some(call.response_chars.unwrap_or(0) + response_text.chars().count() as u64);
    }
    if !activity.edited_files.is_empty() {
        let mut files = std::mem::take(&mut call.edited_files);
        files.extend(activity.edited_files);
        call.edited_files = jsonl::dedup_files(files);
    }
    if !activity.referenced_files.is_empty() {
        let mut files = std::mem::take(&mut call.referenced_files);
        files.extend(activity.referenced_files);
        call.referenced_files = jsonl::dedup_files(files);
    }
}

/// Each `advisor_message` iteration inside `usage.iterations` is a separately
/// billed API call with its own model and usage; `message` and
/// `fallback_message` iterations mirror the top-level usage and are skipped.
/// The dedup ordinal counts advisor entries only, so the key is stable even
/// when other iteration types move around.
fn advisor_calls(
    usage: &Usage,
    base_key: &str,
    main_model: &str,
    timestamp: Option<DateTime<Utc>>,
    session_id: &str,
    project: &str,
    seen: &mut HashSet<String>,
) -> Vec<ParsedCall> {
    let Some(iterations) = usage.iterations.as_ref() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut ordinal = 0usize;
    for iter in iterations {
        if iter.kind != "advisor_message" {
            continue;
        }
        let model = iter
            .model
            .as_deref()
            .filter(|m| !m.is_empty())
            .unwrap_or(main_model)
            .to_string();
        let dedup_key = format!("{base_key}:advisor:{ordinal}");
        ordinal += 1;
        if !seen.insert(dedup_key.clone()) {
            continue;
        }
        let speed = match iter.speed.as_deref().or(usage.speed.as_deref()) {
            Some("fast") => Speed::Fast,
            _ => Speed::Standard,
        };
        let (cache_total, cache_1h) = cache_creation_buckets(
            iter.cache_creation_input_tokens,
            iter.cache_creation.as_ref(),
        );
        let mut call = ParsedCall {
            tool: config::TOOL_ID,
            model: model.clone(),
            input_tokens: iter.input_tokens,
            output_tokens: iter.output_tokens,
            cache_creation_input_tokens: cache_total,
            cache_creation_1h_input_tokens: cache_1h,
            cache_read_input_tokens: iter.cache_read_input_tokens,
            web_search_requests: iter
                .server_tool_use
                .as_ref()
                .map(|s| s.web_search_requests)
                .unwrap_or(0),
            speed,
            timestamp,
            dedup_key,
            session_id: session_id.to_string(),
            project: project.to_string(),
            ..ParsedCall::default()
        };
        call.cost_usd = pricing::cost(&model, &call, speed);
        out.push(call);
    }
    out
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
            collect_subagent_jsonl(&path, &mut files);
        } else if path.is_dir() {
            // Newer Claude Code builds scope subagent transcripts per
            // session: <project>/<session-id>/subagents/agent-*.jsonl.
            let nested = path.join(config::SUBAGENTS_DIR);
            if nested.is_dir() {
                collect_subagent_jsonl(&nested, &mut files);
            }
        }
    }
    files
}

/// Walk the whole subagents tree: workflow/ultracode runs nest their agent
/// transcripts as subagents/workflows/<workflow>/agent-*.jsonl.
fn collect_subagent_jsonl(dir: &Path, files: &mut Vec<std::path::PathBuf>) {
    for sub in WalkDir::new(dir).follow_links(false).into_iter().flatten() {
        let sub_path = sub.path();
        if sub.file_type().is_file()
            && sub_path.extension().and_then(|s| s.to_str()) == Some(config::SESSION_GLOB_EXT)
        {
            files.push(sub_path.to_path_buf());
        }
    }
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
    fn cursor_resume_parses_only_the_appended_tail() {
        let dir = fixture();
        let source =
            SessionSource::session(dir.path().to_path_buf(), "test/project", config::TOOL_ID);
        let mut seen = HashSet::new();
        let first = parse_session_with_cursor(&source, &mut seen, None).unwrap();
        assert_eq!(first.calls.len(), 2);
        assert_eq!(first.resumed_files, 0);
        let cursor = first.cursor.clone().expect("cursor persisted");

        // Append a new turn plus a continuation line of msg_2 carrying an
        // extra tool call (streamed lines split across syncs).
        let path = dir.path().join("session.jsonl");
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap();
        for line in [
            r#"{"type":"assistant","timestamp":"2026-04-26T10:05:21Z","sessionId":"s1","message":{"role":"assistant","id":"msg_2","model":"claude-opus-4-7-20250514","usage":{"input_tokens":10,"output_tokens":5},"content":[{"type":"tool_use","name":"Edit","input":{"file_path":"src/tail.rs","new_string":"x"}}]}}"#,
            r#"{"type":"user","timestamp":"2026-04-26T10:06:00Z","sessionId":"s1","message":{"role":"user","content":"appended prompt"}}"#,
            r#"{"type":"assistant","timestamp":"2026-04-26T10:06:05Z","sessionId":"s1","message":{"role":"assistant","id":"msg_3","model":"claude-opus-4-7-20250514","usage":{"input_tokens":7,"output_tokens":3},"content":[{"type":"text","text":"ok"}]}}"#,
        ] {
            writeln!(f, "{}", line).unwrap();
        }

        let mut seen = HashSet::new();
        let second = parse_session_with_cursor(&source, &mut seen, Some(&cursor)).unwrap();
        assert_eq!(second.resumed_files, 1);
        assert_eq!(
            second.calls.len(),
            2,
            "only the tail parses: the msg_2 continuation and msg_3"
        );
        assert!(second.calls.iter().all(|call| call.merge_activity));
        let continuation = &second.calls[0];
        assert_eq!(continuation.dedup_key, "msg_2");
        assert_eq!(
            continuation.edited_files,
            vec!["src/tail.rs"],
            "tail call carries tail-only activity for the archive merge"
        );
        let appended = &second.calls[1];
        assert_eq!(appended.dedup_key, "msg_3");
        assert_eq!(
            appended.user_message, "appended prompt",
            "user context inside the tail applies"
        );

        // A third sync with the refreshed cursor parses nothing.
        let mut seen = HashSet::new();
        let third =
            parse_session_with_cursor(&source, &mut seen, second.cursor.as_deref()).unwrap();
        assert_eq!(third.resumed_files, 1);
        assert!(third.calls.is_empty());
    }

    #[test]
    fn cursor_carries_user_context_across_the_boundary() {
        let dir = fixture();
        let source =
            SessionSource::session(dir.path().to_path_buf(), "test/project", config::TOOL_ID);
        let mut seen = HashSet::new();
        let first = parse_session_with_cursor(&source, &mut seen, None).unwrap();
        let cursor = first.cursor.clone().expect("cursor persisted");

        // The tail starts with an assistant round whose user prompt was
        // parsed before the boundary ("try again with tests").
        let path = dir.path().join("session.jsonl");
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap();
        writeln!(
            f,
            r#"{{"type":"assistant","timestamp":"2026-04-26T10:05:30Z","sessionId":"s1","message":{{"role":"assistant","id":"msg_4","model":"claude-opus-4-7-20250514","usage":{{"input_tokens":1,"output_tokens":1}},"content":[{{"type":"text","text":"more"}}]}}}}"#
        )
        .unwrap();

        let mut seen = HashSet::new();
        let second = parse_session_with_cursor(&source, &mut seen, Some(&cursor)).unwrap();
        assert_eq!(second.calls.len(), 1);
        assert_eq!(second.calls[0].user_message, "try again with tests");
        assert_eq!(second.calls[0].prompt_chars, Some(20));
    }

    #[test]
    fn cursor_rejects_rewritten_prefixes_and_stale_versions() {
        let dir = fixture();
        let source =
            SessionSource::session(dir.path().to_path_buf(), "test/project", config::TOOL_ID);
        let mut seen = HashSet::new();
        let first = parse_session_with_cursor(&source, &mut seen, None).unwrap();
        let cursor = first.cursor.clone().expect("cursor persisted");

        // A stale parse version falls back to a full re-parse.
        let stale = cursor.replace(
            &format!("\"v\":\"{PARSE_VERSION}\""),
            "\"v\":\"tokenuse-test-stale\"",
        );
        let mut seen = HashSet::new();
        let full = parse_session_with_cursor(&source, &mut seen, Some(&stale)).unwrap();
        assert_eq!(full.resumed_files, 0);
        assert_eq!(full.calls.len(), 2);

        // A rewritten prefix (same length or longer, different bytes at the
        // stored offset) fails the probe and re-parses fully.
        let path = dir.path().join("session.jsonl");
        let original = std::fs::read(&path).unwrap();
        let mut rewritten = vec![b' '; original.len()];
        let replay = b"{\"type\":\"user\"}\n";
        rewritten[..replay.len()].copy_from_slice(replay);
        std::fs::write(&path, rewritten).unwrap();
        let mut seen = HashSet::new();
        let reparsed = parse_session_with_cursor(&source, &mut seen, Some(&cursor)).unwrap();
        assert_eq!(reparsed.resumed_files, 0);
        assert!(reparsed.calls.is_empty(), "blank rewrite has no calls");
    }

    #[test]
    fn partial_trailing_lines_wait_for_the_next_sync() {
        let dir = fixture();
        let source =
            SessionSource::session(dir.path().to_path_buf(), "test/project", config::TOOL_ID);
        let mut seen = HashSet::new();
        let first = parse_session_with_cursor(&source, &mut seen, None).unwrap();
        let cursor = first.cursor.clone().expect("cursor persisted");

        // Append half a line (no trailing newline): mid-append snapshot.
        let path = dir.path().join("session.jsonl");
        let half = r#"{"type":"assistant","timestamp":"2026-04-26T10:07:00Z","sessionId":"s1","message":{"role":"assistant","id":"msg_5","#;
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap();
        write!(f, "{}", half).unwrap();

        let mut seen = HashSet::new();
        let second = parse_session_with_cursor(&source, &mut seen, Some(&cursor)).unwrap();
        assert!(second.calls.is_empty());

        // Complete the line: the whole message parses exactly once.
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap();
        writeln!(
            f,
            r#""model":"claude-opus-4-7-20250514","usage":{{"input_tokens":1,"output_tokens":1}},"content":[{{"type":"text","text":"late"}}]}}}}"#
        )
        .unwrap();
        let mut seen = HashSet::new();
        let third =
            parse_session_with_cursor(&source, &mut seen, second.cursor.as_deref()).unwrap();
        assert_eq!(third.calls.len(), 1);
        assert_eq!(third.calls[0].dedup_key, "msg_5");
    }

    #[test]
    fn parses_assistant_entries_and_dedups() {
        let dir = fixture();
        let source =
            SessionSource::session(dir.path().to_path_buf(), "test/project", config::TOOL_ID);
        let mut seen = HashSet::new();
        let calls = parse_session(&source, &mut seen).unwrap();
        assert_eq!(calls.len(), 2, "duplicate msg.id lines merge into one call");
        let call = &calls[0];
        assert_eq!(
            call.input_tokens, 100,
            "token counts stay from the first line of a message"
        );
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
    fn streamed_content_block_lines_merge_into_one_call() {
        let dir = tempfile_lite::TempDir::new();
        let path = dir.path().join("session.jsonl");
        let mut f = std::fs::File::create(&path).unwrap();
        // Claude Code writes one line per content block: same message id and
        // usage on every line, tool_result user lines interleaved between the
        // non-adjacent block lines.
        for line in [
            r#"{"type":"user","timestamp":"2026-04-26T10:00:00Z","sessionId":"s1","message":{"role":"user","content":"add a helper"}}"#,
            r#"{"type":"assistant","timestamp":"2026-04-26T10:00:01Z","sessionId":"s1","message":{"role":"assistant","id":"msg_a","model":"claude-opus-4-7","usage":{"input_tokens":100,"output_tokens":50},"content":[{"type":"text","text":"On it."}]}}"#,
            r#"{"type":"assistant","timestamp":"2026-04-26T10:00:02Z","sessionId":"s1","message":{"role":"assistant","id":"msg_a","model":"claude-opus-4-7","usage":{"input_tokens":100,"output_tokens":50},"content":[{"type":"tool_use","name":"Bash","input":{"command":"cargo check"}}]}}"#,
            r#"{"type":"user","timestamp":"2026-04-26T10:00:03Z","sessionId":"s1","message":{"role":"user","content":[{"type":"tool_result","content":"ok"}]}}"#,
            r#"{"type":"assistant","timestamp":"2026-04-26T10:00:04Z","sessionId":"s1","message":{"role":"assistant","id":"msg_a","model":"claude-opus-4-7","usage":{"input_tokens":100,"output_tokens":50},"content":[{"type":"tool_use","name":"Edit","input":{"file_path":"src/lib.rs","new_string":"fn helper() {}"}},{"type":"text","text":"Done."}]}}"#,
        ] {
            writeln!(f, "{}", line).unwrap();
        }

        let source = SessionSource::session(dir.path().to_path_buf(), "p", config::TOOL_ID);
        let mut seen = HashSet::new();
        let calls = parse_session(&source, &mut seen).unwrap();

        assert_eq!(calls.len(), 1, "all block lines fold into one call");
        let call = &calls[0];
        assert_eq!(call.input_tokens, 100);
        assert_eq!(call.output_tokens, 50);
        assert_eq!(
            call.tools,
            vec!["Bash", "Edit"],
            "tool_use blocks on later lines must not be lost"
        );
        assert_eq!(call.bash_commands, vec!["cargo check"]);
        assert_eq!(call.edited_files, vec!["src/lib.rs"]);
        assert_eq!(
            call.response_chars,
            Some(("On it.".chars().count() + "Done.".chars().count()) as u64),
            "text blocks on later lines accumulate"
        );
        assert_eq!(call.code_blocks.len(), 1, "Edit payload counts as output");
        assert_eq!(
            call.user_message, "add a helper",
            "tool_result user lines never become prompts"
        );
        assert_eq!(
            call.timestamp,
            parse_timestamp("2026-04-26T10:00:01Z"),
            "merged call keeps the first line's timestamp"
        );
    }

    #[test]
    fn workflow_subagent_transcripts_are_collected() {
        let dir = tempfile_lite::TempDir::new();
        let write_session = |rel: &str, id: &str| {
            let path = dir.path().join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            let mut f = std::fs::File::create(&path).unwrap();
            writeln!(
                f,
                r#"{{"type":"assistant","timestamp":"2026-04-26T10:00:01Z","sessionId":"{id}","message":{{"role":"assistant","id":"{id}","model":"claude-opus-4-7","usage":{{"input_tokens":1,"output_tokens":1}}}}}}"#
            )
            .unwrap();
        };
        write_session("session.jsonl", "msg_main");
        write_session("subagents/agent-1.jsonl", "msg_sub");
        write_session("subagents/workflows/wf-1/agent-2.jsonl", "msg_wf");
        write_session(
            "cb62da0c-3a12-4572-8a4e-4f8c9e37343f/subagents/agent-3.jsonl",
            "msg_session_scoped",
        );

        let source = SessionSource::session(dir.path().to_path_buf(), "p", config::TOOL_ID);
        let mut seen = HashSet::new();
        let calls = parse_session(&source, &mut seen).unwrap();

        let mut ids: Vec<_> = calls.iter().map(|c| c.dedup_key.as_str()).collect();
        ids.sort_unstable();
        assert_eq!(
            ids,
            vec!["msg_main", "msg_session_scoped", "msg_sub", "msg_wf"],
            "nested workflow and session-scoped subagent transcripts must be ingested"
        );
    }

    #[test]
    fn advisor_iterations_emit_separate_calls() {
        let dir = tempfile_lite::TempDir::new();
        let path = dir.path().join("session.jsonl");
        let mut f = std::fs::File::create(&path).unwrap();
        // The "message" iteration mirrors the top-level usage (never double
        // counted); the advisor_message iteration is its own billed call.
        writeln!(
            f,
            r#"{{"type":"assistant","timestamp":"2026-04-26T10:00:01Z","sessionId":"s1","message":{{"role":"assistant","id":"msg_adv","model":"claude-opus-4-7","usage":{{"input_tokens":9118,"output_tokens":344,"cache_read_input_tokens":15874,"iterations":[{{"type":"message","input_tokens":9118,"output_tokens":344}},{{"type":"advisor_message","model":"claude-haiku-4-5","input_tokens":1200,"output_tokens":80,"cache_read_input_tokens":500}}]}}}}}}"#
        )
        .unwrap();

        let source = SessionSource::session(dir.path().to_path_buf(), "p", config::TOOL_ID);
        let mut seen = HashSet::new();
        let calls = parse_session(&source, &mut seen).unwrap();

        assert_eq!(calls.len(), 2, "advisor iteration becomes its own call");
        let advisor = &calls[0];
        assert_eq!(advisor.dedup_key, "msg_adv:advisor:0");
        assert_eq!(advisor.model, "claude-haiku-4-5");
        assert_eq!(advisor.input_tokens, 1200);
        assert_eq!(advisor.output_tokens, 80);
        assert_eq!(advisor.cache_read_input_tokens, 500);
        assert!(advisor.cost_usd > 0.0);
        let main = &calls[1];
        assert_eq!(main.dedup_key, "msg_adv");
        assert_eq!(main.input_tokens, 9118);

        let second = parse_session(&source, &mut seen).unwrap();
        assert!(second.is_empty(), "advisor keys dedup across reparses");
    }

    #[test]
    fn one_hour_cache_writes_are_extracted_and_priced_at_a_premium() {
        let build = |name: &str, cache_creation: &str| {
            let dir = tempfile_lite::TempDir::new();
            let path = dir.path().join(name);
            let mut f = std::fs::File::create(&path).unwrap();
            writeln!(
                f,
                r#"{{"type":"assistant","timestamp":"2026-04-26T10:00:01Z","sessionId":"s1","message":{{"role":"assistant","id":"{name}","model":"claude-opus-4-7","usage":{{"input_tokens":10,"output_tokens":5,"cache_creation_input_tokens":6304,"cache_creation":{cache_creation}}}}}}}"#
            )
            .unwrap();
            dir
        };

        let five_m = build(
            "five.jsonl",
            r#"{"ephemeral_5m_input_tokens":6304,"ephemeral_1h_input_tokens":0}"#,
        );
        let one_h = build(
            "hour.jsonl",
            r#"{"ephemeral_5m_input_tokens":0,"ephemeral_1h_input_tokens":6304}"#,
        );

        let mut seen = HashSet::new();
        let five_calls = parse_session(
            &SessionSource::session(five_m.path().to_path_buf(), "p", config::TOOL_ID),
            &mut seen,
        )
        .unwrap();
        let hour_calls = parse_session(
            &SessionSource::session(one_h.path().to_path_buf(), "p", config::TOOL_ID),
            &mut seen,
        )
        .unwrap();

        assert_eq!(five_calls[0].cache_creation_input_tokens, 6304);
        assert_eq!(five_calls[0].cache_creation_1h_input_tokens, 0);
        assert_eq!(hour_calls[0].cache_creation_input_tokens, 6304);
        assert_eq!(hour_calls[0].cache_creation_1h_input_tokens, 6304);
        assert!(
            hour_calls[0].cost_usd > five_calls[0].cost_usd,
            "1h cache writes must price above the 5m rate"
        );
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
