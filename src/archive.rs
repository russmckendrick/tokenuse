use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::time::Duration;

use chrono::{DateTime, Utc};
use color_eyre::{
    eyre::{eyre, Context},
    Result,
};
use rusqlite::{params, Connection, OptionalExtension, Transaction};

use crate::config::ConfigPaths;
use crate::ingest::Ingested;
use crate::tools::{
    self, InteractionMode, LimitSnapshot, ParsedCall, SessionSourceKind, Speed, TimestampQuality,
    TokenQuality, ToolAdapter,
};

pub const SYNC_INTERVAL: Duration = crate::ingest_cache::TTL;

const ARCHIVE_SCHEMA_VERSION: u32 = 8;

/// Rates the v8 repair reasons about, frozen as literals rather than read
/// from the books: the repair has to reproduce what was actually charged at
/// import time, and the books move underneath us.
///
/// Until Opus 5 was priced, `claude-opus-5` matched no row and fell through
/// to the books' Sonnet 4.6 fallback, so every Opus 5 call was billed at
/// $3/$15 per MTok instead of $5/$25.
mod opus_5_repair {
    /// Sonnet 4.6 per-token rates, i.e. what these calls were charged at.
    pub const FALLBACK_INPUT: f64 = 3e-6;
    pub const FALLBACK_OUTPUT: f64 = 15e-6;
    pub const FALLBACK_CACHE_WRITE: f64 = 3.75e-6;
    pub const FALLBACK_CACHE_READ: f64 = 3e-7;

    /// Opus 5's input, output, cache-write, and cache-read rates are each
    /// exactly 5/3 of the Sonnet 4.6 rate above, so a single factor restores
    /// the true cost. Rescaling also preserves the 1-hour cache-write
    /// premium, which scales with the cache-write rate but is not persisted
    /// on the row and so could not survive a recompute.
    pub const FACTOR: f64 = 5.0 / 3.0;

    /// Web search bills at $0.01 per request under both rows, so that part of
    /// the stored cost is held out of the rescale.
    pub const WEB_SEARCH: f64 = 0.01;

    /// Fast mode is 2x on Opus 5. The Sonnet 4.6 fallback carries no fast
    /// multiplier, so fast-speed rows were charged at 1x and need it applied.
    pub const FAST_MULTIPLIER: f64 = 2.0;
}

pub struct Archive {
    conn: Connection,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SyncStats {
    pub sources_seen: usize,
    pub sources_parsed: usize,
    pub calls_inserted: usize,
    pub limits_inserted: usize,
    /// Session files whose already-parsed prefix was skipped via a stored
    /// byte-offset cursor instead of being re-read.
    pub files_resumed: usize,
}

pub struct StartupLoad {
    pub ingested: Ingested,
    pub loaded_existing_archive: bool,
    pub legacy_records_imported: usize,
    pub sync_stats: Option<SyncStats>,
}

pub fn load_startup(paths: &ConfigPaths) -> Result<StartupLoad> {
    let mut archive = Archive::open(paths)?;
    let loaded_existing_archive = !archive.is_empty()?;
    if loaded_existing_archive {
        return Ok(StartupLoad {
            ingested: archive.load()?,
            loaded_existing_archive,
            legacy_records_imported: 0,
            sync_stats: None,
        });
    }

    let legacy_records_imported = archive.import_legacy_cache_if_empty()?;
    let sync_stats = archive.sync()?;
    Ok(StartupLoad {
        ingested: archive.load()?,
        loaded_existing_archive,
        legacy_records_imported,
        sync_stats: Some(sync_stats),
    })
}

pub fn sync_and_load(paths: &ConfigPaths) -> Result<Ingested> {
    sync_and_load_with_stats(paths).map(|(ingested, _)| ingested)
}

pub fn sync_and_load_with_stats(paths: &ConfigPaths) -> Result<(Ingested, SyncStats)> {
    let mut archive = Archive::open(paths)?;
    if archive.is_empty()? {
        let _ = archive.import_legacy_cache_if_empty()?;
    }
    let stats = archive.sync()?;
    Ok((archive.load()?, stats))
}

pub fn reset_and_load(paths: &ConfigPaths) -> Result<(Ingested, SyncStats)> {
    paths.ensure_dir()?;
    match fs::remove_file(&paths.archive_db_file) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(remove_err) => {
            let mut archive = Archive::open(paths).wrap_err_with(|| {
                format!(
                    "remove {} failed ({remove_err}); open existing archive",
                    paths.archive_db_file.display()
                )
            })?;
            archive.reset_database().wrap_err_with(|| {
                format!("drop existing archive after remove failed ({remove_err})")
            })?;
            let stats = archive.sync()?;
            let ingested = archive.load()?;
            return Ok((ingested, stats));
        }
    }

    let mut archive = Archive::open(paths)?;
    let stats = archive.sync()?;
    let ingested = archive.load()?;
    Ok((ingested, stats))
}

impl Archive {
    pub fn open(paths: &ConfigPaths) -> Result<Self> {
        paths.ensure_dir()?;
        let conn = Connection::open(&paths.archive_db_file)
            .wrap_err_with(|| format!("open {}", paths.archive_db_file.display()))?;
        conn.busy_timeout(Duration::from_secs(5))?;
        let archive = Self { conn };
        archive.migrate()?;
        Ok(archive)
    }

    pub fn is_empty(&self) -> Result<bool> {
        let calls: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM calls", [], |row| row.get(0))?;
        let limits: i64 =
            self.conn
                .query_row("SELECT COUNT(*) FROM limit_snapshots", [], |row| row.get(0))?;
        Ok(calls == 0 && limits == 0)
    }

    pub fn load(&self) -> Result<Ingested> {
        Ok(Ingested {
            calls: self.load_calls()?,
            limits: self.load_limits()?,
        })
    }

    pub fn sync(&mut self) -> Result<SyncStats> {
        let adapters = tools::registry();
        self.sync_with_adapters(&adapters)
    }

    pub fn reset_database(&mut self) -> Result<()> {
        self.conn.execute_batch(
            "
            DROP TABLE IF EXISTS transcripts_fts;
            DROP TABLE IF EXISTS transcripts;
            DROP TABLE IF EXISTS source_state;
            DROP TABLE IF EXISTS limit_snapshots;
            DROP TABLE IF EXISTS calls;
            PRAGMA user_version = 0;
            ",
        )?;
        self.migrate()
    }

    pub fn sync_with_adapters(&mut self, adapters: &[Box<dyn ToolAdapter>]) -> Result<SyncStats> {
        let mut seen = HashSet::new();
        let mut stats = SyncStats::default();

        for adapter in adapters {
            let sources = match adapter.discover() {
                Ok(sources) => sources,
                Err(_) => continue,
            };

            for source in sources {
                stats.sources_seen += 1;
                let path = source.path.to_string_lossy().to_string();
                let fingerprint = adapter.source_fingerprint(&source).ok();

                if let Some(fingerprint) = fingerprint.as_deref() {
                    if self.source_fingerprint(source.tool, &path)?.as_deref() == Some(fingerprint)
                    {
                        continue;
                    }
                }

                let calls_result = if source.kind == SessionSourceKind::Limit {
                    Ok(crate::tools::AdapterParse::default())
                } else {
                    let stored_cursor = self.source_cursor(source.tool, &path)?;
                    adapter.parse_with_cursor(&source, &mut seen, stored_cursor.as_deref())
                };
                let limits_result = adapter.parse_limits(&source);
                let parsed_calls_ok = calls_result.is_ok();
                let parsed_limits_ok = limits_result.is_ok();
                if source.kind == SessionSourceKind::Limit && !parsed_limits_ok {
                    continue;
                }
                if source.kind == SessionSourceKind::Session
                    && !parsed_calls_ok
                    && !parsed_limits_ok
                {
                    continue;
                }

                let parsed = calls_result.unwrap_or_default();
                let limits = limits_result.unwrap_or_default();
                stats.files_resumed += parsed.resumed_files;
                // IMMEDIATE takes the write lock at BEGIN so the fingerprint
                // re-check below is race-free: two refreshers (TUI, desktop,
                // MCP) that both saw a stale fingerprint serialize here, and
                // the loser skips instead of re-applying the same parse —
                // which would double-append claude tail-merge transcripts.
                let tx = self
                    .conn
                    .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
                if let Some(fingerprint) = fingerprint.as_deref() {
                    let already_synced: Option<String> = tx
                        .query_row(
                            "SELECT fingerprint FROM source_state WHERE tool = ?1 AND path = ?2",
                            params![source.tool, path.as_str()],
                            |row| row.get(0),
                        )
                        .optional()?;
                    if already_synced.as_deref() == Some(fingerprint) {
                        continue;
                    }
                }
                for call in &parsed.calls {
                    if insert_call(&tx, call)? {
                        stats.calls_inserted += 1;
                    }
                }
                for limit in &limits {
                    if insert_limit(&tx, limit)? {
                        stats.limits_inserted += 1;
                    }
                }
                let should_store_fingerprint = match source.kind {
                    SessionSourceKind::Session => parsed_calls_ok,
                    SessionSourceKind::Limit => parsed_limits_ok,
                };
                if should_store_fingerprint {
                    if let Some(fingerprint) = fingerprint.as_deref() {
                        upsert_source_fingerprint(
                            &tx,
                            source.tool,
                            &path,
                            fingerprint,
                            parsed.cursor.as_deref().unwrap_or(""),
                        )?;
                    }
                }
                tx.commit()?;
                stats.sources_parsed += 1;
            }
        }

        Ok(stats)
    }

    pub fn import_legacy_cache_if_empty(&mut self) -> Result<usize> {
        if !self.is_empty()? {
            return Ok(0);
        }
        let Some(path) = crate::ingest_cache::path() else {
            return Ok(0);
        };
        self.import_legacy_cache_from_path(&path)
    }

    pub fn import_legacy_cache_from_path(&mut self, path: &Path) -> Result<usize> {
        if !self.is_empty()? {
            return Ok(0);
        }
        let Some(hit) = crate::ingest_cache::read_path(path) else {
            return Ok(0);
        };
        self.insert_ingested(&hit.ingested)
    }

    pub fn insert_ingested(&mut self, ingested: &Ingested) -> Result<usize> {
        let tx = self.conn.transaction()?;
        let mut inserted = 0;
        for call in &ingested.calls {
            if insert_call(&tx, call)? {
                inserted += 1;
            }
        }
        for limit in &ingested.limits {
            if insert_limit(&tx, limit)? {
                inserted += 1;
            }
        }
        tx.commit()?;
        Ok(inserted)
    }

    fn migrate(&self) -> Result<()> {
        let version: u32 = self
            .conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))?;
        if version > ARCHIVE_SCHEMA_VERSION {
            return Err(eyre!(
                "archive schema v{version} is newer than this binary supports"
            ));
        }

        if version < 1 {
            self.conn.execute_batch(
                "
                CREATE TABLE IF NOT EXISTS calls (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    tool TEXT NOT NULL,
                    dedup_key TEXT NOT NULL,
                    model TEXT NOT NULL,
                    input_tokens INTEGER NOT NULL,
                    output_tokens INTEGER NOT NULL,
                    cache_creation_input_tokens INTEGER NOT NULL,
                    cache_read_input_tokens INTEGER NOT NULL,
                    cached_input_tokens INTEGER NOT NULL,
                    reasoning_tokens INTEGER NOT NULL,
                    web_search_requests INTEGER NOT NULL,
                    cost_usd REAL NOT NULL,
                    tools_json TEXT NOT NULL,
                    bash_commands_json TEXT NOT NULL,
                    timestamp TEXT,
                    speed TEXT NOT NULL,
                    user_message TEXT NOT NULL,
                    session_id TEXT NOT NULL,
                    project TEXT NOT NULL,
                    imported_at TEXT NOT NULL,
                    UNIQUE(tool, dedup_key)
                );

                CREATE INDEX IF NOT EXISTS idx_calls_timestamp ON calls(timestamp);
                CREATE INDEX IF NOT EXISTS idx_calls_tool ON calls(tool);
                CREATE INDEX IF NOT EXISTS idx_calls_project ON calls(project);

                CREATE TABLE IF NOT EXISTS limit_snapshots (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    tool TEXT NOT NULL,
                    limit_id TEXT NOT NULL,
                    limit_name TEXT,
                    plan_type TEXT,
                    observed_at TEXT,
                    primary_json TEXT,
                    secondary_json TEXT,
                    credits_json TEXT,
                    rate_limit_reached_type TEXT,
                    imported_at TEXT NOT NULL,
                    snapshot_key TEXT NOT NULL UNIQUE
                );

                CREATE INDEX IF NOT EXISTS idx_limit_snapshots_tool
                    ON limit_snapshots(tool, limit_id, observed_at);

                CREATE TABLE IF NOT EXISTS source_state (
                    tool TEXT NOT NULL,
                    path TEXT NOT NULL,
                    fingerprint TEXT NOT NULL,
                    synced_at TEXT NOT NULL,
                    PRIMARY KEY(tool, path)
                );

                PRAGMA user_version = 1;
                ",
            )?;
        }

        if version < 3 {
            // v2 added the advice tables; v3 removes the advice engine, so
            // drop them whether or not the v2 migration ever ran.
            self.conn.execute_batch(
                "
                DROP TABLE IF EXISTS advice_items;
                DROP TABLE IF EXISTS advice_runs;

                PRAGMA user_version = 3;
                ",
            )?;
        }

        if version < 4 {
            // v4 adds per-call enrichment for the coach engine. Clearing
            // source_state forces one full re-parse so rows whose source
            // files still exist get enriched via the insert_call backfill.
            // ALTER TABLE is not idempotent, so the batch is transactional:
            // an interrupted migration must not leave half-added columns
            // behind at user_version 3.
            self.conn.execute_batch(
                "
                BEGIN;

                ALTER TABLE calls ADD COLUMN is_canceled INTEGER NOT NULL DEFAULT 0;
                ALTER TABLE calls ADD COLUMN prompt_chars INTEGER;
                ALTER TABLE calls ADD COLUMN response_chars INTEGER;
                ALTER TABLE calls ADD COLUMN elapsed_ms INTEGER;
                ALTER TABLE calls ADD COLUMN code_blocks_json TEXT NOT NULL DEFAULT '[]';
                ALTER TABLE calls ADD COLUMN edited_files_json TEXT NOT NULL DEFAULT '[]';
                ALTER TABLE calls ADD COLUMN referenced_files_json TEXT NOT NULL DEFAULT '[]';

                DELETE FROM source_state;

                PRAGMA user_version = 4;

                COMMIT;
                ",
            )?;
        }
        if version < 5 {
            self.conn.execute_batch(
                "
                BEGIN;

                ALTER TABLE calls ADD COLUMN interaction_mode TEXT NOT NULL DEFAULT 'unknown';
                ALTER TABLE calls ADD COLUMN token_quality TEXT NOT NULL DEFAULT 'unknown';
                ALTER TABLE calls ADD COLUMN timestamp_quality TEXT NOT NULL DEFAULT 'unknown';

                UPDATE calls
                SET timestamp_quality = 'exact'
                WHERE timestamp IS NOT NULL;

                DELETE FROM source_state;

                PRAGMA user_version = 5;

                COMMIT;
                ",
            )?;
        }
        if version < 6 {
            // v6 adds the incremental-parse cursor (adapter-owned JSON with
            // per-file byte offsets and a parse version). Purely additive:
            // existing fingerprints stay valid, so no re-parse is forced.
            self.conn.execute_batch(
                "
                BEGIN;

                ALTER TABLE source_state ADD COLUMN cursor_json TEXT NOT NULL DEFAULT '';

                PRAGMA user_version = 6;

                COMMIT;
                ",
            )?;
        }
        if version < 7 {
            // v7 adds the Scrollback transcript store: full user/assistant
            // text per turn plus an external-content FTS5 index. Rows already
            // archived get their truncated prompts seeded as 'prompt'-origin
            // fallbacks so vanished sources stay searchable; clearing
            // source_state forces one full re-parse so surviving sources are
            // re-read with transcript capture and upgraded to 'full'.
            self.conn.execute_batch(
                "
                BEGIN;

                CREATE TABLE transcripts (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    tool TEXT NOT NULL,
                    dedup_key TEXT NOT NULL,
                    session_id TEXT NOT NULL,
                    project TEXT NOT NULL,
                    timestamp TEXT,
                    user_text TEXT NOT NULL DEFAULT '',
                    assistant_text TEXT NOT NULL DEFAULT '',
                    origin TEXT NOT NULL DEFAULT 'prompt',
                    UNIQUE(tool, dedup_key)
                );
                CREATE INDEX idx_transcripts_session ON transcripts(tool, session_id);
                CREATE INDEX idx_transcripts_project ON transcripts(project);

                CREATE VIRTUAL TABLE transcripts_fts USING fts5(
                    user_text, assistant_text,
                    content='transcripts', content_rowid='id',
                    tokenize='unicode61 remove_diacritics 2'
                );

                CREATE TRIGGER transcripts_ai AFTER INSERT ON transcripts BEGIN
                    INSERT INTO transcripts_fts(rowid, user_text, assistant_text)
                    VALUES (new.id, new.user_text, new.assistant_text);
                END;
                CREATE TRIGGER transcripts_ad AFTER DELETE ON transcripts BEGIN
                    INSERT INTO transcripts_fts(transcripts_fts, rowid, user_text, assistant_text)
                    VALUES ('delete', old.id, old.user_text, old.assistant_text);
                END;
                CREATE TRIGGER transcripts_au AFTER UPDATE OF user_text, assistant_text ON transcripts BEGIN
                    INSERT INTO transcripts_fts(transcripts_fts, rowid, user_text, assistant_text)
                    VALUES ('delete', old.id, old.user_text, old.assistant_text);
                    INSERT INTO transcripts_fts(rowid, user_text, assistant_text)
                    VALUES (new.id, new.user_text, new.assistant_text);
                END;

                -- Superseded Copilot estimate rows were zeroed in place (all
                -- token and cost columns null out; no parser emits all-zero
                -- rows) and their turns re-archived under usage keys; seeding
                -- them too would double every upgraded turn in the index.
                INSERT INTO transcripts (tool, dedup_key, session_id, project, timestamp, user_text, origin)
                SELECT tool, dedup_key, session_id, project, timestamp, user_message, 'prompt'
                FROM calls
                WHERE user_message != ''
                  AND NOT (
                    tool = 'copilot' AND cost_usd = 0
                    AND input_tokens = 0 AND output_tokens = 0
                    AND cache_read_input_tokens = 0 AND cache_creation_input_tokens = 0
                  );

                CREATE INDEX idx_calls_session ON calls(tool, session_id);

                DELETE FROM source_state;

                PRAGMA user_version = 7;

                COMMIT;
                ",
            )?;
        }
        if version < 8 {
            // v8 repairs Opus 5 calls archived before the model was priced.
            // Costs are otherwise frozen at import time on purpose, so this
            // is a deliberate one-shot correction rather than a reprice pass.
            let tx = self.conn.unchecked_transaction()?;
            reprice_fallback_priced_opus_5_calls(&tx)?;
            tx.execute_batch("PRAGMA user_version = 8;")?;
            tx.commit()?;
        }
        Ok(())
    }

    fn source_fingerprint(&self, tool: &str, path: &str) -> Result<Option<String>> {
        self.conn
            .query_row(
                "SELECT fingerprint FROM source_state WHERE tool = ?1 AND path = ?2",
                params![tool, path],
                |row| row.get(0),
            )
            .optional()
            .map_err(Into::into)
    }

    /// The adapter-owned incremental-parse cursor stored with this source's
    /// fingerprint; empty strings read as absent.
    fn source_cursor(&self, tool: &str, path: &str) -> Result<Option<String>> {
        let cursor: Option<String> = self
            .conn
            .query_row(
                "SELECT cursor_json FROM source_state WHERE tool = ?1 AND path = ?2",
                params![tool, path],
                |row| row.get(0),
            )
            .optional()?;
        Ok(cursor.filter(|cursor| !cursor.is_empty()))
    }

    fn load_calls(&self) -> Result<Vec<ParsedCall>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT
                tool, model, input_tokens, output_tokens,
                cache_creation_input_tokens, cache_read_input_tokens,
                cached_input_tokens, reasoning_tokens, web_search_requests,
                cost_usd, tools_json, bash_commands_json, timestamp,
                speed, dedup_key, user_message, session_id, project,
                is_canceled, prompt_chars, response_chars, elapsed_ms,
                code_blocks_json, edited_files_json, referenced_files_json,
                interaction_mode, token_quality, timestamp_quality
            FROM calls
            ORDER BY id ASC
            ",
        )?;
        let rows = stmt.query_map([], |row| {
            let tool: String = row.get(0)?;
            let tools_json: String = row.get(10)?;
            let bash_json: String = row.get(11)?;
            let timestamp: Option<String> = row.get(12)?;
            let speed: String = row.get(13)?;
            let code_blocks_json: String = row.get(22)?;
            let edited_files_json: String = row.get(23)?;
            let referenced_files_json: String = row.get(24)?;
            Ok(ParsedCall {
                tool: static_tool(tool),
                model: row.get(1)?,
                input_tokens: i64_to_u64(row.get(2)?),
                output_tokens: i64_to_u64(row.get(3)?),
                cache_creation_input_tokens: i64_to_u64(row.get(4)?),
                // Import-time pricing input only; the archived row keeps the
                // cost it produced.
                cache_creation_1h_input_tokens: 0,
                cache_read_input_tokens: i64_to_u64(row.get(5)?),
                cached_input_tokens: i64_to_u64(row.get(6)?),
                reasoning_tokens: i64_to_u64(row.get(7)?),
                web_search_requests: i64_to_u64(row.get(8)?),
                cost_usd: row.get(9)?,
                tools: serde_json::from_str(&tools_json).unwrap_or_default(),
                bash_commands: serde_json::from_str(&bash_json).unwrap_or_default(),
                timestamp: parse_datetime(timestamp),
                speed: speed_from_db(&speed),
                dedup_key: row.get(14)?,
                user_message: row.get(15)?,
                session_id: row.get(16)?,
                project: row.get(17)?,
                is_canceled: row.get::<_, i64>(18)? != 0,
                prompt_chars: opt_i64_to_u64(row.get(19)?),
                response_chars: opt_i64_to_u64(row.get(20)?),
                elapsed_ms: opt_i64_to_u64(row.get(21)?),
                code_blocks: serde_json::from_str(&code_blocks_json).unwrap_or_default(),
                edited_files: serde_json::from_str(&edited_files_json).unwrap_or_default(),
                referenced_files: serde_json::from_str(&referenced_files_json).unwrap_or_default(),
                interaction_mode: InteractionMode::parse(&row.get::<_, String>(25)?),
                token_quality: TokenQuality::parse(&row.get::<_, String>(26)?),
                timestamp_quality: TimestampQuality::parse(&row.get::<_, String>(27)?),
                superseded_dedup_keys: Vec::new(),
                merge_activity: false,
                // Transcript text lives only in the transcripts table; the
                // resident dataset stays text-free.
                transcript_user: None,
                transcript_assistant: None,
            })
        })?;

        let mut calls = Vec::new();
        for row in rows {
            calls.push(row?);
        }
        Ok(calls)
    }

    fn load_limits(&self) -> Result<Vec<LimitSnapshot>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT
                tool, limit_id, limit_name, plan_type, observed_at,
                primary_json, secondary_json, credits_json, rate_limit_reached_type
            FROM limit_snapshots
            ORDER BY id ASC
            ",
        )?;
        let rows = stmt.query_map([], |row| {
            let tool: String = row.get(0)?;
            let observed_at: Option<String> = row.get(4)?;
            let primary_json: Option<String> = row.get(5)?;
            let secondary_json: Option<String> = row.get(6)?;
            let credits_json: Option<String> = row.get(7)?;
            Ok(LimitSnapshot {
                tool: static_tool(tool),
                limit_id: row.get(1)?,
                limit_name: row.get(2)?,
                plan_type: row.get(3)?,
                observed_at: parse_datetime(observed_at),
                primary: json_opt(primary_json),
                secondary: json_opt(secondary_json),
                credits: json_opt(credits_json),
                rate_limit_reached_type: row.get(8)?,
            })
        })?;

        let mut limits = Vec::new();
        for row in rows {
            limits.push(row?);
        }
        Ok(limits)
    }
}

fn insert_call(tx: &Transaction<'_>, call: &ParsedCall) -> Result<bool> {
    let tools_json = serde_json::to_string(&call.tools)?;
    let bash_json = serde_json::to_string(&call.bash_commands)?;
    let code_blocks_json = serde_json::to_string(&call.code_blocks)?;
    let edited_files_json = serde_json::to_string(&call.edited_files)?;
    let referenced_files_json = serde_json::to_string(&call.referenced_files)?;
    let inserted = tx.execute(
        "
        INSERT OR IGNORE INTO calls (
            tool, dedup_key, model, input_tokens, output_tokens,
            cache_creation_input_tokens, cache_read_input_tokens,
            cached_input_tokens, reasoning_tokens, web_search_requests,
            cost_usd, tools_json, bash_commands_json, timestamp, speed,
            user_message, session_id, project, imported_at,
            is_canceled, prompt_chars, response_chars, elapsed_ms,
            code_blocks_json, edited_files_json, referenced_files_json,
            interaction_mode, token_quality, timestamp_quality
        ) VALUES (
            ?1, ?2, ?3, ?4, ?5,
            ?6, ?7, ?8, ?9, ?10,
            ?11, ?12, ?13, ?14, ?15,
            ?16, ?17, ?18, ?19,
            ?20, ?21, ?22, ?23,
            ?24, ?25, ?26,
            ?27, ?28, ?29
        )
        ",
        params![
            call.tool,
            call.dedup_key,
            call.model,
            u64_to_i64(call.input_tokens),
            u64_to_i64(call.output_tokens),
            u64_to_i64(call.cache_creation_input_tokens),
            u64_to_i64(call.cache_read_input_tokens),
            u64_to_i64(call.cached_input_tokens),
            u64_to_i64(call.reasoning_tokens),
            u64_to_i64(call.web_search_requests),
            call.cost_usd,
            tools_json,
            bash_json,
            datetime_to_db(call.timestamp),
            speed_to_db(call.speed),
            call.user_message,
            call.session_id,
            call.project,
            Utc::now().to_rfc3339(),
            call.is_canceled as i64,
            call.prompt_chars.map(u64_to_i64),
            call.response_chars.map(u64_to_i64),
            call.elapsed_ms.map(u64_to_i64),
            code_blocks_json,
            edited_files_json,
            referenced_files_json,
            call.interaction_mode.as_str(),
            call.token_quality.as_str(),
            effective_timestamp_quality(call).as_str(),
        ],
    )?;
    if inserted == 0 {
        update_existing_cursor_project(tx, call)?;
        update_existing_cursor_tokens(tx, call)?;
        update_existing_copilot_cli_totals(tx, call)?;
        update_existing_codex_tool_activity(tx, call, &tools_json, &bash_json)?;
        update_existing_call_enrichment(
            tx,
            call,
            &code_blocks_json,
            &edited_files_json,
            &referenced_files_json,
        )?;
        if call.merge_activity {
            merge_existing_claude_tail_activity(tx, call)?;
        } else {
            update_existing_claude_tool_activity(
                tx,
                call,
                &tools_json,
                &bash_json,
                &code_blocks_json,
                &edited_files_json,
                &referenced_files_json,
            )?;
        }
    } else {
        zero_superseded_copilot_estimates(tx, call)?;
    }
    upsert_transcript(tx, call)?;
    remove_superseded_cursor_rows(tx, call)?;
    remove_superseded_codex_rows(tx, call, inserted > 0)?;
    zero_superseded_copilot_turn_estimates(tx, call)?;
    Ok(inserted > 0)
}

/// Strip the private-use sentinel characters the search snippets use as
/// highlight markers; corpus text containing them could otherwise forge
/// highlight spans in rendered results.
fn clean_transcript_text(text: &str) -> std::borrow::Cow<'_, str> {
    if text.contains(['\u{E000}', '\u{E001}']) {
        std::borrow::Cow::Owned(text.replace(['\u{E000}', '\u{E001}'], ""))
    } else {
        std::borrow::Cow::Borrowed(text)
    }
}

/// Write a call's full transcript text into the Scrollback store. Calls that
/// carry no text (metadata-only reparses, limit rows, legacy-cache imports)
/// never touch existing rows. Two conflict shapes:
/// - Claude tail continuations append their assistant blocks (prefix and tail
///   blocks are disjoint by construction, mirroring the response_chars sum in
///   merge_existing_claude_tail_activity). The suffix guard makes the append
///   idempotent: a replayed tail (e.g. two refreshers racing on the same
///   grown file) is detected as already applied and skipped.
/// - Everything else is grow-only per column: a fresh parse of an append-only
///   source is a superset of what was archived, so longer text wins and a
///   weaker parse can never clobber previously captured text. This also
///   upgrades migration-seeded 'prompt' fallback rows to 'full'.
///
/// Both branches refresh `project`: parsers refine it across syncs (cursor
/// workspace resolution, claude cwd lines) and the calls row tracks it via
/// update_existing_cursor_project, so the transcript row must follow.
fn upsert_transcript(tx: &Transaction<'_>, call: &ParsedCall) -> Result<()> {
    let user_text = clean_transcript_text(call.transcript_user.as_deref().unwrap_or(""));
    let assistant_text = clean_transcript_text(call.transcript_assistant.as_deref().unwrap_or(""));
    if user_text.is_empty() && assistant_text.is_empty() {
        return Ok(());
    }

    let is_claude_tail_merge =
        call.tool == crate::tools::claude_code::config::TOOL_ID && call.merge_activity;
    if is_claude_tail_merge {
        tx.execute(
            "
            INSERT INTO transcripts (
                tool, dedup_key, session_id, project, timestamp,
                user_text, assistant_text, origin
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'full')
            ON CONFLICT(tool, dedup_key) DO UPDATE SET
                assistant_text = CASE
                    WHEN excluded.assistant_text = '' THEN assistant_text
                    WHEN assistant_text = '' THEN excluded.assistant_text
                    WHEN substr(assistant_text, -length(excluded.assistant_text))
                        = excluded.assistant_text THEN assistant_text
                    ELSE assistant_text || char(10) || excluded.assistant_text
                END,
                user_text = CASE
                    WHEN excluded.user_text != '' AND (user_text = '' OR origin = 'prompt')
                        THEN excluded.user_text
                    ELSE user_text
                END,
                project = CASE
                    WHEN excluded.project != '' THEN excluded.project
                    ELSE project
                END,
                origin = 'full'
            ",
            params![
                call.tool,
                call.dedup_key,
                call.session_id,
                call.project,
                datetime_to_db(call.timestamp),
                user_text,
                assistant_text,
            ],
        )?;
    } else {
        tx.execute(
            "
            INSERT INTO transcripts (
                tool, dedup_key, session_id, project, timestamp,
                user_text, assistant_text, origin
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'full')
            ON CONFLICT(tool, dedup_key) DO UPDATE SET
                user_text = CASE
                    WHEN length(excluded.user_text) > length(user_text)
                        THEN excluded.user_text
                    ELSE user_text
                END,
                assistant_text = CASE
                    WHEN length(excluded.assistant_text) > length(assistant_text)
                        THEN excluded.assistant_text
                    ELSE assistant_text
                END,
                project = CASE
                    WHEN excluded.project != '' THEN excluded.project
                    ELSE project
                END,
                origin = 'full'
            ",
            params![
                call.tool,
                call.dedup_key,
                call.session_id,
                call.project,
                datetime_to_db(call.timestamp),
                user_text,
                assistant_text,
            ],
        )?;
    }
    Ok(())
}

/// A Copilot OTel span row carries the transcript-style key of the same turn
/// as a superseded hint. Zero that row's token and cost fields (keeping its
/// message metadata) so archives that ingested the estimate before the OTel
/// store was read stop double counting.
fn zero_superseded_copilot_turn_estimates(tx: &Transaction<'_>, call: &ParsedCall) -> Result<()> {
    if call.tool != crate::tools::copilot::config::TOOL_ID
        || call.superseded_dedup_keys.is_empty()
        || !call
            .dedup_key
            .starts_with(crate::tools::copilot::config::OTEL_DEDUP_PREFIX)
    {
        return Ok(());
    }
    for dedup_key in &call.superseded_dedup_keys {
        if dedup_key == &call.dedup_key {
            continue;
        }
        tx.execute(
            "
            UPDATE calls
            SET input_tokens = 0, output_tokens = 0,
                cache_creation_input_tokens = 0, cache_read_input_tokens = 0,
                cached_input_tokens = 0, reasoning_tokens = 0, cost_usd = 0
            WHERE tool = ?1
              AND dedup_key = ?2
            ",
            params![call.tool, dedup_key],
        )?;
        remove_transcript_if_superseding_carries_text(tx, call, dedup_key)?;
    }
    Ok(())
}

/// A superseded row's transcript is deleted only when the superseding call
/// carries its own transcript text — otherwise (e.g. Copilot OTel spans,
/// which have usage but no message text) the estimate row holds the turn's
/// only copy and deleting it would erase the transcript entirely.
fn remove_transcript_if_superseding_carries_text(
    tx: &Transaction<'_>,
    call: &ParsedCall,
    superseded_key: &str,
) -> Result<()> {
    let has_text = call
        .transcript_user
        .as_deref()
        .is_some_and(|text| !text.is_empty())
        || call
            .transcript_assistant
            .as_deref()
            .is_some_and(|text| !text.is_empty());
    if !has_text {
        return Ok(());
    }
    tx.execute(
        "DELETE FROM transcripts WHERE tool = ?1 AND dedup_key = ?2",
        params![call.tool, superseded_key],
    )?;
    Ok(())
}

/// Restore the true cost of Opus 5 calls that were archived while the model
/// was still unpriced. Returns how many rows were corrected.
///
/// Every row is checked against the fallback rates before being touched: a
/// row that already carries Opus 5 pricing is left alone, so a user who
/// downloaded corrected pricing books before upgrading the binary cannot be
/// double-charged. The check separates the two cleanly. A fallback-priced row
/// costs at most `fallback_tokens + 0.6 * cache_write_rate * cache_creation`,
/// and that premium is always smaller than the `2/3 * fallback_tokens` of
/// headroom below the corrected floor, because `fallback_tokens` already
/// contains a full `cache_write_rate * cache_creation` term.
fn reprice_fallback_priced_opus_5_calls(tx: &Transaction<'_>) -> Result<usize> {
    use opus_5_repair as rates;

    let mut candidates = Vec::new();
    {
        let mut stmt = tx.prepare(
            "
            SELECT id, model, speed, cost_usd, input_tokens, output_tokens,
                   cache_creation_input_tokens, cache_read_input_tokens,
                   web_search_requests
            FROM calls
            WHERE model LIKE '%claude-opus-5%'
            ",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, f64>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, i64>(6)?,
                row.get::<_, i64>(7)?,
                row.get::<_, i64>(8)?,
            ))
        })?;
        for row in rows {
            candidates.push(row?);
        }
    }

    let mut repriced = 0usize;
    for (id, model, speed, cost_usd, input, output, cache_write, cache_read, web) in candidates {
        // The SQL prefilter is deliberately loose; the shared canonicalization
        // decides, so dated and vendor-prefixed ids fold in but neighbours
        // such as a Cursor `claude-opus-5-fast` row do not.
        if crate::models::canonical_key(&model) != "claude-opus-5" {
            continue;
        }

        let speed_multiplier = if speed == "fast" {
            rates::FAST_MULTIPLIER
        } else {
            1.0
        };
        let web_cost = web as f64 * rates::WEB_SEARCH;
        let fallback_tokens = input as f64 * rates::FALLBACK_INPUT
            + output as f64 * rates::FALLBACK_OUTPUT
            + cache_write as f64 * rates::FALLBACK_CACHE_WRITE
            + cache_read as f64 * rates::FALLBACK_CACHE_READ;

        let corrected_floor = speed_multiplier * (fallback_tokens * rates::FACTOR + web_cost);
        if cost_usd >= corrected_floor {
            continue;
        }

        // The stored cost was charged at 1x: the fallback row has no fast
        // multiplier, so the speed multiplier is applied here, not unwound.
        let corrected = speed_multiplier * ((cost_usd - web_cost) * rates::FACTOR + web_cost);
        tx.execute(
            "UPDATE calls SET cost_usd = ?1 WHERE id = ?2",
            params![corrected, id],
        )?;
        repriced += 1;
    }

    Ok(repriced)
}

/// Codex v6 re-keyed calls onto lineage-addressed dedup keys. Each reparsed
/// call carries the legacy path-based key(s) it replaces: its own pre-v6 row
/// plus any replayed-history rows a forked rollout had double-counted. When
/// the replaced row is the same event (token buckets match), the fresh row
/// inherits its import-time cost so history is never silently repriced; every
/// legacy row is then deleted.
fn remove_superseded_codex_rows(
    tx: &Transaction<'_>,
    call: &ParsedCall,
    inherit_cost: bool,
) -> Result<()> {
    if call.tool != crate::tools::codex::config::TOOL_ID || call.superseded_dedup_keys.is_empty() {
        return Ok(());
    }

    for dedup_key in &call.superseded_dedup_keys {
        if dedup_key == &call.dedup_key {
            continue;
        }
        if inherit_cost {
            let old: Option<(f64, i64, i64, i64, i64)> = tx
                .query_row(
                    "
                    SELECT cost_usd, input_tokens, output_tokens,
                           cache_read_input_tokens, reasoning_tokens
                    FROM calls WHERE tool = ?1 AND dedup_key = ?2
                    ",
                    params![call.tool, dedup_key],
                    |row| {
                        Ok((
                            row.get(0)?,
                            row.get(1)?,
                            row.get(2)?,
                            row.get(3)?,
                            row.get(4)?,
                        ))
                    },
                )
                .optional()?;
            if let Some((cost_usd, input, output, cache_read, reasoning)) = old {
                if input == u64_to_i64(call.input_tokens)
                    && output == u64_to_i64(call.output_tokens)
                    && cache_read == u64_to_i64(call.cache_read_input_tokens)
                    && reasoning == u64_to_i64(call.reasoning_tokens)
                {
                    tx.execute(
                        "UPDATE calls SET cost_usd = ?1 WHERE tool = ?2 AND dedup_key = ?3",
                        params![cost_usd, call.tool, call.dedup_key],
                    )?;
                }
            }
        }
        tx.execute(
            "DELETE FROM calls WHERE tool = ?1 AND dedup_key = ?2",
            params![call.tool, dedup_key],
        )?;
        tx.execute(
            "DELETE FROM transcripts WHERE tool = ?1 AND dedup_key = ?2",
            params![call.tool, dedup_key],
        )?;
    }
    Ok(())
}

fn effective_timestamp_quality(call: &ParsedCall) -> TimestampQuality {
    if call.timestamp_quality == TimestampQuality::Unknown && call.timestamp.is_some() {
        TimestampQuality::Exact
    } else {
        call.timestamp_quality
    }
}

fn remove_superseded_cursor_rows(tx: &Transaction<'_>, call: &ParsedCall) -> Result<()> {
    if call.tool != crate::tools::cursor::config::TOOL_ID || call.superseded_dedup_keys.is_empty() {
        return Ok(());
    }

    for dedup_key in &call.superseded_dedup_keys {
        if dedup_key == &call.dedup_key {
            continue;
        }
        tx.execute(
            "DELETE FROM calls WHERE tool = ?1 AND dedup_key = ?2",
            params![call.tool, dedup_key],
        )?;
        tx.execute(
            "DELETE FROM transcripts WHERE tool = ?1 AND dedup_key = ?2",
            params![call.tool, dedup_key],
        )?;
    }
    Ok(())
}

/// Backfill the v4 enrichment columns onto rows archived before the columns
/// existed (or before their parser learned to populate them). Fills exactly
/// once: rows that already carry any enrichment are left alone, so a later
/// parse can never clobber previously archived enrichment with weaker data.
fn update_existing_call_enrichment(
    tx: &Transaction<'_>,
    call: &ParsedCall,
    code_blocks_json: &str,
    edited_files_json: &str,
    referenced_files_json: &str,
) -> Result<()> {
    tx.execute(
        "
        UPDATE calls
        SET interaction_mode = CASE WHEN interaction_mode = 'unknown' THEN ?1 ELSE interaction_mode END,
            token_quality = CASE WHEN token_quality = 'unknown' THEN ?2 ELSE token_quality END,
            timestamp_quality = CASE WHEN timestamp_quality = 'unknown' THEN ?3 ELSE timestamp_quality END
        WHERE tool = ?4 AND dedup_key = ?5
        ",
        params![
            call.interaction_mode.as_str(),
            call.token_quality.as_str(),
            effective_timestamp_quality(call).as_str(),
            call.tool,
            call.dedup_key,
        ],
    )?;

    let has_enrichment = call.is_canceled
        || call.prompt_chars.is_some()
        || call.response_chars.is_some()
        || call.elapsed_ms.is_some()
        || !call.code_blocks.is_empty()
        || !call.edited_files.is_empty()
        || !call.referenced_files.is_empty();
    if !has_enrichment {
        return Ok(());
    }

    tx.execute(
        "
        UPDATE calls
        SET is_canceled = ?1, prompt_chars = ?2, response_chars = ?3,
            elapsed_ms = ?4, code_blocks_json = ?5, edited_files_json = ?6,
            referenced_files_json = ?7
        WHERE tool = ?8
          AND dedup_key = ?9
          AND is_canceled = 0
          AND prompt_chars IS NULL
          AND response_chars IS NULL
          AND elapsed_ms IS NULL
          AND code_blocks_json = '[]'
          AND edited_files_json = '[]'
          AND referenced_files_json = '[]'
        ",
        params![
            call.is_canceled as i64,
            call.prompt_chars.map(u64_to_i64),
            call.response_chars.map(u64_to_i64),
            call.elapsed_ms.map(u64_to_i64),
            code_blocks_json,
            edited_files_json,
            referenced_files_json,
            call.tool,
            call.dedup_key,
        ],
    )?;
    Ok(())
}

/// Newly archived per-request Copilot usage rows supersede two kinds of
/// previously archived estimates for the same session: the chars/4 turn row
/// they cover (whose dedup key is encoded in the usage key) and the data.db
/// aggregate row. Zero the superseded rows' token and cost fields so upgraded
/// archives don't double count; their message metadata stays.
fn zero_superseded_copilot_estimates(tx: &Transaction<'_>, call: &ParsedCall) -> Result<()> {
    if call.tool != crate::tools::copilot::config::TOOL_ID {
        return Ok(());
    }
    let Some((turn_part, _)) = call
        .dedup_key
        .split_once(crate::tools::copilot::config::CLI_USAGE_DEDUP_MARKER)
    else {
        return Ok(());
    };

    let mut superseded = vec![format!(
        "{}{}",
        crate::tools::copilot::config::CLI_APP_DEDUP_PREFIX,
        call.session_id
    )];
    if turn_part.contains(crate::tools::copilot::config::CLI_TURN_DEDUP_MARKER) {
        superseded.push(turn_part.to_string());
    }
    for dedup_key in superseded {
        tx.execute(
            "
            UPDATE calls
            SET input_tokens = 0, output_tokens = 0,
                cache_creation_input_tokens = 0, cache_read_input_tokens = 0,
                cached_input_tokens = 0, reasoning_tokens = 0, cost_usd = 0
            WHERE tool = ?1
              AND dedup_key = ?2
            ",
            params![call.tool, dedup_key],
        )?;
        remove_transcript_if_superseding_carries_text(tx, call, &dedup_key)?;
    }
    Ok(())
}

/// Claude Code rows archived before the streamed-content-block merge carry
/// only the first block line's activity. A reparse (forced by the adapter's
/// fingerprint bump) emits the same dedup key with the full merged activity,
/// so replace the activity columns outright and never shrink the response
/// length. Token and cost columns are untouched: usage is identical across a
/// message's streamed lines.
/// Merge path for tail-resumed Claude parses: the conflicting call is the
/// continuation of a message whose earlier streamed lines were parsed in a
/// previous sync, so it carries only the tail's content blocks. Unlike the
/// full-reparse overwrite (where the new parse is a superset), the arrays
/// concatenate — each streamed line contributes distinct blocks — file
/// lists re-dedup, response chars sum (prefix and tail counts are
/// disjoint), and a tail-observed interruption sticks.
fn merge_existing_claude_tail_activity(tx: &Transaction<'_>, call: &ParsedCall) -> Result<()> {
    if call.tool != crate::tools::claude_code::config::TOOL_ID {
        return Ok(());
    }

    let existing = tx
        .query_row(
            "
            SELECT tools_json, bash_commands_json, code_blocks_json,
                   edited_files_json, referenced_files_json, response_chars
            FROM calls
            WHERE tool = ?1 AND dedup_key = ?2
            ",
            params![call.tool, call.dedup_key],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, Option<i64>>(5)?,
                ))
            },
        )
        .optional()?;
    let Some((tools_json, bash_json, code_json, edited_json, referenced_json, response_chars)) =
        existing
    else {
        return Ok(());
    };

    let mut tools: Vec<String> = serde_json::from_str(&tools_json).unwrap_or_default();
    tools.extend(call.tools.iter().cloned());
    let mut bash: Vec<String> = serde_json::from_str(&bash_json).unwrap_or_default();
    bash.extend(call.bash_commands.iter().cloned());
    let mut code_blocks: Vec<crate::tools::CodeBlock> =
        serde_json::from_str(&code_json).unwrap_or_default();
    code_blocks.extend(call.code_blocks.iter().cloned());
    let code_blocks = crate::tools::jsonl::merge_code_blocks(code_blocks);
    let mut edited: Vec<String> = serde_json::from_str(&edited_json).unwrap_or_default();
    edited.extend(call.edited_files.iter().cloned());
    let edited = crate::tools::jsonl::dedup_files(edited);
    let mut referenced: Vec<String> = serde_json::from_str(&referenced_json).unwrap_or_default();
    referenced.extend(call.referenced_files.iter().cloned());
    let referenced = crate::tools::jsonl::dedup_files(referenced);
    let merged_response_chars = match (response_chars, call.response_chars) {
        (Some(existing), Some(new)) => Some(existing.saturating_add(u64_to_i64(new))),
        (Some(existing), None) => Some(existing),
        (None, Some(new)) => Some(u64_to_i64(new)),
        (None, None) => None,
    };

    tx.execute(
        "
        UPDATE calls
        SET tools_json = ?1, bash_commands_json = ?2, code_blocks_json = ?3,
            edited_files_json = ?4, referenced_files_json = ?5,
            response_chars = ?6,
            is_canceled = MAX(is_canceled, ?7)
        WHERE tool = ?8
          AND dedup_key = ?9
        ",
        params![
            serde_json::to_string(&tools)?,
            serde_json::to_string(&bash)?,
            serde_json::to_string(&code_blocks)?,
            serde_json::to_string(&edited)?,
            serde_json::to_string(&referenced)?,
            merged_response_chars,
            call.is_canceled as i64,
            call.tool,
            call.dedup_key,
        ],
    )?;
    Ok(())
}

fn update_existing_claude_tool_activity(
    tx: &Transaction<'_>,
    call: &ParsedCall,
    tools_json: &str,
    bash_json: &str,
    code_blocks_json: &str,
    edited_files_json: &str,
    referenced_files_json: &str,
) -> Result<()> {
    if call.tool != crate::tools::claude_code::config::TOOL_ID {
        return Ok(());
    }

    tx.execute(
        "
        UPDATE calls
        SET tools_json = ?1, bash_commands_json = ?2, code_blocks_json = ?3,
            edited_files_json = ?4, referenced_files_json = ?5,
            response_chars = COALESCE(MAX(response_chars, ?6), ?6)
        WHERE tool = ?7
          AND dedup_key = ?8
        ",
        params![
            tools_json,
            bash_json,
            code_blocks_json,
            edited_files_json,
            referenced_files_json,
            call.response_chars.map(u64_to_i64),
            call.tool,
            call.dedup_key,
        ],
    )?;
    Ok(())
}

fn update_existing_codex_tool_activity(
    tx: &Transaction<'_>,
    call: &ParsedCall,
    tools_json: &str,
    bash_json: &str,
) -> Result<()> {
    if call.tool != crate::tools::codex::config::TOOL_ID {
        return Ok(());
    }

    // user_message is a v1 column set on INSERT, so rows archived before the
    // parser learned to read Codex `user_message` events carry "" forever
    // unless filled here. Fill only when empty - never rewrite a stored
    // prompt.
    tx.execute(
        "
        UPDATE calls
        SET tools_json = ?1, bash_commands_json = ?2,
            user_message = CASE WHEN user_message = '' THEN ?3 ELSE user_message END
        WHERE tool = ?4
          AND dedup_key = ?5
        ",
        params![
            tools_json,
            bash_json,
            call.user_message,
            call.tool,
            call.dedup_key
        ],
    )?;
    Ok(())
}

/// Copilot's data.db rows are running totals for potentially still-live
/// sessions. Their dedup keys are stable per session, so refresh the archived
/// row whenever a later sync observes updated totals.
fn update_existing_copilot_cli_totals(tx: &Transaction<'_>, call: &ParsedCall) -> Result<()> {
    if call.tool != crate::tools::copilot::config::TOOL_ID
        || !call
            .dedup_key
            .starts_with(crate::tools::copilot::config::CLI_APP_DEDUP_PREFIX)
    {
        return Ok(());
    }

    tx.execute(
        "
        UPDATE calls
        SET model = ?1, input_tokens = ?2, output_tokens = ?3,
            cache_read_input_tokens = ?4, cached_input_tokens = ?5,
            reasoning_tokens = ?6, cost_usd = ?7, timestamp = ?8
        WHERE tool = ?9
          AND dedup_key = ?10
        ",
        params![
            call.model,
            u64_to_i64(call.input_tokens),
            u64_to_i64(call.output_tokens),
            u64_to_i64(call.cache_read_input_tokens),
            u64_to_i64(call.cached_input_tokens),
            u64_to_i64(call.reasoning_tokens),
            call.cost_usd,
            datetime_to_db(call.timestamp),
            call.tool,
            call.dedup_key,
        ],
    )?;
    Ok(())
}

/// Cursor reconstruction is authoritative for its canonical rows: a reparse
/// may move a conversation's input onto the composer-level meter credit, or
/// real token counts may appear on bubbles that previously carried zeros.
/// Refresh the token columns when they changed; cost and quality follow the
/// tokens they were computed from. Rows whose source no longer reparses are
/// untouched.
fn update_existing_cursor_tokens(tx: &Transaction<'_>, call: &ParsedCall) -> Result<()> {
    if call.tool != crate::tools::cursor::config::TOOL_ID {
        return Ok(());
    }

    tx.execute(
        "
        UPDATE calls
        SET input_tokens = ?1, output_tokens = ?2, cost_usd = ?3,
            token_quality = ?4
        WHERE tool = ?5
          AND dedup_key = ?6
          AND (input_tokens != ?1 OR output_tokens != ?2)
        ",
        params![
            u64_to_i64(call.input_tokens),
            u64_to_i64(call.output_tokens),
            call.cost_usd,
            call.token_quality.as_str(),
            call.tool,
            call.dedup_key,
        ],
    )?;
    Ok(())
}

fn update_existing_cursor_project(tx: &Transaction<'_>, call: &ParsedCall) -> Result<()> {
    if call.tool != crate::tools::cursor::config::TOOL_ID || call.project == "cursor-workspace" {
        return Ok(());
    }

    tx.execute(
        "
        UPDATE calls
        SET project = ?1
        WHERE tool = ?2
          AND dedup_key = ?3
          AND project != ?1
        ",
        params![call.project, call.tool, call.dedup_key],
    )?;
    Ok(())
}

fn insert_limit(tx: &Transaction<'_>, limit: &LimitSnapshot) -> Result<bool> {
    let primary_json = json_db(&limit.primary)?;
    let secondary_json = json_db(&limit.secondary)?;
    let credits_json = json_db(&limit.credits)?;
    let snapshot_key = limit_snapshot_key(
        limit,
        primary_json.as_deref(),
        secondary_json.as_deref(),
        credits_json.as_deref(),
    );
    let inserted = tx.execute(
        "
        INSERT OR IGNORE INTO limit_snapshots (
            tool, limit_id, limit_name, plan_type, observed_at,
            primary_json, secondary_json, credits_json,
            rate_limit_reached_type, imported_at, snapshot_key
        ) VALUES (
            ?1, ?2, ?3, ?4, ?5,
            ?6, ?7, ?8,
            ?9, ?10, ?11
        )
        ",
        params![
            limit.tool,
            limit.limit_id,
            limit.limit_name,
            limit.plan_type,
            datetime_to_db(limit.observed_at),
            primary_json,
            secondary_json,
            credits_json,
            limit.rate_limit_reached_type,
            Utc::now().to_rfc3339(),
            snapshot_key,
        ],
    )?;
    Ok(inserted > 0)
}

fn upsert_source_fingerprint(
    tx: &Transaction<'_>,
    tool: &str,
    path: &str,
    fingerprint: &str,
    cursor_json: &str,
) -> Result<()> {
    tx.execute(
        "
        INSERT INTO source_state (tool, path, fingerprint, synced_at, cursor_json)
        VALUES (?1, ?2, ?3, ?4, ?5)
        ON CONFLICT(tool, path) DO UPDATE SET
            fingerprint = excluded.fingerprint,
            synced_at = excluded.synced_at,
            cursor_json = excluded.cursor_json
        ",
        params![
            tool,
            path,
            fingerprint,
            Utc::now().to_rfc3339(),
            cursor_json
        ],
    )?;
    Ok(())
}

fn limit_snapshot_key(
    limit: &LimitSnapshot,
    primary_json: Option<&str>,
    secondary_json: Option<&str>,
    credits_json: Option<&str>,
) -> String {
    format!(
        "{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}",
        limit.tool,
        limit.limit_id,
        limit.limit_name.as_deref().unwrap_or(""),
        limit.plan_type.as_deref().unwrap_or(""),
        datetime_to_db(limit.observed_at).unwrap_or_default(),
        primary_json.unwrap_or(""),
        secondary_json.unwrap_or(""),
        credits_json.unwrap_or(""),
        limit.rate_limit_reached_type.as_deref().unwrap_or("")
    )
}

fn json_db<T: serde::Serialize>(value: &Option<T>) -> Result<Option<String>> {
    value
        .as_ref()
        .map(serde_json::to_string)
        .transpose()
        .map_err(Into::into)
}

fn json_opt<T: serde::de::DeserializeOwned>(raw: Option<String>) -> Option<T> {
    raw.and_then(|s| serde_json::from_str(&s).ok())
}

fn datetime_to_db(dt: Option<DateTime<Utc>>) -> Option<String> {
    dt.map(|dt| dt.to_rfc3339())
}

fn parse_datetime(raw: Option<String>) -> Option<DateTime<Utc>> {
    raw.and_then(|s| {
        DateTime::parse_from_rfc3339(&s)
            .ok()
            .map(|dt| dt.with_timezone(&Utc))
    })
}

fn speed_to_db(speed: Speed) -> &'static str {
    match speed {
        Speed::Standard => "standard",
        Speed::Fast => "fast",
    }
}

fn speed_from_db(raw: &str) -> Speed {
    match raw {
        "fast" => Speed::Fast,
        _ => Speed::Standard,
    }
}

fn u64_to_i64(value: u64) -> i64 {
    value.min(i64::MAX as u64) as i64
}

fn i64_to_u64(value: i64) -> u64 {
    value.max(0) as u64
}

fn opt_i64_to_u64(value: Option<i64>) -> Option<u64> {
    value.map(i64_to_u64)
}

fn static_tool(tool: String) -> &'static str {
    match tool.as_str() {
        crate::tools::claude_code::config::TOOL_ID => crate::tools::claude_code::config::TOOL_ID,
        crate::tools::cursor::config::TOOL_ID => crate::tools::cursor::config::TOOL_ID,
        crate::tools::codex::config::TOOL_ID => crate::tools::codex::config::TOOL_ID,
        crate::tools::copilot::config::TOOL_ID => crate::tools::copilot::config::TOOL_ID,
        crate::tools::gemini::config::TOOL_ID => crate::tools::gemini::config::TOOL_ID,
        _ => Box::leak(tool.into_boxed_str()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    use chrono::TimeZone;

    use crate::tools::{LimitCredits, LimitWindow, SessionSource};

    fn temp_paths(name: &str) -> ConfigPaths {
        let unique = format!(
            "tokenuse-archive-test-{}-{}",
            name,
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        );
        ConfigPaths::new(std::env::temp_dir().join(unique))
    }

    fn sample_call(key: &str) -> ParsedCall {
        ParsedCall {
            tool: crate::tools::codex::config::TOOL_ID,
            model: "gpt-5".into(),
            input_tokens: 100,
            output_tokens: 50,
            cache_creation_input_tokens: 7,
            cache_creation_1h_input_tokens: 0,
            cache_read_input_tokens: 11,
            cached_input_tokens: 11,
            reasoning_tokens: 5,
            web_search_requests: 2,
            cost_usd: 0.1234,
            tools: vec!["exec_command".into(), "apply_patch".into()],
            bash_commands: vec!["cargo test".into()],
            timestamp: Some(Utc.with_ymd_and_hms(2026, 4, 29, 12, 0, 0).unwrap()),
            speed: Speed::Fast,
            dedup_key: key.into(),
            user_message: "build the thing".into(),
            session_id: "sess-1".into(),
            project: "/tmp/tokens".into(),
            is_canceled: true,
            prompt_chars: Some(2048),
            response_chars: Some(4096),
            elapsed_ms: Some(45_000),
            code_blocks: vec![crate::tools::CodeBlock {
                language: "rust".into(),
                loc: 42,
            }],
            edited_files: vec!["src/main.rs".into()],
            referenced_files: vec!["src/lib.rs".into()],
            interaction_mode: crate::tools::InteractionMode::Agent,
            token_quality: crate::tools::TokenQuality::Exact,
            timestamp_quality: crate::tools::TimestampQuality::Exact,
            superseded_dedup_keys: Vec::new(),
            merge_activity: false,
            transcript_user: None,
            transcript_assistant: None,
        }
    }

    fn bare_call(key: &str) -> ParsedCall {
        ParsedCall {
            is_canceled: false,
            prompt_chars: None,
            response_chars: None,
            elapsed_ms: None,
            code_blocks: Vec::new(),
            edited_files: Vec::new(),
            referenced_files: Vec::new(),
            ..sample_call(key)
        }
    }

    fn sample_limit() -> LimitSnapshot {
        LimitSnapshot {
            tool: crate::tools::codex::config::TOOL_ID,
            limit_id: "codex_test".into(),
            limit_name: Some("Codex Test".into()),
            plan_type: Some("pro".into()),
            observed_at: Some(Utc.with_ymd_and_hms(2026, 4, 29, 12, 0, 0).unwrap()),
            primary: Some(LimitWindow {
                used_percent: 33.0,
                window_minutes: 300,
                resets_at: Some(Utc.with_ymd_and_hms(2026, 4, 29, 17, 0, 0).unwrap()),
            }),
            secondary: None,
            credits: Some(LimitCredits {
                has_credits: true,
                unlimited: false,
                balance: Some(12.5),
                total: None,
                additional_usage: None,
            }),
            rate_limit_reached_type: Some("primary".into()),
        }
    }

    #[test]
    fn migration_is_idempotent() {
        let paths = temp_paths("migration");
        let archive = Archive::open(&paths).unwrap();
        assert!(archive.is_empty().unwrap());
        drop(archive);

        let archive = Archive::open(&paths).unwrap();
        assert!(archive.is_empty().unwrap());
        let _ = fs::remove_dir_all(paths.dir);
    }

    #[test]
    fn parsed_calls_and_limits_roundtrip() {
        let paths = temp_paths("roundtrip");
        let mut archive = Archive::open(&paths).unwrap();
        let ingested = Ingested {
            calls: vec![sample_call("k1")],
            limits: vec![sample_limit()],
        };

        assert_eq!(archive.insert_ingested(&ingested).unwrap(), 2);
        let loaded = archive.load().unwrap();

        assert_eq!(loaded.calls, ingested.calls);
        assert_eq!(loaded.limits, ingested.limits);
        let _ = fs::remove_dir_all(paths.dir);
    }

    #[test]
    fn copilot_usage_rows_zero_superseded_estimates() {
        let paths = temp_paths("copilot-usage-supersede");
        let mut archive = Archive::open(&paths).unwrap();

        let mut turn_estimate = sample_call("copilot:sess-1:turn-0");
        turn_estimate.tool = crate::tools::copilot::config::TOOL_ID;
        let mut cli_aggregate = sample_call("copilot:cli:sess-1");
        cli_aggregate.tool = crate::tools::copilot::config::TOOL_ID;
        let mut untouched_turn = sample_call("copilot:sess-1:turn-1");
        untouched_turn.tool = crate::tools::copilot::config::TOOL_ID;
        archive
            .insert_ingested(&Ingested {
                calls: vec![turn_estimate, cli_aggregate, untouched_turn],
                limits: Vec::new(),
            })
            .unwrap();

        let mut usage = sample_call("copilot:sess-1:turn-0:usage-7");
        usage.tool = crate::tools::copilot::config::TOOL_ID;
        usage.input_tokens = 411;
        usage.cache_read_input_tokens = 27_392;
        archive
            .insert_ingested(&Ingested {
                calls: vec![usage],
                limits: Vec::new(),
            })
            .unwrap();

        let loaded = archive.load().unwrap();
        let by_key = |key: &str| {
            loaded
                .calls
                .iter()
                .find(|call| call.dedup_key == key)
                .unwrap()
        };

        let zeroed_turn = by_key("copilot:sess-1:turn-0");
        assert_eq!(zeroed_turn.input_tokens, 0);
        assert_eq!(zeroed_turn.output_tokens, 0);
        assert_eq!(zeroed_turn.cost_usd, 0.0);
        assert_eq!(
            zeroed_turn.user_message, "build the thing",
            "metadata survives the zeroing"
        );

        let zeroed_aggregate = by_key("copilot:cli:sess-1");
        assert_eq!(zeroed_aggregate.input_tokens, 0);
        assert_eq!(zeroed_aggregate.cost_usd, 0.0);

        let untouched = by_key("copilot:sess-1:turn-1");
        assert_eq!(untouched.input_tokens, 100, "other turns keep their tokens");

        let usage_row = by_key("copilot:sess-1:turn-0:usage-7");
        assert_eq!(usage_row.input_tokens, 411);

        let _ = fs::remove_dir_all(paths.dir);
    }

    /// Build an archive file with the frozen pre-v4 table shapes (no
    /// enrichment columns), one legacy call, one source_state row, and
    /// whatever `extra_sql` the era needs (advice tables, version pragma).
    fn create_legacy_db(paths: &ConfigPaths, extra_sql: &str) {
        paths.ensure_dir().unwrap();
        let conn = Connection::open(&paths.archive_db_file).unwrap();
        conn.execute_batch(
            "
            CREATE TABLE calls (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                tool TEXT NOT NULL,
                dedup_key TEXT NOT NULL,
                model TEXT NOT NULL,
                input_tokens INTEGER NOT NULL,
                output_tokens INTEGER NOT NULL,
                cache_creation_input_tokens INTEGER NOT NULL,
                cache_read_input_tokens INTEGER NOT NULL,
                cached_input_tokens INTEGER NOT NULL,
                reasoning_tokens INTEGER NOT NULL,
                web_search_requests INTEGER NOT NULL,
                cost_usd REAL NOT NULL,
                tools_json TEXT NOT NULL,
                bash_commands_json TEXT NOT NULL,
                timestamp TEXT,
                speed TEXT NOT NULL,
                user_message TEXT NOT NULL,
                session_id TEXT NOT NULL,
                project TEXT NOT NULL,
                imported_at TEXT NOT NULL,
                UNIQUE(tool, dedup_key)
            );

            CREATE TABLE limit_snapshots (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                tool TEXT NOT NULL,
                limit_id TEXT NOT NULL,
                limit_name TEXT,
                plan_type TEXT,
                observed_at TEXT,
                primary_json TEXT,
                secondary_json TEXT,
                credits_json TEXT,
                rate_limit_reached_type TEXT,
                imported_at TEXT NOT NULL,
                snapshot_key TEXT NOT NULL UNIQUE
            );

            CREATE TABLE source_state (
                tool TEXT NOT NULL,
                path TEXT NOT NULL,
                fingerprint TEXT NOT NULL,
                synced_at TEXT NOT NULL,
                PRIMARY KEY(tool, path)
            );

            INSERT INTO calls (
                tool, dedup_key, model, input_tokens, output_tokens,
                cache_creation_input_tokens, cache_read_input_tokens,
                cached_input_tokens, reasoning_tokens, web_search_requests,
                cost_usd, tools_json, bash_commands_json, timestamp, speed,
                user_message, session_id, project, imported_at
            ) VALUES (
                'codex', 'legacy-call', 'gpt-5', 10, 5,
                0, 0, 0, 0, 0,
                0.5, '[]', '[]', '2026-04-29T12:00:00Z', 'standard',
                'legacy row', 'sess-legacy', '/tmp/legacy', '2026-04-29T12:00:00Z'
            );

            INSERT INTO source_state (tool, path, fingerprint, synced_at)
            VALUES ('codex', '/tmp/rollout.jsonl', 'fp', '2026-04-29T12:00:00Z');
            ",
        )
        .unwrap();
        conn.execute_batch(extra_sql).unwrap();
    }

    #[test]
    fn migrate_v2_archive_drops_advice_tables_and_keeps_calls() {
        let paths = temp_paths("migrate-v2");
        create_legacy_db(
            &paths,
            "
            CREATE TABLE advice_runs (id INTEGER PRIMARY KEY);
            CREATE TABLE advice_items (id INTEGER PRIMARY KEY);
            PRAGMA user_version = 2;
            ",
        );

        let archive = Archive::open(&paths).unwrap();
        let version: u32 = archive
            .conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, ARCHIVE_SCHEMA_VERSION);
        let advice_tables: u32 = archive
            .conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name IN ('advice_runs', 'advice_items')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(advice_tables, 0);
        let loaded = archive.load().unwrap();
        assert_eq!(loaded.calls.len(), 1, "calls survive the v3+v4 migrations");
        let _ = fs::remove_dir_all(paths.dir);
    }

    #[test]
    fn migrate_v3_archive_gains_enrichment_columns() {
        let paths = temp_paths("migrate-v3");
        create_legacy_db(&paths, "PRAGMA user_version = 3;");

        let archive = Archive::open(&paths).unwrap();
        let version: u32 = archive
            .conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, ARCHIVE_SCHEMA_VERSION);

        let loaded = archive.load().unwrap();
        assert_eq!(loaded.calls.len(), 1, "calls survive the v4 migration");
        let call = &loaded.calls[0];
        assert!(!call.is_canceled);
        assert_eq!(call.prompt_chars, None);
        assert_eq!(call.response_chars, None);
        assert_eq!(call.elapsed_ms, None);
        assert!(call.code_blocks.is_empty());
        assert!(call.edited_files.is_empty());
        assert!(call.referenced_files.is_empty());
        assert_eq!(
            call.timestamp_quality,
            crate::tools::TimestampQuality::Exact
        );
        assert_eq!(call.token_quality, crate::tools::TokenQuality::Unknown);

        let sources: i64 = archive
            .conn
            .query_row("SELECT COUNT(*) FROM source_state", [], |row| row.get(0))
            .unwrap();
        assert_eq!(
            sources, 0,
            "v4 clears source_state so history re-parses and enriches"
        );
        let _ = fs::remove_dir_all(paths.dir);
    }

    #[test]
    fn canonical_cursor_call_removes_only_listed_legacy_rows() {
        let paths = temp_paths("cursor-safe-supersede");
        let mut archive = Archive::open(&paths).unwrap();

        let mut replace = sample_call("cursor:unknown:one");
        replace.tool = crate::tools::cursor::config::TOOL_ID;
        let mut preserve = sample_call("cursor:unknown:two");
        preserve.tool = crate::tools::cursor::config::TOOL_ID;
        archive
            .insert_ingested(&Ingested {
                calls: vec![replace, preserve],
                limits: Vec::new(),
            })
            .unwrap();

        let mut canonical = sample_call("cursor:composer:composer-1:request-1");
        canonical.tool = crate::tools::cursor::config::TOOL_ID;
        canonical.superseded_dedup_keys = vec!["cursor:unknown:one".into()];
        archive
            .insert_ingested(&Ingested {
                calls: vec![canonical],
                limits: Vec::new(),
            })
            .unwrap();

        let keys = archive
            .load()
            .unwrap()
            .calls
            .into_iter()
            .map(|call| call.dedup_key)
            .collect::<Vec<_>>();
        assert!(!keys.contains(&"cursor:unknown:one".to_string()));
        assert!(keys.contains(&"cursor:unknown:two".to_string()));
        assert!(keys.contains(&"cursor:composer:composer-1:request-1".to_string()));
        let _ = fs::remove_dir_all(paths.dir);
    }

    #[test]
    fn reinserting_call_backfills_enrichment_once() {
        let paths = temp_paths("backfill");
        let mut archive = Archive::open(&paths).unwrap();

        archive
            .insert_ingested(&Ingested {
                calls: vec![bare_call("k1")],
                limits: Vec::new(),
            })
            .unwrap();
        assert_eq!(archive.load().unwrap().calls[0].prompt_chars, None);

        // A re-parse of the same source that now carries enrichment fills it.
        archive
            .insert_ingested(&Ingested {
                calls: vec![sample_call("k1")],
                limits: Vec::new(),
            })
            .unwrap();
        let loaded = archive.load().unwrap();
        let call = &loaded.calls[0];
        assert!(call.is_canceled);
        assert_eq!(call.prompt_chars, Some(2048));
        assert_eq!(call.response_chars, Some(4096));
        assert_eq!(call.elapsed_ms, Some(45_000));
        assert_eq!(call.code_blocks.len(), 1);
        assert_eq!(call.edited_files, vec!["src/main.rs".to_string()]);

        // A later parse with different values must not clobber.
        let mut changed = sample_call("k1");
        changed.prompt_chars = Some(9);
        changed.edited_files = vec!["other.rs".into()];
        archive
            .insert_ingested(&Ingested {
                calls: vec![changed],
                limits: Vec::new(),
            })
            .unwrap();
        let loaded = archive.load().unwrap();
        assert_eq!(
            loaded.calls[0].prompt_chars,
            Some(2048),
            "enrichment backfill fills once"
        );
        assert_eq!(
            loaded.calls[0].edited_files,
            vec!["src/main.rs".to_string()]
        );
        let _ = fs::remove_dir_all(paths.dir);
    }

    #[test]
    fn duplicate_calls_keep_import_time_cost() {
        let paths = temp_paths("dedup");
        let mut archive = Archive::open(&paths).unwrap();
        let first = sample_call("k1");
        let mut repriced = first.clone();
        repriced.cost_usd = 999.0;

        assert_eq!(
            archive
                .insert_ingested(&Ingested {
                    calls: vec![first.clone()],
                    limits: Vec::new(),
                })
                .unwrap(),
            1
        );
        assert_eq!(
            archive
                .insert_ingested(&Ingested {
                    calls: vec![repriced],
                    limits: Vec::new(),
                })
                .unwrap(),
            0
        );

        let loaded = archive.load().unwrap();
        assert_eq!(loaded.calls[0].cost_usd, first.cost_usd);
        let _ = fs::remove_dir_all(paths.dir);
    }

    #[test]
    fn duplicate_codex_calls_refresh_tool_activity_only() {
        let paths = temp_paths("codex-tool-refresh");
        let mut archive = Archive::open(&paths).unwrap();
        let mut first = sample_call("codex-k1");
        first.tools = vec!["exec".into()];
        first.bash_commands.clear();

        let mut reparsed = first.clone();
        reparsed.tools = vec![
            "Bash".into(),
            "mcp__codebase_memory_mcp__search_graph".into(),
        ];
        reparsed.bash_commands = vec!["cargo test".into()];
        reparsed.cost_usd = 999.0;

        archive
            .insert_ingested(&Ingested {
                calls: vec![first.clone()],
                limits: Vec::new(),
            })
            .unwrap();
        archive
            .insert_ingested(&Ingested {
                calls: vec![reparsed.clone()],
                limits: Vec::new(),
            })
            .unwrap();

        let loaded = archive.load().unwrap();
        assert_eq!(
            (
                &loaded.calls[0].tools,
                &loaded.calls[0].bash_commands,
                loaded.calls[0].cost_usd,
            ),
            (&reparsed.tools, &reparsed.bash_commands, first.cost_usd,)
        );
        let _ = fs::remove_dir_all(paths.dir);
    }

    #[test]
    fn duplicate_claude_calls_upgrade_streamed_activity() {
        let paths = temp_paths("claude-streamed-upgrade");
        let mut archive = Archive::open(&paths).unwrap();

        // Row archived by a parser that only saw the first streamed block
        // line: partial activity, but real usage and enrichment.
        let mut legacy = sample_call("msg_a");
        legacy.tool = crate::tools::claude_code::config::TOOL_ID;
        legacy.tools = vec!["Bash".into()];
        legacy.bash_commands = vec!["ls".into()];
        legacy.edited_files = Vec::new();
        legacy.code_blocks = Vec::new();
        legacy.response_chars = Some(6);

        // The fingerprint-bump reparse merges every block line.
        let mut reparsed = legacy.clone();
        reparsed.tools = vec!["Bash".into(), "Edit".into()];
        reparsed.bash_commands = vec!["ls".into(), "cargo check".into()];
        reparsed.edited_files = vec!["src/lib.rs".into()];
        reparsed.code_blocks = vec![crate::tools::CodeBlock {
            language: "rust".into(),
            loc: 3,
        }];
        reparsed.response_chars = Some(11);
        reparsed.cost_usd = 999.0;

        archive
            .insert_ingested(&Ingested {
                calls: vec![legacy.clone()],
                limits: Vec::new(),
            })
            .unwrap();
        archive
            .insert_ingested(&Ingested {
                calls: vec![reparsed.clone()],
                limits: Vec::new(),
            })
            .unwrap();

        let loaded = archive.load().unwrap();
        let call = &loaded.calls[0];
        assert_eq!(call.tools, reparsed.tools);
        assert_eq!(call.bash_commands, reparsed.bash_commands);
        assert_eq!(call.edited_files, reparsed.edited_files);
        assert_eq!(call.code_blocks, reparsed.code_blocks);
        assert_eq!(call.response_chars, Some(11));
        assert_eq!(
            call.cost_usd, legacy.cost_usd,
            "usage and cost keep the archived values"
        );

        // The response length never shrinks below the archived value.
        let mut weaker = legacy.clone();
        weaker.response_chars = Some(3);
        archive
            .insert_ingested(&Ingested {
                calls: vec![weaker],
                limits: Vec::new(),
            })
            .unwrap();
        let loaded = archive.load().unwrap();
        assert_eq!(loaded.calls[0].response_chars, Some(11));
        let _ = fs::remove_dir_all(paths.dir);
    }

    #[test]
    fn duplicate_cursor_calls_refresh_reconstructed_tokens() {
        let paths = temp_paths("cursor-token-refresh");
        let mut archive = Archive::open(&paths).unwrap();

        // Archived before the meter existed: chars/4 input estimate.
        let mut estimated = sample_call("cursor:composer:c1:r1");
        estimated.tool = crate::tools::cursor::config::TOOL_ID;
        estimated.input_tokens = 250;
        estimated.token_quality = crate::tools::TokenQuality::Estimated;

        // The reparse moves input to the composer credit; the turn keeps its
        // output side only.
        let mut reparsed = estimated.clone();
        reparsed.input_tokens = 0;
        reparsed.cost_usd = 0.05;

        archive
            .insert_ingested(&Ingested {
                calls: vec![estimated.clone()],
                limits: Vec::new(),
            })
            .unwrap();
        archive
            .insert_ingested(&Ingested {
                calls: vec![reparsed.clone()],
                limits: Vec::new(),
            })
            .unwrap();

        let loaded = archive.load().unwrap();
        assert_eq!(
            loaded.calls[0].input_tokens, 0,
            "reconstruction is authoritative"
        );
        assert_eq!(loaded.calls[0].cost_usd, 0.05);
        let _ = fs::remove_dir_all(paths.dir);
    }

    #[test]
    fn otel_rows_zero_superseded_transcript_estimates() {
        let paths = temp_paths("copilot-otel-zero");
        let mut archive = Archive::open(&paths).unwrap();

        // A transcript estimate of a turn, archived before the OTel store
        // was read.
        let mut estimate = sample_call("copilot:conv-1:turn-1");
        estimate.tool = crate::tools::copilot::config::TOOL_ID;
        archive
            .insert_ingested(&Ingested {
                calls: vec![estimate],
                limits: Vec::new(),
            })
            .unwrap();

        // The OTel span for the same turn carries the transcript key as a
        // superseded hint.
        let mut otel = sample_call("copilot-otel:span-1");
        otel.tool = crate::tools::copilot::config::TOOL_ID;
        otel.superseded_dedup_keys = vec!["copilot:conv-1:turn-1".into()];
        archive
            .insert_ingested(&Ingested {
                calls: vec![otel],
                limits: Vec::new(),
            })
            .unwrap();

        let loaded = archive.load().unwrap();
        assert_eq!(loaded.calls.len(), 2, "the estimate row keeps its metadata");
        let old = loaded
            .calls
            .iter()
            .find(|c| c.dedup_key == "copilot:conv-1:turn-1")
            .unwrap();
        assert_eq!(
            (old.input_tokens, old.output_tokens, old.cost_usd),
            (0, 0, 0.0),
            "superseded estimate is zeroed, not deleted"
        );
        assert_eq!(old.user_message, "build the thing");
        let _ = fs::remove_dir_all(paths.dir);
    }

    #[test]
    fn codex_supersession_retires_legacy_rows_and_inherits_cost() {
        let paths = temp_paths("codex-supersede");
        let mut archive = Archive::open(&paths).unwrap();

        // Rows written by the pre-v6 parser: the event's own path-keyed row
        // and a fork-replay duplicate of another event.
        let mut own_legacy = sample_call("codex:/old/rollout.jsonl:t1:1100+10");
        own_legacy.cost_usd = 0.5;
        let mut replay_dup = sample_call("codex:/fork/rollout.jsonl:t2:700+0");
        replay_dup.input_tokens = 700;
        replay_dup.output_tokens = 0;
        replay_dup.cache_read_input_tokens = 0;
        replay_dup.reasoning_tokens = 0;
        archive
            .insert_ingested(&Ingested {
                calls: vec![own_legacy.clone(), replay_dup],
                limits: Vec::new(),
            })
            .unwrap();

        // The v6 reparse emits the same event under its lineage key, priced
        // with whatever the pricing books say today.
        let mut reparsed = sample_call("codex:sess-1:1100:11:10:5:1121");
        reparsed.cost_usd = 999.0;
        reparsed.superseded_dedup_keys = vec![
            "codex:/fork/rollout.jsonl:t2:700+0".into(),
            "codex:/old/rollout.jsonl:t1:1100+10".into(),
        ];
        archive
            .insert_ingested(&Ingested {
                calls: vec![reparsed.clone()],
                limits: Vec::new(),
            })
            .unwrap();

        let loaded = archive.load().unwrap();
        assert_eq!(loaded.calls.len(), 1, "legacy rows are retired");
        assert_eq!(loaded.calls[0].dedup_key, reparsed.dedup_key);
        assert_eq!(
            loaded.calls[0].cost_usd, 0.5,
            "the same event keeps its import-time cost instead of being repriced"
        );
        let _ = fs::remove_dir_all(paths.dir);
    }

    #[test]
    fn duplicate_codex_calls_fill_empty_user_message_only() {
        let paths = temp_paths("codex-prompt-backfill");
        let mut archive = Archive::open(&paths).unwrap();

        // Row archived before the parser captured Codex prompts.
        let mut legacy = sample_call("codex-k2");
        legacy.user_message = String::new();
        archive
            .insert_ingested(&Ingested {
                calls: vec![legacy],
                limits: Vec::new(),
            })
            .unwrap();

        // Re-parse now carries the prompt: filled.
        let mut reparsed = sample_call("codex-k2");
        reparsed.user_message = "please fix the tests".into();
        archive
            .insert_ingested(&Ingested {
                calls: vec![reparsed],
                limits: Vec::new(),
            })
            .unwrap();
        assert_eq!(
            archive.load().unwrap().calls[0].user_message,
            "please fix the tests"
        );

        // A later parse never rewrites a stored prompt.
        let mut changed = sample_call("codex-k2");
        changed.user_message = "different text".into();
        archive
            .insert_ingested(&Ingested {
                calls: vec![changed],
                limits: Vec::new(),
            })
            .unwrap();
        assert_eq!(
            archive.load().unwrap().calls[0].user_message,
            "please fix the tests"
        );
        let _ = fs::remove_dir_all(paths.dir);
    }

    #[test]
    fn duplicate_cursor_calls_refresh_project_only() {
        let paths = temp_paths("cursor-project-refresh");
        let mut archive = Archive::open(&paths).unwrap();
        let mut first = sample_call("cursor-k1");
        first.tool = crate::tools::cursor::config::TOOL_ID;
        first.project = "cursor-workspace".into();

        let mut reparsed = first.clone();
        reparsed.project = "/Users/me/Code/app".into();
        reparsed.cost_usd = 999.0;

        assert_eq!(
            archive
                .insert_ingested(&Ingested {
                    calls: vec![first.clone()],
                    limits: Vec::new(),
                })
                .unwrap(),
            1
        );
        assert_eq!(
            archive
                .insert_ingested(&Ingested {
                    calls: vec![reparsed],
                    limits: Vec::new(),
                })
                .unwrap(),
            0
        );

        let loaded = archive.load().unwrap();
        assert_eq!(loaded.calls[0].project, "/Users/me/Code/app");
        assert_eq!(loaded.calls[0].cost_usd, first.cost_usd);
        let _ = fs::remove_dir_all(paths.dir);
    }

    #[test]
    fn reset_database_drops_rows_and_source_fingerprints() {
        let paths = temp_paths("reset");
        let mut archive = Archive::open(&paths).unwrap();
        let ingested = Ingested {
            calls: vec![sample_call("reset-k1")],
            limits: vec![sample_limit()],
        };
        archive.insert_ingested(&ingested).unwrap();
        {
            let tx = archive.conn.transaction().unwrap();
            upsert_source_fingerprint(
                &tx,
                crate::tools::codex::config::TOOL_ID,
                "/tmp/source.jsonl",
                "fingerprint",
                "",
            )
            .unwrap();
            tx.commit().unwrap();
        }

        assert!(!archive.is_empty().unwrap());
        assert_eq!(
            archive
                .source_fingerprint(crate::tools::codex::config::TOOL_ID, "/tmp/source.jsonl")
                .unwrap()
                .as_deref(),
            Some("fingerprint")
        );

        archive.reset_database().unwrap();

        assert!(archive.is_empty().unwrap());
        assert_eq!(
            archive
                .source_fingerprint(crate::tools::codex::config::TOOL_ID, "/tmp/source.jsonl")
                .unwrap(),
            None
        );
        let _ = fs::remove_dir_all(paths.dir);
    }

    #[test]
    fn legacy_cache_seeds_empty_archive() {
        let paths = temp_paths("legacy");
        let cache_path = paths.dir.join("legacy-cache.json");
        let ingested = Ingested {
            calls: vec![sample_call("legacy")],
            limits: vec![sample_limit()],
        };
        crate::ingest_cache::write_path(&cache_path, &ingested).unwrap();

        let mut archive = Archive::open(&paths).unwrap();
        assert_eq!(
            archive.import_legacy_cache_from_path(&cache_path).unwrap(),
            2
        );
        let loaded = archive.load().unwrap();
        assert_eq!(loaded.calls, ingested.calls);
        assert_eq!(loaded.limits, ingested.limits);
        let _ = fs::remove_dir_all(paths.dir);
    }

    #[test]
    fn tail_merge_concatenates_activity_instead_of_overwriting() {
        let paths = temp_paths("tail-merge");
        let mut archive = Archive::open(&paths).unwrap();

        let full = ParsedCall {
            tool: crate::tools::claude_code::config::TOOL_ID,
            is_canceled: false,
            ..sample_call("msg_1")
        };
        let tail = ParsedCall {
            tools: vec!["Write".into()],
            bash_commands: vec!["cargo check".into()],
            edited_files: vec!["src/main.rs".into(), "src/tail.rs".into()],
            referenced_files: Vec::new(),
            code_blocks: Vec::new(),
            response_chars: Some(100),
            is_canceled: true,
            merge_activity: true,
            ..full.clone()
        };

        {
            let tx = archive.conn.transaction().unwrap();
            assert!(insert_call(&tx, &full).unwrap());
            assert!(!insert_call(&tx, &tail).unwrap());
            tx.commit().unwrap();
        }

        let loaded = archive.load().unwrap();
        let call = &loaded.calls[0];
        assert_eq!(
            call.tools,
            vec!["exec_command", "apply_patch", "Write"],
            "tail tool occurrences concatenate"
        );
        assert_eq!(call.bash_commands, vec!["cargo test", "cargo check"]);
        assert_eq!(
            call.edited_files,
            vec!["src/main.rs", "src/tail.rs"],
            "file lists union and re-dedup"
        );
        assert_eq!(
            call.response_chars,
            Some(4096 + 100),
            "prefix and tail response chars sum"
        );
        assert!(call.is_canceled, "a tail-observed interruption sticks");
        let _ = fs::remove_dir_all(paths.dir);
    }

    #[test]
    fn sync_persists_and_replays_adapter_cursors() {
        let paths = temp_paths("sync-cursor");
        let mut archive = Archive::open(&paths).unwrap();
        let source = fake_source(paths.dir.join("source.jsonl"));
        let seen_cursor = Arc::new(std::sync::Mutex::new(None::<Option<String>>));

        struct CursorAdapter {
            source: SessionSource,
            fingerprint: String,
            emit_cursor: String,
            resumed_files: usize,
            seen_cursor: Arc<std::sync::Mutex<Option<Option<String>>>>,
        }
        impl ToolAdapter for CursorAdapter {
            fn id(&self) -> &'static str {
                self.source.tool
            }
            fn display_name(&self) -> &'static str {
                "Cursor Fake"
            }
            fn discover(&self) -> Result<Vec<SessionSource>> {
                Ok(vec![self.source.clone()])
            }
            fn parse(
                &self,
                _source: &SessionSource,
                _seen: &mut HashSet<String>,
            ) -> Result<Vec<ParsedCall>> {
                Ok(Vec::new())
            }
            fn parse_with_cursor(
                &self,
                _source: &SessionSource,
                _seen: &mut HashSet<String>,
                cursor: Option<&str>,
            ) -> Result<crate::tools::AdapterParse> {
                *self.seen_cursor.lock().unwrap() = Some(cursor.map(str::to_string));
                Ok(crate::tools::AdapterParse {
                    calls: Vec::new(),
                    cursor: Some(self.emit_cursor.clone()),
                    resumed_files: self.resumed_files,
                })
            }
            fn source_fingerprint(&self, _source: &SessionSource) -> Result<String> {
                Ok(self.fingerprint.clone())
            }
        }

        let adapter: Box<dyn ToolAdapter> = Box::new(CursorAdapter {
            source: source.clone(),
            fingerprint: "v1".into(),
            emit_cursor: "cursor-1".into(),
            resumed_files: 0,
            seen_cursor: seen_cursor.clone(),
        });
        let stats = archive.sync_with_adapters(&[adapter]).unwrap();
        assert_eq!(stats.files_resumed, 0);
        assert_eq!(
            seen_cursor.lock().unwrap().clone(),
            Some(None),
            "first sync has no stored cursor"
        );

        let adapter: Box<dyn ToolAdapter> = Box::new(CursorAdapter {
            source,
            fingerprint: "v2".into(),
            emit_cursor: "cursor-2".into(),
            resumed_files: 3,
            seen_cursor: seen_cursor.clone(),
        });
        let stats = archive.sync_with_adapters(&[adapter]).unwrap();
        assert_eq!(stats.files_resumed, 3);
        assert_eq!(
            seen_cursor.lock().unwrap().clone(),
            Some(Some("cursor-1".into())),
            "second sync replays the cursor persisted by the first"
        );
        let _ = fs::remove_dir_all(paths.dir);
    }

    struct FakeAdapter {
        source: SessionSource,
        fingerprint: String,
        calls: Vec<ParsedCall>,
        parse_count: Arc<AtomicUsize>,
    }

    impl ToolAdapter for FakeAdapter {
        fn id(&self) -> &'static str {
            self.source.tool
        }

        fn display_name(&self) -> &'static str {
            "Fake"
        }

        fn discover(&self) -> Result<Vec<SessionSource>> {
            Ok(vec![self.source.clone()])
        }

        fn parse(
            &self,
            _source: &SessionSource,
            _seen: &mut HashSet<String>,
        ) -> Result<Vec<ParsedCall>> {
            self.parse_count.fetch_add(1, Ordering::SeqCst);
            Ok(self.calls.clone())
        }

        fn source_fingerprint(&self, _source: &SessionSource) -> Result<String> {
            Ok(self.fingerprint.clone())
        }
    }

    fn fake_source(path: PathBuf) -> SessionSource {
        SessionSource::session(path, "fake-project", crate::tools::codex::config::TOOL_ID)
    }

    #[test]
    fn sync_skips_unchanged_sources_and_never_deletes_missing_history() {
        let paths = temp_paths("sync");
        let mut archive = Archive::open(&paths).unwrap();
        let source_path = paths.dir.join("source.jsonl");
        let source = fake_source(source_path);
        let parse_count = Arc::new(AtomicUsize::new(0));
        let adapter: Box<dyn ToolAdapter> = Box::new(FakeAdapter {
            source: source.clone(),
            fingerprint: "v1".into(),
            calls: vec![sample_call("k1")],
            parse_count: parse_count.clone(),
        });

        let stats = archive.sync_with_adapters(&[adapter]).unwrap();
        assert_eq!(stats.calls_inserted, 1);
        assert_eq!(parse_count.load(Ordering::SeqCst), 1);

        let adapter: Box<dyn ToolAdapter> = Box::new(FakeAdapter {
            source: source.clone(),
            fingerprint: "v1".into(),
            calls: vec![sample_call("k1")],
            parse_count: parse_count.clone(),
        });
        let stats = archive.sync_with_adapters(&[adapter]).unwrap();
        assert_eq!(stats.sources_parsed, 0);
        assert_eq!(parse_count.load(Ordering::SeqCst), 1);

        let adapter: Box<dyn ToolAdapter> = Box::new(FakeAdapter {
            source,
            fingerprint: "v2".into(),
            calls: vec![sample_call("k1"), sample_call("k2")],
            parse_count: parse_count.clone(),
        });
        let stats = archive.sync_with_adapters(&[adapter]).unwrap();
        assert_eq!(stats.calls_inserted, 1);
        assert_eq!(parse_count.load(Ordering::SeqCst), 2);

        let stats = archive.sync_with_adapters(&[]).unwrap();
        assert_eq!(stats.sources_seen, 0);
        assert_eq!(archive.load().unwrap().calls.len(), 2);
        let _ = fs::remove_dir_all(paths.dir);
    }

    fn transcript_row(archive: &Archive, key: &str) -> Option<(String, String, String)> {
        archive
            .conn
            .query_row(
                "SELECT user_text, assistant_text, origin FROM transcripts
                 WHERE tool = ?1 AND dedup_key = ?2",
                params![crate::tools::codex::config::TOOL_ID, key],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()
            .unwrap()
    }

    fn fts_match_count(archive: &Archive, needle: &str) -> i64 {
        archive
            .conn
            .query_row(
                "SELECT COUNT(*) FROM transcripts_fts WHERE transcripts_fts MATCH ?1",
                params![format!("\"{needle}\"")],
                |row| row.get(0),
            )
            .unwrap()
    }

    #[test]
    fn migrate_v6_archive_seeds_prompt_fallback_transcripts() {
        let paths = temp_paths("migrate-v6-transcripts");
        create_legacy_db(
            &paths,
            "
            ALTER TABLE calls ADD COLUMN is_canceled INTEGER NOT NULL DEFAULT 0;
            ALTER TABLE calls ADD COLUMN prompt_chars INTEGER;
            ALTER TABLE calls ADD COLUMN response_chars INTEGER;
            ALTER TABLE calls ADD COLUMN elapsed_ms INTEGER;
            ALTER TABLE calls ADD COLUMN code_blocks_json TEXT NOT NULL DEFAULT '[]';
            ALTER TABLE calls ADD COLUMN edited_files_json TEXT NOT NULL DEFAULT '[]';
            ALTER TABLE calls ADD COLUMN referenced_files_json TEXT NOT NULL DEFAULT '[]';
            ALTER TABLE calls ADD COLUMN interaction_mode TEXT NOT NULL DEFAULT 'unknown';
            ALTER TABLE calls ADD COLUMN token_quality TEXT NOT NULL DEFAULT 'unknown';
            ALTER TABLE calls ADD COLUMN timestamp_quality TEXT NOT NULL DEFAULT 'unknown';
            ALTER TABLE source_state ADD COLUMN cursor_json TEXT NOT NULL DEFAULT '';
            PRAGMA user_version = 6;
            ",
        );

        let archive = Archive::open(&paths).unwrap();
        let version: u32 = archive
            .conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, ARCHIVE_SCHEMA_VERSION);

        let seeded = transcript_row(&archive, "legacy-call").unwrap();
        assert_eq!(seeded.0, "legacy row", "truncated prompt seeds user_text");
        assert_eq!(seeded.1, "", "no assistant text for legacy rows");
        assert_eq!(seeded.2, "prompt", "seeded rows are prompt-origin");
        assert_eq!(
            fts_match_count(&archive, "legacy"),
            1,
            "seeded rows are indexed"
        );

        let sources: i64 = archive
            .conn
            .query_row("SELECT COUNT(*) FROM source_state", [], |row| row.get(0))
            .unwrap();
        assert_eq!(sources, 0, "v7 clears source_state to force re-parse");
        let _ = fs::remove_dir_all(paths.dir);
    }

    /// Insert one pre-v8 row with an explicit cost, then migrate.
    fn legacy_opus_5_row(
        label: &str,
        model: &str,
        speed: &str,
        cost: f64,
        call: &ParsedCall,
    ) -> String {
        format!(
            "INSERT INTO calls (tool, dedup_key, model, input_tokens, output_tokens,
                 cache_creation_input_tokens, cache_read_input_tokens, cached_input_tokens,
                 reasoning_tokens, web_search_requests, cost_usd, tools_json,
                 bash_commands_json, timestamp, speed, user_message, session_id,
                 project, imported_at)
             VALUES ('claude-code', '{label}', '{model}', {}, {}, {}, {}, 0, 0, {},
                 {cost}, '[]', '[]', '2026-07-20T12:00:00Z', '{speed}', '', 's', 'p',
                 '2026-07-20T12:00:00Z');",
            call.input_tokens,
            call.output_tokens,
            call.cache_creation_input_tokens,
            call.cache_read_input_tokens,
            call.web_search_requests,
        )
    }

    fn cost_of(archive: &Archive, key: &str) -> f64 {
        archive
            .conn
            .query_row(
                "SELECT cost_usd FROM calls WHERE dedup_key = ?1",
                params![key],
                |row| row.get(0),
            )
            .unwrap()
    }

    #[test]
    fn migrate_v8_reprices_opus_5_rows_to_match_the_real_pricing_formula() {
        // A call with a 1-hour cache-write share, which is the term that
        // cannot be recomputed from the archive because it is never persisted.
        let mut call = bare_call("opus5");
        call.tool = "claude-code";
        call.model = "claude-opus-5".into();
        call.input_tokens = 12_000;
        call.output_tokens = 3_400;
        call.cache_creation_input_tokens = 50_000;
        call.cache_creation_1h_input_tokens = 30_000;
        call.cache_read_input_tokens = 900_000;
        call.web_search_requests = 0;
        call.timestamp = Some(Utc.with_ymd_and_hms(2026, 7, 20, 12, 0, 0).unwrap());

        // What the call was actually billed while Opus 5 was unpriced, and
        // what it should have cost.
        let charged = crate::pricing::cost("claude-sonnet-4-6", &call, Speed::Standard);
        let truth = crate::pricing::cost("claude-opus-5", &call, Speed::Standard);
        assert!(charged < truth, "the gap this migration exists to close");

        let paths = temp_paths("migrate-v8-opus5");
        create_legacy_db(
            &paths,
            &format!(
                "{}
                PRAGMA user_version = 7;
                ",
                legacy_opus_5_row("opus5", "claude-opus-5", "standard", charged, &call)
            ),
        );

        let archive = Archive::open(&paths).unwrap();
        let repaired = cost_of(&archive, "opus5");
        assert!(
            (repaired - truth).abs() < 1e-9,
            "rescale must reproduce the pricing formula exactly, including the \
             1h cache-write premium: got {repaired}, want {truth}"
        );
        let _ = fs::remove_dir_all(paths.dir);
    }

    #[test]
    fn migrate_v8_skips_rows_that_are_already_correctly_priced() {
        let mut call = bare_call("correct");
        call.tool = "claude-code";
        call.input_tokens = 12_000;
        call.output_tokens = 3_400;
        call.cache_creation_input_tokens = 50_000;
        call.cache_creation_1h_input_tokens = 30_000;
        call.cache_read_input_tokens = 900_000;
        call.timestamp = Some(Utc.with_ymd_and_hms(2026, 7, 20, 12, 0, 0).unwrap());

        // Someone who downloaded corrected books before upgrading the binary
        // already has true Opus 5 costs; rescaling those would over-count.
        let already_correct = crate::pricing::cost("claude-opus-5", &call, Speed::Standard);
        // A different model must not be touched at all.
        let sonnet = crate::pricing::cost("claude-sonnet-4-6", &call, Speed::Standard);

        let paths = temp_paths("migrate-v8-guard");
        create_legacy_db(
            &paths,
            &format!(
                "{}
                {}
                PRAGMA user_version = 7;
                ",
                legacy_opus_5_row(
                    "correct",
                    "claude-opus-5",
                    "standard",
                    already_correct,
                    &call
                ),
                legacy_opus_5_row("other", "claude-sonnet-4-6", "standard", sonnet, &call),
            ),
        );

        let archive = Archive::open(&paths).unwrap();
        assert!(
            (cost_of(&archive, "correct") - already_correct).abs() < 1e-12,
            "an already-correct Opus 5 row must be left alone"
        );
        assert!(
            (cost_of(&archive, "other") - sonnet).abs() < 1e-12,
            "non-Opus-5 rows must be untouched"
        );
        let _ = fs::remove_dir_all(paths.dir);
    }

    #[test]
    fn migrate_v8_applies_the_fast_multiplier_the_fallback_row_never_had() {
        let mut call = bare_call("fast");
        call.tool = "claude-code";
        call.input_tokens = 8_000;
        call.output_tokens = 2_000;
        call.cache_creation_input_tokens = 1_000;
        call.cache_read_input_tokens = 40_000;
        call.timestamp = Some(Utc.with_ymd_and_hms(2026, 7, 20, 12, 0, 0).unwrap());

        // Sonnet 4.6 carries no fast multiplier, so fast rows were billed at 1x.
        let charged = crate::pricing::cost("claude-sonnet-4-6", &call, Speed::Fast);
        let truth = crate::pricing::cost("claude-opus-5", &call, Speed::Fast);

        let paths = temp_paths("migrate-v8-fast");
        create_legacy_db(
            &paths,
            &format!(
                "{}
                PRAGMA user_version = 7;
                ",
                legacy_opus_5_row("fast", "claude-opus-5", "fast", charged, &call)
            ),
        );

        let archive = Archive::open(&paths).unwrap();
        let repaired = cost_of(&archive, "fast");
        assert!(
            (repaired - truth).abs() < 1e-9,
            "fast rows need the 2x Opus 5 multiplier applied: got {repaired}, want {truth}"
        );
        let _ = fs::remove_dir_all(paths.dir);
    }

    #[test]
    fn migrate_v8_folds_dated_ids_but_not_neighbouring_models() {
        let mut call = bare_call("dated");
        call.tool = "claude-code";
        call.input_tokens = 5_000;
        call.output_tokens = 1_000;
        call.timestamp = Some(Utc.with_ymd_and_hms(2026, 7, 20, 12, 0, 0).unwrap());

        let charged = crate::pricing::cost("claude-sonnet-4-6", &call, Speed::Standard);

        let paths = temp_paths("migrate-v8-canonical");
        create_legacy_db(
            &paths,
            &format!(
                "{}
                {}
                PRAGMA user_version = 7;
                ",
                legacy_opus_5_row(
                    "dated",
                    "anthropic/claude-opus-5-20260715",
                    "standard",
                    charged,
                    &call
                ),
                legacy_opus_5_row(
                    "neighbour",
                    "claude-opus-5-fast",
                    "standard",
                    charged,
                    &call
                ),
            ),
        );

        let archive = Archive::open(&paths).unwrap();
        let truth = crate::pricing::cost("claude-opus-5", &call, Speed::Standard);
        assert!(
            (cost_of(&archive, "dated") - truth).abs() < 1e-9,
            "dated and vendor-prefixed Opus 5 ids canonicalize into the repair"
        );
        assert!(
            (cost_of(&archive, "neighbour") - charged).abs() < 1e-12,
            "a distinct model id that merely starts with claude-opus-5 is not repriced"
        );
        let _ = fs::remove_dir_all(paths.dir);
    }

    #[test]
    fn transcript_upsert_is_grow_only_and_upgrades_prompt_rows() {
        let paths = temp_paths("transcript-grow");
        let mut archive = Archive::open(&paths).unwrap();

        // First parse: metadata only (no transcript capture).
        archive
            .insert_ingested(&Ingested {
                calls: vec![sample_call("k1")],
                limits: Vec::new(),
            })
            .unwrap();
        assert_eq!(
            transcript_row(&archive, "k1"),
            None,
            "text-less calls create no transcript row"
        );

        // Re-parse with captured text creates the row.
        let mut with_text = sample_call("k1");
        with_text.transcript_user = Some("build the thing properly".into());
        with_text.transcript_assistant = Some("done, ran the tests".into());
        archive
            .insert_ingested(&Ingested {
                calls: vec![with_text],
                limits: Vec::new(),
            })
            .unwrap();
        let row = transcript_row(&archive, "k1").unwrap();
        assert_eq!(row.0, "build the thing properly");
        assert_eq!(row.1, "done, ran the tests");
        assert_eq!(row.2, "full");

        // A weaker later parse never shrinks stored text.
        let mut weaker = sample_call("k1");
        weaker.transcript_user = Some("build".into());
        weaker.transcript_assistant = Some(String::new());
        archive
            .insert_ingested(&Ingested {
                calls: vec![weaker],
                limits: Vec::new(),
            })
            .unwrap();
        let row = transcript_row(&archive, "k1").unwrap();
        assert_eq!(row.0, "build the thing properly");
        assert_eq!(row.1, "done, ran the tests");

        // A longer parse grows it, and the FTS index follows.
        let mut longer = sample_call("k1");
        longer.transcript_user = Some("build the thing properly this time".into());
        longer.transcript_assistant = Some("done, ran the tests, all green".into());
        archive
            .insert_ingested(&Ingested {
                calls: vec![longer],
                limits: Vec::new(),
            })
            .unwrap();
        let row = transcript_row(&archive, "k1").unwrap();
        assert_eq!(row.0, "build the thing properly this time");
        assert_eq!(fts_match_count(&archive, "green"), 1);
        assert_eq!(fts_match_count(&archive, "tests"), 1, "no stale FTS rows");
        let _ = fs::remove_dir_all(paths.dir);
    }

    #[test]
    fn claude_tail_merge_appends_assistant_transcript() {
        let paths = temp_paths("transcript-tail-merge");
        let mut archive = Archive::open(&paths).unwrap();

        let mut prefix = sample_call("msg-1");
        prefix.tool = crate::tools::claude_code::config::TOOL_ID;
        prefix.transcript_user = Some("fix the bug".into());
        prefix.transcript_assistant = Some("Looking at the parser first.".into());
        archive
            .insert_ingested(&Ingested {
                calls: vec![prefix],
                limits: Vec::new(),
            })
            .unwrap();

        // Next sync delivers the same message's tail blocks.
        let mut tail = sample_call("msg-1");
        tail.tool = crate::tools::claude_code::config::TOOL_ID;
        tail.merge_activity = true;
        tail.transcript_user = Some("fix the bug".into());
        tail.transcript_assistant = Some("Fixed and covered by a test.".into());
        archive
            .insert_ingested(&Ingested {
                calls: vec![tail],
                limits: Vec::new(),
            })
            .unwrap();

        let row = archive
            .conn
            .query_row(
                "SELECT user_text, assistant_text, origin FROM transcripts
                 WHERE tool = ?1 AND dedup_key = 'msg-1'",
                params![crate::tools::claude_code::config::TOOL_ID],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(row.0, "fix the bug");
        assert_eq!(
            row.1,
            "Looking at the parser first.\nFixed and covered by a test."
        );
        assert_eq!(row.2, "full");
        let _ = fs::remove_dir_all(paths.dir);
    }

    #[test]
    fn claude_tail_merge_is_idempotent_for_replayed_tails() {
        let paths = temp_paths("transcript-tail-replay");
        let mut archive = Archive::open(&paths).unwrap();

        let mut prefix = sample_call("msg-1");
        prefix.tool = crate::tools::claude_code::config::TOOL_ID;
        prefix.transcript_assistant = Some("Looking at the parser.".into());
        archive
            .insert_ingested(&Ingested {
                calls: vec![prefix],
                limits: Vec::new(),
            })
            .unwrap();

        // Two racing refreshers deliver the same tail twice.
        for _ in 0..2 {
            let mut tail = sample_call("msg-1");
            tail.tool = crate::tools::claude_code::config::TOOL_ID;
            tail.merge_activity = true;
            tail.transcript_assistant = Some("Fixed.".into());
            archive
                .insert_ingested(&Ingested {
                    calls: vec![tail],
                    limits: Vec::new(),
                })
                .unwrap();
        }

        let assistant: String = archive
            .conn
            .query_row(
                "SELECT assistant_text FROM transcripts WHERE tool = ?1 AND dedup_key = 'msg-1'",
                params![crate::tools::claude_code::config::TOOL_ID],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            assistant, "Looking at the parser.\nFixed.",
            "a replayed tail appends once"
        );
        let _ = fs::remove_dir_all(paths.dir);
    }

    #[test]
    fn copilot_usage_upgrade_replaces_estimate_transcripts() {
        let paths = temp_paths("copilot-transcript-supersede");
        let mut archive = Archive::open(&paths).unwrap();

        // Sync N: chars/4 estimate row carries the turn's transcript.
        let mut estimate = sample_call("copilot:sess-1:turn-0");
        estimate.tool = crate::tools::copilot::config::TOOL_ID;
        estimate.transcript_user = Some("write the tests".into());
        estimate.transcript_assistant = Some("Done, added coverage.".into());
        archive
            .insert_ingested(&Ingested {
                calls: vec![estimate],
                limits: Vec::new(),
            })
            .unwrap();

        // Sync N+1: the usage-event row supersedes it with identical text.
        let mut usage = sample_call("copilot:sess-1:turn-0:usage-7");
        usage.tool = crate::tools::copilot::config::TOOL_ID;
        usage.transcript_user = Some("write the tests".into());
        usage.transcript_assistant = Some("Done, added coverage.".into());
        archive
            .insert_ingested(&Ingested {
                calls: vec![usage],
                limits: Vec::new(),
            })
            .unwrap();

        let rows: i64 = archive
            .conn
            .query_row(
                "SELECT COUNT(*) FROM transcripts WHERE tool = ?1 AND session_id = 'sess-1'",
                params![crate::tools::copilot::config::TOOL_ID],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(rows, 1, "the estimate transcript is replaced, not doubled");

        // An OTel-style superseding row without text must NOT delete the
        // turn's only transcript.
        let mut text_estimate = sample_call("copilot:sess-2:turn-0");
        text_estimate.tool = crate::tools::copilot::config::TOOL_ID;
        text_estimate.transcript_user = Some("hello".into());
        archive
            .insert_ingested(&Ingested {
                calls: vec![text_estimate],
                limits: Vec::new(),
            })
            .unwrap();
        let mut otel = sample_call(&format!(
            "{}span-1",
            crate::tools::copilot::config::OTEL_DEDUP_PREFIX
        ));
        otel.tool = crate::tools::copilot::config::TOOL_ID;
        otel.session_id = "sess-2".into();
        otel.superseded_dedup_keys = vec!["copilot:sess-2:turn-0".into()];
        archive
            .insert_ingested(&Ingested {
                calls: vec![otel],
                limits: Vec::new(),
            })
            .unwrap();
        let kept: i64 = archive
            .conn
            .query_row(
                "SELECT COUNT(*) FROM transcripts WHERE tool = ?1 AND dedup_key = 'copilot:sess-2:turn-0'",
                params![crate::tools::copilot::config::TOOL_ID],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(kept, 1, "a text-less superseder keeps the only transcript");
        let _ = fs::remove_dir_all(paths.dir);
    }

    #[test]
    fn superseded_rows_drop_their_transcripts() {
        let paths = temp_paths("transcript-supersede");
        let mut archive = Archive::open(&paths).unwrap();

        let mut legacy = sample_call("codex:legacy-key");
        legacy.transcript_user = Some("old prompt".into());
        archive
            .insert_ingested(&Ingested {
                calls: vec![legacy],
                limits: Vec::new(),
            })
            .unwrap();
        assert!(transcript_row(&archive, "codex:legacy-key").is_some());

        let mut canonical = sample_call("codex:lineage-key");
        canonical.superseded_dedup_keys = vec!["codex:legacy-key".into()];
        canonical.transcript_user = Some("old prompt".into());
        archive
            .insert_ingested(&Ingested {
                calls: vec![canonical],
                limits: Vec::new(),
            })
            .unwrap();

        assert_eq!(
            transcript_row(&archive, "codex:legacy-key"),
            None,
            "superseded transcript rows are deleted with their calls rows"
        );
        assert!(transcript_row(&archive, "codex:lineage-key").is_some());
        assert_eq!(
            fts_match_count(&archive, "old prompt"),
            1,
            "the FTS index drops the superseded row too"
        );
        let _ = fs::remove_dir_all(paths.dir);
    }

    #[test]
    fn reset_database_drops_transcript_tables() {
        let paths = temp_paths("transcript-reset");
        let mut archive = Archive::open(&paths).unwrap();
        let mut call = sample_call("k1");
        call.transcript_user = Some("some text".into());
        archive
            .insert_ingested(&Ingested {
                calls: vec![call],
                limits: Vec::new(),
            })
            .unwrap();

        archive.reset_database().unwrap();
        let count: i64 = archive
            .conn
            .query_row("SELECT COUNT(*) FROM transcripts", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0, "reset rebuilds empty transcript tables");
        assert_eq!(fts_match_count(&archive, "some"), 0);
        let _ = fs::remove_dir_all(paths.dir);
    }
}
