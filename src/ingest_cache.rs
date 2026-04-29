use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;

use chrono::{DateTime, Utc};
use color_eyre::Result;
use serde::{Deserialize, Serialize};

use crate::ingest::Ingested;
use crate::tools::{paths, LimitCredits, LimitSnapshot, LimitWindow, ParsedCall, Speed};

const CACHE_VERSION: u32 = 1;
const CACHE_FILE: &str = "ingest-cache.json";

/// How long a cached snapshot is considered fresh. The same value drives the
/// background refresher cadence.
pub const TTL: Duration = Duration::from_secs(15 * 60);

#[derive(Serialize, Deserialize)]
struct CachedIngest {
    version: u32,
    written_at: DateTime<Utc>,
    calls: Vec<WireParsedCall>,
    limits: Vec<WireLimitSnapshot>,
}

// Wire-format mirrors of the runtime types. They use `String` for the tool ID
// so the file can round-trip without serde fighting the `&'static str` field,
// then we leak the string back to `&'static str` on read.
#[derive(Serialize, Deserialize, Default)]
struct WireParsedCall {
    tool: String,
    model: String,
    input_tokens: u64,
    output_tokens: u64,
    cache_creation_input_tokens: u64,
    cache_read_input_tokens: u64,
    cached_input_tokens: u64,
    reasoning_tokens: u64,
    web_search_requests: u64,
    cost_usd: f64,
    tools: Vec<String>,
    bash_commands: Vec<String>,
    timestamp: Option<DateTime<Utc>>,
    speed: Speed,
    dedup_key: String,
    user_message: String,
    session_id: String,
    project: String,
}

#[derive(Serialize, Deserialize)]
struct WireLimitSnapshot {
    tool: String,
    limit_id: String,
    limit_name: Option<String>,
    plan_type: Option<String>,
    observed_at: Option<DateTime<Utc>>,
    primary: Option<LimitWindow>,
    secondary: Option<LimitWindow>,
    credits: Option<LimitCredits>,
    rate_limit_reached_type: Option<String>,
}

fn leak(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

impl From<&ParsedCall> for WireParsedCall {
    fn from(c: &ParsedCall) -> Self {
        Self {
            tool: c.tool.to_string(),
            model: c.model.clone(),
            input_tokens: c.input_tokens,
            output_tokens: c.output_tokens,
            cache_creation_input_tokens: c.cache_creation_input_tokens,
            cache_read_input_tokens: c.cache_read_input_tokens,
            cached_input_tokens: c.cached_input_tokens,
            reasoning_tokens: c.reasoning_tokens,
            web_search_requests: c.web_search_requests,
            cost_usd: c.cost_usd,
            tools: c.tools.clone(),
            bash_commands: c.bash_commands.clone(),
            timestamp: c.timestamp,
            speed: c.speed,
            dedup_key: c.dedup_key.clone(),
            user_message: c.user_message.clone(),
            session_id: c.session_id.clone(),
            project: c.project.clone(),
        }
    }
}

impl From<WireParsedCall> for ParsedCall {
    fn from(w: WireParsedCall) -> Self {
        Self {
            tool: leak(w.tool),
            model: w.model,
            input_tokens: w.input_tokens,
            output_tokens: w.output_tokens,
            cache_creation_input_tokens: w.cache_creation_input_tokens,
            cache_read_input_tokens: w.cache_read_input_tokens,
            cached_input_tokens: w.cached_input_tokens,
            reasoning_tokens: w.reasoning_tokens,
            web_search_requests: w.web_search_requests,
            cost_usd: w.cost_usd,
            tools: w.tools,
            bash_commands: w.bash_commands,
            timestamp: w.timestamp,
            speed: w.speed,
            dedup_key: w.dedup_key,
            user_message: w.user_message,
            session_id: w.session_id,
            project: w.project,
        }
    }
}

impl From<&LimitSnapshot> for WireLimitSnapshot {
    fn from(l: &LimitSnapshot) -> Self {
        Self {
            tool: l.tool.to_string(),
            limit_id: l.limit_id.clone(),
            limit_name: l.limit_name.clone(),
            plan_type: l.plan_type.clone(),
            observed_at: l.observed_at,
            primary: l.primary,
            secondary: l.secondary,
            credits: l.credits.clone(),
            rate_limit_reached_type: l.rate_limit_reached_type.clone(),
        }
    }
}

impl From<WireLimitSnapshot> for LimitSnapshot {
    fn from(w: WireLimitSnapshot) -> Self {
        Self {
            tool: leak(w.tool),
            limit_id: w.limit_id,
            limit_name: w.limit_name,
            plan_type: w.plan_type,
            observed_at: w.observed_at,
            primary: w.primary,
            secondary: w.secondary,
            credits: w.credits,
            rate_limit_reached_type: w.rate_limit_reached_type,
        }
    }
}

pub struct CacheHit {
    pub ingested: Ingested,
    pub age: Duration,
    pub written_at: DateTime<Utc>,
}

pub fn path() -> Option<PathBuf> {
    paths::cache_dir().map(|d| d.join(CACHE_FILE))
}

/// Read and validate the cache. Returns `None` if the file is missing,
/// unreadable, malformed, or written by a different schema version - bad
/// caches must never block startup.
pub fn read() -> Option<CacheHit> {
    let path = path()?;
    let bytes = fs::read(&path).ok()?;
    let cached: CachedIngest = serde_json::from_slice(&bytes).ok()?;
    if cached.version != CACHE_VERSION {
        return None;
    }
    let now = Utc::now();
    let age = (now - cached.written_at)
        .to_std()
        .unwrap_or(Duration::from_secs(0));
    let calls: Vec<ParsedCall> = cached.calls.into_iter().map(Into::into).collect();
    let limits: Vec<LimitSnapshot> = cached.limits.into_iter().map(Into::into).collect();
    Some(CacheHit {
        ingested: Ingested { calls, limits },
        age,
        written_at: cached.written_at,
    })
}

/// Write the snapshot atomically (tmp file + rename). Best-effort: callers
/// should ignore errors so a read-only cache dir doesn't break startup.
pub fn write(ingested: &Ingested) -> Result<()> {
    let path = path().ok_or_else(|| color_eyre::eyre::eyre!("no cache directory available"))?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let payload = CachedIngest {
        version: CACHE_VERSION,
        written_at: Utc::now(),
        calls: ingested.calls.iter().map(WireParsedCall::from).collect(),
        limits: ingested.limits.iter().map(WireLimitSnapshot::from).collect(),
    };
    let bytes = serde_json::to_vec(&payload)?;

    let tmp = path.with_extension("json.tmp");
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(&bytes)?;
        f.sync_all()?;
    }
    fs::rename(&tmp, &path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn sample_call(tool: &'static str) -> ParsedCall {
        ParsedCall {
            tool,
            model: "claude-opus-4-7".into(),
            input_tokens: 100,
            output_tokens: 50,
            cost_usd: 0.0123,
            tools: vec!["bash".into(), "read".into()],
            bash_commands: vec!["ls".into()],
            timestamp: Some(Utc.with_ymd_and_hms(2026, 4, 29, 12, 0, 0).unwrap()),
            speed: Speed::Fast,
            dedup_key: "abc".into(),
            user_message: "hi".into(),
            session_id: "sess-1".into(),
            project: "tokens".into(),
            ..Default::default()
        }
    }

    fn sample_limit(tool: &'static str) -> LimitSnapshot {
        LimitSnapshot {
            tool,
            limit_id: "l1".into(),
            limit_name: Some("weekly".into()),
            plan_type: Some("max".into()),
            observed_at: Some(Utc.with_ymd_and_hms(2026, 4, 29, 12, 0, 0).unwrap()),
            primary: Some(LimitWindow {
                used_percent: 42.5,
                window_minutes: 10080,
                resets_at: Some(Utc.with_ymd_and_hms(2026, 5, 1, 0, 0, 0).unwrap()),
            }),
            secondary: None,
            credits: Some(LimitCredits {
                has_credits: true,
                unlimited: false,
                balance: Some(99.0),
            }),
            rate_limit_reached_type: None,
        }
    }

    #[test]
    fn parsed_call_roundtrips_through_wire_format() {
        let original = sample_call("claude-code");
        let wire = WireParsedCall::from(&original);
        let json = serde_json::to_string(&wire).unwrap();
        let restored: WireParsedCall = serde_json::from_str(&json).unwrap();
        let back: ParsedCall = restored.into();
        assert_eq!(back.tool, original.tool);
        assert_eq!(back.model, original.model);
        assert_eq!(back.input_tokens, original.input_tokens);
        assert_eq!(back.cost_usd, original.cost_usd);
        assert_eq!(back.tools, original.tools);
        assert_eq!(back.timestamp, original.timestamp);
        assert_eq!(back.speed, original.speed);
        assert_eq!(back.session_id, original.session_id);
        assert_eq!(back.project, original.project);
    }

    #[test]
    fn limit_snapshot_roundtrips_through_wire_format() {
        let original = sample_limit("cursor");
        let wire = WireLimitSnapshot::from(&original);
        let json = serde_json::to_string(&wire).unwrap();
        let restored: WireLimitSnapshot = serde_json::from_str(&json).unwrap();
        let back: LimitSnapshot = restored.into();
        assert_eq!(back, original);
    }

    #[test]
    fn cache_payload_serialises_with_version_and_timestamp() {
        let payload = CachedIngest {
            version: CACHE_VERSION,
            written_at: Utc.with_ymd_and_hms(2026, 4, 29, 12, 0, 0).unwrap(),
            calls: vec![WireParsedCall::from(&sample_call("codex"))],
            limits: vec![WireLimitSnapshot::from(&sample_limit("copilot"))],
        };
        let bytes = serde_json::to_vec(&payload).unwrap();
        let restored: CachedIngest = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(restored.version, CACHE_VERSION);
        assert_eq!(restored.calls.len(), 1);
        assert_eq!(restored.limits.len(), 1);
    }
}
