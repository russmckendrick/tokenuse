//! Per-day session Gantt rows.
//!
//! Ported from Microsoft's AI-Engineering-Coach `analyzer-timeline.ts` (MIT,
//! Copyright (c) Microsoft Corporation): one row per session active on the
//! day, blocks from call timestamps split at >15 minute gaps, and a
//! start/end event sweep for the maximum number of concurrent sessions.

use chrono::{DateTime, Local, NaiveDate, Utc};

use super::sessions::CoachSession;

pub(super) const BLOCK_GAP_MIN: i64 = 15;

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

/// Widest trailing window of the timeline activity calendar, in days
/// (53 weeks — a GitHub-style full year, used for All Time).
const GRID_MAX_DAYS: i64 = 371;

/// Trailing context window for period-scoped calendars, in days (9
/// Monday-first weeks — short periods keep enough surrounding history that
/// the grid never collapses to a sliver).
const GRID_CONTEXT_DAYS: i64 = 63;

/// Calendar window for a period: All Time keeps the full trailing year,
/// every scoped period gets the trailing context window (each spans at most
/// 31 days, well inside [`GRID_CONTEXT_DAYS`]).
pub fn grid_window_days(period: crate::app::Period) -> i64 {
    match period {
        crate::app::Period::AllTime => GRID_MAX_DAYS,
        _ => GRID_CONTEXT_DAYS,
    }
}

/// Turns per local day (a turn lands on its first timestamped call's day),
/// ascending, capped to the trailing `window_days` anchored at `today` — so
/// the shipped grid always lines up with the rendered calendar window, and
/// history older than the window drops even when nothing newer exists.
pub fn daily_turn_counts(
    sessions: &[CoachSession<'_>],
    window_days: i64,
    today: NaiveDate,
) -> Vec<(NaiveDate, u64)> {
    let mut days: std::collections::BTreeMap<NaiveDate, u64> = std::collections::BTreeMap::new();
    for session in sessions {
        for turn in &session.turns {
            let Some(ts) = turn.calls.iter().find_map(|call| call.timestamp) else {
                continue;
            };
            *days
                .entry(ts.with_timezone(&Local).date_naive())
                .or_default() += 1;
        }
    }
    let cutoff = today - chrono::Duration::days(window_days - 1);
    days.into_iter()
        .filter(|(day, _)| *day >= cutoff && *day <= today)
        .collect()
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
    use crate::app::Period;
    use crate::coach::testutil::{call, ctx_calls};
    use crate::coach::CoachContext;

    /// Newest local day across the synthetic sessions — a deterministic
    /// `today` anchor regardless of the host timezone.
    fn last_local_day(sessions: &[CoachSession<'_>]) -> NaiveDate {
        sessions
            .iter()
            .flat_map(|s| s.turns.iter())
            .flat_map(|t| t.calls.iter())
            .filter_map(|c| c.timestamp)
            .map(|ts| ts.with_timezone(&Local).date_naive())
            .max()
            .expect("synthetic calls carry timestamps")
    }

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
        let today = last_local_day(&ctx.sessions);
        let days = daily_turn_counts(&ctx.sessions, GRID_MAX_DAYS, today);
        assert_eq!(days.len(), 1);
        let day = timeline_day(&ctx.sessions, days[0].0).unwrap();
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
        let today = last_local_day(&ctx.sessions);
        let days = daily_turn_counts(&ctx.sessions, GRID_MAX_DAYS, today);
        let day = timeline_day(&ctx.sessions, days[0].0).unwrap();
        assert_eq!(day.rows.len(), 1);
        assert_eq!(day.rows[0].blocks.len(), 2, "a 115-minute gap splits");
        assert_eq!(day.rows[0].turns, 3);
    }

    #[test]
    fn daily_turn_counts_bucket_by_day_and_cap_the_window() {
        // Two turns on one day, one turn ~400 days later: the old day falls
        // outside the trailing 371-day window and is dropped.
        let far = 400 * 24 * 60;
        let calls = vec![
            call("a", 0, "first turn of the early day"),
            call("a", 120, "second turn after a long gap"),
            call("b", far, "a much later day of work"),
        ];
        let refs = ctx_calls(&calls);
        let ctx = CoachContext::new(&refs);
        let today = last_local_day(&ctx.sessions);

        let counts = daily_turn_counts(&ctx.sessions, GRID_MAX_DAYS, today);
        assert_eq!(counts.len(), 1, "early day dropped by the cap");
        assert_eq!(counts[0].1, 1);

        let early_only = vec![
            call("a", 0, "first turn of the early day"),
            call("a", 120, "second turn after a long gap"),
        ];
        let refs = ctx_calls(&early_only);
        let ctx = CoachContext::new(&refs);
        let today = last_local_day(&ctx.sessions);
        let counts = daily_turn_counts(&ctx.sessions, GRID_MAX_DAYS, today);
        assert_eq!(counts.len(), 1);
        assert_eq!(counts[0].1, 2, "both turns on the same day");
    }

    #[test]
    fn context_window_trims_to_the_trailing_days() {
        // Two active days 100 days apart: the 63-day context window keeps
        // only the newest, the 371-day window keeps both.
        let far = 100 * 24 * 60;
        let calls = vec![
            call("a", 0, "an early day of work"),
            call("b", far, "a much later day of work"),
        ];
        let refs = ctx_calls(&calls);
        let ctx = CoachContext::new(&refs);
        let today = last_local_day(&ctx.sessions);

        let scoped = daily_turn_counts(&ctx.sessions, GRID_CONTEXT_DAYS, today);
        assert_eq!(scoped.len(), 1, "early day outside the context window");
        assert_eq!(scoped[0].0, today);
        assert_eq!(
            daily_turn_counts(&ctx.sessions, GRID_MAX_DAYS, today).len(),
            2
        );
    }

    #[test]
    fn window_anchors_at_today_not_the_newest_data() {
        // Data whose newest day is older than the window ships nothing:
        // the window trails today, not the last active day.
        let calls = vec![call("a", 0, "a stale archive of work")];
        let refs = ctx_calls(&calls);
        let ctx = CoachContext::new(&refs);
        let today = last_local_day(&ctx.sessions) + chrono::Duration::days(400);
        assert!(daily_turn_counts(&ctx.sessions, GRID_MAX_DAYS, today).is_empty());
    }

    #[test]
    fn grid_window_days_by_period() {
        assert_eq!(grid_window_days(Period::AllTime), GRID_MAX_DAYS);
        for period in [
            Period::Today,
            Period::Week,
            Period::ThirtyDays,
            Period::Month,
        ] {
            assert_eq!(grid_window_days(period), GRID_CONTEXT_DAYS);
        }
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
