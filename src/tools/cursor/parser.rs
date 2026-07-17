use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use color_eyre::Result;
use rusqlite::{Connection, OpenFlags};
use serde_json::Value;

use crate::pricing;
use crate::tools::jsonl::{
    dedup_files, extension_language, extract_code_fences, merge_code_blocks, truncate_chars,
};
use crate::tools::{
    CodeBlock, InteractionMode, ParsedCall, SessionSource, Speed, TimestampQuality, TokenQuality,
};

use super::{config, discovery};

const COST_MODEL_FALLBACK: &str = "cursor-auto";
pub fn parse_session(
    source: &SessionSource,
    seen: &mut HashSet<String>,
) -> Result<Vec<ParsedCall>> {
    let files = discovery::aggregate_files();
    let transcript_paths = files
        .iter()
        .filter(|path| is_transcript_file(path))
        .cloned()
        .collect::<Vec<_>>();
    let store_paths = files
        .iter()
        .filter(|path| path.file_name().and_then(|name| name.to_str()) == Some(config::STORE_DB))
        .cloned()
        .collect::<Vec<_>>();

    let transcripts = load_transcripts(&transcript_paths);
    let tracking = config::agent_tracking_db_path()
        .as_deref()
        .map(load_tracking)
        .unwrap_or_default();
    let stores = load_stores(&store_paths);

    let mut calls = Vec::new();
    let mut state_sessions = HashSet::new();
    if let Some(state_path) = config::state_db_path().filter(|path| path.exists()) {
        if let Some(conn) = open_sqlite_read_only(&state_path) {
            let workspace_projects = load_workspace_projects(config::workspace_storage_root());
            let reconstructed = reconstruct_state(
                &conn,
                source,
                &transcripts,
                &tracking,
                &stores,
                &workspace_projects,
            )?;
            state_sessions.extend(reconstructed.iter().map(|call| call.session_id.clone()));
            calls.extend(reconstructed);
        }
    }

    for (conversation_id, store) in &stores {
        if state_sessions.contains(conversation_id) {
            continue;
        }
        calls.extend(calls_from_store(
            store,
            transcripts.get(conversation_id),
            tracking.get(conversation_id),
            &source.project,
        ));
        state_sessions.insert(conversation_id.clone());
    }

    for (conversation_id, transcript) in &transcripts {
        if state_sessions.contains(conversation_id) {
            continue;
        }
        calls.extend(calls_from_transcript(
            conversation_id,
            transcript,
            tracking.get(conversation_id),
            &source.project,
        ));
    }

    calls.retain(|call| seen.insert(call.dedup_key.clone()));
    Ok(calls)
}

fn open_sqlite_read_only(path: &Path) -> Option<Connection> {
    let uri = format!("file:{}?mode=ro", path.display());
    Connection::open_with_flags(
        &uri,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    )
    .or_else(|_| {
        let immutable = format!("file:{}?immutable=1", path.display());
        Connection::open_with_flags(
            immutable,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
        )
    })
    .ok()
}

#[derive(Debug, Clone, Default)]
struct RichTurn {
    request_id: Option<String>,
    user_message: String,
    response_text: String,
    reasoning_text: String,
    model: Option<String>,
    tools: Vec<String>,
    bash_commands: Vec<String>,
    code_blocks: Vec<CodeBlock>,
    edited_files: Vec<String>,
    referenced_files: Vec<String>,
    elapsed_ms: Option<u64>,
}

impl RichTurn {
    fn prompt_chars(&self) -> u64 {
        self.user_message.chars().count() as u64
    }

    fn response_chars(&self) -> u64 {
        self.response_text.chars().count() as u64 + self.reasoning_text.chars().count() as u64
    }

    fn finish(&mut self) {
        let mut blocks = std::mem::take(&mut self.code_blocks);
        blocks.extend(extract_code_fences(&self.response_text));
        self.code_blocks = merge_code_blocks(blocks);
        self.edited_files = dedup_files(std::mem::take(&mut self.edited_files));
        self.referenced_files = dedup_files(std::mem::take(&mut self.referenced_files));
        dedup_strings(&mut self.tools);
        dedup_strings(&mut self.bash_commands);
    }

    fn fill_from(&mut self, other: &RichTurn) {
        if self.request_id.is_none() {
            self.request_id.clone_from(&other.request_id);
        }
        if self.user_message.is_empty() {
            self.user_message.clone_from(&other.user_message);
        }
        if self.response_text.is_empty() {
            self.response_text.clone_from(&other.response_text);
        }
        if self.reasoning_text.is_empty() {
            self.reasoning_text.clone_from(&other.reasoning_text);
        }
        if self.model.is_none() {
            self.model.clone_from(&other.model);
        }
        merge_unique(&mut self.tools, &other.tools);
        merge_unique(&mut self.bash_commands, &other.bash_commands);
        merge_unique(&mut self.edited_files, &other.edited_files);
        merge_unique(&mut self.referenced_files, &other.referenced_files);
        if self.code_blocks.is_empty() {
            self.code_blocks.clone_from(&other.code_blocks);
        }
        if self.elapsed_ms.is_none() {
            self.elapsed_ms = other.elapsed_ms;
        }
    }
}

#[derive(Debug, Clone, Default)]
struct TranscriptSession {
    turns: Vec<RichTurn>,
    project: Option<String>,
    timestamp: Option<DateTime<Utc>>,
    source_paths: Vec<PathBuf>,
}

#[derive(Debug, Clone, Default)]
struct TrackingSession {
    model: Option<String>,
    timestamp: Option<DateTime<Utc>>,
    project: Option<String>,
    mode: InteractionMode,
    edited_files: Vec<String>,
}

#[derive(Debug, Clone, Default)]
struct StoreSession {
    conversation_id: String,
    model: Option<String>,
    mode: InteractionMode,
    timestamp: Option<DateTime<Utc>>,
    project: Option<String>,
    turns: Vec<RichTurn>,
}

#[derive(Debug, Clone, Default)]
struct Composer {
    id: String,
    order: Vec<String>,
    model: Option<String>,
    mode: InteractionMode,
    aborted: bool,
    created_at: Option<DateTime<Utc>>,
    /// Cursor's own per-conversation context meter
    /// (`promptTokenBreakdown.totalUsedTokens`, falling back to
    /// `contextTokensUsed`): the latest context-window snapshot, not a
    /// per-turn sum. On current builds it is the only real input-token
    /// figure the local database carries.
    meter_tokens: u64,
}

#[derive(Debug, Clone, Default)]
struct Bubble {
    key: String,
    id: String,
    composer_id: String,
    request_id: Option<String>,
    bubble_type: i64,
    text: String,
    created_raw: String,
    timestamp: Option<DateTime<Utc>>,
    input_tokens: u64,
    output_tokens: u64,
    model: Option<String>,
    turn_duration_ms: Option<u64>,
    mode: InteractionMode,
    legacy_conversation_id: Option<String>,
    tool_name: Option<String>,
    tool_status: Option<String>,
    tool_args: Vec<Value>,
    attached_files: Vec<String>,
    edited_files: Vec<String>,
}

#[derive(Debug, Clone, Default)]
struct RequestContext {
    referenced_files: Vec<String>,
    edited_files: Vec<String>,
}

#[derive(Debug, Clone, Default)]
struct StateTurn {
    user: Bubble,
    assistants: Vec<Bubble>,
}

fn reconstruct_state(
    conn: &Connection,
    source: &SessionSource,
    transcripts: &HashMap<String, TranscriptSession>,
    tracking: &HashMap<String, TrackingSession>,
    stores: &HashMap<String, StoreSession>,
    workspace_projects: &HashMap<String, String>,
) -> Result<Vec<ParsedCall>> {
    if conn
        .query_row::<i64, _, _>(config::VALIDATE_STATE_QUERY, [], |row| row.get(0))
        .is_err()
    {
        return Ok(Vec::new());
    }

    let composers = load_composers(conn);
    let mut bubbles = load_bubbles(conn);
    let contexts = load_request_contexts(conn);
    let agent = load_agent_turns(conn);
    let mut calls = Vec::new();

    let mut composer_ids = composers.keys().cloned().collect::<Vec<_>>();
    for composer_id in bubbles.keys() {
        if !composer_ids.contains(composer_id) {
            composer_ids.push(composer_id.clone());
        }
    }
    composer_ids.sort();

    for composer_id in composer_ids {
        let composer = composers
            .get(&composer_id)
            .cloned()
            .unwrap_or_else(|| Composer {
                id: composer_id.clone(),
                ..Composer::default()
            });
        let mut session_bubbles = bubbles.remove(&composer_id).unwrap_or_default();
        order_bubbles(&mut session_bubbles, &composer.order);
        let turns = group_state_turns(session_bubbles);
        if turns.is_empty() {
            continue;
        }

        let transcript = transcripts.get(&composer_id);
        let store = stores.get(&composer_id);
        let tracking_session = tracking.get(&composer_id);

        // Current Cursor builds write {0,0} per-bubble token counts; the
        // composer's context meter is then the only real input figure. When
        // it is present and no bubble carries explicit tokens, per-turn
        // chars/4 input estimates are dropped and the meter is credited once
        // per conversation below. Any explicit bubble tokens (older builds)
        // win outright.
        let has_explicit_bubble_tokens = turns.iter().any(|turn| {
            std::iter::once(&turn.user)
                .chain(turn.assistants.iter())
                .any(|bubble| bubble.input_tokens > 0 || bubble.output_tokens > 0)
        });
        let meter_active = composer.meter_tokens > 0 && !has_explicit_bubble_tokens;
        let mut credit_context: Option<(String, String)> = None;
        let mut first_turn_ts: Option<DateTime<Utc>> = None;

        for (idx, state_turn) in turns.iter().enumerate() {
            let mut rich = rich_from_state_turn(state_turn);
            let request_id = state_turn.user.request_id.clone().or_else(|| {
                state_turn
                    .assistants
                    .iter()
                    .find_map(|bubble| bubble.request_id.clone())
            });
            rich.request_id.clone_from(&request_id);

            let store_turn =
                store.and_then(|session| match_turn(&session.turns, request_id.as_deref(), idx));
            if let Some(store_turn) = store_turn {
                merge_store_turn(&mut rich, store_turn);
            }
            let agent_turn = request_id.as_deref().and_then(|request| agent.get(request));
            if let Some(agent_turn) = agent_turn {
                let preferred_model = agent_turn.model.clone();
                rich.fill_from(agent_turn);
                if preferred_model.is_some() {
                    rich.model = preferred_model;
                }
            }
            if let Some(context) = contexts.get(&(composer_id.clone(), state_turn.user.id.clone()))
            {
                merge_unique(&mut rich.referenced_files, &context.referenced_files);
                merge_unique(&mut rich.edited_files, &context.edited_files);
            }
            if let Some(transcript_turn) = transcript
                .and_then(|session| match_turn(&session.turns, request_id.as_deref(), idx))
            {
                rich.fill_from(transcript_turn);
            }
            if rich.edited_files.is_empty() && idx + 1 == turns.len() {
                if let Some(files) = tracking_session.map(|session| &session.edited_files) {
                    rich.edited_files.clone_from(files);
                }
            }
            rich.finish();

            let final_assistant = state_turn
                .assistants
                .iter()
                .rev()
                .find(|bubble| {
                    bubble.tool_name.is_none()
                        && (!bubble.text.is_empty() || bubble.output_tokens > 0)
                })
                .or_else(|| state_turn.assistants.last());
            let timestamp = final_assistant
                .and_then(|bubble| bubble.timestamp)
                .or(state_turn.user.timestamp);
            let elapsed_ms = state_turn
                .assistants
                .iter()
                .rev()
                .find_map(|bubble| bubble.turn_duration_ms)
                .or_else(|| non_negative_elapsed(timestamp, state_turn.user.timestamp))
                .or(rich.elapsed_ms);

            let (input_tokens, output_tokens, token_quality) =
                state_tokens(state_turn, &rich, meter_active);
            let bubble_model = state_turn
                .assistants
                .iter()
                .rev()
                .find_map(|bubble| bubble.model.clone())
                .or_else(|| state_turn.user.model.clone());
            let model = agent_turn
                .and_then(|turn| turn.model.clone())
                .or(bubble_model)
                .or_else(|| composer.model.clone())
                .or_else(|| tracking_session.and_then(|session| session.model.clone()))
                .or_else(|| store.and_then(|session| session.model.clone()))
                .unwrap_or_else(|| COST_MODEL_FALLBACK.to_string());
            let mode = state_turn_mode(state_turn)
                .or_known(composer.mode)
                .or_known(
                    tracking_session
                        .map(|session| session.mode)
                        .unwrap_or_default(),
                )
                .or_known(store.map(|session| session.mode).unwrap_or_default());
            let project = project_for_state(
                &rich,
                transcript,
                tracking_session,
                store,
                workspace_projects.get(&composer_id).map(String::as_str),
                &source.project,
            );
            let identity = request_id
                .clone()
                .unwrap_or_else(|| state_turn.user.id.clone());
            let dedup_key = format!("cursor:composer:{composer_id}:{identity}");
            let mut superseded = legacy_state_keys(state_turn);
            if let Some(request_id) = request_id.as_deref() {
                superseded.push(format!("cursor:agentKv:{request_id}"));
            }
            superseded.extend(legacy_transcript_keys(transcript, &composer_id, idx));

            let mut call = ParsedCall {
                tool: config::TOOL_ID,
                model: model.clone(),
                input_tokens,
                output_tokens,
                timestamp,
                speed: speed_from_model(&model),
                dedup_key,
                user_message: truncate_chars(&rich.user_message, 500),
                session_id: composer.id.clone(),
                project,
                is_canceled: composer.aborted && idx + 1 == turns.len(),
                prompt_chars: Some(rich.prompt_chars()),
                response_chars: Some(rich.response_chars()),
                elapsed_ms,
                tools: rich.tools,
                bash_commands: rich.bash_commands,
                code_blocks: rich.code_blocks,
                edited_files: rich.edited_files,
                referenced_files: rich.referenced_files,
                interaction_mode: mode,
                token_quality,
                timestamp_quality: if timestamp.is_some() {
                    TimestampQuality::Exact
                } else {
                    TimestampQuality::Unknown
                },
                superseded_dedup_keys: superseded,
                ..ParsedCall::default()
            };
            if credit_context.is_none() {
                credit_context = Some((call.project.clone(), model.clone()));
            }
            if first_turn_ts.is_none() {
                first_turn_ts = timestamp;
            }

            call.cost_usd = pricing::cost(&model, &call, call.speed);
            calls.push(call);
        }

        // One input credit per conversation from Cursor's own context meter,
        // anchored at the conversation's creation so the credited day stays
        // stable across re-parses. The meter is the LATEST context size, not
        // a per-turn sum: growth after the anchor stays uncounted — an
        // undercount versus the Cursor admin console, never a double count.
        if meter_active {
            let (project, model) = credit_context.unwrap_or_else(|| {
                (
                    source.project.clone(),
                    composer
                        .model
                        .clone()
                        .unwrap_or_else(|| COST_MODEL_FALLBACK.to_string()),
                )
            });
            let timestamp = composer.created_at.or(first_turn_ts);
            let mut credit = ParsedCall {
                tool: config::TOOL_ID,
                model: model.clone(),
                input_tokens: composer.meter_tokens,
                timestamp,
                speed: speed_from_model(&model),
                dedup_key: format!("{}{}", config::COMPOSER_INPUT_DEDUP_PREFIX, composer.id),
                session_id: composer.id.clone(),
                project,
                interaction_mode: composer.mode,
                token_quality: TokenQuality::Estimated,
                timestamp_quality: if timestamp.is_some() {
                    TimestampQuality::Session
                } else {
                    TimestampQuality::Unknown
                },
                ..ParsedCall::default()
            };
            credit.cost_usd = pricing::cost(&model, &credit, credit.speed);
            calls.push(credit);
        }
    }
    Ok(calls)
}

trait KnownMode {
    fn or_known(self, fallback: InteractionMode) -> InteractionMode;
}

impl KnownMode for InteractionMode {
    fn or_known(self, fallback: InteractionMode) -> InteractionMode {
        if self == InteractionMode::Unknown {
            fallback
        } else {
            self
        }
    }
}

fn load_composers(conn: &Connection) -> HashMap<String, Composer> {
    let mut out = HashMap::new();
    let Ok(mut stmt) = conn.prepare(config::STATE_COMPOSER_QUERY) else {
        return out;
    };
    let Ok(rows) = stmt.query_map([], |row| {
        let key: String = row.get(0)?;
        let key_id = key
            .strip_prefix("composerData:")
            .unwrap_or(&key)
            .to_string();
        let json_id: Option<String> = row.get(1)?;
        let headers: Option<String> = row.get(2)?;
        let model: Option<String> = row.get(3)?;
        let force_mode: Option<String> = row.get(4)?;
        let unified_mode: Option<String> = row.get(5)?;
        let is_agentic: Option<bool> = row.get(6)?;
        let status: Option<String> = row.get(7)?;
        let created_raw = column_as_string(row, 8)?;
        let meter_used = column_as_string(row, 9)?;
        let meter_context = column_as_string(row, 10)?;
        Ok((
            key_id,
            json_id,
            headers,
            model,
            force_mode,
            unified_mode,
            is_agentic,
            status,
            created_raw,
            meter_used,
            meter_context,
        ))
    }) else {
        return out;
    };
    for row in rows.flatten() {
        let id = row.1.filter(|id| !id.is_empty()).unwrap_or(row.0);
        // A stored zero `totalUsedTokens` falls through to
        // `contextTokensUsed`.
        let meter_tokens = meter_value(row.9.as_deref())
            .or_else(|| meter_value(row.10.as_deref()))
            .unwrap_or(0);
        out.insert(
            id.clone(),
            Composer {
                id,
                order: parse_header_order(row.2.as_deref()),
                model: row.3,
                mode: interaction_mode(row.4.as_deref(), row.5.as_deref(), row.6, false),
                aborted: row.7.as_deref() == Some("aborted"),
                created_at: row.8.as_deref().and_then(parse_timestamp),
                meter_tokens,
            },
        );
    }
    out
}

fn meter_value(raw: Option<&str>) -> Option<u64> {
    let tokens = raw?.trim().parse::<f64>().ok()?;
    if tokens.is_finite() && tokens >= 1.0 {
        Some(tokens as u64)
    } else {
        None
    }
}

fn parse_header_order(raw: Option<&str>) -> Vec<String> {
    let Some(raw) = raw else { return Vec::new() };
    serde_json::from_str::<Value>(raw)
        .ok()
        .and_then(|value| value.as_array().cloned())
        .unwrap_or_default()
        .iter()
        .filter_map(|header| {
            header
                .get("bubbleId")
                .and_then(Value::as_str)
                .map(ToString::to_string)
        })
        .collect()
}

fn load_bubbles(conn: &Connection) -> HashMap<String, Vec<Bubble>> {
    let mut out: HashMap<String, Vec<Bubble>> = HashMap::new();
    let Ok(mut stmt) = conn.prepare(config::STATE_BUBBLE_QUERY) else {
        return out;
    };
    let Ok(rows) = stmt.query_map([], |row| {
        let key: String = row.get(0)?;
        let (composer_id, key_bubble_id) = bubble_key_parts(&key).unwrap_or_default();
        let json_bubble_id: Option<String> = row.get(1)?;
        let attached = [18usize, 19, 20];
        let edited = [21usize, 22, 23];
        let mut attached_files = Vec::new();
        let mut edited_files = Vec::new();
        for idx in attached {
            collect_paths_from_json(
                row.get::<_, Option<String>>(idx)?.as_deref(),
                &mut attached_files,
            );
        }
        for idx in edited {
            collect_paths_from_json(
                row.get::<_, Option<String>>(idx)?.as_deref(),
                &mut edited_files,
            );
        }
        let args = [
            row.get::<_, Option<String>>(16)?,
            row.get::<_, Option<String>>(17)?,
        ]
        .into_iter()
        .flatten()
        .map(|raw| serde_json::from_str::<Value>(&raw).unwrap_or(Value::String(raw)))
        .collect::<Vec<_>>();
        let created_raw = column_as_string(row, 5)?.unwrap_or_default();
        Ok(Bubble {
            key,
            id: json_bubble_id
                .filter(|id| !id.is_empty())
                .unwrap_or(key_bubble_id),
            composer_id,
            request_id: row.get(2)?,
            bubble_type: row.get::<_, Option<i64>>(3)?.unwrap_or(-1),
            text: row.get::<_, Option<String>>(4)?.unwrap_or_default(),
            timestamp: parse_timestamp(&created_raw),
            created_raw,
            input_tokens: non_negative_i64(row.get(6)?),
            output_tokens: non_negative_i64(row.get(7)?),
            model: row.get(8)?,
            turn_duration_ms: row
                .get::<_, Option<i64>>(9)?
                .and_then(|value| u64::try_from(value).ok()),
            mode: interaction_mode(
                None,
                row.get::<_, Option<String>>(10)?.as_deref(),
                row.get(11)?,
                row.get::<_, Option<bool>>(12)?.unwrap_or(false),
            ),
            legacy_conversation_id: row.get(13)?,
            tool_name: row.get(14)?,
            tool_status: row.get(15)?,
            tool_args: args,
            attached_files,
            edited_files,
        })
    }) else {
        return out;
    };
    for bubble in rows.flatten() {
        if !bubble.composer_id.is_empty() {
            out.entry(bubble.composer_id.clone())
                .or_default()
                .push(bubble);
        }
    }
    out
}

fn bubble_key_parts(key: &str) -> Option<(String, String)> {
    let rest = key.strip_prefix("bubbleId:")?;
    let (composer, bubble) = rest.split_once(':')?;
    Some((composer.to_string(), bubble.to_string()))
}

fn order_bubbles(bubbles: &mut [Bubble], header_order: &[String]) {
    let positions = header_order
        .iter()
        .enumerate()
        .map(|(idx, id)| (id.as_str(), idx))
        .collect::<HashMap<_, _>>();
    bubbles.sort_by(|a, b| {
        positions
            .get(a.id.as_str())
            .copied()
            .unwrap_or(usize::MAX)
            .cmp(&positions.get(b.id.as_str()).copied().unwrap_or(usize::MAX))
            .then_with(|| a.timestamp.cmp(&b.timestamp))
            .then_with(|| a.key.cmp(&b.key))
    });
}

fn group_state_turns(bubbles: Vec<Bubble>) -> Vec<StateTurn> {
    let mut turns = Vec::new();
    let mut current: Option<StateTurn> = None;
    for bubble in bubbles {
        if bubble.bubble_type == 1 {
            if let Some(turn) = current.take() {
                turns.push(turn);
            }
            current = Some(StateTurn {
                user: bubble,
                assistants: Vec::new(),
            });
        } else if matches!(bubble.bubble_type, 0 | 2) {
            if let Some(turn) = current.as_mut() {
                turn.assistants.push(bubble);
            }
        }
    }
    if let Some(turn) = current {
        turns.push(turn);
    }
    turns
}

fn rich_from_state_turn(turn: &StateTurn) -> RichTurn {
    let mut rich = RichTurn {
        user_message: extract_user_query(&turn.user.text),
        referenced_files: turn.user.attached_files.clone(),
        edited_files: turn.user.edited_files.clone(),
        ..RichTurn::default()
    };
    for bubble in &turn.assistants {
        if !bubble.text.is_empty() {
            if !rich.response_text.is_empty() {
                rich.response_text.push('\n');
            }
            rich.response_text.push_str(&bubble.text);
        }
        merge_unique(&mut rich.referenced_files, &bubble.attached_files);
        merge_unique(&mut rich.edited_files, &bubble.edited_files);
        if bubble.tool_status.as_deref() == Some("cancelled") {
            continue;
        }
        if let Some(name) = bubble.tool_name.as_deref() {
            apply_tool(name, &bubble.tool_args, &mut rich);
        }
    }
    rich
}

fn load_request_contexts(conn: &Connection) -> HashMap<(String, String), RequestContext> {
    let mut out = HashMap::new();
    let Ok(mut stmt) = conn.prepare(config::STATE_CONTEXT_QUERY) else {
        return out;
    };
    let Ok(rows) = stmt.query_map([], |row| {
        let key: String = row.get(0)?;
        let rest = key.strip_prefix("messageRequestContext:").unwrap_or(&key);
        let (composer, bubble) = rest.split_once(':').unwrap_or(("", ""));
        let mut context = RequestContext::default();
        for idx in [1usize, 4] {
            collect_paths_from_json(
                row.get::<_, Option<String>>(idx)?.as_deref(),
                &mut context.referenced_files,
            );
        }
        for idx in [2usize, 3] {
            collect_paths_from_json(
                row.get::<_, Option<String>>(idx)?.as_deref(),
                &mut context.edited_files,
            );
        }
        Ok(((composer.to_string(), bubble.to_string()), context))
    }) else {
        return out;
    };
    for (key, mut context) in rows.flatten() {
        context.referenced_files = dedup_files(context.referenced_files);
        context.edited_files = dedup_files(context.edited_files);
        out.insert(key, context);
    }
    out
}

fn load_agent_turns(conn: &Connection) -> HashMap<String, RichTurn> {
    let mut out = HashMap::new();
    let Ok(mut stmt) = conn.prepare(config::STATE_AGENT_QUERY) else {
        return out;
    };
    let Ok(rows) = stmt.query_map([], |row| {
        Ok((
            row.get::<_, Option<String>>(0)?.unwrap_or_default(),
            row.get::<_, Option<String>>(1)?.unwrap_or_default(),
            row.get::<_, Option<String>>(2)?,
            row.get::<_, Option<i64>>(3)?,
            row.get::<_, Option<i64>>(4)?,
        ))
    }) else {
        return out;
    };
    let mut current_request: Option<String> = None;
    for (role, content, request_id, execution_time, local_time) in rows.flatten() {
        if request_id.is_some() {
            current_request.clone_from(&request_id);
        }
        let Some(request_id) = request_id.or_else(|| current_request.clone()) else {
            continue;
        };
        let turn = out.entry(request_id.clone()).or_insert_with(|| RichTurn {
            request_id: Some(request_id),
            ..RichTurn::default()
        });
        if role == "tool" {
            let duration = execution_time
                .or(local_time)
                .and_then(|value| u64::try_from(value).ok());
            if let Some(duration) = duration {
                turn.elapsed_ms = Some(turn.elapsed_ms.unwrap_or(0).saturating_add(duration));
            }
            continue;
        }
        let value = serde_json::from_str::<Value>(&content).unwrap_or(Value::String(content));
        analyze_message_content(&role, &value, turn);
    }
    for turn in out.values_mut() {
        turn.finish();
    }
    out
}

fn merge_store_turn(target: &mut RichTurn, store: &RichTurn) {
    if !store.user_message.is_empty() {
        target.user_message.clone_from(&store.user_message);
    }
    if !store.response_text.is_empty() {
        target.response_text.clone_from(&store.response_text);
    }
    if !store.reasoning_text.is_empty() {
        target.reasoning_text.clone_from(&store.reasoning_text);
    }
    target.fill_from(store);
}

fn analyze_message_content(role: &str, content: &Value, turn: &mut RichTurn) {
    match content {
        Value::String(text) => match role {
            "user" if turn.user_message.is_empty() => turn.user_message = extract_user_query(text),
            "assistant" => append_text(&mut turn.response_text, text),
            _ => {}
        },
        Value::Array(blocks) => {
            for block in blocks {
                let block_type = block.get("type").and_then(Value::as_str).unwrap_or("");
                match (role, block_type) {
                    ("user", "text") if turn.user_message.is_empty() => {
                        turn.user_message = block
                            .get("text")
                            .and_then(Value::as_str)
                            .map(extract_user_query)
                            .unwrap_or_default();
                    }
                    ("assistant", "text") => {
                        if let Some(text) = block.get("text").and_then(Value::as_str) {
                            append_text(&mut turn.response_text, text);
                        }
                    }
                    ("assistant", "reasoning" | "redacted-reasoning") => {
                        if let Some(text) = block.get("text").and_then(Value::as_str) {
                            append_text(&mut turn.reasoning_text, text);
                        }
                    }
                    ("assistant", "tool-call" | "tool_use") => {
                        let name = block
                            .get("toolName")
                            .or_else(|| block.get("name"))
                            .and_then(Value::as_str)
                            .unwrap_or("unknown");
                        let args = block.get("args").or_else(|| block.get("input"));
                        apply_tool(name, &args.into_iter().cloned().collect::<Vec<_>>(), turn);
                    }
                    _ => {}
                }
                if let Some(model) = block
                    .pointer("/providerOptions/cursor/modelName")
                    .and_then(Value::as_str)
                {
                    turn.model = Some(model.to_string());
                }
                if turn.elapsed_ms.is_none() {
                    turn.elapsed_ms = find_duration_ms(block);
                }
            }
        }
        _ => {}
    }
}

fn apply_tool(name: &str, args: &[Value], turn: &mut RichTurn) {
    let normalized = normalize_tool_name(name);
    if !normalized.is_empty() {
        turn.tools.push(normalized.clone());
    }
    let mut paths = Vec::new();
    for value in args {
        collect_paths(value, &mut paths);
        if normalized == "Bash" {
            collect_commands(value, &mut turn.bash_commands);
        }
    }
    let lower = normalized.to_ascii_lowercase();
    if is_edit_tool(&lower) {
        merge_unique(&mut turn.edited_files, &paths);
        if let Some(block) = code_block_from_tool(args, &paths) {
            turn.code_blocks.push(block);
        }
    } else if is_reference_tool(&lower) {
        merge_unique(&mut turn.referenced_files, &paths);
    }
}

fn is_edit_tool(name: &str) -> bool {
    ["write", "edit", "delete", "apply", "strreplace", "create"]
        .iter()
        .any(|needle| name.contains(needle))
}

fn is_reference_tool(name: &str) -> bool {
    ["read", "grep", "glob", "search", "find", "list"]
        .iter()
        .any(|needle| name.contains(needle))
}

fn code_block_from_tool(args: &[Value], paths: &[String]) -> Option<CodeBlock> {
    let content = args.iter().find_map(find_generated_content)?;
    let loc = content.lines().count() as u64;
    (loc > 0).then(|| CodeBlock {
        language: paths
            .first()
            .map(|path| extension_language(path))
            .unwrap_or_else(|| "unknown".to_string()),
        loc,
    })
}

fn find_generated_content(value: &Value) -> Option<&str> {
    match value {
        Value::Object(map) => {
            for key in ["content", "new_string", "newText", "code"] {
                if let Some(text) = map.get(key).and_then(Value::as_str) {
                    return Some(text);
                }
            }
            map.values().find_map(find_generated_content)
        }
        Value::Array(values) => values.iter().find_map(find_generated_content),
        _ => None,
    }
}

fn collect_commands(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (key, value) in map {
                if matches!(key.as_str(), "command" | "cmd") {
                    if let Some(command) = value.as_str() {
                        out.push(command.to_string());
                    }
                } else {
                    collect_commands(value, out);
                }
            }
        }
        Value::Array(values) => values.iter().for_each(|value| collect_commands(value, out)),
        _ => {}
    }
}

fn find_duration_ms(value: &Value) -> Option<u64> {
    match value {
        Value::Object(map) => {
            for key in [
                "turnDurationMs",
                "localExecutionTimeMs",
                "durationMs",
                "executionTime",
            ] {
                if let Some(value) = map.get(key) {
                    if let Some(number) = value.as_u64() {
                        return Some(number);
                    }
                    if let Some(number) = value.as_str().and_then(|raw| raw.parse().ok()) {
                        return Some(number);
                    }
                }
            }
            map.values().find_map(find_duration_ms)
        }
        Value::Array(values) => values.iter().find_map(find_duration_ms),
        _ => None,
    }
}

fn state_tokens(
    turn: &StateTurn,
    rich: &RichTurn,
    input_covered_by_meter: bool,
) -> (u64, u64, TokenQuality) {
    let explicit_input = turn
        .assistants
        .iter()
        .map(|bubble| bubble.input_tokens)
        .chain(std::iter::once(turn.user.input_tokens))
        .sum::<u64>();
    let explicit_output = turn
        .assistants
        .iter()
        .map(|bubble| bubble.output_tokens)
        .chain(std::iter::once(turn.user.output_tokens))
        .sum::<u64>();
    if input_covered_by_meter {
        // The conversation-level meter credit accounts for input; the turn
        // keeps its output side only.
        return token_totals(0, explicit_output, 0, rich.response_chars());
    }
    token_totals(
        explicit_input,
        explicit_output,
        rich.prompt_chars(),
        rich.response_chars(),
    )
}

fn token_totals(
    explicit_input: u64,
    explicit_output: u64,
    prompt_chars: u64,
    response_chars: u64,
) -> (u64, u64, TokenQuality) {
    let input_estimated = explicit_input == 0 && prompt_chars > 0;
    let output_estimated = explicit_output == 0 && response_chars > 0;
    let input = if input_estimated {
        estimate_tokens(prompt_chars)
    } else {
        explicit_input
    };
    let output = if output_estimated {
        estimate_tokens(response_chars)
    } else {
        explicit_output
    };
    let has_exact = explicit_input > 0 || explicit_output > 0;
    let has_estimate = input_estimated || output_estimated;
    let quality = match (has_exact, has_estimate) {
        (true, true) => TokenQuality::Mixed,
        (true, false) => TokenQuality::Exact,
        (false, true) => TokenQuality::Estimated,
        (false, false) => TokenQuality::Unknown,
    };
    (input, output, quality)
}

fn estimate_tokens(chars: u64) -> u64 {
    chars.saturating_add(3) / 4
}

fn state_turn_mode(turn: &StateTurn) -> InteractionMode {
    turn.assistants
        .iter()
        .rev()
        .map(|bubble| bubble.mode)
        .find(|mode| *mode != InteractionMode::Unknown)
        .unwrap_or(turn.user.mode)
}

fn project_for_state(
    rich: &RichTurn,
    transcript: Option<&TranscriptSession>,
    tracking: Option<&TrackingSession>,
    store: Option<&StoreSession>,
    workspace: Option<&str>,
    fallback: &str,
) -> String {
    extract_workspace_path(&rich.user_message)
        .or_else(|| transcript.and_then(|session| session.project.clone()))
        .or_else(|| tracking.and_then(|session| session.project.clone()))
        .or_else(|| store.and_then(|session| session.project.clone()))
        .or_else(|| workspace.map(str::to_string))
        .unwrap_or_else(|| fallback.to_string())
}

/// Map composer ids to workspace folder paths: each `workspaceStorage/<hash>`
/// pairs a `workspace.json` folder URI with a per-workspace `state.vscdb`
/// whose ItemTable lists the composers opened in that workspace. Roughly a
/// third of composers are unmapped (multi-root workspaces, "no folder"
/// windows, deleted workspaces) and keep the existing fallback chain.
fn load_workspace_projects(root: Option<PathBuf>) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let Some(root) = root else {
        return out;
    };
    let Ok(entries) = std::fs::read_dir(&root) else {
        return out;
    };
    for entry in entries.flatten() {
        let hash_dir = entry.path();
        if !hash_dir.is_dir() {
            continue;
        }
        let Some(folder) = workspace_folder_path(&hash_dir) else {
            continue;
        };
        let db = hash_dir.join(config::STATE_DB);
        if !db.is_file() {
            continue;
        }
        let Some(conn) = open_sqlite_read_only(&db) else {
            continue;
        };
        for composer_id in workspace_composer_ids(&conn) {
            out.entry(composer_id).or_insert_with(|| folder.clone());
        }
    }
    out
}

/// `workspace.json` `folder` is a URI. `file://` folders decode to a plain
/// absolute path (so project-identity git-root folding applies, matching how
/// Claude Code cwd values behave); other schemes (`vscode-remote://…`) keep
/// the decoded remainder as a label. Multi-root workspaces have no `folder`
/// key and are skipped.
fn workspace_folder_path(hash_dir: &Path) -> Option<String> {
    let raw = std::fs::read_to_string(hash_dir.join("workspace.json")).ok()?;
    let value: Value = serde_json::from_str(&raw).ok()?;
    let folder = value.get("folder").and_then(|v| v.as_str())?;
    let decoded = percent_decode(folder.strip_prefix("file://").unwrap_or(folder));
    if decoded.trim().is_empty() {
        None
    } else {
        Some(decoded)
    }
}

fn workspace_composer_ids(conn: &Connection) -> Vec<String> {
    let mut ids = Vec::new();
    let Ok(mut stmt) = conn.prepare(config::WORKSPACE_COMPOSER_QUERY) else {
        return ids;
    };
    let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(0)) else {
        return ids;
    };
    for raw in rows.flatten() {
        let Ok(value) = serde_json::from_str::<Value>(&raw) else {
            continue;
        };
        let Some(composers) = value.get("allComposers").and_then(|v| v.as_array()) else {
            continue;
        };
        for composer in composers {
            if let Some(id) = composer.get("composerId").and_then(|v| v.as_str()) {
                if !id.is_empty() {
                    ids.push(id.to_string());
                }
            }
        }
    }
    ids
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(hi), Some(lo)) = (hex_digit(bytes[i + 1]), hex_digit(bytes[i + 2])) {
                out.push((hi << 4) | lo);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

fn hex_digit(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn legacy_state_keys(turn: &StateTurn) -> Vec<String> {
    std::iter::once(&turn.user)
        .chain(turn.assistants.iter())
        .map(|bubble| {
            format!(
                "cursor:{}:{}:{}:{}",
                bubble
                    .legacy_conversation_id
                    .as_deref()
                    .unwrap_or("unknown"),
                bubble.created_raw,
                bubble.input_tokens,
                bubble.output_tokens
            )
        })
        .collect()
}

fn legacy_transcript_keys(
    transcript: Option<&TranscriptSession>,
    conversation_id: &str,
    idx: usize,
) -> Vec<String> {
    transcript
        .into_iter()
        .flat_map(|session| session.source_paths.iter())
        .map(|path| {
            format!(
                "cursor:transcript:{}:{conversation_id}:{idx}",
                path.display()
            )
        })
        .collect()
}

fn non_negative_elapsed(
    final_timestamp: Option<DateTime<Utc>>,
    user_timestamp: Option<DateTime<Utc>>,
) -> Option<u64> {
    let delta = final_timestamp?
        .signed_duration_since(user_timestamp?)
        .num_milliseconds();
    u64::try_from(delta).ok()
}

fn match_turn<'a>(
    turns: &'a [RichTurn],
    request_id: Option<&str>,
    idx: usize,
) -> Option<&'a RichTurn> {
    request_id
        .and_then(|request_id| {
            turns
                .iter()
                .find(|turn| turn.request_id.as_deref() == Some(request_id))
        })
        .or_else(|| turns.get(idx))
}

fn load_transcripts(paths: &[PathBuf]) -> HashMap<String, TranscriptSession> {
    let mut out: HashMap<String, TranscriptSession> = HashMap::new();
    for path in paths {
        let raw = match fs::read_to_string(path) {
            Ok(raw) => raw,
            Err(_) => continue,
        };
        let conversation_id = transcript_conversation_id(path);
        if conversation_id == "unknown" {
            continue;
        }
        let source_project_id = transcript_project_id(path);
        let turns = match path.extension().and_then(|ext| ext.to_str()) {
            Some("jsonl") => parse_jsonl_transcript(&raw, source_project_id.as_deref()),
            Some("txt") => parse_legacy_transcript(&raw),
            _ => Vec::new(),
        };
        if turns.is_empty() {
            continue;
        }
        let project = turns
            .iter()
            .find_map(|turn| extract_workspace_path(&turn.user_message))
            .or_else(|| {
                source_project_id
                    .as_deref()
                    .map(discovery::project_from_folder_id)
            });
        let session = out.entry(conversation_id).or_default();
        if session.turns.is_empty() || turns.len() > session.turns.len() {
            session.turns = turns;
        }
        if session.project.is_none() {
            session.project = project;
        }
        session.timestamp = file_modified_timestamp(path).or(session.timestamp);
        session.source_paths.push(path.clone());
    }
    out
}

fn parse_jsonl_transcript(raw: &str, source_project_id: Option<&str>) -> Vec<RichTurn> {
    let mut turns = Vec::new();
    let mut current: Option<RichTurn> = None;
    for line in raw.lines().filter(|line| !line.trim().is_empty()) {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let Some(role) = value.get("role").and_then(Value::as_str) else {
            continue;
        };
        match role {
            "user" => {
                flush_turn(&mut current, &mut turns);
                let content = value
                    .pointer("/message/content")
                    .or_else(|| value.get("content"));
                let mut turn = RichTurn {
                    request_id: value
                        .pointer("/providerOptions/cursor/requestId")
                        .and_then(Value::as_str)
                        .map(ToString::to_string),
                    ..RichTurn::default()
                };
                if let Some(content) = content {
                    analyze_message_content("user", content, &mut turn);
                }
                current = Some(turn);
            }
            "assistant" => {
                if let Some(turn) = current.as_mut() {
                    let content = value
                        .pointer("/message/content")
                        .or_else(|| value.get("content"));
                    if let Some(content) = content {
                        analyze_message_content("assistant", content, turn);
                    }
                    if extract_workspace_path(&turn.user_message).is_none() {
                        let project = source_project_id.map(discovery::project_from_folder_id);
                        if let Some(project) = project {
                            turn.referenced_files
                                .retain(|path| !path.starts_with(&project));
                        }
                    }
                }
            }
            _ => {}
        }
    }
    flush_turn(&mut current, &mut turns);
    turns
}

fn parse_legacy_transcript(raw: &str) -> Vec<RichTurn> {
    let mut turns = Vec::new();
    let mut current: Option<RichTurn> = None;
    enum Block {
        None,
        User,
        Assistant,
    }
    let mut block = Block::None;
    for line in raw.lines() {
        if let Some(text) = strip_case_prefix(line, "user:") {
            flush_turn(&mut current, &mut turns);
            current = Some(RichTurn {
                user_message: extract_user_query(text),
                ..RichTurn::default()
            });
            block = Block::User;
        } else if let Some(text) = line.trim_start().strip_prefix("A:") {
            if let Some(turn) = current.as_mut() {
                append_text(&mut turn.response_text, text);
            }
            block = Block::Assistant;
        } else if let Some(turn) = current.as_mut() {
            match block {
                Block::User => append_text(&mut turn.user_message, line),
                Block::Assistant => {
                    if let Some(tool) = strip_case_prefix(line, "[tool call]") {
                        turn.tools.push(normalize_tool_name(tool));
                    } else if !line
                        .trim_start()
                        .to_ascii_lowercase()
                        .starts_with("[tool result]")
                    {
                        append_text(&mut turn.response_text, line);
                    }
                }
                Block::None => {}
            }
        }
    }
    flush_turn(&mut current, &mut turns);
    turns
}

fn flush_turn(current: &mut Option<RichTurn>, turns: &mut Vec<RichTurn>) {
    let Some(mut turn) = current.take() else {
        return;
    };
    turn.finish();
    if !turn.user_message.is_empty()
        && (!turn.response_text.is_empty()
            || !turn.tools.is_empty()
            || !turn.reasoning_text.is_empty())
    {
        turns.push(turn);
    }
}

fn load_tracking(path: &Path) -> HashMap<String, TrackingSession> {
    let Some(conn) = open_sqlite_read_only(path) else {
        return HashMap::new();
    };
    let mut out = HashMap::new();
    if let Ok(mut stmt) = conn.prepare(config::TRACKING_SUMMARY_QUERY) {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<String>>(2)?,
                column_as_string(row, 3)?,
            ))
        }) {
            for (id, model, mode, timestamp) in rows.flatten() {
                out.insert(
                    id,
                    TrackingSession {
                        model,
                        mode: interaction_mode(mode.as_deref(), None, None, false),
                        timestamp: timestamp.as_deref().and_then(parse_timestamp),
                        ..TrackingSession::default()
                    },
                );
            }
        }
    }
    if let Ok(mut stmt) = conn.prepare(config::TRACKING_HASH_QUERY) {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                column_as_string(row, 2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        }) {
            for (id, model, timestamp, file_name) in rows.flatten() {
                let session = out.entry(id).or_insert_with(TrackingSession::default);
                if session.model.is_none() {
                    session.model = model;
                }
                if session.timestamp.is_none() {
                    session.timestamp = timestamp.as_deref().and_then(parse_timestamp);
                }
                if let Some(file_name) = file_name.filter(|path| looks_file_path(path)) {
                    if session.project.is_none() {
                        session.project = project_from_path(&file_name);
                    }
                    session.edited_files.push(clean_file_path(&file_name));
                }
            }
        }
    }
    for session in out.values_mut() {
        session.edited_files = dedup_files(std::mem::take(&mut session.edited_files));
    }
    out
}

fn load_stores(paths: &[PathBuf]) -> HashMap<String, StoreSession> {
    paths
        .iter()
        .filter_map(|path| parse_store(path))
        .map(|session| (session.conversation_id.clone(), session))
        .collect()
}

fn parse_store(path: &Path) -> Option<StoreSession> {
    let conn = open_sqlite_read_only(path)?;
    let meta_hex: String = conn
        .query_row(config::STORE_META_QUERY, [], |row| row.get(0))
        .ok()?;
    let meta_bytes = decode_hex(&meta_hex)?;
    let meta: Value = serde_json::from_slice(&meta_bytes).ok()?;
    let conversation_id = meta
        .get("agentId")
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .or_else(|| {
            path.parent()
                .and_then(Path::file_name)
                .and_then(|name| name.to_str())
                .map(ToString::to_string)
        })?;
    let root_id = meta.get("latestRootBlobId").and_then(Value::as_str)?;
    let root: Vec<u8> = conn
        .query_row(config::STORE_BLOB_QUERY, [root_id], |row| row.get(0))
        .ok()?;
    let ids = protobuf_field_one_ids(&root);
    if ids.is_empty() {
        return None;
    }
    let mut messages = Vec::new();
    for id in ids {
        let data: Vec<u8> = match conn.query_row(config::STORE_BLOB_QUERY, [id], |row| row.get(0)) {
            Ok(data) => data,
            Err(_) => continue,
        };
        if let Ok(message) = serde_json::from_slice::<Value>(&data) {
            if message.get("role").and_then(Value::as_str).is_some() {
                messages.push(message);
            }
        }
    }
    if messages.is_empty() {
        return None;
    }
    let turns = turns_from_store_messages(&messages);
    if turns.is_empty() {
        return None;
    }
    let meta_json = path
        .parent()
        .map(|dir| dir.join("meta.json"))
        .and_then(|path| fs::read_to_string(path).ok())
        .and_then(|raw| serde_json::from_str::<Value>(&raw).ok());
    let timestamp = meta_json
        .as_ref()
        .and_then(|value| value.get("updatedAtMs"))
        .and_then(value_timestamp)
        .or_else(|| meta.get("createdAt").and_then(value_timestamp));
    Some(StoreSession {
        conversation_id,
        model: meta
            .get("lastUsedModel")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        mode: interaction_mode(meta.get("mode").and_then(Value::as_str), None, None, false),
        timestamp,
        project: None,
        turns,
    })
}

fn turns_from_store_messages(messages: &[Value]) -> Vec<RichTurn> {
    let mut turns = Vec::new();
    let mut current: Option<RichTurn> = None;
    for message in messages {
        let role = message.get("role").and_then(Value::as_str).unwrap_or("");
        match role {
            "user" => {
                flush_turn(&mut current, &mut turns);
                let mut turn = RichTurn {
                    request_id: message
                        .pointer("/providerOptions/cursor/requestId")
                        .and_then(Value::as_str)
                        .map(ToString::to_string),
                    ..RichTurn::default()
                };
                if let Some(content) = message.get("content") {
                    analyze_message_content("user", content, &mut turn);
                }
                current = Some(turn);
            }
            "assistant" => {
                if let (Some(turn), Some(content)) = (current.as_mut(), message.get("content")) {
                    analyze_message_content("assistant", content, turn);
                }
            }
            "tool" => {
                if let Some(turn) = current.as_mut() {
                    if turn.elapsed_ms.is_none() {
                        turn.elapsed_ms = find_duration_ms(message);
                    }
                }
            }
            _ => {}
        }
    }
    flush_turn(&mut current, &mut turns);
    turns
}

fn protobuf_field_one_ids(raw: &[u8]) -> Vec<String> {
    let mut ids = Vec::new();
    let mut offset = 0usize;
    while offset < raw.len() {
        let Some((key, used)) = read_varint(&raw[offset..]) else {
            break;
        };
        offset += used;
        let field = key >> 3;
        let wire = key & 7;
        match wire {
            0 => {
                let Some((_, used)) = read_varint(&raw[offset..]) else {
                    break;
                };
                offset += used;
            }
            1 => offset = offset.saturating_add(8).min(raw.len()),
            2 => {
                let Some((len, used)) = read_varint(&raw[offset..]) else {
                    break;
                };
                offset += used;
                let Ok(len) = usize::try_from(len) else { break };
                let Some(end) = offset.checked_add(len).filter(|end| *end <= raw.len()) else {
                    break;
                };
                if field == 1 && len == 32 {
                    ids.push(encode_hex(&raw[offset..end]));
                }
                offset = end;
            }
            5 => offset = offset.saturating_add(4).min(raw.len()),
            _ => break,
        }
    }
    ids
}

fn read_varint(raw: &[u8]) -> Option<(u64, usize)> {
    let mut value = 0u64;
    for (idx, byte) in raw.iter().copied().take(10).enumerate() {
        value |= u64::from(byte & 0x7f) << (idx * 7);
        if byte & 0x80 == 0 {
            return Some((value, idx + 1));
        }
    }
    None
}

fn decode_hex(raw: &str) -> Option<Vec<u8>> {
    if !raw.len().is_multiple_of(2) {
        return None;
    }
    (0..raw.len())
        .step_by(2)
        .map(|idx| u8::from_str_radix(&raw[idx..idx + 2], 16).ok())
        .collect()
}

fn encode_hex(raw: &[u8]) -> String {
    use std::fmt::Write;
    let mut out = String::with_capacity(raw.len() * 2);
    for byte in raw {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

fn calls_from_store(
    store: &StoreSession,
    transcript: Option<&TranscriptSession>,
    tracking: Option<&TrackingSession>,
    fallback_project: &str,
) -> Vec<ParsedCall> {
    store
        .turns
        .iter()
        .enumerate()
        .map(|(idx, store_turn)| {
            let mut rich = store_turn.clone();
            if let Some(transcript_turn) = transcript
                .and_then(|session| match_turn(&session.turns, rich.request_id.as_deref(), idx))
            {
                rich.fill_from(transcript_turn);
            }
            if rich.edited_files.is_empty() && idx + 1 == store.turns.len() {
                if let Some(files) = tracking.map(|session| &session.edited_files) {
                    rich.edited_files.clone_from(files);
                }
            }
            rich.finish();
            let model = rich
                .model
                .clone()
                .or_else(|| tracking.and_then(|session| session.model.clone()))
                .or_else(|| store.model.clone())
                .unwrap_or_else(|| COST_MODEL_FALLBACK.to_string());
            let (input_tokens, output_tokens, token_quality) =
                token_totals(0, 0, rich.prompt_chars(), rich.response_chars());
            let identity = rich.request_id.clone().unwrap_or_else(|| idx.to_string());
            let mut call = ParsedCall {
                tool: config::TOOL_ID,
                model: model.clone(),
                input_tokens,
                output_tokens,
                timestamp: store
                    .timestamp
                    .or_else(|| tracking.and_then(|session| session.timestamp)),
                speed: speed_from_model(&model),
                dedup_key: format!("cursor:chat:{}:{identity}", store.conversation_id),
                user_message: truncate_chars(&rich.user_message, 500),
                session_id: store.conversation_id.clone(),
                project: transcript
                    .and_then(|session| session.project.clone())
                    .or_else(|| tracking.and_then(|session| session.project.clone()))
                    .or_else(|| store.project.clone())
                    .unwrap_or_else(|| fallback_project.to_string()),
                prompt_chars: Some(rich.prompt_chars()),
                response_chars: Some(rich.response_chars()),
                elapsed_ms: rich.elapsed_ms,
                tools: rich.tools,
                bash_commands: rich.bash_commands,
                code_blocks: rich.code_blocks,
                edited_files: rich.edited_files,
                referenced_files: rich.referenced_files,
                interaction_mode: store
                    .mode
                    .or_known(tracking.map(|session| session.mode).unwrap_or_default()),
                token_quality,
                timestamp_quality: if store.timestamp.is_some()
                    || tracking.and_then(|session| session.timestamp).is_some()
                {
                    TimestampQuality::Session
                } else {
                    TimestampQuality::Unknown
                },
                superseded_dedup_keys: legacy_transcript_keys(
                    transcript,
                    &store.conversation_id,
                    idx,
                ),
                ..ParsedCall::default()
            };
            call.cost_usd = pricing::cost(&model, &call, call.speed);
            call
        })
        .collect()
}

fn calls_from_transcript(
    conversation_id: &str,
    transcript: &TranscriptSession,
    tracking: Option<&TrackingSession>,
    fallback_project: &str,
) -> Vec<ParsedCall> {
    transcript
        .turns
        .iter()
        .enumerate()
        .map(|(idx, transcript_turn)| {
            let mut rich = transcript_turn.clone();
            if rich.edited_files.is_empty() && idx + 1 == transcript.turns.len() {
                if let Some(files) = tracking.map(|session| &session.edited_files) {
                    rich.edited_files.clone_from(files);
                }
            }
            rich.finish();
            let model = rich
                .model
                .clone()
                .or_else(|| tracking.and_then(|session| session.model.clone()))
                .unwrap_or_else(|| COST_MODEL_FALLBACK.to_string());
            let (input_tokens, output_tokens, token_quality) =
                token_totals(0, 0, rich.prompt_chars(), rich.response_chars());
            let timestamp = tracking
                .and_then(|session| session.timestamp)
                .or(transcript.timestamp);
            let mut call = ParsedCall {
                tool: config::TOOL_ID,
                model: model.clone(),
                input_tokens,
                output_tokens,
                timestamp,
                speed: speed_from_model(&model),
                dedup_key: format!("cursor:transcript:{conversation_id}:{idx}"),
                user_message: truncate_chars(&rich.user_message, 500),
                session_id: conversation_id.to_string(),
                project: transcript
                    .project
                    .clone()
                    .or_else(|| tracking.and_then(|session| session.project.clone()))
                    .unwrap_or_else(|| fallback_project.to_string()),
                prompt_chars: Some(rich.prompt_chars()),
                response_chars: Some(rich.response_chars()),
                elapsed_ms: rich.elapsed_ms,
                tools: rich.tools,
                bash_commands: rich.bash_commands,
                code_blocks: rich.code_blocks,
                edited_files: rich.edited_files,
                referenced_files: rich.referenced_files,
                interaction_mode: tracking.map(|session| session.mode).unwrap_or_default(),
                token_quality,
                timestamp_quality: if tracking.and_then(|session| session.timestamp).is_some() {
                    TimestampQuality::Session
                } else if transcript.timestamp.is_some() {
                    TimestampQuality::File
                } else {
                    TimestampQuality::Unknown
                },
                superseded_dedup_keys: legacy_transcript_keys(
                    Some(transcript),
                    conversation_id,
                    idx,
                ),
                ..ParsedCall::default()
            };
            call.cost_usd = pricing::cost(&model, &call, call.speed);
            call
        })
        .collect()
}

fn interaction_mode(
    primary: Option<&str>,
    secondary: Option<&str>,
    is_agentic: Option<bool>,
    is_plan: bool,
) -> InteractionMode {
    if is_plan {
        return InteractionMode::Plan;
    }
    for raw in [primary, secondary].into_iter().flatten() {
        let lower = raw.to_ascii_lowercase();
        if lower.contains("plan") {
            return InteractionMode::Plan;
        }
        if lower.contains("chat") || lower == "ask" {
            return InteractionMode::Chat;
        }
        if lower.contains("agent") || lower.contains("auto") || lower == "edit" {
            return InteractionMode::Agent;
        }
    }
    match is_agentic {
        Some(true) => InteractionMode::Agent,
        Some(false) => InteractionMode::Chat,
        None => InteractionMode::Unknown,
    }
}

fn normalize_tool_name(raw: &str) -> String {
    let clean = raw.trim();
    let lower = clean.to_ascii_lowercase().replace(['_', ' '], "-");
    match lower.as_str() {
        "bash" | "shell" | "terminal" | "run-terminal-command" => "Bash".to_string(),
        "read" | "read-file" => "Read".to_string(),
        "grep" | "search" | "codebase-search" => "Grep".to_string(),
        "glob" | "find-files" => "Glob".to_string(),
        "write" | "write-file" => "Write".to_string(),
        "strreplace" | "str-replace" | "edit-file" => "StrReplace".to_string(),
        "createplan" | "create-plan" => "CreatePlan".to_string(),
        "readlints" | "read-lints" => "ReadLints".to_string(),
        _ => clean.to_string(),
    }
}

fn speed_from_model(model: &str) -> Speed {
    if model.to_ascii_lowercase().contains("fast") {
        Speed::Fast
    } else {
        Speed::Standard
    }
}

fn extract_user_query(text: &str) -> String {
    if let Some(start) = text.find("<user_query>") {
        let inner = &text[start + "<user_query>".len()..];
        if let Some(end) = inner.find("</user_query>") {
            return inner[..end].trim().to_string();
        }
    }
    text.trim().to_string()
}

fn extract_workspace_path(text: &str) -> Option<String> {
    for line in text.lines() {
        let trimmed = line.trim();
        let Some((label, value)) = trimmed.split_once(':') else {
            continue;
        };
        if label.eq_ignore_ascii_case("workspace path") && looks_file_path(value.trim()) {
            return Some(value.trim().to_string());
        }
    }
    None
}

fn collect_paths_from_json(raw: Option<&str>, out: &mut Vec<String>) {
    let Some(raw) = raw else { return };
    if let Ok(value) = serde_json::from_str::<Value>(raw) {
        collect_paths(&value, out);
    }
}

fn collect_paths(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::String(text) if looks_file_path(text) => out.push(clean_file_path(text)),
        Value::Array(values) => values.iter().for_each(|value| collect_paths(value, out)),
        Value::Object(map) => {
            for (key, value) in map {
                if !matches!(
                    key.as_str(),
                    "projectLayouts" | "terminalFiles" | "result" | "output"
                ) {
                    collect_paths(value, out);
                }
            }
        }
        _ => {}
    }
}

fn looks_file_path(raw: &str) -> bool {
    let raw = raw.trim();
    if raw.is_empty() || raw.len() > 512 || raw.contains('\n') || raw.contains("/.cursor/") {
        return false;
    }
    let path = raw.strip_prefix("file://").unwrap_or(raw);
    (path.starts_with('/') || path.starts_with("~/") || path.contains(":\\"))
        && Path::new(path).file_name().is_some()
}

fn clean_file_path(raw: &str) -> String {
    raw.trim()
        .trim_matches(['"', '\'', '`', ',', ';', ')', '(', '[', ']', '{', '}'])
        .strip_prefix("file://")
        .unwrap_or(raw.trim())
        .to_string()
}

fn project_from_path(raw: &str) -> Option<String> {
    let clean = clean_file_path(raw);
    for anchor in ["/Code/", "/Desktop/", "/Documents/"] {
        if let Some(idx) = clean.find(anchor) {
            let end = idx + anchor.len();
            let project = clean[end..].split('/').next()?;
            return Some(format!("{}{project}", &clean[..end]));
        }
    }
    Path::new(&clean)
        .parent()
        .map(|path| path.to_string_lossy().to_string())
}

fn transcript_conversation_id(path: &Path) -> String {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .unwrap_or("unknown")
        .to_string()
}

fn transcript_project_id(path: &Path) -> Option<String> {
    let mut dir = path.parent();
    while let Some(current) = dir {
        if current.file_name().and_then(|name| name.to_str()) == Some(config::AGENT_TRANSCRIPTS_DIR)
        {
            return current
                .parent()
                .and_then(Path::file_name)
                .and_then(|name| name.to_str())
                .map(ToString::to_string);
        }
        dir = current.parent();
    }
    None
}

fn is_transcript_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some("jsonl" | "txt")
    )
}

fn file_modified_timestamp(path: &Path) -> Option<DateTime<Utc>> {
    fs::metadata(path)
        .ok()
        .and_then(|metadata| metadata.modified().ok())
        .map(DateTime::<Utc>::from)
}

fn value_timestamp(value: &Value) -> Option<DateTime<Utc>> {
    value
        .as_i64()
        .map(|value| value.to_string())
        .or_else(|| value.as_str().map(ToString::to_string))
        .as_deref()
        .and_then(parse_timestamp)
}

fn parse_timestamp(raw: &str) -> Option<DateTime<Utc>> {
    if let Ok(timestamp) = DateTime::parse_from_rfc3339(raw) {
        return Some(timestamp.with_timezone(&Utc));
    }
    let value = raw.parse::<i64>().ok()?;
    if value.abs() < 1_000_000_000_000 {
        DateTime::from_timestamp(value, 0)
    } else {
        DateTime::from_timestamp_millis(value)
    }
}

fn column_as_string(row: &rusqlite::Row<'_>, idx: usize) -> rusqlite::Result<Option<String>> {
    use rusqlite::types::ValueRef;
    match row.get_ref(idx)? {
        ValueRef::Null => Ok(None),
        ValueRef::Text(bytes) | ValueRef::Blob(bytes) => {
            Ok(Some(String::from_utf8_lossy(bytes).into_owned()))
        }
        ValueRef::Integer(value) => Ok(Some(value.to_string())),
        ValueRef::Real(value) => Ok(Some(value.to_string())),
    }
}

fn non_negative_i64(value: Option<i64>) -> u64 {
    value
        .and_then(|value| u64::try_from(value).ok())
        .unwrap_or(0)
}

fn append_text(target: &mut String, text: &str) {
    if text.is_empty() {
        return;
    }
    if !target.is_empty() {
        target.push('\n');
    }
    target.push_str(text);
}

fn merge_unique(target: &mut Vec<String>, values: &[String]) {
    for value in values {
        if !value.is_empty() && !target.contains(value) {
            target.push(value.clone());
        }
    }
}

fn dedup_strings(values: &mut Vec<String>) {
    let mut seen = HashSet::new();
    values.retain(|value| !value.is_empty() && seen.insert(value.clone()));
}

fn strip_case_prefix<'a>(line: &'a str, prefix: &str) -> Option<&'a str> {
    let trimmed = line.trim_start();
    trimmed
        .to_ascii_lowercase()
        .starts_with(&prefix.to_ascii_lowercase())
        .then(|| trimmed[prefix.len()..].trim())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let suffix = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let counter = TEMP_COUNTER.fetch_add(1, Ordering::SeqCst);
            let path = std::env::temp_dir().join(format!(
                "tokenuse-cursor-store-{}-{suffix}-{counter}",
                std::process::id()
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn seed_state() -> Connection {
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

    #[test]
    fn modern_bubbles_reconstruct_one_call_per_user_turn() {
        let conn = seed_state();
        insert(
            &conn,
            "composerData:composer-1",
            r#"{"composerId":"composer-1","fullConversationHeadersOnly":[{"bubbleId":"u1","type":1},{"bubbleId":"a1","type":2},{"bubbleId":"t1","type":2}],"modelConfig":{"modelName":"composer-2.5"},"forceMode":"agent","status":"completed"}"#,
        );
        insert(
            &conn,
            "bubbleId:composer-1:u1",
            r#"{"bubbleId":"u1","type":1,"requestId":"request-1","createdAt":"2026-07-01T10:00:00Z","text":"fix the bug","tokenCount":{"inputTokens":0,"outputTokens":0}}"#,
        );
        insert(
            &conn,
            "bubbleId:composer-1:a1",
            r#"{"bubbleId":"a1","type":2,"requestId":"request-1","createdAt":"2026-07-01T10:00:02Z","text":"Done.","tokenCount":{"inputTokens":100,"outputTokens":20},"modelInfo":{"modelName":"claude-4.5-sonnet-thinking"},"turnDurationMs":2000}"#,
        );
        insert(
            &conn,
            "bubbleId:composer-1:t1",
            r#"{"bubbleId":"t1","type":2,"requestId":"request-1","createdAt":"2026-07-01T10:00:01Z","text":"","tokenCount":{"inputTokens":0,"outputTokens":0},"toolFormerData":{"name":"Shell","status":"completed","params":"{\"command\":\"cargo test\"}"}}"#,
        );
        let source = SessionSource::session(PathBuf::from("unused"), "workspace", config::TOOL_ID);
        let calls = reconstruct_state(
            &conn,
            &source,
            &HashMap::new(),
            &HashMap::new(),
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].dedup_key, "cursor:composer:composer-1:request-1");
        assert_eq!(calls[0].session_id, "composer-1");
        assert_eq!(calls[0].tools, vec!["Bash"]);
        assert_eq!(calls[0].bash_commands, vec!["cargo test"]);
        assert_eq!(calls[0].timestamp_quality, TimestampQuality::Exact);
        assert_eq!(calls[0].token_quality, TokenQuality::Exact);
    }

    #[test]
    fn context_meter_credits_input_once_per_conversation() {
        let conn = seed_state();
        insert(
            &conn,
            "composerData:composer-m",
            r#"{"composerId":"composer-m","createdAt":1780157113020,"promptTokenBreakdown":{"totalUsedTokens":32000},"fullConversationHeadersOnly":[{"bubbleId":"u1","type":1},{"bubbleId":"a1","type":2}],"modelConfig":{"modelName":"claude-4.5-sonnet"},"forceMode":"agent","status":"completed"}"#,
        );
        // Current builds: bubbles carry {0,0} token counts.
        insert(
            &conn,
            "bubbleId:composer-m:u1",
            r#"{"bubbleId":"u1","type":1,"requestId":"request-1","createdAt":"2026-07-01T10:00:00Z","text":"fix the bug","tokenCount":{"inputTokens":0,"outputTokens":0}}"#,
        );
        insert(
            &conn,
            "bubbleId:composer-m:a1",
            r#"{"bubbleId":"a1","type":2,"requestId":"request-1","createdAt":"2026-07-01T10:00:02Z","text":"Done.","tokenCount":{"inputTokens":0,"outputTokens":0}}"#,
        );

        let source = SessionSource::session(PathBuf::from("unused"), "workspace", config::TOOL_ID);
        let calls = reconstruct_state(
            &conn,
            &source,
            &HashMap::new(),
            &HashMap::new(),
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();

        assert_eq!(calls.len(), 2, "one turn call plus the meter credit");
        let turn = &calls[0];
        assert_eq!(
            turn.input_tokens, 0,
            "per-turn chars/4 input estimates are dropped when the meter covers input"
        );
        assert!(turn.output_tokens > 0, "output side stays with the turn");
        assert_eq!(turn.token_quality, TokenQuality::Estimated);

        let credit = &calls[1];
        assert_eq!(credit.dedup_key, "cursor:composer-input:composer-m");
        assert_eq!(credit.input_tokens, 32000, "Cursor's own context meter");
        assert_eq!(credit.output_tokens, 0);
        assert_eq!(credit.session_id, "composer-m");
        assert_eq!(credit.token_quality, TokenQuality::Estimated);
        assert_eq!(
            credit.timestamp_quality,
            TimestampQuality::Session,
            "anchored at conversation creation, not a message time"
        );
        assert!(credit.timestamp.is_some());
        assert!(credit.cost_usd > 0.0);
    }

    #[test]
    fn explicit_bubble_tokens_disable_the_meter_credit() {
        let conn = seed_state();
        insert(
            &conn,
            "composerData:composer-x",
            r#"{"composerId":"composer-x","createdAt":1780157113020,"promptTokenBreakdown":{"totalUsedTokens":32000},"fullConversationHeadersOnly":[{"bubbleId":"u1","type":1},{"bubbleId":"a1","type":2}],"status":"completed"}"#,
        );
        insert(
            &conn,
            "bubbleId:composer-x:u1",
            r#"{"bubbleId":"u1","type":1,"requestId":"request-1","createdAt":"2026-07-01T10:00:00Z","text":"fix","tokenCount":{"inputTokens":0,"outputTokens":0}}"#,
        );
        insert(
            &conn,
            "bubbleId:composer-x:a1",
            r#"{"bubbleId":"a1","type":2,"requestId":"request-1","createdAt":"2026-07-01T10:00:02Z","text":"Done.","tokenCount":{"inputTokens":100,"outputTokens":20}}"#,
        );

        let source = SessionSource::session(PathBuf::from("unused"), "workspace", config::TOOL_ID);
        let calls = reconstruct_state(
            &conn,
            &source,
            &HashMap::new(),
            &HashMap::new(),
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();

        assert_eq!(calls.len(), 1, "explicit bubble tokens win outright");
        assert_eq!(calls[0].input_tokens, 100);
        assert_eq!(calls[0].output_tokens, 20);
    }

    #[test]
    fn workspace_folder_maps_composers_to_projects() {
        let dir = TempDir::new();
        let hash = dir.path().join("abc123");
        std::fs::create_dir_all(&hash).unwrap();
        std::fs::write(
            hash.join("workspace.json"),
            r#"{"folder":"file:///Users/me/Code/my%20app"}"#,
        )
        .unwrap();
        {
            let ws_conn = Connection::open(hash.join(config::STATE_DB)).unwrap();
            ws_conn
                .execute(
                    "CREATE TABLE ItemTable (key TEXT PRIMARY KEY, value TEXT)",
                    [],
                )
                .unwrap();
            ws_conn
                .execute(
                    "INSERT INTO ItemTable(key, value) VALUES ('composer.composerHeaders', ?1)",
                    [r#"{"allComposers":[{"composerId":"composer-w"}]}"#],
                )
                .unwrap();
        }

        let map = load_workspace_projects(Some(dir.path().to_path_buf()));
        assert_eq!(
            map.get("composer-w").map(String::as_str),
            Some("/Users/me/Code/my app"),
            "file:// folders decode to plain absolute paths"
        );

        // The mapping slots in ahead of the source fallback.
        let conn = seed_state();
        insert(
            &conn,
            "composerData:composer-w",
            r#"{"composerId":"composer-w","fullConversationHeadersOnly":[{"bubbleId":"u1","type":1},{"bubbleId":"a1","type":2}],"status":"completed"}"#,
        );
        insert(
            &conn,
            "bubbleId:composer-w:u1",
            r#"{"bubbleId":"u1","type":1,"requestId":"request-1","createdAt":"2026-07-01T10:00:00Z","text":"hello","tokenCount":{"inputTokens":0,"outputTokens":0}}"#,
        );
        insert(
            &conn,
            "bubbleId:composer-w:a1",
            r#"{"bubbleId":"a1","type":2,"requestId":"request-1","createdAt":"2026-07-01T10:00:02Z","text":"hi","tokenCount":{"inputTokens":1,"outputTokens":1}}"#,
        );
        let source = SessionSource::session(PathBuf::from("unused"), "workspace", config::TOOL_ID);
        let calls = reconstruct_state(
            &conn,
            &source,
            &HashMap::new(),
            &HashMap::new(),
            &HashMap::new(),
            &map,
        )
        .unwrap();
        assert_eq!(calls[0].project, "/Users/me/Code/my app");
    }

    #[test]
    fn malformed_protobuf_stops_cleanly() {
        assert!(protobuf_field_one_ids(&[0xff, 0xff]).is_empty());
    }

    #[test]
    fn protobuf_reads_ordered_field_one_blob_ids() {
        let first = [1u8; 32];
        let second = [2u8; 32];
        let mut bytes = vec![0x0a, 0x20];
        bytes.extend(first);
        bytes.extend([0x0a, 0x20]);
        bytes.extend(second);
        assert_eq!(
            protobuf_field_one_ids(&bytes),
            vec![encode_hex(&first), encode_hex(&second)]
        );
    }

    #[test]
    fn cancelled_tools_do_not_cancel_turns() {
        let mut turn = RichTurn::default();
        apply_tool(
            "Read",
            &[serde_json::json!({"path":"/tmp/a.rs"})],
            &mut turn,
        );
        assert_eq!(turn.referenced_files, vec!["/tmp/a.rs"]);
    }

    #[test]
    fn reversed_claude_style_keeps_fast_speed_separate() {
        assert_eq!(
            speed_from_model("claude-4.5-sonnet-thinking"),
            Speed::Standard
        );
        assert_eq!(speed_from_model("composer-2.5-fast"), Speed::Fast);
    }

    #[test]
    fn current_store_db_decodes_ordered_messages_through_wal() {
        let dir = TempDir::new();
        let path = dir.0.join(config::STORE_DB);
        let writer = Connection::open(&path).unwrap();
        writer.pragma_update(None, "journal_mode", "WAL").unwrap();
        writer
            .execute_batch(
                "CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT);
                 CREATE TABLE blobs (id TEXT PRIMARY KEY, data BLOB);",
            )
            .unwrap();

        let user_id = [1u8; 32];
        let assistant_id = [2u8; 32];
        let root_id = [3u8; 32];
        let mut root = vec![0x0a, 0x20];
        root.extend(user_id);
        root.extend([0x0a, 0x20]);
        root.extend(assistant_id);
        let meta = serde_json::json!({
            "agentId": "conversation-1",
            "latestRootBlobId": encode_hex(&root_id),
            "createdAt": 1_783_000_000_000_i64,
            "lastUsedModel": "composer-2.5",
            "mode": "auto-run"
        });
        writer
            .execute(
                "INSERT INTO meta(key, value) VALUES ('0', ?1)",
                [encode_hex(meta.to_string().as_bytes())],
            )
            .unwrap();
        writer
            .execute(
                "INSERT INTO blobs(id, data) VALUES (?1, ?2)",
                rusqlite::params![encode_hex(&root_id), root],
            )
            .unwrap();
        writer
            .execute(
                "INSERT INTO blobs(id, data) VALUES (?1, ?2)",
                rusqlite::params![
                    encode_hex(&user_id),
                    br#"{"role":"user","content":[{"type":"text","text":"<user_query>fix it</user_query>"}],"providerOptions":{"cursor":{"requestId":"request-1"}}}"#.as_slice()
                ],
            )
            .unwrap();
        writer
            .execute(
                "INSERT INTO blobs(id, data) VALUES (?1, ?2)",
                rusqlite::params![
                    encode_hex(&assistant_id),
                    br#"{"role":"assistant","content":[{"type":"reasoning","text":"check","providerOptions":{"cursor":{"modelName":"composer-2.5"}}},{"type":"text","text":"done"}]}"#.as_slice()
                ],
            )
            .unwrap();

        let session = parse_store(&path).unwrap();
        assert_eq!(session.conversation_id, "conversation-1");
        assert_eq!(session.turns.len(), 1);
        assert_eq!(session.turns[0].request_id.as_deref(), Some("request-1"));
        assert_eq!(session.turns[0].reasoning_text, "check");
    }

    #[test]
    fn malformed_legacy_store_is_skipped_without_panicking() {
        let dir = TempDir::new();
        let path = dir.0.join(config::STORE_DB);
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(
            "CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT);
             CREATE TABLE blobs (id TEXT PRIMARY KEY, data BLOB);",
        )
        .unwrap();
        conn.execute("INSERT INTO meta VALUES ('0', 'not-hex')", [])
            .unwrap();
        assert!(parse_store(&path).is_none());
    }

    #[test]
    #[ignore = "requires a private CURSOR_AGENT_HOME corpus"]
    fn private_cursor_acceptance_has_only_canonical_turn_keys() {
        let source = super::discovery::discover()
            .unwrap()
            .into_iter()
            .next()
            .expect("CURSOR_AGENT_HOME must contain Cursor data");
        let mut warmup_seen = HashSet::new();
        let warmup = parse_session(&source, &mut warmup_seen).unwrap();
        assert!(!warmup.is_empty());

        let started = std::time::Instant::now();
        let mut seen = HashSet::new();
        let calls = parse_session(&source, &mut seen).unwrap();
        let elapsed = started.elapsed();
        assert!(
            elapsed <= std::time::Duration::from_secs(5),
            "warm parse took {elapsed:?}"
        );
        assert!(calls.iter().all(|call| {
            call.dedup_key.starts_with("cursor:composer:")
                || call.dedup_key.starts_with("cursor:chat:")
                || call.dedup_key.starts_with("cursor:transcript:")
        }));
        assert!(calls.iter().all(|call| {
            !call.dedup_key.starts_with("cursor:unknown:")
                && !call.dedup_key.starts_with("cursor:agentKv:")
        }));
    }
}
