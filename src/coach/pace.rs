//! Sustainable-pace analysis.
//!
//! Ported from Microsoft's AI-Engineering-Coach `analyzer-insights.ts` (MIT,
//! Copyright (c) Microsoft Corporation): late-night = local hour >= 22 or
//! < 5; weekend = Saturday/Sunday; streak = consecutive active local days;
//! burnout alerts on streak >= 14, rising late-night/weekend trends (±20%
//! bands over 3-week means), late-night rate > 0.15, weekend rate > 0.25;
//! risk is high at 3+ alerts (or a 14-day streak with rising late-night),
//! medium at 1+, else low.

use chrono::{DateTime, Datelike, Local, NaiveDate, Timelike, Weekday};

use super::CoachContext;

pub const BURNOUT_STREAK_DAYS: u64 = 14;
const LATE_NIGHT_RATE: f64 = 0.15;
const WEEKEND_RATE: f64 = 0.25;
const TREND_BAND: f64 = 0.2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BurnoutRisk {
    Low,
    Medium,
    High,
}

impl BurnoutRisk {
    pub fn id(self) -> &'static str {
        match self {
            BurnoutRisk::Low => "low",
            BurnoutRisk::Medium => "medium",
            BurnoutRisk::High => "high",
        }
    }
}

/// Alert ids map to copy keys `coach.pace.alert_<id>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaceAlert {
    LongStreak,
    LateNightRising,
    WeekendRising,
    LateNightHeavy,
    WeekendHeavy,
}

impl PaceAlert {
    pub fn id(self) -> &'static str {
        match self {
            PaceAlert::LongStreak => "long_streak",
            PaceAlert::LateNightRising => "late_night_rising",
            PaceAlert::WeekendRising => "weekend_rising",
            PaceAlert::LateNightHeavy => "late_night_heavy",
            PaceAlert::WeekendHeavy => "weekend_heavy",
        }
    }
}

pub struct PaceStats {
    pub current_streak: u64,
    pub longest_streak: u64,
    pub late_night_pct: u64,
    pub weekend_pct: u64,
    pub risk: BurnoutRisk,
    pub alerts: Vec<PaceAlert>,
}

fn is_late_night(ts: DateTime<Local>) -> bool {
    !(5..22).contains(&ts.hour())
}

fn is_weekend(ts: DateTime<Local>) -> bool {
    matches!(ts.weekday(), Weekday::Sat | Weekday::Sun)
}

pub fn pace_stats(ctx: &CoachContext, today: NaiveDate) -> PaceStats {
    let mut total = 0u64;
    let mut late = 0u64;
    let mut weekend = 0u64;
    let mut active_days: std::collections::BTreeSet<NaiveDate> = std::collections::BTreeSet::new();
    // Weekly (ISO) late-night/weekend counts for trend bands.
    let mut weekly: std::collections::BTreeMap<i64, (u64, u64)> = std::collections::BTreeMap::new();

    for turn in ctx.timestamped_turns() {
        let local = turn.timestamp.expect("timestamped").with_timezone(&Local);
        total += 1;
        active_days.insert(local.date_naive());
        let iso = local.iso_week();
        let week = i64::from(iso.year()) * 53 + i64::from(iso.week());
        let entry = weekly.entry(week).or_insert((0, 0));
        if is_late_night(local) {
            late += 1;
            entry.0 += 1;
        }
        if is_weekend(local) {
            weekend += 1;
            entry.1 += 1;
        }
    }

    let (current_streak, longest_streak) = streaks(&active_days, today);

    let late_rate = if total == 0 {
        0.0
    } else {
        late as f64 / total as f64
    };
    let weekend_rate = if total == 0 {
        0.0
    } else {
        weekend as f64 / total as f64
    };

    let weeks: Vec<(u64, u64)> = weekly.into_values().collect();
    let late_rising = rising_trend(weeks.iter().map(|w| w.0));
    let weekend_rising = rising_trend(weeks.iter().map(|w| w.1));

    let mut alerts = Vec::new();
    if current_streak >= BURNOUT_STREAK_DAYS {
        alerts.push(PaceAlert::LongStreak);
    }
    if late_rising {
        alerts.push(PaceAlert::LateNightRising);
    }
    if weekend_rising {
        alerts.push(PaceAlert::WeekendRising);
    }
    if late_rate > LATE_NIGHT_RATE {
        alerts.push(PaceAlert::LateNightHeavy);
    }
    if weekend_rate > WEEKEND_RATE {
        alerts.push(PaceAlert::WeekendHeavy);
    }

    let risk = if alerts.len() >= 3 || (current_streak >= BURNOUT_STREAK_DAYS && late_rising) {
        BurnoutRisk::High
    } else if !alerts.is_empty() {
        BurnoutRisk::Medium
    } else {
        BurnoutRisk::Low
    };

    PaceStats {
        current_streak,
        longest_streak,
        late_night_pct: (late_rate * 100.0).round() as u64,
        weekend_pct: (weekend_rate * 100.0).round() as u64,
        risk,
        alerts,
    }
}

/// Mean of the last 3 weeks vs the prior 3, rising when +20% above.
fn rising_trend(counts: impl Iterator<Item = u64>) -> bool {
    let values: Vec<u64> = counts.collect();
    if values.len() < 6 {
        return false;
    }
    let recent: f64 = values[values.len() - 3..].iter().sum::<u64>() as f64 / 3.0;
    let prior: f64 = values[values.len() - 6..values.len() - 3]
        .iter()
        .sum::<u64>() as f64
        / 3.0;
    prior > 0.0 && recent > prior * (1.0 + TREND_BAND)
}

/// (streak ending today-or-yesterday, longest streak in range).
fn streaks(days: &std::collections::BTreeSet<NaiveDate>, today: NaiveDate) -> (u64, u64) {
    let mut longest = 0u64;
    let mut run = 0u64;
    let mut prev: Option<NaiveDate> = None;
    for day in days {
        run = match prev {
            Some(p) if *day == p + chrono::Duration::days(1) => run + 1,
            _ => 1,
        };
        longest = longest.max(run);
        prev = Some(*day);
    }

    // Current streak must reach today or yesterday.
    let mut current = 0u64;
    let mut cursor = if days.contains(&today) {
        Some(today)
    } else {
        let yesterday = today - chrono::Duration::days(1);
        days.contains(&yesterday).then_some(yesterday)
    };
    while let Some(day) = cursor {
        if days.contains(&day) {
            current += 1;
            cursor = Some(day - chrono::Duration::days(1));
        } else {
            break;
        }
    }
    (current, longest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn streaks_track_current_and_longest_runs() {
        let mut days = std::collections::BTreeSet::new();
        let base = NaiveDate::from_ymd_opt(2026, 6, 1).unwrap();
        // 5-day run, gap, then 2-day run ending yesterday.
        for offset in [0, 1, 2, 3, 4, 10, 11] {
            days.insert(base + chrono::Duration::days(offset));
        }
        let today = base + chrono::Duration::days(12);
        let (current, longest) = streaks(&days, today);
        assert_eq!(current, 2);
        assert_eq!(longest, 5);

        let (current, _) = streaks(&days, base + chrono::Duration::days(20));
        assert_eq!(current, 0, "stale runs are not a current streak");
    }

    #[test]
    fn rising_trend_needs_six_weeks_and_a_real_jump() {
        assert!(rising_trend([1, 1, 1, 2, 2, 2].into_iter()));
        assert!(!rising_trend([2, 2, 2, 2, 2, 2].into_iter()));
        assert!(!rising_trend([1, 1, 2, 2].into_iter()));
    }
}
