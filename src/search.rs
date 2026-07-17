//! Scrollback's search core: full-text queries over the archive's transcript
//! store. Every query opens its own read-only connection so callers on any
//! thread (TUI key handler, Tauri command, MCP server) never contend with the
//! sync writer's connection, and search never mutates the archive.

use color_eyre::Result;
use rusqlite::{params, types::Value as SqlValue, Connection, OpenFlags};
use serde::Serialize;

use crate::config::ConfigPaths;

/// Sessions returned when the caller does not ask for a specific limit.
pub const DEFAULT_SESSION_LIMIT: usize = 20;
/// Hard cap on sessions per query; keeps the per-session snippet loop bounded.
pub const MAX_SESSION_LIMIT: usize = 50;
/// Best-ranked matching turns quoted per session.
pub const SNIPPETS_PER_SESSION: usize = 3;

/// Private-use sentinels wrapped around match terms by `snippet()`; surfaces
/// split on them to render native highlights (TUI spans, desktop `<mark>`).
const HIT_START: char = '\u{E000}';
const HIT_END: char = '\u{E001}';
/// Approximate tokens of context `snippet()` keeps around a match.
const SNIPPET_TOKENS: u32 = 12;
/// bm25 column weights: user prompts count double so prose outranks
/// assistant output (which skews toward code and tool chatter).
const BM25_WEIGHTS: &str = "2.0, 1.0";

#[derive(Debug, Clone, Default)]
pub struct SearchFilters {
    /// Exact match against the archived project identity.
    pub project: Option<String>,
    /// Tool id as archived, e.g. "claude-code".
    pub tool: Option<String>,
    /// Clamped to `1..=MAX_SESSION_LIMIT`; 0 means `DEFAULT_SESSION_LIMIT`.
    pub session_limit: usize,
}

#[derive(Debug, Clone, Serialize, Default, PartialEq)]
pub struct SearchResults {
    pub query: String,
    /// Matching sessions before the limit was applied.
    pub total_sessions: u64,
    pub sessions: Vec<SessionSearchResult>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SessionSearchResult {
    /// `"{tool}:{session_id}"` — the same key the session drill-downs use.
    pub key: String,
    pub session_id: String,
    pub tool: String,
    pub project: String,
    pub last_timestamp: Option<String>,
    pub cost_usd: f64,
    /// Display form of `cost_usd`; empty until `format_costs` runs with the
    /// caller's currency.
    pub cost: String,
    pub match_count: u64,
    /// True when every matching row is a migration-seeded 500-char prompt
    /// fallback (the session's source files vanished before full capture).
    pub prompt_only: bool,
    pub matches: Vec<SearchMatch>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SearchMatch {
    pub dedup_key: String,
    pub timestamp: Option<String>,
    pub role: SnippetRole,
    pub snippet: Vec<SnippetPart>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SnippetRole {
    User,
    Assistant,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SnippetPart {
    pub text: String,
    pub hit: bool,
}

/// Search the transcript index. Returns empty results (never an error) when
/// the archive or its transcript tables do not exist yet, or when the query
/// contains no searchable tokens.
pub fn search_transcripts(
    paths: &ConfigPaths,
    query: &str,
    filters: &SearchFilters,
) -> Result<SearchResults> {
    let empty = || SearchResults {
        query: query.to_string(),
        ..SearchResults::default()
    };
    let Some(match_expr) = fts_query(query) else {
        return Ok(empty());
    };
    let Ok(conn) = Connection::open_with_flags(
        &paths.archive_db_file,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) else {
        return Ok(empty());
    };
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    if !transcripts_table_exists(&conn)? {
        return Ok(empty());
    }

    let limit = match filters.session_limit {
        0 => DEFAULT_SESSION_LIMIT,
        n => n.min(MAX_SESSION_LIMIT),
    };

    // The project filter arrives as a project identity (the drill-down
    // grouping key); the archive stores raw per-call project strings, so
    // expand the identity into every raw form it groups.
    let project_variants = match &filters.project {
        None => None,
        Some(filter) => Some(matching_projects(&conn, filter)?),
    };
    if matches!(&project_variants, Some(variants) if variants.is_empty()) {
        return Ok(empty());
    }

    // Shared filter fragment + bind values for both queries below; `?`
    // placeholders bind in order of appearance.
    let mut filter_sql = String::new();
    let mut filter_params: Vec<SqlValue> = Vec::new();
    if let Some(tool) = &filters.tool {
        filter_sql.push_str(" AND t.tool = ?");
        filter_params.push(SqlValue::Text(tool.clone()));
    }
    if let Some(variants) = &project_variants {
        let placeholders = vec!["?"; variants.len()].join(", ");
        filter_sql.push_str(&format!(" AND t.project IN ({placeholders})"));
        filter_params.extend(variants.iter().cloned().map(SqlValue::Text));
    }

    let count_params: Vec<SqlValue> = std::iter::once(SqlValue::Text(match_expr.clone()))
        .chain(filter_params.iter().cloned())
        .collect();
    let total_sessions: u64 = conn.query_row(
        &format!(
            "
            SELECT COUNT(*) FROM (
                SELECT 1
                FROM transcripts_fts
                JOIN transcripts t ON t.id = transcripts_fts.rowid
                WHERE transcripts_fts MATCH ?{filter_sql}
                GROUP BY t.tool, t.session_id
            )
            "
        ),
        rusqlite::params_from_iter(count_params),
        |row| row.get::<_, i64>(0),
    )? as u64;
    if total_sessions == 0 {
        return Ok(empty());
    }

    // bm25() is an FTS5 auxiliary function and only resolves in a SELECT
    // whose FROM is the FTS table itself. The MATERIALIZED hint stops the
    // query flattener folding that SELECT into the outer join, which would
    // recreate the "unable to use function bm25" context error.
    let mut stmt = conn.prepare(&format!(
        "
        WITH m AS MATERIALIZED (
            SELECT rowid AS rid, bm25(transcripts_fts, {BM25_WEIGHTS}) AS rank
            FROM transcripts_fts
            WHERE transcripts_fts MATCH ?
        )
        SELECT t.tool, t.session_id, t.project,
               MAX(t.timestamp)                                   AS last_ts,
               COUNT(*)                                           AS match_count,
               MIN(m.rank)                                        AS best_rank,
               MIN(CASE WHEN t.origin = 'full' THEN 0 ELSE 1 END) AS prompt_only
        FROM m
        JOIN transcripts t ON t.id = m.rid
        WHERE 1 = 1{filter_sql}
        GROUP BY t.tool, t.session_id
        ORDER BY best_rank, last_ts DESC, t.tool, t.session_id
        LIMIT ?
        "
    ))?;
    let grouped_params: Vec<SqlValue> = std::iter::once(SqlValue::Text(match_expr.clone()))
        .chain(filter_params.iter().cloned())
        .chain(std::iter::once(SqlValue::Integer(limit as i64)))
        .collect();
    let session_rows: Vec<(String, String, String, Option<String>, u64, bool)> = stmt
        .query_map(rusqlite::params_from_iter(grouped_params), |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, i64>(4)? as u64,
                row.get::<_, i64>(6)? != 0,
            ))
        })?
        .collect::<rusqlite::Result<_>>()?;

    let mut sessions = Vec::with_capacity(session_rows.len());
    for (tool, session_id, project, last_ts, match_count, prompt_only) in session_rows {
        let matches = session_matches(
            &conn,
            &match_expr,
            &tool,
            &session_id,
            project_variants.as_deref(),
        )?;
        let cost_usd: f64 = conn.query_row(
            "SELECT COALESCE(SUM(cost_usd), 0) FROM calls WHERE tool = ?1 AND session_id = ?2",
            params![tool, session_id],
            |row| row.get(0),
        )?;
        sessions.push(SessionSearchResult {
            key: format!("{tool}:{session_id}"),
            session_id,
            tool,
            project,
            last_timestamp: last_ts,
            cost_usd,
            cost: String::new(),
            match_count,
            prompt_only,
            matches,
        });
    }

    Ok(SearchResults {
        query: query.to_string(),
        total_sessions,
        sessions,
    })
}

/// Raw archived project strings matched by a filter given as either a
/// project identity (drill-down grouping key) or an exact raw value.
fn matching_projects(conn: &Connection, filter: &str) -> Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT DISTINCT project FROM transcripts")?;
    let rows: Vec<String> = stmt
        .query_map([], |row| row.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows
        .into_iter()
        .filter(|raw| raw == filter || crate::ingest::projects::project_identity(raw) == filter)
        .collect())
}

fn transcripts_table_exists(conn: &Connection) -> Result<bool> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'transcripts_fts'",
        [],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

fn session_matches(
    conn: &Connection,
    match_expr: &str,
    tool: &str,
    session_id: &str,
    project_variants: Option<&[String]>,
) -> Result<Vec<SearchMatch>> {
    // Rows of a session can carry different raw projects (e.g. cursor
    // workspace resolution refining mid-session); snippets honour the same
    // project filter as the grouped query so quoted turns are never ones the
    // filter excluded.
    let mut project_sql = String::new();
    let mut project_params: Vec<SqlValue> = Vec::new();
    if let Some(variants) = project_variants {
        let placeholders = vec!["?"; variants.len()].join(", ");
        project_sql = format!(" AND project IN ({placeholders})");
        project_params.extend(variants.iter().cloned().map(SqlValue::Text));
    }
    // Same aux-function constraint as the session query: snippet()/bm25()
    // live in the bare-FTS subquery; the rowid IN filter keeps it scoped to
    // one session without leaving direct-query context.
    let mut stmt = conn.prepare(&format!(
        "
        SELECT t.dedup_key, t.timestamp, m.user_snip, m.assistant_snip
        FROM (
            SELECT rowid AS rid,
                   snippet(transcripts_fts, 0, ?2, ?3, '…', {SNIPPET_TOKENS}) AS user_snip,
                   snippet(transcripts_fts, 1, ?2, ?3, '…', {SNIPPET_TOKENS}) AS assistant_snip,
                   bm25(transcripts_fts, {BM25_WEIGHTS}) AS rank
            FROM transcripts_fts
            WHERE transcripts_fts MATCH ?1
              AND rowid IN (
                  SELECT id FROM transcripts
                  WHERE tool = ?4 AND session_id = ?5{project_sql}
              )
            ORDER BY rank
            LIMIT {SNIPPETS_PER_SESSION}
        ) m
        JOIN transcripts t ON t.id = m.rid
        ORDER BY m.rank
        "
    ))?;
    let query_params: Vec<SqlValue> = [
        SqlValue::Text(match_expr.to_string()),
        SqlValue::Text(HIT_START.to_string()),
        SqlValue::Text(HIT_END.to_string()),
        SqlValue::Text(tool.to_string()),
        SqlValue::Text(session_id.to_string()),
    ]
    .into_iter()
    .chain(project_params)
    .collect();
    let rows: Vec<(String, Option<String>, String, String)> = stmt
        .query_map(rusqlite::params_from_iter(query_params), |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })?
        .collect::<rusqlite::Result<_>>()?;

    Ok(rows
        .into_iter()
        .map(|(dedup_key, timestamp, user_snippet, assistant_snippet)| {
            // Prefer quoting the user's own words when both columns hit.
            let (role, raw) = if user_snippet.contains(HIT_START) {
                (SnippetRole::User, user_snippet)
            } else {
                (SnippetRole::Assistant, assistant_snippet)
            };
            SearchMatch {
                dedup_key,
                timestamp,
                role,
                snippet: snippet_parts(&raw),
            }
        })
        .collect())
}

/// Split `snippet()` output on the sentinel pair into renderable spans.
fn snippet_parts(raw: &str) -> Vec<SnippetPart> {
    let mut parts = Vec::new();
    let mut rest = raw;
    loop {
        let Some(hit_at) = rest.find(HIT_START) else {
            if !rest.is_empty() {
                parts.push(SnippetPart {
                    text: rest.to_string(),
                    hit: false,
                });
            }
            break;
        };
        if hit_at > 0 {
            parts.push(SnippetPart {
                text: rest[..hit_at].to_string(),
                hit: false,
            });
        }
        rest = &rest[hit_at + HIT_START.len_utf8()..];
        let Some(end_at) = rest.find(HIT_END) else {
            // Unbalanced sentinel (should not happen): keep the tail visible.
            if !rest.is_empty() {
                parts.push(SnippetPart {
                    text: rest.to_string(),
                    hit: false,
                });
            }
            break;
        };
        parts.push(SnippetPart {
            text: rest[..end_at].to_string(),
            hit: true,
        });
        rest = &rest[end_at + HIT_END.len_utf8()..];
    }
    parts
}

/// Plain-text form of a snippet for surfaces without highlighting (MCP).
pub fn snippet_text(snippet: &[SnippetPart]) -> String {
    snippet.iter().map(|part| part.text.as_str()).collect()
}

impl SearchResults {
    /// Fill each session's display `cost` in the caller's currency.
    pub fn format_costs(&mut self, formatter: &crate::currency::CurrencyFormatter) {
        for session in &mut self.sessions {
            session.cost = formatter.format_money(session.cost_usd);
        }
    }
}

/// Turn raw user input into a safe FTS5 MATCH expression: every
/// whitespace-separated token becomes a quoted phrase (neutralising MATCH
/// operators like `-`, `OR`, `NEAR(`), the final token matches by prefix, and
/// tokens are ANDed. `None` when nothing searchable remains.
fn fts_query(raw: &str) -> Option<String> {
    let tokens: Vec<String> = raw
        .split_whitespace()
        .map(|token| token.replace('"', "\"\""))
        .filter(|token| !token.is_empty())
        .collect();
    if tokens.is_empty() {
        return None;
    }
    // A token that was only quote characters doubles into quote pairs that
    // FTS5 reads as an empty phrase; drop those too.
    let tokens: Vec<&String> = tokens
        .iter()
        .filter(|token| token.chars().any(|c| c != '"'))
        .collect();
    if tokens.is_empty() {
        return None;
    }
    let last = tokens.len() - 1;
    Some(
        tokens
            .iter()
            .enumerate()
            .map(|(idx, token)| {
                if idx == last {
                    format!("\"{token}\"*")
                } else {
                    format!("\"{token}\"")
                }
            })
            .collect::<Vec<_>>()
            .join(" "),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::Archive;
    use crate::ingest::Ingested;
    use crate::tools::ParsedCall;
    use chrono::{TimeZone, Utc};

    fn temp_paths(name: &str) -> ConfigPaths {
        let unique = format!(
            "tokenuse-search-test-{}-{}",
            name,
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        );
        ConfigPaths::new(std::env::temp_dir().join(unique))
    }

    fn call(
        tool: &'static str,
        session: &str,
        key: &str,
        user: &str,
        assistant: &str,
    ) -> ParsedCall {
        ParsedCall {
            tool,
            model: "test-model".into(),
            cost_usd: 0.5,
            timestamp: Some(Utc.with_ymd_and_hms(2026, 7, 17, 12, 0, 0).unwrap()),
            dedup_key: key.into(),
            user_message: crate::tools::jsonl::truncate_chars(user, 500),
            session_id: session.into(),
            project: "/tmp/project-a".into(),
            transcript_user: Some(user.into()),
            transcript_assistant: Some(assistant.into()),
            ..ParsedCall::default()
        }
    }

    fn seeded_paths(name: &str) -> ConfigPaths {
        let paths = temp_paths(name);
        let mut archive = Archive::open(&paths).unwrap();
        let mut other_project = call(
            "codex",
            "sess-b",
            "b1",
            "teach me about lifetimes",
            "Lifetimes describe how long the references live.",
        );
        other_project.project = "/tmp/project-b".into();
        archive
            .insert_ingested(&Ingested {
                calls: vec![
                    call(
                        "claude-code",
                        "sess-a",
                        "a1",
                        "please fix the flaky retry test",
                        "The retry loop was missing a backoff cap; fixed.",
                    ),
                    call(
                        "claude-code",
                        "sess-a",
                        "a2",
                        "now add coverage for the backoff cap",
                        "Added a regression test for the backoff cap.",
                    ),
                    other_project,
                ],
                limits: Vec::new(),
            })
            .unwrap();
        paths
    }

    #[test]
    fn missing_archive_returns_empty_results() {
        let paths = temp_paths("missing");
        let results = search_transcripts(&paths, "anything", &SearchFilters::default()).unwrap();
        assert_eq!(
            results,
            SearchResults {
                query: "anything".into(),
                ..SearchResults::default()
            },
            "empty results still echo the submitted query"
        );
    }

    #[test]
    fn groups_matches_by_session_with_snippets() {
        let paths = seeded_paths("grouping");
        let results = search_transcripts(&paths, "backoff cap", &SearchFilters::default()).unwrap();
        assert_eq!(results.total_sessions, 1);
        assert_eq!(results.sessions.len(), 1);
        let session = &results.sessions[0];
        assert_eq!(session.key, "claude-code:sess-a");
        assert_eq!(session.match_count, 2);
        assert!(!session.prompt_only);
        assert!((session.cost_usd - 1.0).abs() < 1e-9, "sums both calls");
        assert!(!session.matches.is_empty());
        let hit_text: String = session.matches[0]
            .snippet
            .iter()
            .filter(|part| part.hit)
            .map(|part| part.text.as_str())
            .collect();
        assert!(hit_text.to_lowercase().contains("backoff"));
        let _ = std::fs::remove_dir_all(paths.dir);
    }

    #[test]
    fn tool_and_project_filters_narrow_results() {
        let paths = seeded_paths("filters");
        let all = search_transcripts(&paths, "the", &SearchFilters::default()).unwrap();
        assert_eq!(all.total_sessions, 2);

        let codex_only = search_transcripts(
            &paths,
            "the",
            &SearchFilters {
                tool: Some("codex".into()),
                ..SearchFilters::default()
            },
        )
        .unwrap();
        assert_eq!(codex_only.total_sessions, 1);
        assert_eq!(codex_only.sessions[0].tool, "codex");

        let project_a = search_transcripts(
            &paths,
            "the",
            &SearchFilters {
                project: Some("/tmp/project-a".into()),
                ..SearchFilters::default()
            },
        )
        .unwrap();
        assert_eq!(project_a.total_sessions, 1);
        assert_eq!(project_a.sessions[0].project, "/tmp/project-a");
        let _ = std::fs::remove_dir_all(paths.dir);
    }

    #[test]
    fn prefix_matching_covers_partial_final_token() {
        let paths = seeded_paths("prefix");
        let results = search_transcripts(&paths, "lifeti", &SearchFilters::default()).unwrap();
        assert_eq!(results.total_sessions, 1);
        assert_eq!(results.sessions[0].tool, "codex");
        let _ = std::fs::remove_dir_all(paths.dir);
    }

    #[test]
    fn user_hits_outrank_and_label_the_snippet_role() {
        let paths = seeded_paths("roles");
        let results = search_transcripts(&paths, "flaky", &SearchFilters::default()).unwrap();
        assert_eq!(results.sessions[0].matches[0].role, SnippetRole::User);
        let results = search_transcripts(&paths, "regression", &SearchFilters::default()).unwrap();
        assert_eq!(results.sessions[0].matches[0].role, SnippetRole::Assistant);
        let _ = std::fs::remove_dir_all(paths.dir);
    }

    #[test]
    fn session_limit_is_clamped_and_total_reported() {
        let paths = temp_paths("limit");
        let mut archive = Archive::open(&paths).unwrap();
        let calls = (0..5)
            .map(|i| {
                call(
                    "codex",
                    &format!("sess-{i}"),
                    &format!("k{i}"),
                    "shared needle phrase",
                    "reply",
                )
            })
            .collect();
        archive
            .insert_ingested(&Ingested {
                calls,
                limits: Vec::new(),
            })
            .unwrap();

        let limited = search_transcripts(
            &paths,
            "needle",
            &SearchFilters {
                session_limit: 2,
                ..SearchFilters::default()
            },
        )
        .unwrap();
        assert_eq!(limited.total_sessions, 5);
        assert_eq!(limited.sessions.len(), 2);
        let _ = std::fs::remove_dir_all(paths.dir);
    }

    #[test]
    fn prompt_only_flags_migration_seeded_sessions() {
        let paths = temp_paths("prompt-only");
        let _archive = Archive::open(&paths).unwrap();
        Connection::open(&paths.archive_db_file)
            .unwrap()
            .execute(
                "INSERT INTO transcripts (tool, dedup_key, session_id, project, user_text, origin)
                 VALUES ('codex', 'old1', 'sess-old', '/tmp/project-a', 'ancient prompt words', 'prompt')",
                [],
            )
            .unwrap();
        let results = search_transcripts(&paths, "ancient", &SearchFilters::default()).unwrap();
        assert_eq!(results.total_sessions, 1);
        assert!(results.sessions[0].prompt_only);
        let _ = std::fs::remove_dir_all(paths.dir);
    }

    #[test]
    fn fts_query_sanitises_operators_and_quotes() {
        assert_eq!(fts_query("hello"), Some("\"hello\"*".into()));
        assert_eq!(
            fts_query("hello world"),
            Some("\"hello\" \"world\"*".into())
        );
        assert_eq!(
            fts_query("he said \"do it\""),
            Some("\"he\" \"said\" \"\"\"do\" \"it\"\"\"*".into())
        );
        assert_eq!(fts_query("-flag"), Some("\"-flag\"*".into()));
        assert_eq!(fts_query("OR"), Some("\"OR\"*".into()));
        assert_eq!(fts_query("NEAR(x"), Some("\"NEAR(x\"*".into()));
        assert_eq!(fts_query("don't"), Some("\"don't\"*".into()));
        assert_eq!(fts_query("café"), Some("\"café\"*".into()));
        assert_eq!(fts_query("\""), None);
        assert_eq!(fts_query("   "), None);
        assert_eq!(fts_query(""), None);
    }

    #[test]
    fn hostile_queries_never_error() {
        let paths = seeded_paths("hostile");
        for query in [
            "he said \"do it\"",
            "-flag",
            "OR",
            "NEAR(x",
            "don't",
            "café",
            "\"",
            "a AND b OR c",
            "col:value",
            "*",
            "(((",
        ] {
            let result = search_transcripts(&paths, query, &SearchFilters::default());
            assert!(result.is_ok(), "query {query:?} must not error");
        }
        let _ = std::fs::remove_dir_all(paths.dir);
    }

    #[test]
    fn snippet_parts_split_on_sentinels() {
        let raw = format!("before {HIT_START}match{HIT_END} after");
        let parts = snippet_parts(&raw);
        assert_eq!(parts.len(), 3);
        assert!(!parts[0].hit);
        assert_eq!(parts[1].text, "match");
        assert!(parts[1].hit);
        assert_eq!(snippet_text(&parts), "before match after");
    }
}
