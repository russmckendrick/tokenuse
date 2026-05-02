use std::collections::HashSet;
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
use crate::tools::{self, LimitSnapshot, ParsedCall, Speed, ToolAdapter};

pub const SYNC_INTERVAL: Duration = crate::ingest_cache::TTL;

const ARCHIVE_SCHEMA_VERSION: u32 = 1;

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

                let calls_result = adapter.parse(&source, &mut seen);
                let limits_result = adapter.parse_limits(&source);
                let parsed_calls_ok = calls_result.is_ok();
                if !parsed_calls_ok && limits_result.is_err() {
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
                if parsed_calls_ok {
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
                speed, dedup_key, user_message, session_id, project
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
    let inserted = tx.execute(
        "
        INSERT OR IGNORE INTO calls (
            tool, dedup_key, model, input_tokens, output_tokens,
            cache_creation_input_tokens, cache_read_input_tokens,
            cached_input_tokens, reasoning_tokens, web_search_requests,
            cost_usd, tools_json, bash_commands_json, timestamp, speed,
            user_message, session_id, project, imported_at
        ) VALUES (
            ?1, ?2, ?3, ?4, ?5,
            ?6, ?7, ?8, ?9, ?10,
            ?11, ?12, ?13, ?14, ?15,
            ?16, ?17, ?18, ?19
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
        ],
    )?;
    if inserted == 0 {
        update_existing_cursor_project(tx, call)?;
    }
    Ok(inserted > 0)
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
        SessionSource {
            path,
            project: "fake-project".into(),
            tool: crate::tools::codex::config::TOOL_ID,
        }
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
