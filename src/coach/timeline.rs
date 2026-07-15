//! Per-day session Gantt rows.
//!
//! Ported from Microsoft's AI-Engineering-Coach `analyzer-timeline.ts` (MIT,
//! Copyright (c) Microsoft Corporation): one row per session active on the
//! day, blocks from call timestamps split at >15 minute gaps, and a
//! start/end event sweep for the maximum number of concurrent sessions.

use chrono::{DateTime, Local, NaiveDate, Utc};

use super::sessions::CoachSession;

const BLOCK_GAP_MIN: i64 = 15;

pub struct TimelineDay {
    pub day: NaiveDate,
    pub max_concurrent: u64,
    /// Minutes since local midnight covered by any session.
    pub window_start_min: u64,
    pub window_end_min: u64,
    pub rows: Vec<TimelineRow>,
}

pub struct TimelineRow {
    pub session_key: String,
    pub tool: &'static str,
    pub project: String,
    pub turns: u64,
    pub cost_usd: f64,
    pub blocks: Vec<(u64, u64)>,
}

/// Local dates with at least one timestamped call, newest first.
pub fn active_days(sessions: &[CoachSession<'_>]) -> Vec<NaiveDate> {
    let mut days: std::collections::BTreeSet<NaiveDate> = std::collections::BTreeSet::new();
    for session in sessions {
        for turn in &session.turns {
            for call in &turn.calls {
                if let Some(ts) = call.timestamp {
                    days.insert(ts.with_timezone(&Local).date_naive());
                }
            }
        }
    }
    days.into_iter().rev().collect()
}

fn minute_of_day(ts: DateTime<Utc>, day: NaiveDate) -> Option<u64> {
    let local = ts.with_timezone(&Local);
    if local.date_naive() != day {
        return None;
    }
    let midnight = day.and_hms_opt(0, 0, 0)?;
    let minutes = local
        .naive_local()
        .signed_duration_since(midnight)
        .num_minutes();
    Some(minutes.clamp(0, 24 * 60 - 1) as u64)
}

pub fn timeline_day(sessions: &[CoachSession<'_>], day: NaiveDate) -> Option<TimelineDay> {
    let mut rows: Vec<TimelineRow> = Vec::new();

    for session in sessions {
        let mut minutes: Vec<u64> = Vec::new();
        let mut turns = 0u64;
        let mut cost = 0.0f64;
        for turn in &session.turns {
            let mut turn_on_day = false;
            for call in &turn.calls {
                if let Some(min) = call.timestamp.and_then(|ts| minute_of_day(ts, day)) {
                    minutes.push(min);
                    turn_on_day = true;
                    cost += call.cost_usd;
                }
            }
            if turn_on_day {
                turns += 1;
            }
        }
        if minutes.is_empty() {
            continue;
        }
        minutes.sort_unstable();

        let mut blocks: Vec<(u64, u64)> = Vec::new();
        for min in minutes {
            match blocks.last_mut() {
                Some((_, end)) if min as i64 - *end as i64 <= BLOCK_GAP_MIN => {
                    *end = (*end).max(min);
                }
                _ => blocks.push((min, min)),
            }
        }
        // Give zero-width blocks one visible minute.
        for (start, end) in &mut blocks {
            if end == start {
                *end += 1;
            }
        }

        rows.push(TimelineRow {
            session_key: session.key.clone(),
            tool: session.tool,
            project: session.project.clone(),
            turns,
            cost_usd: cost,
            blocks,
        });
    }

    if rows.is_empty() {
        return None;
    }
    rows.sort_by_key(|r| r.blocks.first().map(|b| b.0).unwrap_or(0));

    // Start/end sweep for max concurrency over session spans.
    let mut events: Vec<(u64, i64)> = Vec::new();
    for row in &rows {
        let start = row.blocks.first().map(|b| b.0).unwrap_or(0);
        let end = row.blocks.last().map(|b| b.1).unwrap_or(start);
        events.push((start, 1));
        events.push((end + 1, -1));
    }
    events.sort_unstable();
    let mut live = 0i64;
    let mut max_concurrent = 0i64;
    for (_, delta) in events {
        live += delta;
        max_concurrent = max_concurrent.max(live);
    }

    let window_start_min = rows
        .iter()
        .filter_map(|r| r.blocks.first().map(|b| b.0))
        .min()
        .unwrap_or(0);
    let window_end_min = rows
        .iter()
        .filter_map(|r| r.blocks.last().map(|b| b.1))
        .max()
        .unwrap_or(0);

    Some(TimelineDay {
        day,
        max_concurrent: max_concurrent.max(0) as u64,
        window_start_min,
        window_end_min,
        rows,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coach::testutil::{call, ctx_calls};
    use crate::coach::CoachContext;

    #[test]
    fn overlapping_sessions_raise_max_concurrency() {
        // Session A 9:00-9:30, session B 9:10-9:40 -> 2 concurrent.
        let mut calls = Vec::new();
        for i in [0, 15, 30] {
            calls.push(call("a", i, &format!("session a step {i}")));
        }
        for i in [10, 25, 40] {
            calls.push(call("b", i, &format!("session b step {i}")));
        }
        let refs = ctx_calls(&calls);
        let ctx = CoachContext::new(&refs);
        let days = active_days(&ctx.sessions);
        assert_eq!(days.len(), 1);
        let day = timeline_day(&ctx.sessions, days[0]).unwrap();
        assert_eq!(day.rows.len(), 2);
        assert_eq!(day.max_concurrent, 2);
        assert!(day.window_end_min > day.window_start_min);
    }

    #[test]
    fn long_gaps_split_session_blocks() {
        let calls = vec![
            call("s", 0, "morning start of the work"),
            call("s", 5, "still going strong here"),
            call("s", 120, "back after a long meeting"),
        ];
        let refs = ctx_calls(&calls);
        let ctx = CoachContext::new(&refs);
        let days = active_days(&ctx.sessions);
        let day = timeline_day(&ctx.sessions, days[0]).unwrap();
        assert_eq!(day.rows.len(), 1);
        assert_eq!(day.rows[0].blocks.len(), 2, "a 115-minute gap splits");
        assert_eq!(day.rows[0].turns, 3);
    }

    #[test]
    fn empty_days_return_none() {
        let calls = vec![call("s", 0, "only one day of data")];
        let refs = ctx_calls(&calls);
        let ctx = CoachContext::new(&refs);
        let missing = NaiveDate::from_ymd_opt(2020, 1, 1).unwrap();
        assert!(timeline_day(&ctx.sessions, missing).is_none());
    }
}
