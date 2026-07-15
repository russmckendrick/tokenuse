//! Flow-state scoring.
//!
//! Ported from Microsoft's AI-Engineering-Coach `analyzer-flow.ts` (MIT,
//! Copyright (c) Microsoft Corporation): per-session flow score =
//! 0.40·rapid-follow-up rate + 0.30·median-gap band + 0.15·duration band +
//! 0.15·density band; day work blocks split at >15 min gaps.

use chrono::{DateTime, Local, NaiveDate, Utc};

use super::sessions::{think_gap_ms, CoachSession};

pub const FLOW_DEEP: u64 = 70;
pub const FLOW_MODERATE: u64 = 45;
pub const FLOW_SHALLOW: u64 = 25;

const RAPID_FOLLOWUP_MS: i64 = 30_000;
const WORK_BLOCK_GAP_MIN: i64 = 15;
const SESSION_MIN_TURNS: usize = 3;

pub fn flow_label(score: u64) -> &'static str {
    if score >= FLOW_DEEP {
        "deep"
    } else if score >= FLOW_MODERATE {
        "moderate"
    } else if score >= FLOW_SHALLOW {
        "shallow"
    } else {
        "fragmented"
    }
}

pub struct SessionFlow {
    pub day: NaiveDate,
    pub score: u64,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub turns: usize,
}

pub struct FlowDay {
    pub day: NaiveDate,
    pub score: u64,
    pub longest_block_min: u64,
    pub active_min: u64,
    pub sessions: u64,
    pub avg_followup_ms: Option<u64>,
}

pub struct FlowSummaryStats {
    pub days: Vec<FlowDay>,
    pub overall_score: u64,
    pub deep_days: u64,
    pub fragmented_days: u64,
    pub avg_followup_ms: Option<u64>,
    pub avg_longest_block_min: u64,
}

/// Score one session; `None` when it has fewer than 3 timestamped turns.
pub fn session_flow(session: &CoachSession<'_>) -> Option<SessionFlow> {
    let turns: Vec<_> = session
        .turns
        .iter()
        .filter(|t| t.timestamp.is_some())
        .collect();
    if turns.len() < SESSION_MIN_TURNS {
        return None;
    }

    let mut gaps: Vec<i64> = Vec::new();
    for pair in turns.windows(2) {
        if let Some(gap) = think_gap_ms(pair[0], pair[1]) {
            if gap >= 0 {
                gaps.push(gap);
            }
        }
    }
    if gaps.is_empty() {
        return None;
    }

    let rapid = gaps.iter().filter(|g| **g <= RAPID_FOLLOWUP_MS).count() as f64;
    let rapid_rate = rapid / gaps.len() as f64;

    let mut sorted = gaps.clone();
    sorted.sort_unstable();
    let median_gap_sec = sorted[sorted.len() / 2] / 1000;
    let latency_score = match median_gap_sec {
        ..=10 => 100.0,
        11..=30 => 80.0,
        31..=60 => 60.0,
        61..=120 => 40.0,
        121..=300 => 20.0,
        _ => 0.0,
    };

    let start = turns.first().and_then(|t| t.timestamp)?;
    let end = turns.last().and_then(|t| t.end_timestamp.or(t.timestamp))?;
    let duration_min = end.signed_duration_since(start).num_minutes();
    let duration_score = match duration_min {
        ..=5 => 20.0,
        6..=15 => 50.0,
        16..=40 => 90.0,
        41..=90 => 80.0,
        _ => 60.0,
    };

    let density = if duration_min > 0 {
        turns.len() as f64 / duration_min as f64
    } else {
        turns.len() as f64
    };
    let density_score = (density * 50.0).min(100.0);

    let score = (0.40 * rapid_rate * 100.0
        + 0.30 * latency_score
        + 0.15 * duration_score
        + 0.15 * density_score)
        .round()
        .clamp(0.0, 100.0) as u64;

    Some(SessionFlow {
        day: start.with_timezone(&Local).date_naive(),
        score,
        start,
        end,
        turns: turns.len(),
    })
}

/// Per-day flow: sessions merged into work blocks (gap > 15 min splits),
/// day score = mean of its session scores.
pub fn flow_days(sessions: &[CoachSession<'_>]) -> Vec<FlowDay> {
    let mut flows: Vec<SessionFlow> = sessions.iter().filter_map(session_flow).collect();
    let mut followups: std::collections::HashMap<NaiveDate, Vec<i64>> =
        std::collections::HashMap::new();
    for session in sessions {
        let turns: Vec<_> = session
            .turns
            .iter()
            .filter(|t| t.timestamp.is_some())
            .collect();
        for pair in turns.windows(2) {
            if let (Some(gap), Some(ts)) = (think_gap_ms(pair[0], pair[1]), pair[0].end_timestamp) {
                if gap >= 0 {
                    followups
                        .entry(ts.with_timezone(&Local).date_naive())
                        .or_default()
                        .push(gap);
                }
            }
        }
    }
    flows.sort_by_key(|f| f.start);

    let mut days: Vec<FlowDay> = Vec::new();
    let mut by_day: std::collections::BTreeMap<NaiveDate, Vec<&SessionFlow>> =
        std::collections::BTreeMap::new();
    for flow in &flows {
        by_day.entry(flow.day).or_default().push(flow);
    }

    for (day, day_flows) in by_day {
        // Merge session spans into work blocks.
        let mut spans: Vec<(DateTime<Utc>, DateTime<Utc>)> =
            day_flows.iter().map(|f| (f.start, f.end)).collect();
        spans.sort_by_key(|(start, _)| *start);
        let mut blocks: Vec<(DateTime<Utc>, DateTime<Utc>)> = Vec::new();
        for (start, end) in spans {
            match blocks.last_mut() {
                Some((_, block_end))
                    if start.signed_duration_since(*block_end).num_minutes()
                        <= WORK_BLOCK_GAP_MIN =>
                {
                    *block_end = (*block_end).max(end);
                }
                _ => blocks.push((start, end)),
            }
        }
        let longest_block_min = blocks
            .iter()
            .map(|(s, e)| e.signed_duration_since(*s).num_minutes().max(0) as u64)
            .max()
            .unwrap_or(0);
        let active_min: u64 = blocks
            .iter()
            .map(|(s, e)| e.signed_duration_since(*s).num_minutes().max(0) as u64)
            .sum();
        let score = (day_flows.iter().map(|f| f.score).sum::<u64>() as f64 / day_flows.len() as f64)
            .round() as u64;
        let day_gaps = followups.remove(&day).unwrap_or_default();
        let avg_followup_ms = if day_gaps.is_empty() {
            None
        } else {
            Some((day_gaps.iter().sum::<i64>() / day_gaps.len() as i64).max(0) as u64)
        };
        days.push(FlowDay {
            day,
            score,
            longest_block_min,
            active_min,
            sessions: day_flows.len() as u64,
            avg_followup_ms,
        });
    }
    days
}

pub fn flow_summary(sessions: &[CoachSession<'_>]) -> FlowSummaryStats {
    let days = flow_days(sessions);
    let overall_score = if days.is_empty() {
        0
    } else {
        (days.iter().map(|d| d.score).sum::<u64>() as f64 / days.len() as f64).round() as u64
    };
    let deep_days = days.iter().filter(|d| d.score >= FLOW_DEEP).count() as u64;
    let fragmented_days = days.iter().filter(|d| d.score < FLOW_SHALLOW).count() as u64;
    let followups: Vec<u64> = days.iter().filter_map(|d| d.avg_followup_ms).collect();
    let avg_followup_ms = if followups.is_empty() {
        None
    } else {
        Some(followups.iter().sum::<u64>() / followups.len() as u64)
    };
    let avg_longest_block_min = if days.is_empty() {
        0
    } else {
        days.iter().map(|d| d.longest_block_min).sum::<u64>() / days.len() as u64
    };
    FlowSummaryStats {
        days,
        overall_score,
        deep_days,
        fragmented_days,
        avg_followup_ms,
        avg_longest_block_min,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coach::testutil::{call, ctx_calls};
    use crate::coach::CoachContext;

    #[test]
    fn tight_session_scores_deep_and_sparse_session_scores_low() {
        // 10 turns 1 minute apart with 5s latency: rapid follow-ups.
        let tight: Vec<_> = (0..10)
            .map(|i| {
                let mut c = call("tight", i, &format!("step {i} of the tight loop"));
                c.elapsed_ms = Some(55_000);
                c
            })
            .collect();
        let refs = ctx_calls(&tight);
        let ctx = CoachContext::new(&refs);
        let flow = session_flow(&ctx.sessions[0]).unwrap();
        assert!(
            flow.score >= FLOW_DEEP,
            "tight loop should be deep, got {}",
            flow.score
        );

        // 4 turns 40 minutes apart: fragmented.
        let sparse: Vec<_> = (0..4)
            .map(|i| call("sparse", i * 40, &format!("occasional question {i}")))
            .collect();
        let refs = ctx_calls(&sparse);
        let ctx = CoachContext::new(&refs);
        let flow = session_flow(&ctx.sessions[0]).unwrap();
        assert!(
            flow.score < FLOW_MODERATE,
            "sparse session should score low, got {}",
            flow.score
        );
    }

    #[test]
    fn sessions_under_three_turns_do_not_score() {
        let calls = vec![call("s1", 0, "one"), call("s1", 1, "two")];
        let refs = ctx_calls(&calls);
        let ctx = CoachContext::new(&refs);
        assert!(session_flow(&ctx.sessions[0]).is_none());
    }

    #[test]
    fn day_blocks_split_on_long_gaps() {
        // Two 3-turn sessions separated by 3 hours on the same day.
        let mut calls: Vec<_> = (0..3)
            .map(|i| call("morning", i * 5, &format!("morning task {i}")))
            .collect();
        calls.extend((0..3).map(|i| call("afternoon", 180 + i * 5, &format!("afternoon {i}"))));
        let refs = ctx_calls(&calls);
        let ctx = CoachContext::new(&refs);
        let days = flow_days(&ctx.sessions);
        assert_eq!(days.len(), 1);
        assert_eq!(days[0].sessions, 2);
        assert_eq!(days[0].longest_block_min, 10);
        assert_eq!(days[0].active_min, 20);
    }
}
