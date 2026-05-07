use std::fs;

use chrono::{DateTime, Utc};
use color_eyre::{
    eyre::{eyre, Context},
    Result,
};
use serde::Deserialize;

use super::config;
use crate::tools::{LimitSnapshot, LimitWindow, SessionSource};

#[derive(Debug, Deserialize)]
struct StatusLineInput {
    rate_limits: Option<ClaudeRateLimits>,
}

#[derive(Debug, Deserialize)]
struct ClaudeRateLimits {
    five_hour: Option<ClaudeWindow>,
    seven_day: Option<ClaudeWindow>,
}

#[derive(Debug, Deserialize)]
struct ClaudeWindow {
    used_percentage: f64,
    resets_at: Option<i64>,
}

pub fn parse_sidecar(source: &SessionSource) -> Result<Vec<LimitSnapshot>> {
    if source.tool != config::TOOL_ID {
        return Err(eyre!("Claude Code limit source had wrong tool id"));
    }

    let raw = fs::read_to_string(&source.path)
        .wrap_err_with(|| format!("read {}", source.path.display()))?;
    let observed_at = fs::metadata(&source.path)
        .ok()
        .and_then(|metadata| metadata.modified().ok())
        .map(DateTime::<Utc>::from);
    parse_sidecar_str(&raw, observed_at)
}

fn parse_sidecar_str(raw: &str, observed_at: Option<DateTime<Utc>>) -> Result<Vec<LimitSnapshot>> {
    let input: StatusLineInput =
        serde_json::from_str(raw).map_err(|e| eyre!("parse Claude Code limits sidecar: {e}"))?;
    let Some(rate_limits) = input.rate_limits else {
        return Ok(Vec::new());
    };

    let primary = rate_limits.five_hour.map(|window| LimitWindow {
        used_percent: window.used_percentage,
        window_minutes: 300,
        resets_at: unix_seconds(window.resets_at),
    });
    let secondary = rate_limits.seven_day.map(|window| LimitWindow {
        used_percent: window.used_percentage,
        window_minutes: 10_080,
        resets_at: unix_seconds(window.resets_at),
    });

    if primary.is_none() && secondary.is_none() {
        return Ok(Vec::new());
    }

    Ok(vec![LimitSnapshot {
        tool: config::TOOL_ID,
        limit_id: config::TOOL_ID.into(),
        limit_name: None,
        plan_type: None,
        observed_at,
        primary,
        secondary,
        credits: None,
        rate_limit_reached_type: None,
    }])
}

fn unix_seconds(value: Option<i64>) -> Option<DateTime<Utc>> {
    DateTime::<Utc>::from_timestamp(value?, 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_status_line_rate_limits() {
        let raw = r#"{
          "rate_limits": {
            "five_hour": { "used_percentage": 42.3, "resets_at": 1774036800 },
            "seven_day": { "used_percentage": 85.7, "resets_at": 1774580400 }
          }
        }"#;

        let limits = parse_sidecar_str(raw, None).unwrap();

        assert_eq!(limits.len(), 1);
        assert_eq!(limits[0].tool, config::TOOL_ID);
        assert_eq!(limits[0].limit_id, config::TOOL_ID);
        assert_eq!(limits[0].primary.unwrap().used_percent, 42.3);
        assert_eq!(limits[0].primary.unwrap().window_minutes, 300);
        assert_eq!(limits[0].secondary.unwrap().used_percent, 85.7);
        assert_eq!(limits[0].secondary.unwrap().window_minutes, 10_080);
    }

    #[test]
    fn skips_when_rate_limits_are_absent() {
        let limits = parse_sidecar_str(r#"{"model":{"id":"claude-opus-4-7"}}"#, None).unwrap();

        assert!(limits.is_empty());
    }
}
