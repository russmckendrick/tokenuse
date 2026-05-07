use std::collections::HashMap;
use std::fs;
use std::path::Path;

use chrono::{DateTime, NaiveDate, Utc};
use color_eyre::{
    eyre::{eyre, Context},
    Result,
};
use serde::Deserialize;
use serde_json::Value;

use super::config;
use crate::tools::{LimitCredits, LimitSnapshot, LimitWindow, SessionSource};

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Sidecar {
    Wrapped {
        observed_at: Option<DateTime<Utc>>,
        payload: CopilotUsage,
    },
    Raw(CopilotUsage),
}

#[derive(Debug, Deserialize)]
struct CopilotUsage {
    copilot_plan: Option<String>,
    quota_reset_date: Option<String>,
    quota_snapshots: Option<HashMap<String, QuotaSnapshot>>,
}

#[derive(Debug, Deserialize)]
struct QuotaSnapshot {
    entitlement: Option<f64>,
    percent_remaining: Option<f64>,
    remaining: Option<f64>,
    unlimited: Option<bool>,
    timestamp_utc: Option<DateTime<Utc>>,
}

pub fn parse_sidecar(source: &SessionSource) -> Result<Vec<LimitSnapshot>> {
    if source.tool != config::TOOL_ID {
        return Err(eyre!("Copilot limit source had wrong tool id"));
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
        serde_json::from_str(raw).map_err(|e| eyre!("parse Copilot limits sidecar: {e}"))?;
    let (observed_at, usage) = match sidecar {
        Sidecar::Wrapped {
            observed_at,
            payload,
        } => (observed_at.or(fallback_observed_at), payload),
        Sidecar::Raw(usage) => (fallback_observed_at, usage),
    };

    let Some(snapshots) = usage.quota_snapshots else {
        return Ok(Vec::new());
    };
    let reset = usage.quota_reset_date.as_deref().and_then(parse_reset);

    let mut rows = Vec::new();
    let mut snapshots: Vec<_> = snapshots.into_iter().collect();
    snapshots.sort_by(|a, b| a.0.cmp(&b.0));
    for (id, snapshot) in snapshots {
        if snapshot.unlimited.unwrap_or(false) && snapshot.entitlement.unwrap_or(0.0) <= 0.0 {
            continue;
        }

        let Some(percent_remaining) = snapshot
            .percent_remaining
            .or_else(|| percent_remaining_from_balance(&snapshot))
        else {
            continue;
        };
        let used_percent = (100.0 - percent_remaining).clamp(0.0, 100.0);
        let reached = snapshot
            .remaining
            .is_some_and(|remaining| remaining <= 0.0)
            .then(|| "primary".to_string());

        rows.push(LimitSnapshot {
            tool: config::TOOL_ID,
            limit_id: id.clone(),
            limit_name: Some(human_limit_name(&id)),
            plan_type: usage.copilot_plan.clone(),
            observed_at: snapshot.timestamp_utc.or(observed_at),
            primary: Some(LimitWindow {
                used_percent,
                window_minutes: window_minutes_for(&id),
                resets_at: reset,
            }),
            secondary: None,
            credits: Some(LimitCredits {
                has_credits: snapshot.entitlement.is_some() || snapshot.remaining.is_some(),
                unlimited: snapshot.unlimited.unwrap_or(false),
                balance: snapshot.remaining,
            }),
            rate_limit_reached_type: reached,
        });
    }

    Ok(rows)
}

#[cfg(feature = "quota-sync")]
pub fn refresh_sidecar(output: &Path) -> Result<usize> {
    let token = find_oauth_token()
        .ok_or_else(|| eyre!("Copilot OAuth token not found in github-copilot config files"))?;
    let raw = ureq::get(config::COPILOT_INTERNAL_USER_URL)
        .set("Accept", "application/json")
        .set("User-Agent", "tokenuse")
        .set("Authorization", &format!("Bearer {token}"))
        .call()
        .map_err(|e| eyre!("fetch Copilot limits: {e}"))?
        .into_string()
        .map_err(|e| eyre!("read Copilot limits: {e}"))?;
    let payload: Value =
        serde_json::from_str(&raw).map_err(|e| eyre!("parse Copilot limits json: {e}"))?;
    let count = payload
        .get("quota_snapshots")
        .and_then(Value::as_object)
        .map_or(0, serde_json::Map::len);
    write_sidecar(output, payload)?;
    Ok(count)
}

#[cfg(not(feature = "quota-sync"))]
pub fn refresh_sidecar(_output: &Path) -> Result<usize> {
    Err(eyre!("Copilot limit sync unavailable in this build"))
}

#[cfg(feature = "quota-sync")]
fn write_sidecar(output: &Path, payload: Value) -> Result<()> {
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).wrap_err_with(|| format!("create {}", parent.display()))?;
    }
    let wrapped = serde_json::json!({
        "observed_at": Utc::now().to_rfc3339(),
        "source": config::COPILOT_INTERNAL_USER_URL,
        "payload": payload,
    });
    let mut pretty = serde_json::to_string_pretty(&wrapped)?;
    pretty.push('\n');
    fs::write(output, pretty).wrap_err_with(|| format!("write {}", output.display()))?;
    Ok(())
}

#[cfg(feature = "quota-sync")]
fn find_oauth_token() -> Option<String> {
    for file in config::credential_files() {
        if let Ok(raw) = fs::read_to_string(file) {
            if let Ok(value) = serde_json::from_str::<Value>(&raw) {
                if let Some(token) = find_token_in_value(&value) {
                    return Some(token);
                }
            }
        }
    }
    None
}

#[cfg(feature = "quota-sync")]
fn find_token_in_value(value: &Value) -> Option<String> {
    match value {
        Value::Object(map) => {
            for key in ["oauth_token", "access_token", "token"] {
                if let Some(token) = map.get(key).and_then(Value::as_str).and_then(clean_token) {
                    return Some(token);
                }
            }
            map.values().find_map(find_token_in_value)
        }
        Value::Array(items) => items.iter().find_map(find_token_in_value),
        _ => None,
    }
}

#[cfg(feature = "quota-sync")]
fn clean_token(raw: &str) -> Option<String> {
    let token = raw.trim();
    (!token.is_empty()).then(|| token.to_string())
}

fn percent_remaining_from_balance(snapshot: &QuotaSnapshot) -> Option<f64> {
    let entitlement = snapshot.entitlement?;
    if entitlement <= 0.0 {
        return None;
    }
    Some((snapshot.remaining? / entitlement * 100.0).clamp(0.0, 100.0))
}

fn parse_reset(raw: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(raw)
        .map(|dt| dt.with_timezone(&Utc))
        .ok()
        .or_else(|| {
            let date = NaiveDate::parse_from_str(raw, "%Y-%m-%d").ok()?;
            date.and_hms_opt(0, 0, 0)
                .map(|dt| DateTime::from_naive_utc_and_offset(dt, Utc))
        })
}

fn window_minutes_for(limit_id: &str) -> u64 {
    let id = limit_id.to_ascii_lowercase();
    if id.contains("week") || id.contains("seven_day") || id.contains("7_day") {
        10_080
    } else {
        43_200
    }
}

fn human_limit_name(limit_id: &str) -> String {
    let mut words = Vec::new();
    for part in limit_id.split(['_', '-']).filter(|part| !part.is_empty()) {
        let mut chars = part.chars();
        let Some(first) = chars.next() else {
            continue;
        };
        words.push(first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase());
    }
    if words.is_empty() {
        "Copilot".into()
    } else {
        words.join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_quota_snapshots() {
        let raw = r#"{
          "observed_at": "2026-01-15T12:00:00Z",
          "payload": {
            "copilot_plan": "individual_pro",
            "quota_reset_date": "2026-02-01",
            "quota_snapshots": {
              "chat": {
                "entitlement": 0,
                "percent_remaining": 100.0,
                "unlimited": true,
                "timestamp_utc": "2026-01-15T12:01:00Z"
              },
              "premium_interactions": {
                "entitlement": 300,
                "percent_remaining": 31.16,
                "remaining": 93,
                "unlimited": false,
                "timestamp_utc": "2026-01-15T12:02:00Z"
              }
            }
          }
        }"#;

        let limits = parse_sidecar_str(raw, None).unwrap();

        assert_eq!(limits.len(), 1);
        assert_eq!(limits[0].tool, config::TOOL_ID);
        assert_eq!(limits[0].limit_id, "premium_interactions");
        assert_eq!(
            limits[0].limit_name.as_deref(),
            Some("Premium Interactions")
        );
        assert_eq!(limits[0].plan_type.as_deref(), Some("individual_pro"));
        assert_eq!(limits[0].primary.unwrap().window_minutes, 43_200);
        assert_eq!(limits[0].primary.unwrap().used_percent, 68.84);
        assert_eq!(
            limits[0].credits.as_ref().and_then(|c| c.balance),
            Some(93.0)
        );
    }

    #[test]
    fn derives_percent_remaining_from_balance_when_needed() {
        let raw = r#"{
          "copilot_plan": "individual_pro",
          "quota_reset_date": "2026-02-01",
          "quota_snapshots": {
            "premium_interactions": {
              "entitlement": 200,
              "remaining": 50,
              "unlimited": false
            }
          }
        }"#;

        let limits = parse_sidecar_str(raw, None).unwrap();

        assert_eq!(limits.len(), 1);
        assert_eq!(limits[0].primary.unwrap().used_percent, 75.0);
    }
}
