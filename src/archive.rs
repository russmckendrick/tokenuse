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

const ARCHIVE_SCHEMA_VERSION: u32 = 5;

pub struct Archive {
    conn: Connection,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SyncStats {
    pub sources_seen: usize,
    pub sources_parsed: usize,
    pub calls_inserted: usize,
    pub limits_inserted: usize,
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
    let mut archive = Archive::open(paths)?;
    if archive.is_empty()? {
        let _ = archive.import_legacy_cache_if_empty()?;
    }
    archive.sync()?;
    archive.load()
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
                    Ok(Vec::new())
                } else {
                    adapter.parse(&source, &mut seen)
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

                let calls = calls_result.unwrap_or_default();
                let limits = limits_result.unwrap_or_default();
                let tx = self.conn.transaction()?;
                for call in &calls {
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
                        upsert_source_fingerprint(&tx, source.tool, &path, fingerprint)?;
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
        update_existing_copilot_cli_totals(tx, call)?;
        update_existing_codex_tool_activity(tx, call, &tools_json, &bash_json)?;
        update_existing_call_enrichment(
            tx,
            call,
            &code_blocks_json,
            &edited_files_json,
            &referenced_files_json,
        )?;
    } else {
        zero_superseded_copilot_estimates(tx, call)?;
    }
    remove_superseded_cursor_rows(tx, call)?;
    Ok(inserted > 0)
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
    }
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
) -> Result<()> {
    tx.execute(
        "
        INSERT INTO source_state (tool, path, fingerprint, synced_at)
        VALUES (?1, ?2, ?3, ?4)
        ON CONFLICT(tool, path) DO UPDATE SET
            fingerprint = excluded.fingerprint,
            synced_at = excluded.synced_at
        ",
        params![tool, path, fingerprint, Utc::now().to_rfc3339()],
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
        assert_eq!(version, 5);
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
        assert_eq!(version, 5);

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
}
