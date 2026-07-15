//! Session-hygiene rules.
//!
//! Ported from Microsoft's AI-Engineering-Coach rule files (MIT, Copyright
//! (c) Microsoft Corporation): session-drift, late-night-coding,
//! weekend-overwork, mega-sessions, abandon-sessions, high-cancellation,
//! broken-flow-state, slow-responses, runaway-agent-loops.

use chrono::{Datelike, Local, Timelike, Weekday};

use super::{example, ratio_pct, RuleDef, RuleGroup, RuleHit, Severity, MAX_EXAMPLES};
use crate::coach::{flow, signals, CoachContext};

pub fn rules() -> Vec<RuleDef> {
    vec![
        RuleDef {
            id: "session-drift",
            group: RuleGroup::SessionHygiene,
            severity: Severity::Medium,
            detect: session_drift,
        },
        RuleDef {
            id: "late-night-coding",
            group: RuleGroup::SessionHygiene,
            severity: Severity::Low,
            detect: late_night_coding,
        },
        RuleDef {
            id: "weekend-overwork",
            group: RuleGroup::SessionHygiene,
            severity: Severity::Low,
            detect: weekend_overwork,
        },
        RuleDef {
            id: "mega-sessions",
            group: RuleGroup::SessionHygiene,
            severity: Severity::High,
            detect: mega_sessions,
        },
        RuleDef {
            id: "abandon-sessions",
            group: RuleGroup::SessionHygiene,
            severity: Severity::Low,
            detect: abandon_sessions,
        },
        RuleDef {
            id: "high-cancellation",
            group: RuleGroup::SessionHygiene,
            severity: Severity::Medium,
            detect: high_cancellation,
        },
        RuleDef {
            id: "broken-flow-state",
            group: RuleGroup::SessionHygiene,
            severity: Severity::Medium,
            detect: broken_flow_state,
        },
        RuleDef {
            id: "slow-responses",
            group: RuleGroup::SessionHygiene,
            severity: Severity::Low,
            detect: slow_responses,
        },
        RuleDef {
            id: "runaway-agent-loops",
            group: RuleGroup::SessionHygiene,
            severity: Severity::High,
            detect: runaway_agent_loops,
        },
    ]
}

/// Sessions covering 4+ work types across 5+ turns (min 4 sessions).
fn session_drift(ctx: &CoachContext) -> Option<RuleHit> {
    const MAX_WORK_TYPES: usize = 4;
    const MIN_TURNS_PER_SESSION: usize = 5;
    const MIN_SESSIONS: u64 = 3;

    let mut count = 0u64;
    let mut total = 0u64;
    let mut examples = Vec::new();
    for session in ctx.conversational_sessions() {
        total += 1;
        if session.turns.len() < MIN_TURNS_PER_SESSION {
            continue;
        }
        let mut kinds: Vec<&'static str> = session
            .turns
            .iter()
            .filter(|t| !t.user_message.is_empty())
            .map(|t| crate::coach::text::classify_work(t.user_message))
            .collect();
        kinds.sort_unstable();
        kinds.dedup();
        if kinds.len() >= MAX_WORK_TYPES {
            count += 1;
            if examples.len() < MAX_EXAMPLES {
                examples.push(example(
                    &session.project,
                    format!("{} work types in one session", kinds.len()),
                ));
            }
        }
    }
    (count > MIN_SESSIONS).then_some(RuleHit {
        occurrences: count,
        total,
        pct: None,
        stat: None,
        escalate: false,
        examples,
    })
}

/// More than 10 turns between midnight and 5am local time.
fn late_night_coding(ctx: &CoachContext) -> Option<RuleHit> {
    const LATE_NIGHT_HOUR: u32 = 5;
    const MIN_SAMPLE: u64 = 10;

    let mut count = 0u64;
    let mut total = 0u64;
    let mut examples = Vec::new();
    for turn in ctx.timestamped_turns() {
        total += 1;
        let local = turn.timestamp.expect("timestamped").with_timezone(&Local);
        if local.hour() < LATE_NIGHT_HOUR {
            count += 1;
            if examples.len() < MAX_EXAMPLES && !turn.user_message.is_empty() {
                examples.push(example(
                    turn.user_message,
                    local.format("%Y-%m-%d %H:%M").to_string(),
                ));
            }
        }
    }
    (count > MIN_SAMPLE).then(|| RuleHit {
        occurrences: count,
        total,
        pct: ratio_pct(count, total),
        stat: None,
        escalate: false,
        examples,
    })
}

/// More than 25% of turns on Saturday/Sunday (min 20 weekend turns).
fn weekend_overwork(ctx: &CoachContext) -> Option<RuleHit> {
    const MAX_WEEKEND_RATE: f64 = 0.25;
    const MIN_WEEKEND_TURNS: u64 = 20;

    let mut count = 0u64;
    let mut total = 0u64;
    for turn in ctx.timestamped_turns() {
        total += 1;
        let weekday = turn
            .timestamp
            .expect("timestamped")
            .with_timezone(&Local)
            .weekday();
        if weekday == Weekday::Sat || weekday == Weekday::Sun {
            count += 1;
        }
    }
    (total > 0 && count > MIN_WEEKEND_TURNS && count as f64 / total as f64 > MAX_WEEKEND_RATE).then(
        || RuleHit {
            occurrences: count,
            total,
            pct: ratio_pct(count, total),
            stat: Some((total - count).to_string()),
            escalate: false,
            examples: Vec::new(),
        },
    )
}

/// Sessions with 50+ turns.
fn mega_sessions(ctx: &CoachContext) -> Option<RuleHit> {
    const MAX_TURNS: usize = 50;

    let mut count = 0u64;
    let mut total = 0u64;
    let mut biggest = 0usize;
    let mut examples = Vec::new();
    for session in ctx.conversational_sessions() {
        total += 1;
        biggest = biggest.max(session.turns.len());
        if session.turns.len() >= MAX_TURNS {
            count += 1;
            if examples.len() < MAX_EXAMPLES {
                examples.push(example(
                    &session.project,
                    format!("{} messages", session.turns.len()),
                ));
            }
        }
    }
    (count > 0).then(|| RuleHit {
        occurrences: count,
        total,
        pct: None,
        stat: Some(biggest.to_string()),
        escalate: false,
        examples,
    })
}

/// More than 40% of sessions end after a single turn (min 10 abandoned).
fn abandon_sessions(ctx: &CoachContext) -> Option<RuleHit> {
    const MAX_ABANDON_RATE: f64 = 0.4;
    const MIN_SAMPLE: u64 = 10;

    let mut count = 0u64;
    let mut total = 0u64;
    let mut examples = Vec::new();
    for session in ctx.conversational_sessions() {
        total += 1;
        if session.turns.len() == 1 {
            count += 1;
            if examples.len() < MAX_EXAMPLES {
                let first = &session.turns[0];
                if !first.user_message.is_empty() {
                    examples.push(example(first.user_message, session.project.clone()));
                }
            }
        }
    }
    (total > 0 && count > MIN_SAMPLE && count as f64 / total as f64 > MAX_ABANDON_RATE).then(|| {
        RuleHit {
            occurrences: count,
            total,
            pct: ratio_pct(count, total),
            stat: None,
            escalate: false,
            examples,
        }
    })
}

/// More than 15% of turns canceled; escalates past 30%. Only tools with a
/// cancellation signal enter the denominator.
fn high_cancellation(ctx: &CoachContext) -> Option<RuleHit> {
    const MAX_CANCEL_RATE: f64 = 0.15;
    const HIGH_SEVERITY_RATE: f64 = 0.3;

    let mut count = 0u64;
    let mut total = 0u64;
    let mut examples = Vec::new();
    for turn in ctx.turns().filter(|t| signals::supports_cancel(t.tool)) {
        total += 1;
        if turn.is_canceled {
            count += 1;
            if examples.len() < MAX_EXAMPLES && !turn.user_message.is_empty() {
                examples.push(example(turn.user_message, String::new()));
            }
        }
    }
    let rate = if total == 0 {
        0.0
    } else {
        count as f64 / total as f64
    };
    (total > 0 && rate > MAX_CANCEL_RATE).then(|| RuleHit {
        occurrences: count,
        total,
        pct: ratio_pct(count, total),
        stat: None,
        escalate: rate > HIGH_SEVERITY_RATE,
        examples,
    })
}

/// More than 60% of active days show fragmented flow (min 5 scored days);
/// escalates past 80%.
fn broken_flow_state(ctx: &CoachContext) -> Option<RuleHit> {
    const LOW_SCORE_RATE: f64 = 0.6;
    const HIGH_RATE: f64 = 0.8;
    const MIN_DAYS: u64 = 5;

    let days = flow::flow_days(&ctx.sessions);
    let total = days.len() as u64;
    if total < MIN_DAYS {
        return None;
    }
    let fragmented = days.iter().filter(|d| d.score < flow::FLOW_SHALLOW).count() as u64;
    let avg_score =
        (days.iter().map(|d| d.score).sum::<u64>() as f64 / total as f64).round() as u64;
    let rate = fragmented as f64 / total as f64;
    (rate > LOW_SCORE_RATE).then(|| RuleHit {
        occurrences: fragmented,
        total,
        pct: ratio_pct(fragmented, total),
        stat: Some(avg_score.to_string()),
        escalate: rate > HIGH_RATE,
        examples: Vec::new(),
    })
}

/// More than 5 turns taking over 5 minutes.
///
/// Deviation from the reference: the threshold is raised from 30s to 300s
/// because tokens' turns span whole agentic CLI runs, where 30s is routine
/// rather than a smell. 300s matches the flow score's slowest latency band.
fn slow_responses(ctx: &CoachContext) -> Option<RuleHit> {
    const SLOW_MS: u64 = 300_000;
    const MIN_COUNT: u64 = 5;

    let mut count = 0u64;
    let mut total = 0u64;
    let mut slow_sum_ms = 0u64;
    let mut examples = Vec::new();
    for turn in ctx.turns().filter(|t| t.elapsed_ms.is_some()) {
        total += 1;
        let elapsed = turn.elapsed_ms.unwrap_or(0);
        if elapsed > SLOW_MS {
            count += 1;
            slow_sum_ms += elapsed;
            if examples.len() < MAX_EXAMPLES && !turn.user_message.is_empty() {
                examples.push(example(turn.user_message, format!("{}s", elapsed / 1000)));
            }
        }
    }
    (count > MIN_COUNT).then(|| RuleHit {
        occurrences: count,
        total,
        pct: None,
        stat: Some((slow_sum_ms / count / 1000).to_string()),
        escalate: false,
        examples,
    })
}

/// Turns running 40+ tool calls, three or more times.
///
/// Deviation from the reference: the threshold is raised from 15 to 40
/// because tokens' rows aggregate full agentic CLI turns, where 15+ tool
/// calls is routine rather than a loop signal.
fn runaway_agent_loops(ctx: &CoachContext) -> Option<RuleHit> {
    const MIN_TOOLS_PER_TURN: u64 = 40;
    const MIN_TURNS: u64 = 3;

    let mut count = 0u64;
    let mut total = 0u64;
    let mut tools_sum = 0u64;
    let mut examples = Vec::new();
    for turn in ctx.turns() {
        total += 1;
        if turn.tool_calls >= MIN_TOOLS_PER_TURN {
            count += 1;
            tools_sum += turn.tool_calls;
            if examples.len() < MAX_EXAMPLES && !turn.user_message.is_empty() {
                examples.push(example(
                    turn.user_message,
                    format!("{} tool calls", turn.tool_calls),
                ));
            }
        }
    }
    (count >= MIN_TURNS).then(|| RuleHit {
        occurrences: count,
        total,
        pct: None,
        stat: Some((tools_sum / count.max(1)).to_string()),
        escalate: false,
        examples,
    })
}

#[cfg(test)]
mod tests {
    use super::super::run_rules;
    use crate::coach::testutil::{call, ctx_calls};
    use crate::coach::CoachContext;

    fn triggered_ids(calls: &[crate::tools::ParsedCall]) -> Vec<&'static str> {
        let refs = ctx_calls(calls);
        let ctx = CoachContext::new(&refs);
        run_rules(&ctx).into_iter().map(|o| o.id).collect()
    }

    #[test]
    fn mega_sessions_triggers_at_fifty_turns() {
        let calls: Vec<_> = (0..50)
            .map(|i| call("mega", i, &format!("turn number {i} doing more work")))
            .collect();
        assert!(triggered_ids(&calls).contains(&"mega-sessions"));

        let calls: Vec<_> = (0..20)
            .map(|i| call("ok", i, &format!("turn number {i} doing more work")))
            .collect();
        assert!(!triggered_ids(&calls).contains(&"mega-sessions"));
    }

    #[test]
    fn abandon_sessions_needs_single_turn_majority() {
        // 11 single-turn sessions + 2 healthy ones: ratio 11/13 > 0.4.
        let mut calls = Vec::new();
        for i in 0..11 {
            calls.push(call(&format!("solo{i}"), i, "one and done question"));
        }
        for i in 0..4 {
            calls.push(call("healthy-a", 20 + i, &format!("iterating message {i}")));
            calls.push(call("healthy-b", 30 + i, &format!("other thread step {i}")));
        }
        assert!(triggered_ids(&calls).contains(&"abandon-sessions"));
    }

    #[test]
    fn high_cancellation_only_counts_tools_with_signal() {
        let mut calls = Vec::new();
        for i in 0..10 {
            let mut c = call(&format!("s{i}"), i, &format!("do the thing {i}"));
            c.is_canceled = i % 2 == 0;
            calls.push(c);
        }
        let ids = triggered_ids(&calls);
        assert!(ids.contains(&"high-cancellation"));

        for c in &mut calls {
            c.tool = crate::tools::cursor::config::TOOL_ID;
        }
        assert!(!triggered_ids(&calls).contains(&"high-cancellation"));
    }

    #[test]
    fn slow_responses_uses_the_raised_cli_threshold() {
        let calls: Vec<_> = (0..7)
            .map(|i| {
                let mut c = call(&format!("s{i}"), i, &format!("long running request {i}"));
                c.elapsed_ms = Some(400_000);
                c
            })
            .collect();
        assert!(triggered_ids(&calls).contains(&"slow-responses"));

        let routine: Vec<_> = (0..7)
            .map(|i| {
                let mut c = call(&format!("s{i}"), i, &format!("normal agentic turn {i}"));
                c.elapsed_ms = Some(45_000);
                c
            })
            .collect();
        assert!(!triggered_ids(&routine).contains(&"slow-responses"));
    }

    #[test]
    fn runaway_agent_loops_uses_the_raised_cli_threshold() {
        let heavy: Vec<_> = (0..3)
            .map(|i| {
                let mut c = call(&format!("s{i}"), i, &format!("massive agentic task {i}"));
                c.tools = (0..45).map(|n| format!("Tool{n}")).collect();
                c
            })
            .collect();
        assert!(triggered_ids(&heavy).contains(&"runaway-agent-loops"));

        let normal: Vec<_> = (0..3)
            .map(|i| {
                let mut c = call(&format!("s{i}"), i, &format!("normal agentic task {i}"));
                c.tools = (0..20).map(|n| format!("Tool{n}")).collect();
                c
            })
            .collect();
        assert!(!triggered_ids(&normal).contains(&"runaway-agent-loops"));
    }

    #[test]
    fn session_drift_counts_distinct_work_types() {
        let topics = [
            "fix the crash in the parser",
            "write tests for the archive",
            "update the readme documentation",
            "deploy the docker pipeline",
            "restyle the css layout",
        ];
        let mut calls = Vec::new();
        for s in 0..4 {
            for (i, topic) in topics.iter().enumerate() {
                calls.push(call(&format!("drift{s}"), (s * 10 + i) as i64, topic));
            }
        }
        assert!(triggered_ids(&calls).contains(&"session-drift"));
    }
}
