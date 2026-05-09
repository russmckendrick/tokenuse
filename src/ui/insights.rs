use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    prelude::{Color, Frame, Line, Modifier, Span},
    widgets::{Paragraph, Widget, Wrap},
};

use crate::{
    advice::{AdviceHistory, AdviceItemView, AdviceRunView},
    app::{App, InsightsTab},
    copy::{copy, template},
    insights::RecommendationView,
    theme,
};

use super::sections::{render_footer, render_title_bar};

pub(super) fn render_insights(frame: &mut Frame<'_>, area: Rect, _root: Rect, app: &App) {
    let view = app.insights();
    let advice = app.advice_history();

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
    render_insight_tabs(frame, sections[3], app);

    match app.insights_tab() {
        InsightsTab::Advice => render_advice(frame, sections[4], &advice, app.insights_scroll()),
        InsightsTab::Signals => render_recommendations(
            frame,
            sections[4],
            &view.recommendations,
            app.insights_scroll(),
        ),
    }

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

fn render_insight_tabs(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let copy = copy();
    let active = app.insights_tab();
    let spans = vec![
        tab_span(
            copy.insights.advice_title.as_str(),
            active == InsightsTab::Advice,
        ),
        Span::raw("    "),
        tab_span(
            copy.insights.signals_title.as_str(),
            active == InsightsTab::Signals,
        ),
    ];
    Paragraph::new(Line::from(spans))
        .alignment(Alignment::Center)
        .style(theme::base())
        .render(area, frame.buffer_mut());
}

fn tab_span(label: &str, active: bool) -> Span<'static> {
    if active {
        Span::styled(
            format!("[ {label} ]"),
            theme::key().add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled(label.to_string(), theme::dim())
    }
}

fn render_advice(frame: &mut Frame<'_>, area: Rect, history: &AdviceHistory, scroll: usize) {
    let copy = copy();
    let block = theme::panel_block(copy.insights.advice_title.as_str(), theme::GREEN);
    let inner = block.inner(area);
    block.render(area, frame.buffer_mut());

    if history.runs.is_empty() {
        Paragraph::new(Line::from(Span::styled(
            copy.insights.advice_empty.as_str(),
            theme::muted(),
        )))
        .alignment(Alignment::Center)
        .style(theme::base())
        .render(inner, frame.buffer_mut());
        return;
    }

    let mut lines: Vec<Line<'static>> = Vec::new();
    for (run_idx, run) in history.runs.iter().enumerate() {
        if run_idx > 0 {
            lines.push(Line::raw(""));
        }
        lines.push(Line::from(Span::styled(
            advice_run_label(run, run_idx == 0),
            theme::dim(),
        )));

        if let Some(summary) = run.summary.clone() {
            lines.push(Line::from(Span::styled(summary, theme::base())));
        }

        if run.status == "failed" {
            let error = run.error.clone().unwrap_or_else(|| run.status.clone());
            lines.push(Line::from(Span::styled(
                template(&copy.insights.advice_failed, &[("error", error)]),
                theme::base().fg(theme::RED),
            )));
        }

        if run.items.is_empty() && run.status != "failed" {
            lines.push(Line::from(Span::styled(
                copy.insights.advice_empty.as_str(),
                theme::muted(),
            )));
        }

        for item in &run.items {
            lines.push(Line::raw(""));
            push_advice_item_lines(&mut lines, item);
        }
    }

    Paragraph::new(lines)
        .scroll((scroll_u16(scroll), 0))
        .wrap(Wrap { trim: false })
        .style(theme::base())
        .render(inner, frame.buffer_mut());
}

fn push_advice_item_lines(lines: &mut Vec<Line<'static>>, item: &AdviceItemView) {
    lines.push(Line::from(vec![
        Span::styled(
            item.severity.to_uppercase(),
            theme::base()
                .fg(severity_color(&item.severity))
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            item.title.clone(),
            theme::base().add_modifier(Modifier::BOLD),
        ),
        Span::raw("   "),
        Span::styled(format!("[{}]", item.category), theme::dim()),
        Span::raw("   "),
        Span::styled(
            template(
                &copy().insights.advice_confidence,
                &[("confidence", format!("{:.0}%", item.confidence * 100.0))],
            ),
            theme::dim(),
        ),
        Span::raw(if item.status == "open" { "" } else { "   " }),
        Span::styled(
            if item.status == "open" {
                String::new()
            } else {
                item.status.clone()
            },
            theme::muted(),
        ),
    ]));
    lines.push(Line::from(Span::styled(item.body.clone(), theme::base())));
    lines.push(Line::from(Span::styled(item.impact.clone(), theme::dim())));
    if !item.evidence.is_empty() {
        lines.push(Line::from(Span::styled(
            template(
                &copy().insights.advice_evidence,
                &[("evidence", item.evidence.join(" · "))],
            ),
            theme::dim(),
        )));
    }
    lines.push(Line::from(Span::styled(
        template(
            &copy().insights.advice_next_step,
            &[("step", item.next_step.clone())],
        ),
        theme::dim(),
    )));
}

fn advice_run_label(run: &AdviceRunView, latest: bool) -> String {
    let copy = copy();
    let scope = if run.data_scope == "prompt_snippets" {
        copy.insights.advice_scope_snippets.clone()
    } else {
        copy.insights.advice_scope_redacted.clone()
    };
    let label = if latest {
        &copy.insights.advice_latest
    } else {
        &copy.insights.advice_run
    };
    template(
        label,
        &[
            ("tool", run.tool_label.clone()),
            ("scope", scope),
            ("status", run.status.clone()),
        ],
    )
}

fn render_recommendations(
    frame: &mut Frame<'_>,
    area: Rect,
    recs: &[RecommendationView],
    scroll: usize,
) {
    let copy = copy();
    let block = theme::panel_block(copy.insights.signals_title.as_str(), theme::PRIMARY);
    let inner = block.inner(area);
    block.render(area, frame.buffer_mut());

    if recs.is_empty() {
        Paragraph::new(Line::from(Span::styled(
            copy.insights.empty.as_str(),
            theme::muted(),
        )))
        .alignment(Alignment::Center)
        .style(theme::base())
        .render(inner, frame.buffer_mut());
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
        .scroll((scroll_u16(scroll), 0))
        .wrap(Wrap { trim: false })
        .style(theme::base())
        .render(inner, frame.buffer_mut());
}

fn scroll_u16(scroll: usize) -> u16 {
    scroll.min(u16::MAX as usize) as u16
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
