//! Per-project activity detail for the Activity → Projects view.
//!
//! Aggregates each project's turns, estimated active time, code-output
//! languages, most-edited files, and a coarse work-pattern classification.
//! Active time reuses the timeline's block model: call timestamps split into
//! blocks at gaps over 15 minutes, each block contributing its span (minimum
//! one minute).
//! The pattern thresholds are display heuristics of this port, not upstream
//! rules; they are documented in docs/development/coach.md.

use chrono::{Datelike, Local, Timelike, Weekday};

use super::sessions::CoachSession;
use super::CoachContext;

/// Weekend share of timestamped turns at or above this reads as
/// weekend-dominant; at or below [`WEEKDAY_SHARE_MAX`] as weekday-dominant
/// (uniform activity across a week puts ~29% on weekends).
const WEEKEND_SHARE_MIN: f64 = 0.6;
const WEEKDAY_SHARE_MAX: f64 = 0.2;

/// A daypart must carry at least this share of timestamped turns to be
/// called out (four buckets - a uniform spread sits at 25%).
const DAYPART_SHARE_MIN: f64 = 0.4;

/// Dayparts, index-aligned with [`DAYPART_IDS`]: morning 05-11, afternoon
/// 12-16, evening 17-21, late night 22-04 (matching the pace module's
/// late-night window).
const DAYPART_IDS: [&str; 4] = ["mornings", "afternoons", "evenings", "late_nights"];

pub struct ProjectActivity {
    /// Project identity (feed through the dashboard label lookup for display).
    pub identity: String,
    pub turns: u64,
    /// Estimated active minutes across the project's session blocks.
    pub active_minutes: u64,
    /// (language, loc), descending - the project's observed tech stack.
    pub languages: Vec<(String, u64)>,
    /// (display path, edit count), descending.
    pub hot_files: Vec<(String, u64)>,
    /// Weekday/weekend mix id ("mostly_weekdays" | "mostly_weekends" |
    /// "mixed_days"); empty without timestamped turns.
    pub days_id: &'static str,
    /// Dominant daypart id from [`DAYPART_IDS`]; empty when none dominates.
    pub time_id: &'static str,
}

#[derive(Default)]
struct Acc {
    turns: u64,
    timestamped_turns: u64,
    weekend_turns: u64,
    daypart_turns: [u64; 4],
    active_minutes: u64,
    languages: std::collections::HashMap<String, u64>,
    files: std::collections::HashMap<String, u64>,
}

/// Aggregate per-project activity, ordered by active minutes then turns.
pub fn project_activity(ctx: &CoachContext) -> Vec<ProjectActivity> {
    let mut by_project: std::collections::HashMap<String, Acc> = std::collections::HashMap::new();

    for session in &ctx.sessions {
        let identity = crate::ingest::projects::project_identity(&session.project);
        let acc = by_project.entry(identity).or_default();
        acc.active_minutes += session_active_minutes(session);
        for turn in &session.turns {
            acc.turns += 1;
            if let Some(ts) = turn.timestamp {
                let local = ts.with_timezone(&Local);
                acc.timestamped_turns += 1;
                if matches!(local.weekday(), Weekday::Sat | Weekday::Sun) {
                    acc.weekend_turns += 1;
                }
                acc.daypart_turns[daypart_index(local.hour())] += 1;
            }
            for block in &turn.code_blocks {
                *acc.languages.entry(block.language.clone()).or_insert(0) += block.loc;
            }
            for call in &turn.calls {
                for file in &call.edited_files {
                    *acc.files
                        .entry(display_file(&call.project, file))
                        .or_insert(0) += 1;
                }
            }
        }
    }

    let mut rows: Vec<ProjectActivity> = by_project
        .into_iter()
        .map(|(identity, acc)| ProjectActivity {
            identity,
            turns: acc.turns,
            active_minutes: acc.active_minutes,
            languages: sorted_desc(acc.languages),
            hot_files: sorted_desc(acc.files),
            days_id: days_id(acc.weekend_turns, acc.timestamped_turns),
            time_id: time_id(&acc.daypart_turns, acc.timestamped_turns),
        })
        .collect();
    rows.sort_by(|a, b| {
        b.active_minutes
            .cmp(&a.active_minutes)
            .then_with(|| b.turns.cmp(&a.turns))
            .then_with(|| a.identity.cmp(&b.identity))
    });
    rows
}

/// Estimated active minutes for one session: sorted call timestamps split
/// into blocks at >15 minute gaps (the timeline Gantt's model), each block
/// spanning at least one minute.
fn session_active_minutes(session: &CoachSession<'_>) -> u64 {
    let mut minutes: Vec<i64> = session
        .turns
        .iter()
        .flat_map(|turn| turn.calls.iter())
        .filter_map(|call| call.timestamp.map(|ts| ts.timestamp() / 60))
        .collect();
    if minutes.is_empty() {
        return 0;
    }
    minutes.sort_unstable();

    let mut total = 0u64;
    let mut block_start = minutes[0];
    let mut block_end = minutes[0];
    for minute in minutes.into_iter().skip(1) {
        if minute - block_end > super::timeline::BLOCK_GAP_MIN {
            total += (block_end - block_start).max(1) as u64;
            block_start = minute;
        }
        block_end = minute;
    }
    total + (block_end - block_start).max(1) as u64
}

fn daypart_index(hour: u32) -> usize {
    match hour {
        5..=11 => 0,
        12..=16 => 1,
        17..=21 => 2,
        _ => 3,
    }
}

fn days_id(weekend_turns: u64, timestamped_turns: u64) -> &'static str {
    if timestamped_turns == 0 {
        return "";
    }
    let share = weekend_turns as f64 / timestamped_turns as f64;
    if share >= WEEKEND_SHARE_MIN {
        "mostly_weekends"
    } else if share <= WEEKDAY_SHARE_MAX {
        "mostly_weekdays"
    } else {
        "mixed_days"
    }
}

fn time_id(daypart_turns: &[u64; 4], timestamped_turns: u64) -> &'static str {
    if timestamped_turns == 0 {
        return "";
    }
    let (index, &top) = daypart_turns
        .iter()
        .enumerate()
        .max_by_key(|(_, count)| **count)
        .expect("four dayparts");
    if top as f64 / timestamped_turns as f64 >= DAYPART_SHARE_MIN {
        DAYPART_IDS[index]
    } else {
        ""
    }
}

/// Shorten an edited-file path for display: project-relative when the path
/// sits under the call's project directory, otherwise the trailing two
/// segments of an absolute path.
fn display_file(project: &str, file: &str) -> String {
    let project = project.trim_end_matches('/');
    if !project.is_empty() {
        if let Some(rest) = file.strip_prefix(project) {
            let rest = rest.trim_start_matches('/');
            if !rest.is_empty() {
                return rest.to_string();
            }
        }
    }
    if file.starts_with('/') {
        let segments: Vec<&str> = file.rsplit('/').take(2).collect();
        return segments.into_iter().rev().collect::<Vec<_>>().join("/");
    }
    file.to_string()
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
    use crate::coach::CoachContext;

    #[test]
    fn blocks_split_at_gaps_and_lone_calls_count_one_minute() {
        // 0..10 continuous, then a two-hour gap to a lone call: 10 + 1.
        let calls = vec![
            call("s", 0, "start the refactor"),
            call("s", 10, "keep going with it"),
            call("s", 130, "one more tweak later"),
        ];
        let refs = ctx_calls(&calls);
        let ctx = CoachContext::new(&refs);
        let rows = project_activity(&ctx);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].active_minutes, 11);
        assert_eq!(rows[0].turns, 3);
    }

    #[test]
    fn parallel_sessions_accumulate_per_project() {
        // Two overlapping sessions of the same project each contribute their
        // own block span.
        let calls = vec![
            call("a", 0, "session a starts here"),
            call("a", 10, "session a continues on"),
            call("b", 5, "session b starts here"),
            call("b", 12, "session b continues on"),
        ];
        let refs = ctx_calls(&calls);
        let ctx = CoachContext::new(&refs);
        let rows = project_activity(&ctx);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].active_minutes, 17);
    }

    #[test]
    fn languages_and_hot_files_rank_by_volume() {
        let mut a = with_code(call("s", 0, "write the rust core"), "rust", 120);
        a.edited_files = vec!["/tmp/proj/src/main.rs".into(), "src/lib.rs".into()];
        let mut b = with_code(call("s", 5, "now the svelte view"), "svelte", 40);
        b.edited_files = vec!["/tmp/proj/src/main.rs".into()];
        let calls = vec![a, b];
        let refs = ctx_calls(&calls);
        let ctx = CoachContext::new(&refs);
        let rows = project_activity(&ctx);
        assert_eq!(rows[0].languages[0], ("rust".to_string(), 120));
        assert_eq!(rows[0].languages[1], ("svelte".to_string(), 40));
        assert_eq!(rows[0].hot_files[0], ("src/main.rs".to_string(), 2));
        assert_eq!(rows[0].hot_files[1], ("src/lib.rs".to_string(), 1));
    }

    #[test]
    fn day_mix_classification_uses_weekend_share() {
        assert_eq!(days_id(0, 0), "");
        assert_eq!(days_id(0, 10), "mostly_weekdays");
        assert_eq!(days_id(2, 10), "mostly_weekdays");
        assert_eq!(days_id(4, 10), "mixed_days");
        assert_eq!(days_id(6, 10), "mostly_weekends");
    }

    #[test]
    fn daypart_needs_a_dominant_share() {
        assert_eq!(time_id(&[10, 0, 0, 0], 10), "mornings");
        assert_eq!(time_id(&[4, 3, 2, 1], 10), "mornings");
        assert_eq!(time_id(&[3, 3, 2, 2], 10), "");
        assert_eq!(time_id(&[0, 0, 1, 9], 10), "late_nights");
        assert_eq!(time_id(&[0, 0, 0, 0], 0), "");
    }

    #[test]
    fn display_paths_are_project_relative_or_trailing_segments() {
        assert_eq!(
            display_file("/tmp/proj", "/tmp/proj/src/app.rs"),
            "src/app.rs"
        );
        assert_eq!(display_file("/tmp/proj", "src/app.rs"), "src/app.rs");
        assert_eq!(
            display_file("/tmp/proj", "/somewhere/else/deep/file.rs"),
            "deep/file.rs"
        );
        assert_eq!(display_file("", "notes.md"), "notes.md");
    }
}
