//! AI code output analysis.
//!
//! Ported from Microsoft's AI-Engineering-Coach `analyzer-production.ts`
//! (MIT, Copyright (c) Microsoft Corporation): fold per-call code blocks
//! into LoC totals by language, day, project, and model, plus a coverage
//! note naming tools that contribute calls but no code-output signal.

use chrono::{DateTime, Datelike, Duration, Local, NaiveDate, TimeZone, Timelike};

use super::CoachContext;
use crate::app::Period;

pub struct OutputStats {
    pub total_loc: u64,
    /// (language, loc), descending.
    pub by_language: Vec<(String, u64)>,
    /// (local day, loc), ascending by day.
    pub by_day: Vec<(NaiveDate, u64)>,
    /// Timestamped LoC used to build period-aware output trend buckets.
    pub timestamped: Vec<(DateTime<Local>, u64)>,
    /// (project identity, loc), descending.
    pub by_project: Vec<(String, u64)>,
    /// (model, loc), descending.
    pub by_model: Vec<(String, u64)>,
    /// Tools present in the data that never produced a code-output signal.
    pub uncovered_tools: Vec<&'static str>,
}

pub fn output_stats(ctx: &CoachContext) -> OutputStats {
    let mut total = 0u64;
    let mut by_language: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
    let mut by_day: std::collections::BTreeMap<NaiveDate, u64> = std::collections::BTreeMap::new();
    let mut timestamped = Vec::new();
    let mut by_project: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
    let mut by_model: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
    let mut tools_with_loc: std::collections::HashSet<&'static str> =
        std::collections::HashSet::new();
    let mut tools_seen: std::collections::HashSet<&'static str> = std::collections::HashSet::new();

    for session in &ctx.sessions {
        for turn in &session.turns {
            tools_seen.insert(turn.tool);
            if turn.ai_loc == 0 {
                continue;
            }
            tools_with_loc.insert(turn.tool);
            total += turn.ai_loc;
            for block in &turn.code_blocks {
                *by_language.entry(block.language.clone()).or_insert(0) += block.loc;
            }
            if let Some(ts) = turn.timestamp {
                let local = ts.with_timezone(&Local);
                *by_day.entry(local.date_naive()).or_insert(0) += turn.ai_loc;
                timestamped.push((local, turn.ai_loc));
            }
            *by_project
                .entry(crate::ingest::projects::project_identity(&session.project))
                .or_insert(0) += turn.ai_loc;
            *by_model.entry(turn.model.to_string()).or_insert(0) += turn.ai_loc;
        }
    }

    let mut uncovered_tools: Vec<&'static str> =
        tools_seen.difference(&tools_with_loc).copied().collect();
    uncovered_tools.sort_unstable();

    OutputStats {
        total_loc: total,
        by_language: sorted_desc(by_language),
        by_day: by_day.into_iter().collect(),
        timestamped,
        by_project: sorted_desc(by_project),
        by_model: sorted_desc(by_model),
        uncovered_tools,
    }
}

/// Build the Output chart at a resolution suited to the selected period.
/// The 24-hour view uses 30-minute buckets, the seven-day view uses hourly
/// buckets, and longer views retain the existing daily aggregation.
pub fn trend_rows(stats: &OutputStats, period: Period, now: DateTime<Local>) -> Vec<(String, u64)> {
    match period {
        Period::Today => bucketed_rows(
            &stats.timestamped,
            floor_to_interval(now - Duration::hours(24), 30),
            floor_to_interval(now, 30),
            now,
            30,
        ),
        Period::Week => {
            let first_day = now.date_naive() - Duration::days(6);
            let start = Local
                .with_ymd_and_hms(
                    first_day.year(),
                    first_day.month(),
                    first_day.day(),
                    0,
                    0,
                    0,
                )
                .earliest()
                .unwrap_or_else(|| floor_to_interval(now, 60));
            bucketed_rows(
                &stats.timestamped,
                start,
                floor_to_interval(now, 60),
                now,
                60,
            )
        }
        Period::ThirtyDays => dense_daily_rows(
            &stats.by_day,
            now.date_naive() - Duration::days(29),
            now.date_naive(),
        ),
        Period::Month => {
            let today = now.date_naive();
            let start = NaiveDate::from_ymd_opt(today.year(), today.month(), 1).unwrap_or(today);
            dense_daily_rows(&stats.by_day, start, today)
        }
        Period::AllTime => monthly_rows(&stats.by_day, now.date_naive()),
    }
}

fn dense_daily_rows(
    values: &[(NaiveDate, u64)],
    start: NaiveDate,
    end: NaiveDate,
) -> Vec<(String, u64)> {
    let mut totals: std::collections::BTreeMap<NaiveDate, u64> = values.iter().copied().collect();
    let mut rows = Vec::new();
    let mut day = start;
    while day <= end {
        rows.push((
            day.format("%Y-%m-%d").to_string(),
            totals.remove(&day).unwrap_or(0),
        ));
        day += Duration::days(1);
    }
    rows
}

fn monthly_rows(values: &[(NaiveDate, u64)], today: NaiveDate) -> Vec<(String, u64)> {
    let Some((first_day, _)) = values.first() else {
        return Vec::new();
    };
    let mut totals = std::collections::BTreeMap::new();
    for (day, loc) in values {
        *totals.entry((day.year(), day.month())).or_insert(0u64) += *loc;
    }

    let end = (today.year(), today.month());
    let mut current = (first_day.year(), first_day.month());
    let mut rows = Vec::new();
    while current <= end {
        rows.push((
            format!("{:04}-{:02}", current.0, current.1),
            totals.remove(&current).unwrap_or(0),
        ));
        current = if current.1 == 12 {
            (current.0 + 1, 1)
        } else {
            (current.0, current.1 + 1)
        };
    }
    rows
}

fn floor_to_interval(timestamp: DateTime<Local>, minutes: u32) -> DateTime<Local> {
    let minute = timestamp.minute() - timestamp.minute() % minutes;
    timestamp
        .with_minute(minute)
        .and_then(|value| value.with_second(0))
        .and_then(|value| value.with_nanosecond(0))
        .unwrap_or(timestamp)
}

fn bucketed_rows(
    values: &[(DateTime<Local>, u64)],
    start: DateTime<Local>,
    end: DateTime<Local>,
    now: DateTime<Local>,
    minutes: u32,
) -> Vec<(String, u64)> {
    let step = Duration::minutes(i64::from(minutes));
    let mut buckets = std::collections::BTreeMap::new();
    let mut cursor = start;
    while cursor <= end {
        buckets.insert(cursor, 0u64);
        cursor += step;
    }

    for (timestamp, loc) in values {
        if *timestamp < start || *timestamp > now {
            continue;
        }
        let bucket = floor_to_interval(*timestamp, minutes);
        if let Some(total) = buckets.get_mut(&bucket) {
            *total += *loc;
        }
    }

    buckets
        .into_iter()
        .map(|(timestamp, loc)| (timestamp.format("%Y-%m-%d %H:%M").to_string(), loc))
        .collect()
}

fn sorted_desc(map: std::collections::HashMap<String, u64>) -> Vec<(String, u64)> {
    let mut rows: Vec<(String, u64)> = map.into_iter().collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coach::testutil::{call, ctx_calls, with_code};

    #[test]
    fn folds_loc_by_language_and_flags_uncovered_tools() {
        let mut calls = vec![
            with_code(call("s1", 0, "write the parser in rust"), "rust", 100),
            with_code(call("s1", 1, "and helper scripts"), "shell", 20),
            with_code(call("s2", 2, "more rust please"), "rust", 30),
        ];
        let mut bare = call("s3", 3, "cursor style row without code signal");
        bare.tool = crate::tools::cursor::config::TOOL_ID;
        calls.push(bare);

        let refs = ctx_calls(&calls);
        let ctx = crate::coach::CoachContext::new(&refs);
        let stats = output_stats(&ctx);
        assert_eq!(stats.total_loc, 150);
        assert_eq!(stats.by_language[0], ("rust".to_string(), 130));
        assert_eq!(stats.by_language[1], ("shell".to_string(), 20));
        assert_eq!(stats.by_day.len(), 1);
        assert_eq!(stats.uncovered_tools, vec!["cursor"]);
    }

    #[test]
    fn short_period_trends_use_dense_subday_buckets() {
        let now = Local
            .with_ymd_and_hms(2026, 7, 16, 12, 17, 0)
            .single()
            .expect("unambiguous local time");
        let stats = OutputStats {
            total_loc: 120,
            by_language: Vec::new(),
            by_day: vec![
                (
                    NaiveDate::from_ymd_opt(2026, 4, 10).expect("valid older day"),
                    70,
                ),
                (now.date_naive(), 50),
            ],
            timestamped: vec![
                (now - Duration::minutes(5), 30),
                (now - Duration::minutes(35), 20),
            ],
            by_project: Vec::new(),
            by_model: Vec::new(),
            uncovered_tools: Vec::new(),
        };

        let half_hours = trend_rows(&stats, Period::Today, now);
        assert_eq!(half_hours.len(), 49);
        assert_eq!(half_hours.iter().map(|row| row.1).sum::<u64>(), 50);
        assert_eq!(
            half_hours.last().map(|row| row.0.as_str()),
            Some("2026-07-16 12:00")
        );

        let hours = trend_rows(&stats, Period::Week, now);
        assert_eq!(hours.len(), 157);
        assert_eq!(hours.iter().map(|row| row.1).sum::<u64>(), 50);
        assert_eq!(
            hours.first().map(|row| row.0.as_str()),
            Some("2026-07-10 00:00")
        );

        let days = trend_rows(&stats, Period::ThirtyDays, now);
        assert_eq!(days.len(), 30);
        assert_eq!(days.iter().map(|row| row.1).sum::<u64>(), 50);

        let months = trend_rows(&stats, Period::AllTime, now);
        assert_eq!(months.len(), 4);
        assert_eq!(months.iter().map(|row| row.1).sum::<u64>(), 120);
        assert_eq!(months.first().map(|row| row.0.as_str()), Some("2026-04"));
    }
}
