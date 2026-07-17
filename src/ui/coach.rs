//! TUI Coach page: the practice report card the desktop Coach page renders,
//! reduced to its text core — overall grade, per-group scores, triggered
//! findings, and the advisory setup panel. Wording resolves from the same
//! `coach.*` copy keys the desktop uses.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    prelude::{Frame, Line, Span},
    style::Modifier,
    text::Text,
    widgets::{Paragraph, Widget},
};

use super::sections::{
    render_currency_modal, render_footer, render_project_modal, render_title_bar,
};
use crate::app::App;
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
    render_findings(frame, sections[2], deck, &data);
    if setup_height > 0 {
        render_setup(frame, sections[3], deck, &data);
    }
    render_footer(frame, sections[4], app);
    render_project_modal(frame, root, app);
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
    if score >= 90 {
        theme::base().fg(theme::GREEN)
    } else if score >= 70 {
        theme::base().fg(theme::CYAN)
    } else if score >= 50 {
        theme::money()
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
fn render_findings(frame: &mut Frame<'_>, area: Rect, deck: &CopyDeck, data: &CoachData) {
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

    let capacity = (inner.height as usize / 2).max(1);
    let mut lines = Vec::new();
    for finding in data.findings.iter().take(capacity) {
        let (name, when) = match deck.coach.rules.get(finding.rule_id) {
            Some(rule) => (rule.name.as_str(), rule.when_triggered.as_str()),
            None => (finding.rule_id, ""),
        };
        lines.push(Line::from(vec![
            severity_span(deck, finding),
            Span::styled(name.to_string(), theme::base().add_modifier(Modifier::BOLD)),
            Span::styled(
                format!("  {}", group_label(deck, finding.group)),
                theme::dim(),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::styled(" ".repeat(16), theme::base()),
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
                theme::muted(),
            ),
        ]));
    }
    if data.findings.len() > capacity {
        let hidden = data.findings.len() - capacity;
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
