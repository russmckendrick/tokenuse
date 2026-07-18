//! TUI Scrollback page: full-text transcript search over the archive.
//! Query input on top (the TUI's only page-level text input), matching
//! sessions grouped below with highlighted snippets, Enter drills into the
//! existing session page.

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
use crate::app::{App, DataSource};
use crate::copy::{copy, template};
use crate::search::{SessionSearchResult, SnippetRole};
use crate::theme;

pub(super) fn render_scrollback(frame: &mut Frame<'_>, area: Rect, root: Rect, app: &App) {
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(2),
            Constraint::Min(5),
            Constraint::Length(3),
        ])
        .split(area);

    render_title_bar(frame, sections[0], app);
    render_query_input(frame, sections[1], app);
    render_results(frame, sections[2], app);
    render_footer(frame, sections[3], app);
    render_project_modal(frame, root, app);
    render_model_modal(frame, root, app);
    render_currency_modal(frame, root, app);
}

fn render_query_input(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let deck = copy();
    let state = &app.scrollback;

    let label_style = if state.input_focused {
        theme::key()
    } else {
        theme::muted().add_modifier(Modifier::BOLD)
    };
    let mut spans = vec![Span::styled(
        format!("{} ", deck.scrollback.input_label),
        label_style,
    )];
    if state.query.is_empty() {
        spans.push(Span::styled(
            deck.scrollback.input_placeholder.as_str(),
            theme::dim(),
        ));
    } else {
        spans.push(Span::styled(
            state.query.clone(),
            theme::base().add_modifier(Modifier::BOLD),
        ));
    }
    if state.input_focused {
        spans.push(Span::styled("_", theme::muted()));
    }

    let status_line = if let Some(error) = &state.error {
        Line::from(Span::styled(error.clone(), theme::base().fg(theme::RED)))
    } else if matches!(app.source, DataSource::Sample) {
        Line::from(Span::styled(
            deck.scrollback.sample_mode_note.as_str(),
            theme::dim(),
        ))
    } else if let Some(results) = &state.results {
        Line::from(Span::styled(
            template(
                &deck.scrollback.result_count,
                &[
                    ("shown", results.sessions.len().to_string()),
                    ("total", results.total_sessions.to_string()),
                ],
            ),
            theme::muted(),
        ))
    } else {
        Line::from(Span::raw(""))
    };

    Paragraph::new(Text::from(vec![Line::from(spans), status_line]))
        .style(theme::base())
        .render(area, frame.buffer_mut());
}

fn render_results(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let deck = copy();
    let state = &app.scrollback;

    let block = theme::panel_block(deck.scrollback.title.as_str(), theme::BLUE);
    let inner = block.inner(area);
    block.render(area, frame.buffer_mut());

    let Some(results) = &state.results else {
        Paragraph::new(Span::styled(
            deck.scrollback.empty_state.as_str(),
            theme::dim(),
        ))
        .style(theme::base())
        .render(inner, frame.buffer_mut());
        return;
    };
    if results.sessions.is_empty() {
        Paragraph::new(Span::styled(
            deck.scrollback.no_results.as_str(),
            theme::dim(),
        ))
        .style(theme::base())
        .render(inner, frame.buffer_mut());
        return;
    }

    // Mirror the session-calls list: the selected group renders at the top
    // and the rest follow until the panel runs out of rows.
    let mut lines: Vec<Line<'static>> = Vec::new();
    let max_lines = inner.height as usize;
    for (idx, session) in results.sessions.iter().enumerate().skip(state.selected) {
        if lines.len() >= max_lines {
            break;
        }
        let selected = idx == state.selected;
        lines.push(session_header_line(session, selected, app));
        for matched in &session.matches {
            let role = match matched.role {
                SnippetRole::User => deck.scrollback.role_user.as_str(),
                SnippetRole::Assistant => deck.scrollback.role_assistant.as_str(),
            };
            let mut spans = vec![
                Span::raw("    "),
                Span::styled(format!("{role:>9} "), theme::dim()),
            ];
            for part in &matched.snippet {
                let text = part.text.replace(['\n', '\r'], " ");
                if part.hit {
                    spans.push(Span::styled(
                        text,
                        theme::base()
                            .fg(theme::PRIMARY)
                            .add_modifier(Modifier::BOLD),
                    ));
                } else {
                    spans.push(Span::styled(text, theme::muted()));
                }
            }
            lines.push(Line::from(spans));
        }
        let extra = session
            .match_count
            .saturating_sub(session.matches.len() as u64);
        if extra > 0 {
            lines.push(Line::from(vec![
                Span::raw("    "),
                Span::styled(
                    template(
                        &deck.scrollback.more_matches,
                        &[("count", extra.to_string())],
                    ),
                    theme::dim(),
                ),
            ]));
        }
        lines.push(Line::from(Span::raw("")));
    }
    lines.truncate(max_lines);

    Paragraph::new(Text::from(lines))
        .style(theme::base())
        .render(inner, frame.buffer_mut());
}

fn session_header_line(session: &SessionSearchResult, selected: bool, _app: &App) -> Line<'static> {
    let deck = copy();
    let marker = if selected { "> " } else { "  " };
    let base = if selected {
        theme::base().bg(theme::RANK_FILL_BG)
    } else {
        theme::base()
    };
    let date = session
        .last_timestamp
        .as_deref()
        .map(|ts| ts.chars().take(10).collect::<String>())
        .unwrap_or_default();
    let cost = session.cost.clone();

    let mut spans = vec![
        Span::styled(marker.to_string(), base.fg(theme::PRIMARY)),
        Span::styled(session.project.clone(), base.add_modifier(Modifier::BOLD)),
        Span::styled(
            format!(
                "  {}  {}  {}",
                crate::ingest::projects::tool_short_label(&session.tool),
                date,
                cost
            ),
            base.fg(theme::MUTED),
        ),
        Span::styled(format!("  {}", session.match_count), base.fg(theme::BLUE)),
    ];
    if session.prompt_only {
        spans.push(Span::styled(
            format!("  {}", deck.scrollback.prompt_only_badge),
            base.fg(theme::DIM),
        ));
    }
    Line::from(spans)
}
