use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    prelude::{Color, Frame, Line, Modifier, Span},
    widgets::{Paragraph, Widget, Wrap},
};

use crate::{app::App, copy::copy, insights::RecommendationView, theme};

use super::sections::{render_footer, render_title_bar};

pub(super) fn render_insights(frame: &mut Frame<'_>, area: Rect, _root: Rect, app: &App) {
    let view = app.insights();

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(2),
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Min(8),
            Constraint::Length(3),
        ])
        .split(area);

    render_title_bar(frame, sections[0], app);
    render_subtitle(frame, sections[1]);
    render_kpi_strip(frame, sections[2], &view);

    let copy = copy();
    Paragraph::new(Line::from(Span::styled(
        copy.insights.subtitle.as_str(),
        theme::dim(),
    )))
    .alignment(Alignment::Center)
    .style(theme::base())
    .render(sections[3], frame.buffer_mut());

    render_recommendations(frame, sections[4], &view.recommendations);

    render_footer(frame, sections[5], app);
}

fn render_subtitle(frame: &mut Frame<'_>, area: Rect) {
    let copy = copy();
    Paragraph::new(Line::from(vec![
        Span::styled(
            copy.insights.title.as_str(),
            theme::key().add_modifier(Modifier::BOLD),
        ),
        Span::raw("   "),
        Span::styled(copy.insights.subtitle.as_str(), theme::dim()),
    ]))
    .style(theme::base())
    .render(area, frame.buffer_mut());
}

fn render_kpi_strip(frame: &mut Frame<'_>, area: Rect, view: &crate::insights::InsightsView) {
    let copy = copy();
    let cells = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Ratio(1, 4),
            Constraint::Ratio(1, 4),
            Constraint::Ratio(1, 4),
            Constraint::Ratio(1, 4),
        ])
        .split(area);

    let risk_count = severity_count(view, "risk");
    let warn_count = severity_count(view, "warn");
    let info_count = severity_count(view, "info");

    render_kpi(
        frame,
        cells[0],
        copy.insights.kpi_savings.as_str(),
        view.summary.total_est_savings.as_str(),
        theme::YELLOW,
    );
    render_kpi(
        frame,
        cells[1],
        copy.insights.kpi_risks.as_str(),
        &risk_count.to_string(),
        theme::RED,
    );
    render_kpi(
        frame,
        cells[2],
        copy.insights.kpi_warns.as_str(),
        &warn_count.to_string(),
        theme::PRIMARY,
    );
    render_kpi(
        frame,
        cells[3],
        copy.insights.kpi_infos.as_str(),
        &info_count.to_string(),
        theme::DIM,
    );
}

fn render_kpi(frame: &mut Frame<'_>, area: Rect, label: &str, value: &str, accent: Color) {
    let block = theme::panel_block(label, accent);
    let inner = block.inner(area);
    block.render(area, frame.buffer_mut());
    Paragraph::new(Line::from(Span::styled(
        value.to_string(),
        theme::base().fg(accent).add_modifier(Modifier::BOLD),
    )))
    .alignment(Alignment::Center)
    .style(theme::base())
    .render(inner, frame.buffer_mut());
}

fn render_recommendations(frame: &mut Frame<'_>, area: Rect, recs: &[RecommendationView]) {
    if recs.is_empty() {
        Paragraph::new(Line::from(Span::styled(
            copy().insights.empty.as_str(),
            theme::muted(),
        )))
        .alignment(Alignment::Center)
        .style(theme::base())
        .render(area, frame.buffer_mut());
        return;
    }

    let mut lines: Vec<Line<'static>> = Vec::with_capacity(recs.len() * 5);
    for (idx, rec) in recs.iter().enumerate() {
        if idx > 0 {
            lines.push(Line::raw(""));
        }
        let severity_color = severity_color(rec.severity);
        let savings = rec.savings.clone().unwrap_or_default();

        lines.push(Line::from(vec![
            Span::styled(
                severity_glyph(rec.severity),
                theme::base()
                    .fg(severity_color)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(
                rec.title.clone(),
                theme::base().add_modifier(Modifier::BOLD),
            ),
            Span::raw("   "),
            Span::styled(format!("[{}]", rec.category_label), theme::dim()),
            Span::raw(if savings.is_empty() { "" } else { "   " }),
            Span::styled(
                savings,
                theme::base().fg(theme::YELLOW).add_modifier(Modifier::BOLD),
            ),
        ]));
        lines.push(Line::from(Span::styled(rec.body.clone(), theme::base())));
        if let Some(silenced) = rec.silenced_reason.clone() {
            lines.push(Line::from(Span::styled(silenced, theme::muted())));
        }
        if let Some(assumption) = rec.assumption.clone() {
            lines.push(Line::from(Span::styled(assumption, theme::dim())));
        }
    }

    Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .style(theme::base())
        .render(area, frame.buffer_mut());
}

fn severity_count(view: &crate::insights::InsightsView, id: &str) -> usize {
    view.summary
        .by_severity
        .iter()
        .find(|s| s.id == id)
        .map(|s| s.count)
        .unwrap_or(0)
}

fn severity_color(id: &str) -> Color {
    match id {
        "risk" => theme::RED,
        "warn" => theme::PRIMARY,
        _ => theme::DIM,
    }
}

fn severity_glyph(id: &str) -> &'static str {
    match id {
        "risk" => "■",
        "warn" => "▲",
        _ => "·",
    }
}
