use std::fs;
use std::path::Path;

use chrono::{DateTime, Utc};
use color_eyre::{
    eyre::{eyre, Context},
    Result,
};
use serde::Deserialize;
#[cfg(feature = "quota-sync")]
use serde_json::Value;

use super::config;
use crate::tools::{LimitCredits, LimitSnapshot, LimitWindow, SessionSource};

const FIVE_HOUR_WINDOW_MINUTES: u64 = 300;
const SEVEN_DAY_WINDOW_MINUTES: u64 = 10_080;
const MONTHLY_WINDOW_MINUTES: u64 = 43_200;
// Limit ID shared with `claude_code/limits.rs` so the freshest observation of
// the 5-hour / 7-day windows wins via `limit_is_newer` instead of duplicating.
const CLAUDE_CODE_LIMIT_ID: &str = "claude-code";

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Sidecar {
    Wrapped {
        observed_at: Option<DateTime<Utc>>,
        #[serde(default)]
        #[allow(dead_code)]
        organization_uuid: Option<String>,
        #[serde(default)]
        #[allow(dead_code)]
        organization_name: Option<String>,
        usage: ClaudeUsage,
        #[serde(default)]
        overage: Option<ClaudeOverage>,
    },
    Raw(ClaudeUsage),
}

#[derive(Debug, Deserialize)]
struct ClaudeUsage {
    #[serde(default)]
    five_hour: Option<ClaudeLimit>,
    #[serde(default)]
    seven_day: Option<ClaudeLimit>,
    #[serde(default)]
    seven_day_opus: Option<ClaudeLimit>,
    #[serde(default)]
    seven_day_sonnet: Option<ClaudeLimit>,
}

#[derive(Debug, Deserialize)]
struct ClaudeLimit {
    utilization: Option<f64>,
    resets_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ClaudeOverage {
    #[serde(default)]
    is_enabled: Option<bool>,
    #[serde(default)]
    monthly_limit: Option<i64>,
    #[serde(default)]
    monthly_credit_limit: Option<i64>,
    #[serde(default)]
    used_credits: Option<f64>,
    #[serde(default)]
    out_of_credits: Option<bool>,
    #[serde(default)]
    #[allow(dead_code)]
    currency: Option<String>,
}

pub fn parse_sidecar(source: &SessionSource) -> Result<Vec<LimitSnapshot>> {
    if source.tool != config::TOOL_ID {
        return Err(eyre!("Claude subscription sidecar had wrong tool id"));
    }

    let raw = fs::read_to_string(&source.path)
        .wrap_err_with(|| format!("read {}", source.path.display()))?;
    let fallback_observed_at = fs::metadata(&source.path)
        .ok()
        .and_then(|metadata| metadata.modified().ok())
        .map(DateTime::<Utc>::from);
    parse_sidecar_str(&raw, fallback_observed_at)
}

fn parse_sidecar_str(
    raw: &str,
    fallback_observed_at: Option<DateTime<Utc>>,
) -> Result<Vec<LimitSnapshot>> {
    let sidecar: Sidecar =
        serde_json::from_str(raw).map_err(|e| eyre!("parse Claude subscription sidecar: {e}"))?;
    let (observed_at, usage, overage) = match sidecar {
        Sidecar::Wrapped {
            observed_at,
            usage,
            overage,
            ..
        } => (observed_at.or(fallback_observed_at), usage, overage),
        Sidecar::Raw(usage) => (fallback_observed_at, usage, None),
    };

    let mut rows = Vec::new();
    let five_hour = make_window(FIVE_HOUR_WINDOW_MINUTES, usage.five_hour.as_ref());
    let seven_day = make_window(SEVEN_DAY_WINDOW_MINUTES, usage.seven_day.as_ref());
    if five_hour.is_some() || seven_day.is_some() {
        // One combined snapshot, keyed by the same `limit_id` and `limit_name`
        // shape as `claude_code/limits.rs`. The pipeline's `limit_is_newer`
        // dedupes against the Claude Code statusLine sidecar so whichever
        // observation is freshest wins.
        rows.push(LimitSnapshot {
            tool: config::DISPLAY_TOOL,
            limit_id: CLAUDE_CODE_LIMIT_ID.to_string(),
            limit_name: None,
            plan_type: None,
            observed_at,
            primary: five_hour,
            secondary: seven_day,
            credits: None,
            rate_limit_reached_type: None,
        });
    }
    push_distinct_window(
        &mut rows,
        observed_at,
        "seven_day_opus",
        "Opus",
        SEVEN_DAY_WINDOW_MINUTES,
        usage.seven_day_opus.as_ref(),
    );
    push_distinct_window(
        &mut rows,
        observed_at,
        "seven_day_sonnet",
        "Sonnet",
        SEVEN_DAY_WINDOW_MINUTES,
        usage.seven_day_sonnet.as_ref(),
    );
    if let Some(overage) = overage {
        if let Some(row) = overage_row(observed_at, &overage) {
            rows.push(row);
        }
    }
    Ok(rows)
}

fn make_window(window_minutes: u64, limit: Option<&ClaudeLimit>) -> Option<LimitWindow> {
    let limit = limit?;
    let utilization = limit.utilization.unwrap_or(0.0);
    let resets_at = limit.resets_at.as_deref().and_then(parse_iso8601);
    // Skip placeholder zero entries with no reset signal (matches Usage4Claude's
    // "not yet started" filter).
    if utilization == 0.0 && resets_at.is_none() {
        return None;
    }
    Some(LimitWindow {
        used_percent: utilization.clamp(0.0, 100.0),
        window_minutes,
        resets_at,
    })
}

fn push_distinct_window(
    rows: &mut Vec<LimitSnapshot>,
    observed_at: Option<DateTime<Utc>>,
    id: &str,
    name: &str,
    window_minutes: u64,
    limit: Option<&ClaudeLimit>,
) {
    let Some(window) = make_window(window_minutes, limit) else {
        return;
    };
    rows.push(LimitSnapshot {
        tool: config::DISPLAY_TOOL,
        limit_id: id.to_string(),
        limit_name: Some(name.to_string()),
        plan_type: None,
        observed_at,
        primary: Some(window),
        secondary: None,
        credits: None,
        rate_limit_reached_type: None,
    });
}

fn overage_row(
    observed_at: Option<DateTime<Utc>>,
    overage: &ClaudeOverage,
) -> Option<LimitSnapshot> {
    let limit_cents = overage.monthly_limit.or(overage.monthly_credit_limit);
    let used_cents = overage.used_credits.unwrap_or(0.0);
    let enabled = overage
        .is_enabled
        .unwrap_or_else(|| limit_cents.map(|c| c > 0).unwrap_or(false));

    let (used_percent, balance) = match limit_cents {
        Some(limit) if limit > 0 => {
            let used_pct = (used_cents / (limit as f64) * 100.0).clamp(0.0, 100.0);
            let balance_remaining = ((limit as f64) - used_cents).max(0.0) / 100.0;
            (used_pct, Some(balance_remaining))
        }
        _ => (0.0, None),
    };

    Some(LimitSnapshot {
        tool: config::DISPLAY_TOOL,
        limit_id: "extra_usage".to_string(),
        limit_name: Some("Extra Usage".to_string()),
        plan_type: None,
        observed_at,
        primary: Some(LimitWindow {
            used_percent,
            window_minutes: MONTHLY_WINDOW_MINUTES,
            resets_at: None,
        }),
        secondary: None,
        credits: Some(LimitCredits {
            has_credits: enabled,
            unlimited: false,
            balance,
        }),
        rate_limit_reached_type: overage
            .out_of_credits
            .filter(|v| *v)
            .map(|_| "out_of_credits".to_string()),
    })
}

fn parse_iso8601(raw: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(raw)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

#[cfg(feature = "quota-sync")]
pub fn refresh_sidecar(output: &Path, session_key: &str) -> Result<usize> {
    let session_key = session_key.trim();
    if session_key.is_empty() {
        return Err(eyre!(
            "Claude session cookie not configured. Add it from the Config page."
        ));
    }

    let organization = fetch_first_organization(session_key)?;
    let usage = fetch_usage_payload(session_key, &organization.uuid)?;
    let overage = fetch_overage_payload(session_key, &organization.uuid);
    let mut rows = 0usize;
    for key in [
        "five_hour",
        "seven_day",
        "seven_day_opus",
        "seven_day_sonnet",
    ] {
        if usage.get(key).map(|v| !v.is_null()).unwrap_or(false) {
            rows += 1;
        }
    }
    if overage.as_ref().map(|v| !v.is_null()).unwrap_or(false) {
        rows += 1;
    }
    write_sidecar(output, &organization, &usage, overage.as_ref())?;
    Ok(rows)
}

#[cfg(not(feature = "quota-sync"))]
pub fn refresh_sidecar(_output: &Path, _session_key: &str) -> Result<usize> {
    Err(eyre!("Claude subscription sync unavailable in this build"))
}

#[cfg(feature = "quota-sync")]
#[derive(Debug, Deserialize)]
struct OrganizationResponse {
    uuid: String,
    #[serde(default)]
    name: Option<String>,
}

#[cfg(feature = "quota-sync")]
fn fetch_first_organization(session_key: &str) -> Result<OrganizationResponse> {
    let response = call_claude(&config::organizations_url(), session_key)?;
    let orgs: Vec<OrganizationResponse> =
        serde_json::from_str(&response).map_err(|e| eyre!("parse Claude organizations: {e}"))?;
    orgs.into_iter()
        .next()
        .ok_or_else(|| eyre!("Claude account has no organizations"))
}

#[cfg(feature = "quota-sync")]
fn fetch_usage_payload(session_key: &str, organization_uuid: &str) -> Result<Value> {
    let response = call_claude(&config::usage_url(organization_uuid), session_key)?;
    serde_json::from_str(&response).map_err(|e| eyre!("parse Claude usage payload: {e}"))
}

#[cfg(feature = "quota-sync")]
fn fetch_overage_payload(session_key: &str, organization_uuid: &str) -> Option<Value> {
    let response = call_claude_optional(&config::overage_url(organization_uuid), session_key)?;
    serde_json::from_str::<Value>(&response).ok()
}

#[cfg(feature = "quota-sync")]
fn call_claude(url: &str, session_key: &str) -> Result<String> {
    apply_claude_headers(ureq::get(url), session_key)
        .call()
        .map_err(map_ureq_error)?
        .into_string()
        .map_err(|e| eyre!("read Claude response body: {e}"))
}

#[cfg(feature = "quota-sync")]
fn call_claude_optional(url: &str, session_key: &str) -> Option<String> {
    match apply_claude_headers(ureq::get(url), session_key).call() {
        Ok(response) => response.into_string().ok(),
        Err(ureq::Error::Status(403 | 404, _)) => None,
        Err(_) => None,
    }
}

#[cfg(feature = "quota-sync")]
fn apply_claude_headers(req: ureq::Request, session_key: &str) -> ureq::Request {
    req.set("accept", "*/*")
        .set("accept-language", "en-US,en;q=0.9")
        .set("content-type", "application/json")
        .set(
            "anthropic-client-platform",
            config::ANTHROPIC_CLIENT_PLATFORM,
        )
        .set("anthropic-client-version", config::ANTHROPIC_CLIENT_VERSION)
        .set("user-agent", config::USER_AGENT)
        .set("origin", config::BASE_URL)
        .set("referer", config::REFERER)
        .set("sec-fetch-dest", "empty")
        .set("sec-fetch-mode", "cors")
        .set("sec-fetch-site", "same-origin")
        .set("cookie", &format!("sessionKey={session_key}"))
}

#[cfg(feature = "quota-sync")]
fn map_ureq_error(err: ureq::Error) -> color_eyre::Report {
    match err {
        ureq::Error::Status(401, _) => {
            eyre!("Claude session expired or unauthorized — reconfigure the session cookie")
        }
        ureq::Error::Status(403, _) => {
            eyre!("Claude request blocked (HTTP 403 — likely Cloudflare challenge)")
        }
        ureq::Error::Status(429, _) => eyre!("Claude rate limited (HTTP 429)"),
        ureq::Error::Status(code, _) => eyre!("Claude HTTP error {code}"),
        ureq::Error::Transport(t) => eyre!("Claude transport error: {t}"),
    }
}

#[cfg(feature = "quota-sync")]
fn write_sidecar(
    output: &Path,
    organization: &OrganizationResponse,
    usage: &Value,
    overage: Option<&Value>,
) -> Result<()> {
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).wrap_err_with(|| format!("create {}", parent.display()))?;
    }
    let wrapped = serde_json::json!({
        "observed_at": Utc::now().to_rfc3339(),
        "organization_uuid": organization.uuid,
        "organization_name": organization.name,
        "source_usage": config::usage_url(&organization.uuid),
        "source_overage": config::overage_url(&organization.uuid),
        "usage": usage,
        "overage": overage,
    });
    let mut pretty = serde_json::to_string_pretty(&wrapped)?;
    pretty.push('\n');
    fs::write(output, pretty).wrap_err_with(|| format!("write {}", output.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_wrapped_sidecar() {
        let raw = r#"{
          "observed_at": "2026-05-11T12:00:00Z",
          "organization_uuid": "abc-123",
          "usage": {
            "five_hour": { "utilization": 42.5, "resets_at": "2026-05-11T18:00:00Z" },
            "seven_day": { "utilization": 17.0, "resets_at": "2026-05-18T00:00:00Z" },
            "seven_day_opus": { "utilization": 0, "resets_at": null },
            "seven_day_sonnet": { "utilization": 25.0, "resets_at": "2026-05-18T00:00:00Z" }
          },
          "overage": {
            "is_enabled": true,
            "monthly_limit": 5000,
            "used_credits": 1250.0,
            "currency": "USD"
          }
        }"#;
        let rows = parse_sidecar_str(raw, None).unwrap();
        let ids: Vec<&str> = rows.iter().map(|r| r.limit_id.as_str()).collect();
        // 5h + 7d are combined under the shared "claude-code" limit id so the
        // pipeline dedupes against the Claude Code statusLine sidecar instead
        // of stacking duplicate rows.
        assert!(ids.contains(&"claude-code"));
        assert!(!ids.contains(&"five_hour"));
        assert!(!ids.contains(&"seven_day"));
        assert!(!ids.contains(&"seven_day_opus"));
        assert!(ids.contains(&"seven_day_sonnet"));
        assert!(ids.contains(&"extra_usage"));
        let combined = rows
            .iter()
            .find(|r| r.limit_id == "claude-code")
            .expect("combined Claude row");
        assert_eq!(combined.tool, "claude-code");
        assert!(combined.limit_name.is_none());
        assert_eq!(combined.primary.unwrap().window_minutes, 300);
        assert_eq!(combined.secondary.unwrap().window_minutes, 10_080);
        let sonnet = rows
            .iter()
            .find(|r| r.limit_id == "seven_day_sonnet")
            .expect("sonnet row");
        assert_eq!(sonnet.limit_name.as_deref(), Some("Sonnet"));
        let extra = rows
            .iter()
            .find(|r| r.limit_id == "extra_usage")
            .expect("extra row");
        let percent = extra.primary.unwrap().used_percent;
        assert!((percent - 25.0).abs() < 0.001);
        assert_eq!(extra.primary.unwrap().window_minutes, 43_200);
        let balance = extra.credits.as_ref().and_then(|c| c.balance).unwrap();
        assert!((balance - 37.5).abs() < 0.001);
    }
}
