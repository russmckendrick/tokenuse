//! TUI Coach page: the practice report card the desktop Coach page renders,
//! reduced to its text core — overall grade, per-group scores, triggered
//! findings, and the advisory setup panel. Findings are selectable; Enter
//! drills into a finding's evidence and again into the evidence's session,
//! and `x` puts a prompt on the Coach ignore list. Wording resolves from the
//! same `coach.*` copy keys the desktop uses.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    prelude::{Frame, Line, Span},
    style::Modifier,
    text::Text,
    widgets::{Paragraph, Widget},
};

use super::sections::{
    render_currency_modal, render_footer, render_model_modal, render_project_modal,
    render_title_bar,
};
use crate::app::App;
use crate::coach::score::{GRADE_TONE_GOOD, GRADE_TONE_WARN};
use crate::copy::{copy, template, CopyDeck};
use crate::data::{CoachData, CoachFinding};
use crate::theme;

pub(super) fn render_coach(frame: &mut Frame<'_>, area: Rect, root: Rect, app: &App) {
    let data = app.coach_for(app.period, app.tool, &app.project_filter);
    let deck = copy();

    let setup_height = if data.setup.is_empty() {
        0
    } else {
        (data.setup.len().min(4) + 2) as u16
    };
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(4),
            Constraint::Min(8),
            Constraint::Length(setup_height),
            Constraint::Length(3),
        ])
        .split(area);

    render_title_bar(frame, sections[0], app);
    render_report_card(frame, sections[1], deck, &data);
    let (selected, evidence) = app.coach_selection(&data);
    match evidence {
        Some(row) => render_evidence(frame, sections[2], deck, &data, selected, row),
        None => render_findings(frame, sections[2], deck, &data, selected),
    }
    if setup_height > 0 {
        render_setup(frame, sections[3], deck, &data);
    }
    render_footer(frame, sections[4], app);
    render_project_modal(frame, root, app);
    render_model_modal(frame, root, app);
    render_currency_modal(frame, root, app);
}

fn grade_label<'a>(deck: &'a CopyDeck, grade_id: &'a str) -> &'a str {
    deck.coach
        .report
        .grade_labels
        .get(grade_id)
        .map(String::as_str)
        .unwrap_or(grade_id)
}

fn score_style(score: u64) -> ratatui::style::Style {
    if score >= GRADE_TONE_GOOD {
        theme::base().fg(theme::GREEN)
    } else if score >= GRADE_TONE_WARN {
        theme::base().fg(theme::YELLOW)
    } else {
        theme::base().fg(theme::RED)
    }
}

fn group_label<'a>(deck: &'a CopyDeck, id: &'a str) -> &'a str {
    let groups = &deck.coach.groups;
    match id {
        "prompt_quality" => groups.prompt_quality.as_str(),
        "session_hygiene" => groups.session_hygiene.as_str(),
        "code_review" => groups.code_review.as_str(),
        "tool_mastery" => groups.tool_mastery.as_str(),
        other => other,
    }
}

fn render_report_card(frame: &mut Frame<'_>, area: Rect, deck: &CopyDeck, data: &CoachData) {
    let triggered: u64 = data.practice_groups.iter().map(|g| g.triggered).sum();
    let total_rules: u64 = data.practice_groups.iter().map(|g| g.total_rules).sum();

    let overall = Line::from(vec![
        Span::styled(format!("{} ", deck.coach.report.overall), theme::key()),
        Span::styled(
            grade_label(deck, data.overall.grade_id).to_string(),
            score_style(data.overall.score).add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!(" {}", data.overall.score), theme::muted()),
        Span::styled("   ", theme::base()),
        Span::styled(
            template(
                &deck.coach.score.rules_triggered,
                &[
                    ("count", triggered.to_string()),
                    ("total", total_rules.to_string()),
                ],
            ),
            theme::muted(),
        ),
    ]);

    let mut groups = Vec::new();
    for (idx, group) in data.practice_groups.iter().enumerate() {
        if idx > 0 {
            groups.push(Span::styled("   ", theme::base()));
        }
        groups.push(Span::styled(
            format!("{} ", group_label(deck, group.id)),
            theme::dim(),
        ));
        groups.push(Span::styled(
            grade_label(deck, group.grade_id).to_string(),
            score_style(group.score).add_modifier(Modifier::BOLD),
        ));
        groups.push(Span::styled(format!(" {}", group.score), theme::muted()));
    }

    Paragraph::new(Text::from(vec![overall, Line::from(groups)]))
        .block(theme::panel_block(deck.nav.coach.as_str(), theme::PRIMARY))
        .style(theme::base())
        .render(area, frame.buffer_mut());
}

fn severity_span(deck: &CopyDeck, finding: &CoachFinding) -> Span<'static> {
    let (label, style) = match finding.severity {
        "high" => (
            deck.coach.findings.high.as_str(),
            theme::base().fg(theme::RED).add_modifier(Modifier::BOLD),
        ),
        "medium" => (deck.coach.findings.medium.as_str(), theme::money()),
        _ => (finding.severity, theme::muted()),
    };
    Span::styled(format!("{label:<15} "), style)
}

/// One finding = a title line and a muted detail line, capped to the panel
/// height; no scrolling, matching the Overview page's static density.
fn render_findings(
    frame: &mut Frame<'_>,
    area: Rect,
    deck: &CopyDeck,
    data: &CoachData,
    selected: usize,
) {
    let block = theme::panel_block(deck.coach.findings.title.as_str(), theme::CYAN);
    let inner = block.inner(area);
    block.render(area, frame.buffer_mut());

    if data.findings.is_empty() {
        Paragraph::new(Line::from(Span::styled(
            deck.coach.findings.empty.as_str(),
            theme::muted(),
        )))
        .style(theme::base())
        .render(inner, frame.buffer_mut());
        return;
    }

    // Two lines per finding; the window follows the selection.
    let capacity = (inner.height as usize / 2).max(1);
    let start = selected.saturating_sub(capacity - 1);
    let mut lines = Vec::new();
    for (idx, finding) in data.findings.iter().enumerate().skip(start).take(capacity) {
        let bg = if idx == selected {
            theme::SURFACE
        } else {
            theme::BACKGROUND
        };
        let (name, when) = match deck.coach.rules.get(finding.rule_id) {
            Some(rule) => (rule.name.as_str(), rule.when_triggered.as_str()),
            None => (finding.rule_id, ""),
        };
        let mut head = vec![
            severity_span(deck, finding),
            Span::styled(
                name.to_string(),
                theme::base().bg(bg).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  {}", group_label(deck, finding.group)),
                theme::dim().bg(bg),
            ),
        ];
        if idx == selected && !finding.examples.is_empty() {
            head.push(Span::styled(
                format!("  {}", deck.coach.findings.evidence_open_hint),
                theme::key().bg(bg),
            ));
        }
        lines.push(Line::from(head).style(theme::base().bg(bg)));
        lines.push(
            Line::from(vec![
                Span::styled(" ".repeat(16), theme::base().bg(bg)),
                Span::styled(
                    template(
                        when,
                        &[
                            ("count", finding.occurrences.to_string()),
                            ("total", finding.total.to_string()),
                            ("pct", finding.pct.to_string()),
                            ("stat", finding.stat.to_string()),
                        ],
                    ),
                    theme::muted().bg(bg),
                ),
            ])
            .style(theme::base().bg(bg)),
        );
    }
    let hidden = data.findings.len().saturating_sub(start + capacity);
    if hidden > 0 {
        if let Some(last) = lines.last_mut() {
            last.spans.push(Span::styled(
                format!(
                    "  {}",
                    template(
                        &deck.coach.findings.more_examples,
                        &[("count", hidden.to_string())],
                    )
                ),
                theme::dim(),
            ));
        }
    }

    Paragraph::new(Text::from(lines))
        .style(theme::base())
        .render(inner, frame.buffer_mut());
}

/// The evidence drill-in: every captured sample for one finding, two lines
/// each (index/when/detail, then the snippet), with the selected row
/// highlighted. Enter opens the row's session; `x` ignores a prompt.
fn render_evidence(
    frame: &mut Frame<'_>,
    area: Rect,
    deck: &CopyDeck,
    data: &CoachData,
    finding_idx: usize,
    selected: usize,
) {
    let Some(finding) = data.findings.get(finding_idx) else {
        return;
    };
    let name = deck
        .coach
        .rules
        .get(finding.rule_id)
        .map(|rule| rule.name.as_str())
        .unwrap_or(finding.rule_id);
    let title = template(
        &deck.coach.findings.evidence_title,
        &[("rule", name.to_string())],
    );
    let block = theme::panel_block(title.as_str(), theme::CYAN);
    let inner = block.inner(area);
    block.render(area, frame.buffer_mut());

    let footer_rows = usize::from(data.ignored_prompts > 0);
    let capacity = ((inner.height as usize).saturating_sub(footer_rows) / 2).max(1);
    let start = selected.saturating_sub(capacity - 1);
    let mut lines = Vec::new();
    for (idx, example) in finding
        .examples
        .iter()
        .enumerate()
        .skip(start)
        .take(capacity)
    {
        let bg = if idx == selected {
            theme::SURFACE
        } else {
            theme::BACKGROUND
        };
        let mut head = vec![Span::styled(
            format!("{:02}  ", idx + 1),
            theme::dim().bg(bg),
        )];
        if !example.when.is_empty() {
            head.push(Span::styled(
                format!("{}  ", example.when),
                theme::muted().bg(bg),
            ));
        }
        if !example.detail.is_empty() {
            head.push(Span::styled(
                format!("{}  ", example.detail),
                theme::muted().bg(bg),
            ));
        }
        let action = if example.session_key.is_empty() {
            deck.coach.findings.no_session.as_str()
        } else {
            deck.coach.findings.open_session.as_str()
        };
        head.push(Span::styled(action.to_string(), theme::dim().bg(bg)));
        if !example.prompt_key.is_empty() {
            head.push(Span::styled(
                format!(
                    "  {} {}",
                    deck.coach.findings.ignore_key, deck.coach.findings.ignore_prompt
                ),
                theme::key().bg(bg),
            ));
        }
        lines.push(Line::from(head).style(theme::base().bg(bg)));
        lines.push(
            Line::from(vec![
                Span::styled("    ", theme::base().bg(bg)),
                Span::styled(format!("\"{}\"", example.text), theme::base().bg(bg)),
            ])
            .style(theme::base().bg(bg)),
        );
    }
    if footer_rows > 0 {
        lines.push(Line::from(vec![
            Span::styled(
                template(
                    &deck.coach.findings.ignored_count,
                    &[("count", data.ignored_prompts.to_string())],
                ),
                theme::dim(),
            ),
            Span::styled(
                format!("  {} ", deck.coach.findings.restore_key),
                theme::key(),
            ),
            Span::styled(deck.coach.findings.restore_ignored.as_str(), theme::muted()),
        ]));
    }

    Paragraph::new(Text::from(lines))
        .style(theme::base())
        .render(inner, frame.buffer_mut());
}

fn render_setup(frame: &mut Frame<'_>, area: Rect, deck: &CopyDeck, data: &CoachData) {
    let block = theme::panel_block(deck.coach.setup.heading.as_str(), theme::YELLOW);
    let inner = block.inner(area);
    block.render(area, frame.buffer_mut());

    let lines: Vec<Line<'_>> = data
        .setup
        .iter()
        .take(inner.height as usize)
        .map(|finding| {
            Line::from(vec![
                Span::styled(finding.title, theme::base()),
                Span::styled(" · ", theme::dim()),
                Span::styled(finding.savings_label, theme::money()),
            ])
        })
        .collect();

    Paragraph::new(Text::from(lines))
        .style(theme::base())
        .render(inner, frame.buffer_mut());
}
