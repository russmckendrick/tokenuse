//! Local, deterministic coaching engine over archived per-call rows.
//!
//! Core algorithms (practice scoring, rule catalog, flow/pace analysis) are
//! ported from Microsoft's AI-Engineering-Coach VS Code extension
//! (https://github.com/microsoft/AI-Engineering-Coach), MIT License,
//! Copyright (c) Microsoft Corporation. See NOTICE at the repository root.
//! Everything runs on-device; there are no network calls and no LLM calls.

pub mod flow;
pub mod output;
pub mod pace;
pub mod rules;
pub mod score;
pub mod sessions;
pub mod signals;
pub mod text;
pub mod timeline;

use chrono::NaiveDate;

use crate::tools::ParsedCall;
use sessions::{CoachSession, Turn};

pub struct CoachContext<'a> {
    pub sessions: Vec<CoachSession<'a>>,
}

impl<'a> CoachContext<'a> {
    /// Build the evaluation context from timestamp-sorted calls.
    pub fn new(calls: &[&'a ParsedCall]) -> Self {
        Self {
            sessions: sessions::group_sessions(calls),
        }
    }

    pub fn turns(&self) -> impl Iterator<Item = &Turn<'a>> {
        self.sessions.iter().flat_map(|s| s.turns.iter())
    }

    /// Turns carrying a prompt-length signal - the denominator for
    /// prompt-shaped ratio rules (rows from pre-enrichment archives or
    /// tools without the signal are excluded).
    pub fn prompt_turns(&self) -> impl Iterator<Item = &Turn<'a>> {
        self.turns()
            .filter(|t| t.prompt_chars.is_some_and(|c| c > 0) && !t.user_message.is_empty())
    }

    pub fn timestamped_turns(&self) -> impl Iterator<Item = &Turn<'a>> {
        self.turns().filter(|t| t.timestamp.is_some())
    }

    /// Sessions of tools whose session ids represent real conversations
    /// (session-shape rules would misfire on aggregate-row tools).
    pub fn conversational_sessions(&self) -> impl Iterator<Item = &CoachSession<'a>> {
        self.sessions
            .iter()
            .filter(|s| signals::has_conversational_sessions(s.tool))
    }
}

const MAX_FLOW_DAYS: usize = 14;
const MAX_TIMELINE_DAYS: usize = 60;
const MAX_LIST_ROWS: usize = 12;

fn leak(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

fn fmt_delta(pct: Option<i64>) -> &'static str {
    match pct {
        Some(v) => leak(format!("{v:+}%")),
        None => "–",
    }
}

fn scale(value: u64, max: u64) -> u64 {
    match (value * 100).checked_div(max) {
        Some(scaled) => scaled.clamp(1, 100),
        None => 0,
    }
}

fn count_rows(rows: Vec<(String, u64)>, cap: usize) -> Vec<crate::data::CountMetric> {
    let max = rows.iter().map(|r| r.1).max().unwrap_or(0);
    rows.into_iter()
        .take(cap)
        .map(|(name, loc)| crate::data::CountMetric {
            name: leak(name),
            calls: loc,
            value: scale(loc, max),
        })
        .collect()
}

/// Build the full Coach page payload from period-filtered calls.
pub fn coach_data(calls: &[&ParsedCall], today: NaiveDate) -> crate::data::CoachData {
    let ctx = CoachContext::new(calls);
    let outcomes = rules::run_rules(&ctx);
    let weekly = score::weekly_group_scores(calls);
    let groups = score::group_scores(&outcomes, &weekly);

    let practice_groups = groups
        .into_iter()
        .map(|g| crate::data::PracticeGroupScore {
            id: g.group.id(),
            score: g.score,
            wow: fmt_delta(g.wow_pct),
            mom: fmt_delta(g.mom_pct),
            triggered: g.triggered,
            total_rules: g.total_rules,
            top_rule_id: g.top_rule.unwrap_or(""),
        })
        .collect();

    let mut sorted = outcomes;
    sorted.sort_by(|a, b| {
        b.effective_severity()
            .penalty()
            .cmp(&a.effective_severity().penalty())
            .then_with(|| b.hit.occurrences.cmp(&a.hit.occurrences))
    });
    let findings = sorted
        .into_iter()
        .map(|o| crate::data::CoachFinding {
            rule_id: o.id,
            group: o.group.id(),
            severity: o.effective_severity().id(),
            occurrences: o.hit.occurrences,
            total: o.hit.total,
            pct: o.hit.pct.map(|p| leak(format!("{p}%"))).unwrap_or(""),
            stat: o.hit.stat.map(leak).unwrap_or(""),
            examples: o
                .hit
                .examples
                .into_iter()
                .map(|e| crate::data::FindingExample {
                    text: leak(e.text),
                    detail: leak(e.detail),
                })
                .collect(),
        })
        .collect();

    let flow_stats = flow::flow_summary(&ctx.sessions);
    let mut flow_days = flow_stats.days;
    flow_days.sort_by_key(|d| std::cmp::Reverse(d.day));
    let flow = crate::data::FlowSummary {
        overall_score: flow_stats.overall_score,
        label_id: flow::flow_label(flow_stats.overall_score),
        avg_followup: flow_stats
            .avg_followup_ms
            .map(|ms| leak(format!("{}s", ms / 1000)))
            .unwrap_or("–"),
        avg_block: leak(format!("{} min", flow_stats.avg_longest_block_min)),
        deep_days: flow_stats.deep_days,
        total_days: flow_days.len() as u64,
        days: flow_days
            .into_iter()
            .take(MAX_FLOW_DAYS)
            .map(|d| crate::data::FlowDayMetric {
                day: leak(d.day.format("%Y-%m-%d").to_string()),
                score: d.score,
                label_id: flow::flow_label(d.score),
                longest_block_min: d.longest_block_min,
                active_min: d.active_min,
                sessions: d.sessions,
            })
            .collect(),
    };

    let pace_stats = pace::pace_stats(&ctx, today);
    let pace = crate::data::PaceSummary {
        current_streak: pace_stats.current_streak,
        longest_streak: pace_stats.longest_streak,
        late_night_pct: pace_stats.late_night_pct,
        weekend_pct: pace_stats.weekend_pct,
        risk_id: pace_stats.risk.id(),
        alert_ids: pace_stats.alerts.iter().map(|a| a.id()).collect(),
    };

    let output_stats = output::output_stats(&ctx);
    let uncovered: Vec<&'static str> = output_stats
        .uncovered_tools
        .iter()
        .map(|t| crate::ingest::projects::tool_short_label(t))
        .collect();
    let output = crate::data::OutputSummary {
        total_loc: leak(format_count(output_stats.total_loc)),
        by_language: count_rows(output_stats.by_language, MAX_LIST_ROWS),
        by_day: count_rows(
            output_stats
                .by_day
                .into_iter()
                .rev()
                .take(MAX_LIST_ROWS)
                .map(|(day, loc)| (day.format("%Y-%m-%d").to_string(), loc))
                .collect(),
            MAX_LIST_ROWS,
        ),
        by_project: count_rows(
            output_stats
                .by_project
                .into_iter()
                .map(|(project, loc)| (crate::ingest::projects::raw_project_display(&project), loc))
                .collect(),
            MAX_LIST_ROWS,
        ),
        by_model: count_rows(output_stats.by_model, MAX_LIST_ROWS),
        uncovered_tools: if uncovered.is_empty() {
            ""
        } else {
            leak(uncovered.join(", "))
        },
    };

    let timeline_days = timeline::active_days(&ctx.sessions)
        .into_iter()
        .take(MAX_TIMELINE_DAYS)
        .map(|d| leak(d.format("%Y-%m-%d").to_string()))
        .collect();

    crate::data::CoachData {
        practice_groups,
        findings,
        flow,
        pace,
        output,
        timeline_days,
    }
}

/// Build one day's Gantt rows from tool/project-filtered calls.
pub fn coach_timeline(
    calls: &[&ParsedCall],
    day: NaiveDate,
    currency: &crate::currency::CurrencyFormatter,
) -> Option<crate::data::CoachTimelineDay> {
    let ctx = CoachContext::new(calls);
    let day_data = timeline::timeline_day(&ctx.sessions, day)?;
    Some(crate::data::CoachTimelineDay {
        day: day_data.day.format("%Y-%m-%d").to_string(),
        max_concurrent: day_data.max_concurrent,
        window_start_min: day_data.window_start_min,
        window_end_min: day_data.window_end_min,
        rows: day_data
            .rows
            .into_iter()
            .map(|row| crate::data::TimelineSessionRow {
                session_key: row.session_key,
                project: crate::ingest::projects::raw_project_display(&row.project),
                tool: row.tool.to_string(),
                tool_label: crate::ingest::projects::tool_short_label(row.tool).to_string(),
                turns: row.turns,
                cost: currency.format_money(row.cost_usd),
                blocks: row
                    .blocks
                    .into_iter()
                    .map(|(start_min, end_min)| crate::data::TimelineBlock { start_min, end_min })
                    .collect(),
            })
            .collect(),
    })
}

/// Parse a "YYYY-MM-DD" day string (the desktop crate has no chrono dep).
pub fn parse_day(day: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(day, "%Y-%m-%d").ok()
}

fn format_count(value: u64) -> String {
    let raw = value.to_string();
    let bytes = raw.as_bytes();
    let mut out = String::new();
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 && (bytes.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(*b as char);
    }
    out
}

#[cfg(test)]
pub(crate) mod testutil {
    use chrono::{TimeZone, Utc};

    use crate::tools::{CodeBlock, ParsedCall};

    /// A synthetic enriched call: `minute` offsets an arbitrary base time.
    pub fn call(session: &str, minute: i64, prompt: &str) -> ParsedCall {
        ParsedCall {
            tool: crate::tools::claude_code::config::TOOL_ID,
            model: "claude-opus-4-7".into(),
            input_tokens: 200,
            output_tokens: 100,
            timestamp: Some(
                Utc.with_ymd_and_hms(2026, 6, 1, 9, 0, 0).unwrap()
                    + chrono::Duration::minutes(minute),
            ),
            session_id: session.into(),
            project: "/tmp/proj".into(),
            user_message: prompt.into(),
            prompt_chars: Some(prompt.chars().count() as u64).filter(|c| *c > 0),
            response_chars: Some(400),
            elapsed_ms: Some(5_000),
            ..ParsedCall::default()
        }
    }

    pub fn with_code(mut c: ParsedCall, language: &str, loc: u64) -> ParsedCall {
        c.code_blocks.push(CodeBlock {
            language: language.into(),
            loc,
        });
        c
    }

    pub fn ctx_calls(calls: &[ParsedCall]) -> Vec<&ParsedCall> {
        calls.iter().collect()
    }
}
