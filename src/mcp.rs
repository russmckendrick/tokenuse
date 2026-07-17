//! `tokenuse mcp`: a read-only MCP (Model Context Protocol) server over
//! stdio. Newline-delimited JSON-RPC 2.0 on stdin/stdout — no network, no
//! async runtime, no new dependencies. Exposes the same summaries as
//! `tokenuse status`/`tokenuse overview` so LLM clients can reason about
//! local spend without reading usage files themselves.
//!
//! Project names are pseudonymised by default with a salted hash. The salt
//! is generated once and persisted next to the archive
//! (`ConfigPaths::mcp_salt_file`) so pseudonyms stay stable across server
//! restarts without being derivable by a client that has never seen the
//! real names. `--real-names` disables the mapping.

use std::fs;
use std::io::{self, BufRead, Write};
use std::time::{Duration, Instant};

use color_eyre::Result;
use serde_json::{json, Map, Value};

use crate::app::{Period, ProjectFilter, SortMode, Tool};
use crate::archive;
use crate::config::{ConfigPaths, UserConfig};
use crate::copy::{copy, template};
use crate::currency::{CurrencyFormatter, CurrencyTable};
use crate::ingest::{self, Ingested, PeriodTotals};

/// Latest protocol revision this server implements; offered when the client
/// requests a revision we do not know.
const PROTOCOL_VERSION: &str = "2025-06-18";
const SUPPORTED_PROTOCOL_VERSIONS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];

/// Matches the dashboard's background refresh cadence: MCP answers may be up
/// to one refresh interval stale, never older.
const DATA_TTL: Duration = Duration::from_secs(15 * 60);

const TOOL_STATUS: &str = "status";
const TOOL_OVERVIEW: &str = "overview";
const TOOL_PROJECTS: &str = "projects";

const JSONRPC_METHOD_NOT_FOUND: i64 = -32601;
const JSONRPC_INVALID_PARAMS: i64 = -32602;
const JSONRPC_PARSE_ERROR: i64 = -32700;

pub fn run(real_names: bool) -> Result<()> {
    let paths = ConfigPaths::default();
    let salt = load_or_create_salt(&paths)?;
    let mut server = McpServer::new(real_names, salt);

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = stdout.lock();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        if let Some(response) = server.handle_line(&line) {
            let mut payload = serde_json::to_string(&response)?;
            payload.push('\n');
            out.write_all(payload.as_bytes())?;
            out.flush()?;
        }
    }
    Ok(())
}

struct CachedData {
    loaded_at: Instant,
    ingested: Ingested,
    formatter: CurrencyFormatter,
}

struct McpServer {
    real_names: bool,
    salt: String,
    cache: Option<CachedData>,
}

impl McpServer {
    fn new(real_names: bool, salt: String) -> Self {
        Self {
            real_names,
            salt,
            cache: None,
        }
    }

    fn handle_line(&mut self, line: &str) -> Option<Value> {
        let message: Value = match serde_json::from_str(line) {
            Ok(value) => value,
            Err(error) => {
                return Some(error_response(
                    Value::Null,
                    JSONRPC_PARSE_ERROR,
                    &error.to_string(),
                ));
            }
        };
        let id = message.get("id").cloned();
        let method = message.get("method").and_then(Value::as_str).unwrap_or("");
        let params = message.get("params").cloned().unwrap_or(Value::Null);
        self.handle_request(id, method, &params)
    }

    fn handle_request(&mut self, id: Option<Value>, method: &str, params: &Value) -> Option<Value> {
        match method {
            "initialize" => {
                let requested = params
                    .get("protocolVersion")
                    .and_then(Value::as_str)
                    .unwrap_or(PROTOCOL_VERSION);
                let negotiated = if SUPPORTED_PROTOCOL_VERSIONS.contains(&requested) {
                    requested
                } else {
                    PROTOCOL_VERSION
                };
                Some(result_response(
                    id?,
                    json!({
                        "protocolVersion": negotiated,
                        "capabilities": { "tools": {} },
                        "serverInfo": {
                            "name": env!("CARGO_PKG_NAME"),
                            "title": copy().brand.name,
                            "version": env!("CARGO_PKG_VERSION"),
                        },
                        "instructions": copy().mcp.instructions,
                    }),
                ))
            }
            "ping" => Some(result_response(id?, json!({}))),
            "tools/list" => Some(result_response(id?, json!({ "tools": tool_definitions() }))),
            "tools/call" => {
                let id = id?;
                let name = params.get("name").and_then(Value::as_str).unwrap_or("");
                match self.call_tool(name) {
                    Ok(Some(payload)) => Some(result_response(id, tool_result(&payload, false))),
                    Ok(None) => Some(error_response(
                        id,
                        JSONRPC_INVALID_PARAMS,
                        &template(&copy().mcp.unknown_tool, &[("name", name.to_string())]),
                    )),
                    Err(error) => Some(result_response(
                        id,
                        tool_result(&Value::String(error.to_string()), true),
                    )),
                }
            }
            // Notifications (initialized, cancelled, …) get no response;
            // unknown *requests* get a method-not-found error.
            _ => {
                let id = id?;
                if method.starts_with("notifications/") {
                    return None;
                }
                Some(error_response(id, JSONRPC_METHOD_NOT_FOUND, method))
            }
        }
    }

    /// `Ok(None)` = unknown tool name; `Err` = tool execution failure, which
    /// MCP reports inside a successful response with `isError: true`.
    fn call_tool(&mut self, name: &str) -> Result<Option<Value>> {
        match name {
            TOOL_STATUS => {
                let data = self.data()?;
                Ok(Some(status_payload(&data.ingested, &data.formatter)))
            }
            TOOL_OVERVIEW => {
                let data = self.data()?;
                Ok(Some(overview_payload(&data.ingested, &data.formatter)))
            }
            TOOL_PROJECTS => {
                let real_names = self.real_names;
                let salt = self.salt.clone();
                let data = self.data()?;
                Ok(Some(projects_payload(
                    &data.ingested,
                    &data.formatter,
                    real_names,
                    &salt,
                )))
            }
            _ => Ok(None),
        }
    }

    fn data(&mut self) -> Result<&CachedData> {
        let stale = self
            .cache
            .as_ref()
            .is_none_or(|cache| cache.loaded_at.elapsed() > DATA_TTL);
        if stale {
            let paths = ConfigPaths::default();
            let config = UserConfig::load(&paths).unwrap_or_default();
            let currency_table = CurrencyTable::load(&paths).unwrap_or_else(|_| {
                CurrencyTable::embedded().expect("embedded currency rates must be valid JSON")
            });
            let formatter = currency_table.formatter(&config.currency);
            let ingested = match archive::sync_and_load(&paths) {
                Ok(ingested) => ingested,
                Err(error) => {
                    // stderr is the MCP logging channel; stdout is protocol-only.
                    eprintln!(
                        "{}",
                        template(
                            &copy().cli.archive_failed_raw_ingest,
                            &[("error", error.to_string())]
                        )
                    );
                    ingest::load()?
                }
            };
            self.cache = Some(CachedData {
                loaded_at: Instant::now(),
                ingested,
                formatter,
            });
        }
        Ok(self.cache.as_ref().expect("cache populated above"))
    }
}

fn tool_definitions() -> Vec<Value> {
    let mcp_copy = &copy().mcp;
    let no_args = json!({ "type": "object", "properties": {} });
    vec![
        json!({
            "name": TOOL_STATUS,
            "description": mcp_copy.tool_status,
            "inputSchema": no_args,
        }),
        json!({
            "name": TOOL_OVERVIEW,
            "description": mcp_copy.tool_overview,
            "inputSchema": no_args,
        }),
        json!({
            "name": TOOL_PROJECTS,
            "description": mcp_copy.tool_projects,
            "inputSchema": no_args,
        }),
    ]
}

fn tool_result(payload: &Value, is_error: bool) -> Value {
    let text = match payload {
        Value::String(text) => text.clone(),
        other => serde_json::to_string_pretty(other).unwrap_or_else(|_| other.to_string()),
    };
    let mut result = json!({
        "content": [{ "type": "text", "text": text }],
        "isError": is_error,
    });
    if !is_error && !payload.is_string() {
        result["structuredContent"] = payload.clone();
    }
    result
}

fn result_response(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn error_response(id: Value, code: i64, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message },
    })
}

fn period_json(totals: PeriodTotals, formatter: &CurrencyFormatter) -> Value {
    let mut object = match serde_json::to_value(totals) {
        Ok(Value::Object(map)) => map,
        _ => Map::new(),
    };
    object.insert(
        "cost".to_string(),
        Value::String(formatter.format_money(totals.cost_usd)),
    );
    Value::Object(object)
}

fn status_payload(ingested: &Ingested, formatter: &CurrencyFormatter) -> Value {
    json!({
        "currency": formatter.code(),
        "today": period_json(ingested.period_totals(Period::Today), formatter),
        "month": period_json(ingested.period_totals(Period::Month), formatter),
    })
}

/// Month summary without project or session identifiers: totals, per-tool
/// split, activity categories, models, and the daily trend.
fn overview_payload(ingested: &Ingested, formatter: &CurrencyFormatter) -> Value {
    let period = Period::Month;
    let dashboard = ingested.dashboard(
        period,
        Tool::All,
        &ProjectFilter::All,
        SortMode::Spend,
        formatter,
    );
    let by_tool: Vec<Value> = ingested
        .tool_totals(period)
        .into_iter()
        .map(|(tool, totals)| {
            let mut object = match period_json(totals, formatter) {
                Value::Object(map) => map,
                _ => Map::new(),
            };
            object.insert("tool".to_string(), Value::String(tool.to_string()));
            Value::Object(object)
        })
        .collect();
    let mut daily: Vec<&crate::data::DailyMetric> = dashboard.daily.iter().collect();
    daily.sort_by(|a, b| a.day.cmp(b.day));
    json!({
        "period": copy().periods.month,
        "currency": formatter.code(),
        "totals": period_json(ingested.period_totals(period), formatter),
        "by_tool": by_tool,
        "by_activity": dashboard.by_activity.iter().map(|activity| json!({
            "label": activity.label,
            "cost": activity.cost,
            "calls": activity.calls,
        })).collect::<Vec<_>>(),
        "models": dashboard.models.iter().map(|model| json!({
            "name": model.name,
            "provider": model.provider_label,
            "cost": model.cost,
            "calls": model.calls,
            "cache_rate": model.cache_rate,
        })).collect::<Vec<_>>(),
        "daily": daily.iter().map(|day| json!({
            "day": day.day,
            "cost": day.cost,
            "calls": day.calls,
        })).collect::<Vec<_>>(),
    })
}

fn projects_payload(
    ingested: &Ingested,
    formatter: &CurrencyFormatter,
    real_names: bool,
    salt: &str,
) -> Value {
    let dashboard = ingested.dashboard(
        Period::Month,
        Tool::All,
        &ProjectFilter::All,
        SortMode::Spend,
        formatter,
    );
    json!({
        "period": copy().periods.month,
        "currency": formatter.code(),
        "pseudonymised": !real_names,
        "projects": dashboard.projects.iter().map(|project| json!({
            "project": if real_names {
                project.name.to_string()
            } else {
                pseudonym(salt, project.name)
            },
            "cost": project.cost,
            "sessions": project.sessions,
            "avg_per_session": project.avg_per_session,
            "tool_mix": project.tool_mix,
        })).collect::<Vec<_>>(),
    })
}

/// Salted FNV-1a: stable for a given salt file, unlinkable without it.
fn pseudonym(salt: &str, name: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in salt
        .bytes()
        .chain(std::iter::once(0xff))
        .chain(name.bytes())
    {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("project-{:012x}", hash & 0xffff_ffff_ffff)
}

fn load_or_create_salt(paths: &ConfigPaths) -> Result<String> {
    if let Ok(existing) = fs::read_to_string(&paths.mcp_salt_file) {
        let trimmed = existing.trim();
        if !trimmed.is_empty() {
            return Ok(trimmed.to_string());
        }
    }
    paths.ensure_dir()?;
    let salt = generate_salt();
    fs::write(&paths.mcp_salt_file, &salt)?;
    Ok(salt)
}

/// OS-seeded without a rand dependency: each `RandomState` draws fresh
/// process entropy. Pseudonyms need unpredictability, not crypto strength.
fn generate_salt() -> String {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};

    let mut salt = String::with_capacity(64);
    for round in 0..4u64 {
        let mut hasher = RandomState::new().build_hasher();
        hasher.write_u64(round);
        salt.push_str(&format!("{:016x}", hasher.finish()));
    }
    salt
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server() -> McpServer {
        McpServer::new(false, "test-salt".to_string())
    }

    #[test]
    fn initialize_negotiates_known_protocol_version() {
        let response = server()
            .handle_line(
                r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26"}}"#,
            )
            .expect("initialize responds");
        assert_eq!(response["result"]["protocolVersion"], "2025-03-26");
        assert_eq!(response["result"]["serverInfo"]["name"], "tokenuse");
        assert!(response["result"]["capabilities"]["tools"].is_object());
    }

    #[test]
    fn initialize_falls_back_on_unknown_protocol_version() {
        let response = server()
            .handle_line(
                r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"1999-01-01"}}"#,
            )
            .expect("initialize responds");
        assert_eq!(response["result"]["protocolVersion"], PROTOCOL_VERSION);
    }

    #[test]
    fn tools_list_exposes_the_three_read_tools() {
        let response = server()
            .handle_line(r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#)
            .expect("tools/list responds");
        let tools = response["result"]["tools"].as_array().expect("tools array");
        let names: Vec<&str> = tools
            .iter()
            .filter_map(|tool| tool["name"].as_str())
            .collect();
        assert_eq!(names, vec![TOOL_STATUS, TOOL_OVERVIEW, TOOL_PROJECTS]);
        assert!(tools
            .iter()
            .all(|tool| tool["description"].as_str().is_some_and(|d| !d.is_empty())));
    }

    #[test]
    fn notifications_get_no_response() {
        assert!(server()
            .handle_line(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#)
            .is_none());
    }

    #[test]
    fn unknown_method_returns_method_not_found() {
        let response = server()
            .handle_line(r#"{"jsonrpc":"2.0","id":3,"method":"resources/list"}"#)
            .expect("unknown request errors");
        assert_eq!(response["error"]["code"], JSONRPC_METHOD_NOT_FOUND);
    }

    #[test]
    fn unknown_tool_returns_invalid_params() {
        let response = server()
            .handle_line(
                r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"nope"}}"#,
            )
            .expect("unknown tool errors");
        assert_eq!(response["error"]["code"], JSONRPC_INVALID_PARAMS);
    }

    #[test]
    fn parse_error_returns_null_id_error() {
        let response = server()
            .handle_line("{not json")
            .expect("parse error responds");
        assert_eq!(response["error"]["code"], JSONRPC_PARSE_ERROR);
        assert!(response["id"].is_null());
    }

    #[test]
    fn ping_returns_empty_result() {
        let response = server()
            .handle_line(r#"{"jsonrpc":"2.0","id":5,"method":"ping"}"#)
            .expect("ping responds");
        assert_eq!(response["result"], json!({}));
    }

    #[test]
    fn pseudonyms_are_stable_and_salt_dependent() {
        assert_eq!(pseudonym("a", "tokens"), pseudonym("a", "tokens"));
        assert_ne!(pseudonym("a", "tokens"), pseudonym("b", "tokens"));
        assert_ne!(pseudonym("a", "tokens"), pseudonym("a", "other"));
        assert!(pseudonym("a", "tokens").starts_with("project-"));
    }

    #[test]
    fn generated_salt_is_long_and_random() {
        let first = generate_salt();
        let second = generate_salt();
        assert_eq!(first.len(), 64);
        assert_ne!(first, second);
    }

    #[test]
    fn salt_round_trips_through_the_config_dir() {
        let dir = std::env::temp_dir().join(format!(
            "tokenuse-mcp-salt-{}",
            crate::tools::paths::test_run_id()
        ));
        let paths = ConfigPaths::new(dir.clone());
        let first = load_or_create_salt(&paths).expect("create salt");
        let second = load_or_create_salt(&paths).expect("reload salt");
        assert_eq!(first, second);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn tool_result_wraps_structured_payloads() {
        let payload = json!({ "cost": "$1.00" });
        let result = tool_result(&payload, false);
        assert_eq!(result["isError"], false);
        assert_eq!(result["structuredContent"], payload);
        assert!(result["content"][0]["text"]
            .as_str()
            .is_some_and(|text| text.contains("$1.00")));
    }
}
