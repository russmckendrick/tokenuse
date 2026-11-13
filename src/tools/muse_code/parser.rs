use std::collections::HashSet;

use chrono::{DateTime, Utc};
use color_eyre::Result;
use serde::Deserialize;
use serde_json::Value;

use crate::pricing;
use crate::tools::{
    InteractionMode, ParsedCall, SessionSource, Speed, TimestampQuality, TokenQuality,
};

use super::config;

/// One line of `session.jsonl`: an event-log envelope. Only the fields the
/// parser needs are modeled; everything else is ignored so format additions
/// do not break ingestion.
#[derive(Debug, Deserialize, Default)]
struct Envelope {
    #[serde(default)]
    id: String,
    #[serde(default)]
    sequence: u64,
    #[serde(default)]
    recorded_at: Option<i64>,
    #[serde(default)]
    payload: Value,
    #[serde(default)]
    stream: Option<StreamRef>,
}

#[derive(Debug, Deserialize, Default)]
struct StreamRef {
    #[serde(default)]
    id: String,
}

#[derive(Debug, Deserialize, Default)]
struct Usage {
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
    #[serde(default)]
    cache_write_tokens: u64,
    #[serde(default)]
    cache_read_tokens: u64,
    #[serde(default)]
    reasoning_tokens: u64,
}

pub fn parse_session(
    source: &SessionSource,
    seen: &mut HashSet<String>,
) -> Result<Vec<ParsedCall>> {
    let raw = match std::fs::read_to_string(&source.path) {
        Ok(raw) => raw,
        Err(_) => return Ok(Vec::new()),
    };

    let mut calls = Vec::new();
    let mut workspace_root = String::new();
    let mut session_model = String::new();
    let mut stream_id = String::new();
    let mut saw_metadata = false;

    for line in raw.lines().filter(|l| !l.trim().is_empty()) {
        let Ok(envelope) = serde_json::from_str::<Envelope>(line) else {
            continue;
        };
        if stream_id.is_empty() {
            if let Some(stream) = envelope.stream.as_ref() {
                if !stream.id.is_empty() {
                    stream_id.clone_from(&stream.id);
                }
            }
        }
        let kind = envelope.payload.get("kind").and_then(|k| k.as_str());
        match kind {
            Some("metadata") => {
                saw_metadata = true;
                let record = envelope.payload.get("record").unwrap_or(&Value::Null);
                if let Some(root) = record.get("workspace_root").and_then(|r| r.as_str()) {
                    if !root.is_empty() {
                        workspace_root = root.to_string();
                    }
                }
                if session_model.is_empty() {
                    if let Some(model) = record.get("model_id").and_then(|m| m.as_str()) {
                        if !model.is_empty() {
                            session_model = model.to_string();
                        }
                    }
                }
            }
            Some("run") => {
                let event = envelope.payload.get("event").unwrap_or(&Value::Null);
                if event.get("kind").and_then(|k| k.as_str()) != Some("model_completed") {
                    continue;
                }
                if let Some(call) = parse_model_completed(
                    &envelope,
                    event,
                    source,
                    &workspace_root,
                    &session_model,
                    &stream_id,
                    seen,
                ) {
                    calls.push(call);
                }
            }
            _ => {}
        }
    }

    // Subagent logs carry no working directory of their own; attribute
    // their calls to the parent session's project instead of the
    // date-shard fallback label.
    if workspace_root.is_empty() && saw_metadata {
        workspace_root = parent_workspace_root(source);
        if !workspace_root.is_empty() {
            for call in &mut calls {
                call.project.clone_from(&workspace_root);
            }
        }
    }

    Ok(calls)
}

/// Workspace root of the parent session for a `subagent/` log, or empty
/// when the log is not a subagent one or the parent is unreadable. Reads
/// only up to the first metadata record that names a root.
fn parent_workspace_root(source: &SessionSource) -> String {
    let path = &source.path;
    let is_subagent = path
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        == Some(config::SUBAGENT_DIR);
    if !is_subagent {
        return String::new();
    }
    let Some(parent_log) = path
        .parent()
        .and_then(|child| child.parent())
        .and_then(|subagents| subagents.parent())
        .map(|parent| parent.join(config::SESSION_FILE))
    else {
        return String::new();
    };
    let Ok(raw) = std::fs::read_to_string(&parent_log) else {
        return String::new();
    };
    for line in raw.lines().filter(|l| !l.trim().is_empty()) {
        let Ok(envelope) = serde_json::from_str::<Envelope>(line) else {
            continue;
        };
        if envelope.payload.get("kind").and_then(|k| k.as_str()) != Some("metadata") {
            continue;
        }
        let root = envelope
            .payload
            .get("record")
            .and_then(|r| r.get("workspace_root"))
            .and_then(|r| r.as_str())
            .unwrap_or("");
        if !root.is_empty() {
            return root.to_string();
        }
    }
    String::new()
}

#[allow(clippy::too_many_arguments)]
fn parse_model_completed(
    envelope: &Envelope,
    event: &Value,
    source: &SessionSource,
    workspace_root: &str,
    session_model: &str,
    stream_id: &str,
    seen: &mut HashSet<String>,
) -> Option<ParsedCall> {
    let usage: Usage = serde_json::from_value(event.get("usage")?.clone()).ok()?;
    if usage.input_tokens == 0
        && usage.output_tokens == 0
        && usage.cache_write_tokens == 0
        && usage.cache_read_tokens == 0
        && usage.reasoning_tokens == 0
    {
        return None;
    }

    let mut model = event
        .get("model")
        .and_then(|m| m.as_str())
        .unwrap_or("")
        .to_string();
    if model.is_empty() {
        model = session_model.to_string();
    }
    if model.is_empty() {
        return None;
    }

    let dedup_key = if envelope.id.is_empty() {
        format!("{}:{stream_id}:{}", config::TOOL_ID, envelope.sequence)
    } else {
        format!("{}:{}", config::TOOL_ID, envelope.id)
    };
    if !seen.insert(dedup_key.clone()) {
        return None;
    }

    let timestamp = envelope
        .recorded_at
        .and_then(DateTime::<Utc>::from_timestamp_micros);
    let session_id = if stream_id.is_empty() {
        source
            .path
            .parent()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default()
    } else {
        stream_id.to_string()
    };
    let project = if workspace_root.is_empty() {
        source.project.clone()
    } else {
        workspace_root.to_string()
    };

    // Per Meta's token-counting docs, `cache_read_tokens` is a subset of
    // `input_tokens`, not an extra charge — so the cached prefix is
    // subtracted before billing input at the full rate (same shape as the
    // gemini parser). The cached leg is still preserved below for rates.
    let mut call = ParsedCall {
        tool: config::TOOL_ID,
        model: model.clone(),
        input_tokens: usage.input_tokens.saturating_sub(usage.cache_read_tokens),
        output_tokens: usage.output_tokens,
        cache_creation_input_tokens: usage.cache_write_tokens,
        cache_read_input_tokens: usage.cache_read_tokens,
        cached_input_tokens: usage.cache_read_tokens,
        reasoning_tokens: usage.reasoning_tokens,
        speed: Speed::Standard,
        timestamp,
        dedup_key,
        session_id,
        project,
        token_quality: TokenQuality::Exact,
        timestamp_quality: TimestampQuality::Exact,
        interaction_mode: InteractionMode::Unknown,
        ..ParsedCall::default()
    };
    call.cost_usd = pricing::cost(&model, &call, Speed::Standard);
    Some(call)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source_for(path: &std::path::Path) -> SessionSource {
        SessionSource::session(path.to_path_buf(), "2026/11/12/abc123", config::TOOL_ID)
    }

    fn envelope(id: &str, sequence: u64, recorded_at: i64, payload: Value) -> String {
        serde_json::to_string(&serde_json::json!({
            "id": id,
            "sequence": sequence,
            "recorded_at": recorded_at,
            "payload_type": "runtime.session",
            "payload": payload,
            "stream": {"kind": "session", "id": "abc123"},
        }))
        .unwrap()
    }

    fn fixture(metadata_model: &str) -> String {
        let lines = [
            envelope(
                "e-meta",
                1,
                1794511182796546,
                serde_json::json!({"kind": "metadata", "record": {
                    "workspace_root": "/Users/me/proj",
                    "model_id": metadata_model,
                }}),
            ),
            envelope(
                "e-run-1",
                2,
                1794511211509309,
                serde_json::json!({"kind": "run", "event": {
                    "kind": "model_completed",
                    "usage": {
                        "input_tokens": 38141, "output_tokens": 262,
                        "cached_tokens": 0, "cache_write_tokens": 0,
                        "cache_read_tokens": 0, "reasoning_tokens": 136,
                    },
                    "duration_ms": 4707,
                    "finish_reason": "tool_calls",
                    "model": "muse-spark-1.3-contributor",
                }}),
            ),
            envelope(
                "e-run-2",
                3,
                1794511220000000,
                serde_json::json!({"kind": "run", "event": {
                    "kind": "model_completed",
                    "usage": {
                        "input_tokens": 39669, "output_tokens": 149,
                        "cached_tokens": 37233, "cache_write_tokens": 0,
                        "cache_read_tokens": 37233, "reasoning_tokens": 32,
                    },
                    "finish_reason": "tool_calls",
                }}),
            ),
            envelope(
                "e-other",
                4,
                1794511230000000,
                serde_json::json!({"kind": "run", "event": {
                    "kind": "assistant_message_committed",
                }}),
            ),
            "not json at all".to_string(),
        ];
        lines.join("\n") + "\n"
    }

    fn write_fixture(contents: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "tokenuse-muse-parser-{}.jsonl",
            crate::tools::paths::test_run_id()
        ));
        std::fs::write(&path, contents).unwrap();
        path
    }

    #[test]
    fn maps_usage_project_and_model() {
        let path = write_fixture(&fixture("muse-spark-1.3-contributor"));
        let source = source_for(&path);
        let mut seen = HashSet::new();
        let calls = parse_session(&source, &mut seen).unwrap();

        assert_eq!(calls.len(), 2);
        let first = &calls[0];
        assert_eq!(first.tool, "muse-code");
        assert_eq!(first.model, "muse-spark-1.3-contributor");
        assert_eq!(first.input_tokens, 38141);
        assert_eq!(first.output_tokens, 262);
        assert_eq!(first.reasoning_tokens, 136);
        assert_eq!(first.cache_read_input_tokens, 0);
        assert_eq!(first.project, "/Users/me/proj");
        assert_eq!(first.session_id, "abc123");
        assert_eq!(first.token_quality, TokenQuality::Exact);
        assert_eq!(first.timestamp_quality, TimestampQuality::Exact);
        assert!(first.timestamp.is_some());
        // End-to-end through the embedded pricing book at Contributor rates:
        // 38141 input x $0.10 + 262 output x $0.20 per MTok.
        let expected_cost = 38141.0 * 1e-7 + 262.0 * 2e-7;
        assert!(
            (first.cost_usd - expected_cost).abs() < 1e-9,
            "unexpected cost: {}",
            first.cost_usd
        );

        // Second record omits its own model, so the session fallback applies;
        // cached tokens are a subset of input, so only the fresh prefix is
        // billed at the input rate.
        let second = &calls[1];
        assert_eq!(second.model, "muse-spark-1.3-contributor");
        assert_eq!(second.input_tokens, 39669 - 37233);
        assert_eq!(second.cache_read_input_tokens, 37233);
        assert_eq!(second.cached_input_tokens, 37233);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn dedup_key_survives_reparse_and_zero_usage_is_skipped() {
        let path = write_fixture(&fixture("muse-spark-1.3-contributor"));
        let source = source_for(&path);
        let mut seen = HashSet::new();
        let first = parse_session(&source, &mut seen).unwrap();
        assert_eq!(first.len(), 2);
        let second = parse_session(&source, &mut seen).unwrap();
        assert!(second.is_empty());
        let _ = std::fs::remove_file(&path);

        let zero = [
            envelope(
                "e-meta",
                1,
                1794511182796546,
                serde_json::json!({"kind": "metadata", "record": {
                    "workspace_root": "/x", "model_id": "m",
                }}),
            ),
            envelope(
                "e-zero",
                2,
                1794511211509309,
                serde_json::json!({"kind": "run", "event": {
                    "kind": "model_completed",
                    "usage": {"input_tokens": 0, "output_tokens": 0},
                    "model": "m",
                }}),
            ),
        ]
        .join("\n")
            + "\n";
        let zero_path = write_fixture(&zero);
        let zero_source = source_for(&zero_path);
        let mut seen = HashSet::new();
        assert!(parse_session(&zero_source, &mut seen).unwrap().is_empty());
        let _ = std::fs::remove_file(&zero_path);
    }

    #[test]
    fn subagent_calls_inherit_parent_project() {
        let base = std::env::temp_dir().join(format!(
            "tokenuse-muse-subagent-{}",
            crate::tools::paths::test_run_id()
        ));
        let parent = base.join("2026/11/04/parent123/session.jsonl");
        let child = base.join("2026/11/04/parent123/subagent/child456/session.jsonl");
        for path in [&parent, &child] {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        }
        std::fs::write(
            &parent,
            envelope(
                "e-meta",
                1,
                1794511182796546,
                serde_json::json!({"kind": "metadata", "record": {
                    "workspace_root": "/Users/me/proj",
                    "model_id": "muse-spark-1.3",
                }}),
            ) + "\n",
        )
        .unwrap();
        std::fs::write(
            &child,
            [
                envelope(
                    "e-meta",
                    1,
                    1794511182796546,
                    serde_json::json!({"kind": "metadata", "record": {
                        "model_id": "muse-spark-1.3",
                    }}),
                ),
                envelope(
                    "e-run",
                    2,
                    1794511211509309,
                    serde_json::json!({"kind": "run", "event": {
                        "kind": "model_completed",
                        "usage": {"input_tokens": 100, "output_tokens": 10},
                        "model": "muse-spark-1.3",
                    }}),
                ),
            ]
            .join("\n")
                + "\n",
        )
        .unwrap();

        let source = source_for(&child);
        let mut seen = HashSet::new();
        let calls = parse_session(&source, &mut seen).unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].project, "/Users/me/proj");

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn missing_file_parses_to_nothing() {
        let missing = std::env::temp_dir().join(format!(
            "tokenuse-muse-parser-missing-{}",
            crate::tools::paths::test_run_id()
        ));
        let source = source_for(&missing);
        let mut seen = HashSet::new();
        assert!(parse_session(&source, &mut seen).unwrap().is_empty());
    }
}
