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
pub mod projects;
pub mod rules;
pub mod score;
pub mod sessions;
pub mod signals;
pub mod text;
pub mod timeline;

use chrono::{DateTime, Local, NaiveDate};

use crate::app::Period;
use crate::tools::ParsedCall;
use sessions::{CoachSession, Turn};

pub struct CoachContext<'a> {
    pub sessions: Vec<CoachSession<'a>>,
}

pub(crate) fn exact_timestamp(call: &ParsedCall) -> Option<DateTime<chrono::Utc>> {
    has_exact_timing(call).then_some(call.timestamp).flatten()
}

pub(crate) fn has_exact_timing(call: &ParsedCall) -> bool {
    call.timestamp_quality == crate::tools::TimestampQuality::Exact
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
const MAX_LIST_ROWS: usize = 12;
const MAX_PROJECT_LANGUAGES: usize = 6;
const MAX_PROJECT_HOT_FILES: usize = 3;

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

/// Build the full Coach page payload. `calls` is period-filtered and drives
/// every panel except the activity calendar; `calendar_calls` carries the
/// same tool/project filters but ignores the period, because the calendar
/// window is wider than the period: scoped periods render a trailing
/// context window ([`timeline::grid_window_days`]) with out-of-period days
/// flagged via `in_period`, and All Time renders the trailing year.
pub fn coach_data(
    calls: &[&ParsedCall],
    calendar_calls: &[&ParsedCall],
    period: Period,
    now: DateTime<Local>,
) -> crate::data::CoachData {
    let ctx = CoachContext::new(calls);
    // Dashboard-consistent short labels: the same period-filtered peer set
    // `build_dashboard` uses, so names join across pages.
    let project_labels =
        crate::ingest::projects::project_label_lookup(calls.iter().map(|c| c.project.as_str()));
    let outcomes = rules::run_rules(&ctx);
    let weekly = score::weekly_group_scores(calls);
    let groups = score::group_scores(&outcomes, &weekly);

    let overall_score = score::composite_score(&groups);
    let overall = crate::data::CoachOverall {
        score: overall_score,
        grade_id: score::grade_id(overall_score),
    };

    let practice_groups = groups
        .into_iter()
        .map(|g| crate::data::PracticeGroupScore {
            id: g.group.id(),
            score: g.score,
            grade_id: score::grade_id(g.score),
            wow: fmt_delta(g.wow_pct),
            mom: fmt_delta(g.mom_pct),
            trend: g.trend,
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
        fragmented_days: flow_stats.fragmented_days,
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

    let pace_stats = pace::pace_stats(&ctx, now.date_naive());
    let pace = crate::data::PaceSummary {
        current_streak: pace_stats.current_streak,
        longest_streak: pace_stats.longest_streak,
        late_night_pct: pace_stats.late_night_pct,
        weekend_pct: pace_stats.weekend_pct,
        risk_id: pace_stats.risk.id(),
        alert_ids: pace_stats.alerts.iter().map(|a| a.id()).collect(),
    };

    let output_stats = output::output_stats(&ctx);
    let trend = count_rows(
        output::trend_rows(&output_stats, period, now)
            .into_iter()
            .rev()
            .collect(),
        usize::MAX,
    );
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
                .map(|(day, loc)| (day.format("%Y-%m-%d").to_string(), loc))
                .collect(),
            usize::MAX,
        ),
        trend,
        by_project: count_rows(
            output_stats
                .by_project
                .into_iter()
                .map(|(project, loc)| {
                    (
                        crate::ingest::projects::project_label(&project_labels, &project),
                        loc,
                    )
                })
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

    let project_rows = projects::project_activity(&ctx)
        .into_iter()
        .map(|row| crate::data::CoachProjectActivity {
            name: leak(crate::ingest::projects::project_label(
                &project_labels,
                &row.identity,
            )),
            active_hours: leak(format_hours(row.active_minutes)),
            turns: row.turns,
            languages: count_rows(row.languages, MAX_PROJECT_LANGUAGES),
            hot_files: row
                .hot_files
                .into_iter()
                .take(MAX_PROJECT_HOT_FILES)
                .map(|(path, _)| leak(path))
                .collect(),
            days_id: row.days_id,
            time_id: row.time_id,
        })
        .collect();

    let calendar_ctx = CoachContext::new(calendar_calls);
    let today = now.date_naive();
    let (period_start, period_end) = period.day_bounds(today);
    let timeline_grid = timeline::daily_turn_counts(
        &calendar_ctx.sessions,
        timeline::grid_window_days(period),
        today,
    )
    .into_iter()
    .map(|(day, turns)| crate::data::TimelineGridDay {
        day: leak(day.format("%Y-%m-%d").to_string()),
        turns,
        in_period: period_start.is_none_or(|start| day >= start) && day <= period_end,
    })
    .collect();

    crate::data::CoachData {
        overall,
        practice_groups,
        findings,
        flow,
        pace,
        output,
        timeline_grid,
        projects: project_rows,
    }
}

/// Build one day's session-table rows from tool/project-filtered calls.
/// Project names use the dashboard-consistent short labels, built over the
/// full filtered call set so the same project reads the same on every day.
pub fn coach_timeline(
    calls: &[&ParsedCall],
    day: NaiveDate,
    currency: &crate::currency::CurrencyFormatter,
) -> Option<crate::data::CoachTimelineDay> {
    let labels =
        crate::ingest::projects::project_label_lookup(calls.iter().map(|c| c.project.as_str()));
    let ctx = CoachContext::new(calls);
    let day_data = timeline::timeline_day(&ctx.sessions, day)?;
    let total_cost: f64 = day_data.rows.iter().map(|row| row.cost_usd).sum();
    Some(crate::data::CoachTimelineDay {
        day: day_data.day.format("%Y-%m-%d").to_string(),
        max_concurrent: day_data.max_concurrent,
        window_start_min: day_data.window_start_min,
        window_end_min: day_data.window_end_min,
        total_cost: currency.format_money(total_cost),
        rows: day_data
            .rows
            .into_iter()
            .map(|row| crate::data::TimelineSessionRow {
                session_key: row.session_key,
                project: crate::ingest::projects::project_label(
                    &labels,
                    &crate::ingest::projects::project_identity(&row.project),
                ),
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

/// Compact duration for the project cards: minutes under an hour, otherwise
/// hours with one decimal (trimmed when whole).
fn format_hours(minutes: u64) -> String {
    if minutes < 60 {
        return format!("{minutes}m");
    }
    let hours = format!("{:.1}", minutes as f64 / 60.0);
    let hours = hours.strip_suffix(".0").unwrap_or(&hours);
    format!("{hours}h")
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
            timestamp_quality: crate::tools::TimestampQuality::Exact,
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

#[cfg(test)]
mod tests {
    use super::testutil::{call, ctx_calls, with_code};
    use super::*;

    /// Local timestamp of the newest synthetic call — a deterministic `now`
    /// anchor regardless of the host timezone.
    fn last_local_time(calls: &[ParsedCall]) -> DateTime<Local> {
        calls
            .iter()
            .filter_map(|c| c.timestamp)
            .max()
            .expect("synthetic calls carry timestamps")
            .with_timezone(&Local)
    }

    #[test]
    fn calendar_days_outside_the_period_are_flagged_as_context() {
        // Activity on two days ten days apart under a Week period: both stay
        // on the calendar (context window), only the newest is in-period.
        let calls = vec![
            call("a", 0, "an early day of work"),
            call("b", 10 * 24 * 60, "a later day of work"),
        ];
        let refs = ctx_calls(&calls);
        let now = last_local_time(&calls);

        let data = coach_data(&refs, &refs, Period::Week, now);
        assert_eq!(data.timeline_grid.len(), 2);
        assert!(!data.timeline_grid[0].in_period, "ten-day-old context day");
        assert!(data.timeline_grid[1].in_period);

        let all_time = coach_data(&refs, &refs, Period::AllTime, now);
        assert!(all_time.timeline_grid.iter().all(|d| d.in_period));
    }

    #[test]
    fn output_by_project_uses_short_labels() {
        // testutil calls live in "/tmp/proj" — the ranking must show the
        // dashboard-style short label, not the raw path.
        let calls = vec![with_code(
            call("s", 0, "write a parser for the config"),
            "rust",
            12,
        )];
        let refs = ctx_calls(&calls);
        let now = last_local_time(&calls);

        let data = coach_data(&refs, &refs, Period::AllTime, now);
        assert_eq!(data.output.by_project.len(), 1);
        assert_eq!(data.output.by_project[0].name, "proj");
        assert_eq!(data.projects.len(), 1);
        assert_eq!(data.projects[0].name, "proj");
        assert_eq!(data.projects[0].turns, 1);
        assert_eq!(data.projects[0].active_hours, "1m");
        assert_eq!(data.projects[0].languages[0].name, "rust");
    }

    #[test]
    fn hours_format_is_compact() {
        assert_eq!(format_hours(0), "0m");
        assert_eq!(format_hours(45), "45m");
        assert_eq!(format_hours(60), "1h");
        assert_eq!(format_hours(126), "2.1h");
        assert_eq!(format_hours(4344), "72.4h");
    }

    #[test]
    fn timeline_rows_use_short_labels() {
        let calls = vec![
            call("s", 0, "start the morning session"),
            call("s", 20, "keep the session going"),
        ];
        let refs = ctx_calls(&calls);
        let day = last_local_time(&calls).date_naive();

        let timeline = coach_timeline(&refs, day, &crate::currency::CurrencyFormatter::usd())
            .expect("synthetic day has rows");
        assert_eq!(timeline.rows.len(), 1);
        assert_eq!(timeline.rows[0].project, "proj");
        assert!(
            !timeline.total_cost.is_empty(),
            "day summary carries a formatted spend total"
        );
    }
}
