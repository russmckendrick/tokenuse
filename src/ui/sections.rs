use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    prelude::{Color, Frame, Line, Modifier, Span, Style},
    text::Text,
    widgets::{Cell, Paragraph, Row, Table, Widget},
};

use crate::{
    app::{App, Period},
    data::{
        ActivityAccent, ActivityMetric, CountMetric, DailyMetric, ModelMetric, ProjectMetric,
        SessionMetric, Summary,
    },
    theme,
};

use super::components::{bar_cell, BAR_WIDTH};

pub(super) fn render_nav(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(74), Constraint::Percentage(26)])
        .split(area);

    let mut spans = Vec::new();
    for period in Period::ALL {
        if period == app.period {
            spans.push(Span::styled(
                format!("[ {} ]", period.label()),
                theme::key(),
            ));
        } else {
            spans.push(Span::styled(period.label(), theme::dim()));
        }
        spans.push(Span::raw("    "));
    }

    Paragraph::new(Line::from(spans))
        .style(theme::base())
        .render(columns[0], frame.buffer_mut());

    Paragraph::new(Line::from(vec![
        Span::styled("|  ", theme::dim()),
        Span::styled("[p] ", theme::key()),
        Span::styled(app.provider.label(), theme::key()),
    ]))
    .style(theme::base())
    .alignment(Alignment::Right)
    .render(columns[1], frame.buffer_mut());
}

pub(super) fn render_summary(frame: &mut Frame<'_>, area: Rect, app: &App, summary: &Summary) {
    let base = match app.view {
        crate::app::View::Dashboard => "tokenuse",
        crate::app::View::Optimize => "tokenuse Optimize",
        crate::app::View::Compare => "tokenuse Compare",
    };
    let title_owned = match &app.status {
        Some(s) => format!("{base}  ·  {s}"),
        None => base.to_string(),
    };
    let title: &str = &title_owned;

    let text = Text::from(vec![
        Line::from(vec![
            Span::styled(title, theme::key()),
            Span::raw("  "),
            Span::styled(app.period.label(), theme::muted()),
        ]),
        Line::from(vec![
            Span::styled(summary.cost, theme::money().add_modifier(Modifier::BOLD)),
            Span::styled(" cost   ", theme::muted()),
            Span::styled(summary.calls, theme::base().add_modifier(Modifier::BOLD)),
            Span::styled(" calls   ", theme::muted()),
            Span::styled(summary.sessions, theme::base().add_modifier(Modifier::BOLD)),
            Span::styled(" sessions   ", theme::muted()),
            Span::styled(
                summary.cache_hit,
                theme::base().add_modifier(Modifier::BOLD),
            ),
            Span::styled(" cache hit", theme::muted()),
        ]),
        Line::from(vec![
            Span::styled(summary.input, theme::muted()),
            Span::styled(" in   ", theme::dim()),
            Span::styled(summary.output, theme::muted()),
            Span::styled(" out   ", theme::dim()),
            Span::styled(summary.cached, theme::muted()),
            Span::styled(" cached   ", theme::dim()),
            Span::styled(summary.written, theme::muted()),
            Span::styled(" written", theme::dim()),
        ]),
    ]);

    Paragraph::new(text)
        .block(theme::panel_block("", theme::PRIMARY))
        .style(theme::base())
        .render(area, frame.buffer_mut());
}

pub(super) fn render_daily(frame: &mut Frame<'_>, area: Rect, rows: &[DailyMetric]) {
    let table_rows = rows.iter().map(|item| {
        Row::new(vec![
            Cell::from(item.day).style(theme::muted()),
            bar_cell(item.value),
            Cell::from(item.cost).style(theme::money()),
            Cell::from(item.calls.to_string()).style(theme::base()),
        ])
    });

    let table = Table::new(
        table_rows,
        [
            Constraint::Length(8),
            Constraint::Length(BAR_WIDTH as u16),
            Constraint::Length(10),
            Constraint::Length(8),
        ],
    )
    .header(Row::new(vec![
        Cell::from("date").style(theme::dim()),
        Cell::from(""),
        Cell::from("cost").style(theme::dim()),
        Cell::from("calls").style(theme::dim()),
    ]))
    .column_spacing(1)
    .block(theme::panel_block("Daily Activity", theme::BLUE));

    frame.render_widget(table, area);
}

pub(super) fn render_projects(frame: &mut Frame<'_>, area: Rect, rows: &[ProjectMetric]) {
    let table_rows = rows.iter().map(|item| {
        Row::new(vec![
            bar_cell(item.value),
            Cell::from(item.name).style(theme::muted()),
            Cell::from(item.cost).style(theme::money()),
            Cell::from(item.avg_per_session).style(theme::money()),
            Cell::from(item.sessions.to_string()).style(theme::base()),
            Cell::from(item.overhead).style(Style::default().fg(theme::BLUE_SOFT)),
        ])
    });

    let table = Table::new(
        table_rows,
        [
            Constraint::Length(BAR_WIDTH as u16),
            Constraint::Min(24),
            Constraint::Length(10),
            Constraint::Length(9),
            Constraint::Length(6),
            Constraint::Length(10),
        ],
    )
    .header(Row::new(vec![
        Cell::from(""),
        Cell::from(""),
        Cell::from("cost").style(theme::dim()),
        Cell::from("avg/s").style(theme::dim()),
        Cell::from("sess").style(theme::dim()),
        Cell::from("overhead").style(theme::dim()),
    ]))
    .column_spacing(1)
    .block(theme::panel_block("By Project", theme::GREEN));

    frame.render_widget(table, area);
}

pub(super) fn render_sessions(frame: &mut Frame<'_>, area: Rect, rows: &[SessionMetric]) {
    let table_rows = rows.iter().map(|item| {
        Row::new(vec![
            bar_cell(item.value),
            Cell::from(item.date).style(theme::muted()),
            Cell::from(item.project).style(theme::muted()),
            Cell::from(item.cost).style(theme::money()),
            Cell::from(item.calls.to_string()).style(theme::base()),
        ])
    });

    let table = Table::new(
        table_rows,
        [
            Constraint::Length(BAR_WIDTH as u16),
            Constraint::Length(10),
            Constraint::Min(20),
            Constraint::Length(10),
            Constraint::Length(8),
        ],
    )
    .header(Row::new(vec![
        Cell::from(""),
        Cell::from("date").style(theme::dim()),
        Cell::from("project").style(theme::dim()),
        Cell::from("cost").style(theme::dim()),
        Cell::from("calls").style(theme::dim()),
    ]))
    .column_spacing(1)
    .block(theme::panel_block("Top Sessions", theme::RED));

    frame.render_widget(table, area);
}

pub(super) fn render_activities(frame: &mut Frame<'_>, area: Rect, rows: &[ActivityMetric]) {
    let table_rows = rows.iter().map(|item| {
        let one_shot_style = if item.one_shot.ends_with('%') {
            theme::base().fg(theme::PRIMARY)
        } else {
            theme::dim()
        };

        Row::new(vec![
            bar_cell(item.value),
            Cell::from(item.name).style(theme::base().fg(activity_color(item.accent))),
            Cell::from(item.cost).style(theme::money()),
            Cell::from(item.turns.to_string()).style(theme::base()),
            Cell::from(item.one_shot).style(one_shot_style),
        ])
    });

    let table = Table::new(
        table_rows,
        [
            Constraint::Length(BAR_WIDTH as u16),
            Constraint::Min(16),
            Constraint::Length(10),
            Constraint::Length(7),
            Constraint::Length(8),
        ],
    )
    .header(Row::new(vec![
        Cell::from(""),
        Cell::from(""),
        Cell::from("cost").style(theme::dim()),
        Cell::from("turns").style(theme::dim()),
        Cell::from("1-shot").style(theme::dim()),
    ]))
    .column_spacing(1)
    .block(theme::panel_block("By Activity", theme::YELLOW));

    frame.render_widget(table, area);
}

pub(super) fn render_models(frame: &mut Frame<'_>, area: Rect, rows: &[ModelMetric]) {
    let table_rows = rows.iter().map(|item| {
        Row::new(vec![
            bar_cell(item.value),
            Cell::from(item.name).style(theme::base()),
            Cell::from(item.cost).style(theme::money()),
            Cell::from(item.cache).style(theme::base()),
            Cell::from(item.calls.to_string()).style(theme::base()),
        ])
    });

    let table = Table::new(
        table_rows,
        [
            Constraint::Length(BAR_WIDTH as u16),
            Constraint::Min(16),
            Constraint::Length(10),
            Constraint::Length(8),
            Constraint::Length(8),
        ],
    )
    .header(Row::new(vec![
        Cell::from(""),
        Cell::from(""),
        Cell::from("cost").style(theme::dim()),
        Cell::from("cache").style(theme::dim()),
        Cell::from("calls").style(theme::dim()),
    ]))
    .column_spacing(1)
    .block(theme::panel_block("By Model", theme::MAGENTA));

    frame.render_widget(table, area);
}

pub(super) fn render_counts(
    frame: &mut Frame<'_>,
    area: Rect,
    title: &str,
    color: Color,
    rows: &[CountMetric],
) {
    let table_rows = rows.iter().map(|item| {
        Row::new(vec![
            bar_cell(item.value),
            Cell::from(item.name).style(theme::base()),
            Cell::from(item.calls.to_string()).style(theme::base()),
        ])
    });

    let table = Table::new(
        table_rows,
        [
            Constraint::Length(BAR_WIDTH as u16),
            Constraint::Min(16),
            Constraint::Length(9),
        ],
    )
    .header(Row::new(vec![
        Cell::from(""),
        Cell::from(""),
        Cell::from("calls").style(theme::dim()),
    ]))
    .column_spacing(1)
    .block(theme::panel_block(title, color));

    frame.render_widget(table, area);
}

pub(super) fn render_footer(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let commands = Line::from(vec![
        Span::styled("<>", theme::key()),
        Span::styled(" switch    ", theme::muted()),
        Span::styled("q", theme::key()),
        Span::styled(" quit    ", theme::muted()),
        footer_period("1", "today", app.period == Period::Today),
        Span::raw("    "),
        footer_period("2", "week", app.period == Period::Week),
        Span::raw("    "),
        footer_period("3", "30 days", app.period == Period::ThirtyDays),
        Span::raw("    "),
        footer_period("4", "month", app.period == Period::Month),
        Span::raw("    "),
        footer_period("5", "all time", app.period == Period::AllTime),
        Span::raw("    "),
        Span::styled("o", theme::key()),
        Span::styled(" optimize    ", theme::muted()),
        Span::styled("c", theme::key()),
        Span::styled(" compare    ", theme::muted()),
        Span::styled("p", theme::key()),
        Span::styled(" provider", theme::muted()),
    ]);

    Paragraph::new(commands)
        .alignment(Alignment::Center)
        .block(theme::panel_block("", theme::DIM))
        .style(theme::base())
        .render(area, frame.buffer_mut());
}

fn footer_period<'a>(key: &'a str, label: &'a str, active: bool) -> Span<'a> {
    let style = if active { theme::key() } else { theme::muted() };
    Span::styled(format!("{key} {label}"), style)
}

fn activity_color(accent: ActivityAccent) -> Color {
    match accent {
        ActivityAccent::Blue => theme::BLUE,
        ActivityAccent::Green => theme::GREEN,
        ActivityAccent::Muted => theme::MUTED,
        ActivityAccent::Cyan => theme::CYAN,
        ActivityAccent::Yellow => theme::YELLOW,
        ActivityAccent::Red => theme::RED,
        ActivityAccent::Magenta => theme::MAGENTA,
    }
}
