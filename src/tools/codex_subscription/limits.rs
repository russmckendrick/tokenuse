use std::fs;
use std::path::Path;

use chrono::{DateTime, Utc};
use color_eyre::{
    eyre::{eyre, Context},
    Result,
};
use serde::Deserialize;
use serde_json::Value;

use super::config;
use crate::tools::{LimitCredits, LimitSnapshot, LimitWindow, SessionSource};

const FIVE_HOUR_WINDOW_SECONDS: i64 = 18_000;
// Limit ID shared with `codex/parser.rs` so the freshest 5h/7d observation
// wins via `limit_is_newer` instead of rendering as duplicate rows.
const CODEX_LIMIT_ID: &str = "codex";

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Sidecar {
    Wrapped {
        observed_at: Option<DateTime<Utc>>,
        usage: CodexUsage,
    },
    Raw(CodexUsage),
}

#[derive(Debug, Deserialize)]
struct CodexUsage {
    #[serde(default)]
    plan_type: Option<String>,
    #[serde(default)]
    rate_limit: Option<RateLimit>,
    #[serde(default)]
    credits: Option<Credits>,
    #[serde(default)]
    spend_control: Option<SpendControl>,
}

#[derive(Debug, Deserialize)]
struct RateLimit {
    #[serde(default)]
    primary_window: Option<Window>,
    #[serde(default)]
    secondary_window: Option<Window>,
    #[serde(default)]
    limit_reached: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct Window {
    #[serde(default)]
    used_percent: Option<f64>,
    #[serde(default)]
    limit_window_seconds: Option<i64>,
    #[serde(default)]
    reset_after_seconds: Option<i64>,
    #[serde(default)]
    reset_at: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct Credits {
    #[serde(default)]
    has_credits: Option<bool>,
    #[serde(default)]
    unlimited: Option<bool>,
    #[serde(default)]
    overage_limit_reached: Option<bool>,
    #[serde(default)]
    balance: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct SpendControl {
    #[serde(default)]
    reached: Option<bool>,
}

pub fn parse_sidecar(source: &SessionSource) -> Result<Vec<LimitSnapshot>> {
    if source.tool != config::TOOL_ID {
        return Err(eyre!("Codex subscription sidecar had wrong tool id"));
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
        serde_json::from_str(raw).map_err(|e| eyre!("parse Codex subscription sidecar: {e}"))?;
    let (observed_at, usage) = match sidecar {
        Sidecar::Wrapped { observed_at, usage } => (observed_at.or(fallback_observed_at), usage),
        Sidecar::Raw(usage) => (fallback_observed_at, usage),
    };

    let mut rows = Vec::new();
    if let Some(rate_limit) = usage.rate_limit.as_ref() {
        let primary = make_window(
            rate_limit.primary_window.as_ref(),
            (FIVE_HOUR_WINDOW_SECONDS as u64) / 60,
        );
        let secondary = make_window(rate_limit.secondary_window.as_ref(), 10_080);
        if primary.is_some() || secondary.is_some() {
            // Combined snapshot keyed by the same `limit_id` as
            // `codex/parser.rs` so `limit_is_newer` dedupes whichever
            // observation (local rollout or live subscription fetch) is
            // freshest, instead of stacking duplicate "Codex 5h" rows.
            rows.push(LimitSnapshot {
                tool: config::DISPLAY_TOOL,
                limit_id: CODEX_LIMIT_ID.to_string(),
                limit_name: None,
                plan_type: usage.plan_type.clone(),
                observed_at,
                primary,
                secondary,
                credits: None,
                rate_limit_reached_type: rate_limit
                    .limit_reached
                    .filter(|v| *v)
                    .map(|_| "limit_reached".to_string()),
            });
        }
    }
    if let Some(row) = credits_row(
        observed_at,
        usage.plan_type.as_deref(),
        usage.credits.as_ref(),
        usage.spend_control.as_ref(),
    ) {
        rows.push(row);
    }
    Ok(rows)
}

fn make_window(window: Option<&Window>, default_minutes: u64) -> Option<LimitWindow> {
    let window = window?;
    let used_percent = window.used_percent.unwrap_or(0.0);
    let resets_at = resolve_reset(window);
    if used_percent == 0.0 && resets_at.is_none() {
        return None;
    }
    let window_minutes = window
        .limit_window_seconds
        .filter(|s| *s > 0)
        .map(|s| (s as u64) / 60)
        .unwrap_or(default_minutes);
    Some(LimitWindow {
        used_percent: used_percent.clamp(0.0, 100.0),
        window_minutes,
        resets_at,
    })
}

fn credits_row(
    observed_at: Option<DateTime<Utc>>,
    plan_type: Option<&str>,
    credits: Option<&Credits>,
    spend_control: Option<&SpendControl>,
) -> Option<LimitSnapshot> {
    let credits = credits?;
    let has_credits = credits.has_credits.unwrap_or(false);
    let unlimited = credits.unlimited.unwrap_or(false);
    let balance = credits.balance.as_ref().and_then(parse_balance);
    let reached = credits.overage_limit_reached.unwrap_or(false)
        || spend_control.and_then(|s| s.reached).unwrap_or(false);
    if !has_credits && !unlimited && balance.is_none() && !reached {
        return None;
    }
    Some(LimitSnapshot {
        tool: config::DISPLAY_TOOL,
        limit_id: "extra_usage".to_string(),
        limit_name: Some("Extra Usage".to_string()),
        plan_type: plan_type.map(str::to_string),
        observed_at,
        primary: None,
        secondary: None,
        credits: Some(LimitCredits {
            has_credits,
            unlimited,
            balance,
        }),
        rate_limit_reached_type: reached.then(|| "out_of_credits".to_string()),
    })
}

fn parse_balance(value: &Value) -> Option<f64> {
    match value {
        Value::String(s) => s.trim().parse::<f64>().ok(),
        Value::Number(n) => n.as_f64(),
        _ => None,
    }
}

fn resolve_reset(window: &Window) -> Option<DateTime<Utc>> {
    if let Some(reset_at) = window.reset_at {
        return DateTime::<Utc>::from_timestamp(reset_at, 0);
    }
    let secs = window.reset_after_seconds?;
    if secs <= 0 {
        return None;
    }
    Some(Utc::now() + chrono::Duration::seconds(secs))
}

#[cfg(feature = "quota-sync")]
pub fn refresh_sidecar(output: &Path, session_token: &str) -> Result<usize> {
    let session_token = session_token.trim();
    if session_token.is_empty() {
        return Err(eyre!(
            "Codex session-token cookie not configured. Add it from the Config page."
        ));
    }
    let access_token = fetch_access_token(session_token)?;
    let usage = fetch_usage(&access_token)?;
    let mut rows = 0usize;
    let rate_limit = usage.get("rate_limit");
    if rate_limit
        .and_then(|v| v.get("primary_window"))
        .map(|v| !v.is_null())
        .unwrap_or(false)
    {
        rows += 1;
    }
    if rate_limit
        .and_then(|v| v.get("secondary_window"))
        .map(|v| !v.is_null())
        .unwrap_or(false)
    {
        rows += 1;
    }
    if usage.get("credits").map(|v| !v.is_null()).unwrap_or(false) {
        rows += 1;
    }
    write_sidecar(output, &usage)?;
    Ok(rows)
}

#[cfg(not(feature = "quota-sync"))]
pub fn refresh_sidecar(_output: &Path, _session_token: &str) -> Result<usize> {
    Err(eyre!("Codex subscription sync unavailable in this build"))
}

#[cfg(feature = "quota-sync")]
fn fetch_access_token(session_token: &str) -> Result<String> {
    let raw = ureq::get(&config::auth_session_url())
        .set("accept", "*/*")
        .set("user-agent", config::USER_AGENT)
        .set("referer", config::REFERER)
        .set("sec-fetch-dest", "empty")
        .set("sec-fetch-mode", "cors")
        .set("sec-fetch-site", "same-origin")
        .set(
            "cookie",
            &format!("__Secure-next-auth.session-token={session_token}"),
        )
        .call()
        .map_err(map_ureq_error)?
        .into_string()
        .map_err(|e| eyre!("read Codex auth response: {e}"))?;
    let value: Value =
        serde_json::from_str(&raw).map_err(|e| eyre!("parse Codex auth response: {e}"))?;
    value
        .get("accessToken")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .ok_or_else(|| eyre!("Codex auth response missing accessToken"))
}

#[cfg(feature = "quota-sync")]
fn fetch_usage(access_token: &str) -> Result<Value> {
    let raw = ureq::get(&config::usage_url())
        .set("accept", "*/*")
        .set("authorization", &format!("Bearer {access_token}"))
        .set("user-agent", config::USER_AGENT)
        .set("referer", config::REFERER)
        .set("sec-fetch-dest", "empty")
        .set("sec-fetch-mode", "cors")
        .set("sec-fetch-site", "same-origin")
        .call()
        .map_err(map_ureq_error)?
        .into_string()
        .map_err(|e| eyre!("read Codex usage response: {e}"))?;
    serde_json::from_str(&raw).map_err(|e| eyre!("parse Codex usage response: {e}"))
}

#[cfg(feature = "quota-sync")]
fn map_ureq_error(err: ureq::Error) -> color_eyre::Report {
    match err {
        ureq::Error::Status(401, _) => {
            eyre!("Codex session expired or unauthorized — reconfigure the session-token cookie")
        }
        ureq::Error::Status(403, _) => {
            eyre!("Codex request blocked (HTTP 403 — likely Cloudflare challenge)")
        }
        ureq::Error::Status(429, _) => eyre!("Codex rate limited (HTTP 429)"),
        ureq::Error::Status(code, _) => eyre!("Codex HTTP error {code}"),
        ureq::Error::Transport(t) => eyre!("Codex transport error: {t}"),
    }
}

#[cfg(feature = "quota-sync")]
fn write_sidecar(output: &Path, usage: &Value) -> Result<()> {
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).wrap_err_with(|| format!("create {}", parent.display()))?;
    }
    let wrapped = serde_json::json!({
        "observed_at": Utc::now().to_rfc3339(),
        "source": config::usage_url(),
        "usage": usage,
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
    fn parses_wrapped_codex_sidecar() {
        let raw = r#"{
          "observed_at": "2026-05-11T12:00:00Z",
          "usage": {
            "plan_type": "plus",
            "rate_limit": {
              "limit_reached": false,
              "primary_window": {
                "used_percent": 33.0,
                "limit_window_seconds": 18000,
                "reset_after_seconds": 7200
              },
              "secondary_window": {
                "used_percent": 12.0,
                "limit_window_seconds": 604800,
                "reset_after_seconds": 432000
              }
            },
            "credits": {
              "has_credits": true,
              "unlimited": false,
              "balance": "45.25"
            },
            "spend_control": { "reached": false }
          }
        }"#;
        let rows = parse_sidecar_str(raw, None).unwrap();
        let ids: Vec<&str> = rows.iter().map(|r| r.limit_id.as_str()).collect();
        // 5h + 7d are combined under the shared "codex" limit id so the
        // pipeline dedupes against the Codex rollout parser instead of
        // stacking duplicate "Codex 5h" / "Codex weekly" rows.
        assert!(ids.contains(&"codex"));
        assert!(!ids.contains(&"primary"));
        assert!(!ids.contains(&"secondary"));
        assert!(ids.contains(&"extra_usage"));
        let combined = rows
            .iter()
            .find(|r| r.limit_id == "codex")
            .expect("combined Codex row");
        assert_eq!(combined.tool, "codex");
        assert!(combined.limit_name.is_none());
        assert_eq!(combined.primary.unwrap().window_minutes, 300);
        assert_eq!(combined.secondary.unwrap().window_minutes, 10_080);
        assert_eq!(combined.plan_type.as_deref(), Some("plus"));
        let credits = rows
            .iter()
            .find(|r| r.limit_id == "extra_usage")
            .and_then(|r| r.credits.clone())
            .expect("credits row");
        assert!((credits.balance.unwrap() - 45.25).abs() < 0.001);
    }
}
