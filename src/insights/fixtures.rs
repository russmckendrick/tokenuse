#![cfg(test)]

use chrono::{DateTime, Duration, TimeZone, Utc};

use crate::tools::{LimitCredits, LimitSnapshot, LimitWindow, ParsedCall, Speed};

pub fn now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 5, 8, 12, 0, 0).unwrap()
}

pub fn at(days_ago: i64) -> Option<DateTime<Utc>> {
    Some(now() - Duration::days(days_ago))
}

pub fn at_hours(hours_ago: i64) -> Option<DateTime<Utc>> {
    Some(now() - Duration::hours(hours_ago))
}

#[derive(Clone)]
pub struct CallBuilder(ParsedCall);

impl CallBuilder {
    pub fn new(tool: &'static str, model: &str, project: &str) -> Self {
        Self(ParsedCall {
            tool,
            model: model.into(),
            project: project.into(),
            session_id: format!("session-{project}-{model}"),
            ..ParsedCall::default()
        })
    }

    pub fn session(mut self, id: &str) -> Self {
        self.0.session_id = id.into();
        self
    }

    pub fn at(mut self, days_ago: i64) -> Self {
        self.0.timestamp = at(days_ago);
        self
    }

    pub fn at_hours(mut self, hours_ago: i64) -> Self {
        self.0.timestamp = at_hours(hours_ago);
        self
    }

    pub fn cost(mut self, usd: f64) -> Self {
        self.0.cost_usd = usd;
        self
    }

    pub fn input(mut self, tokens: u64) -> Self {
        self.0.input_tokens = tokens;
        self
    }

    pub fn output(mut self, tokens: u64) -> Self {
        self.0.output_tokens = tokens;
        self
    }

    pub fn cache_read(mut self, tokens: u64) -> Self {
        self.0.cache_read_input_tokens = tokens;
        self
    }

    pub fn cache_write(mut self, tokens: u64) -> Self {
        self.0.cache_creation_input_tokens = tokens;
        self
    }

    pub fn reasoning(mut self, tokens: u64) -> Self {
        self.0.reasoning_tokens = tokens;
        self
    }

    pub fn fast(mut self) -> Self {
        self.0.speed = Speed::Fast;
        self
    }

    pub fn build(self) -> ParsedCall {
        self.0
    }
}

pub fn limit_snapshot(
    tool: &'static str,
    name: &str,
    used_percent: f64,
    window_minutes: u64,
    resets_in_minutes: i64,
) -> LimitSnapshot {
    LimitSnapshot {
        tool,
        limit_id: format!("{tool}:{name}"),
        limit_name: Some(name.into()),
        plan_type: Some("test".into()),
        observed_at: Some(now()),
        primary: Some(LimitWindow {
            used_percent,
            window_minutes,
            resets_at: Some(now() + Duration::minutes(resets_in_minutes)),
        }),
        secondary: None,
        credits: Some(LimitCredits {
            has_credits: true,
            unlimited: false,
            balance: None,
        }),
        rate_limit_reached_type: None,
    }
}

pub fn limit_with_recent_hit(tool: &'static str, name: &str, hit_type: &str) -> LimitSnapshot {
    LimitSnapshot {
        tool,
        limit_id: format!("{tool}:{name}"),
        limit_name: Some(name.into()),
        plan_type: Some("test".into()),
        observed_at: Some(now() - Duration::hours(2)),
        primary: None,
        secondary: None,
        credits: None,
        rate_limit_reached_type: Some(hit_type.into()),
    }
}
