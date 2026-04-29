use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    prelude::{Color, Frame, Line, Modifier, Span},
    text::Text,
    widgets::{Cell, Clear, Paragraph, Row, Table, Widget},
};

use crate::{
    app::{App, Page, Period},
    data::{
        CountMetric, DailyMetric, ModelMetric, ProjectMetric, ProjectToolMetric, RecentModelMetric,
        RecentUsageMetric, SessionMetric, Summary, ToolLimitSection,
    },
    theme,
};

use super::components::{bar_cell, centered_rect, BAR_WIDTH};

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
        Span::styled("[t] ", theme::key()),
        Span::styled(app.tool.label(), theme::key()),
        Span::styled("  [p] ", theme::key()),
        Span::styled(app.project_filter.label(), theme::muted()),
    ]))
    .style(theme::base())
    .alignment(Alignment::Right)
    .render(columns[1], frame.buffer_mut());
}

pub(super) fn render_summary(frame: &mut Frame<'_>, area: Rect, app: &App, summary: &Summary) {
    let title_owned = match &app.status {
        Some(s) => format!("tokenuse  ·  {s}"),
        None => "tokenuse".to_string(),
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
            Cell::from(item.tool_mix).style(theme::base().fg(theme::BLUE_SOFT)),
        ])
    });

    let table = Table::new(
        table_rows,
        [
            Constraint::Length(BAR_WIDTH as u16),
            Constraint::Min(24),
            Constraint::Length(9),
            Constraint::Length(8),
            Constraint::Length(5),
            Constraint::Length(24),
        ],
    )
    .header(Row::new(vec![
        Cell::from(""),
        Cell::from(""),
        Cell::from("cost").style(theme::dim()),
        Cell::from("avg/s").style(theme::dim()),
        Cell::from("sess").style(theme::dim()),
        Cell::from("tools").style(theme::dim()),
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

pub(super) fn render_project_tools(frame: &mut Frame<'_>, area: Rect, rows: &[ProjectToolMetric]) {
    let table_rows = rows.iter().map(|item| {
        Row::new(vec![
            bar_cell(item.value),
            Cell::from(item.project).style(theme::muted()),
            Cell::from(item.tool).style(theme::base().fg(theme::YELLOW_SOFT)),
            Cell::from(item.cost).style(theme::money()),
            Cell::from(item.calls.to_string()).style(theme::base()),
            Cell::from(item.sessions.to_string()).style(theme::base()),
            Cell::from(item.avg_per_session).style(theme::money()),
        ])
    });

    let table = Table::new(
        table_rows,
        [
            Constraint::Length(BAR_WIDTH as u16),
            Constraint::Min(14),
            Constraint::Length(8),
            Constraint::Length(9),
            Constraint::Length(6),
            Constraint::Length(5),
            Constraint::Length(8),
        ],
    )
    .header(Row::new(vec![
        Cell::from(""),
        Cell::from("project").style(theme::dim()),
        Cell::from("tool").style(theme::dim()),
        Cell::from("cost").style(theme::dim()),
        Cell::from("calls").style(theme::dim()),
        Cell::from("sess").style(theme::dim()),
        Cell::from("avg/s").style(theme::dim()),
    ]))
    .column_spacing(1)
    .block(theme::panel_block("Project Spend by Tool", theme::YELLOW));

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

pub(super) fn render_limits(frame: &mut Frame<'_>, area: Rect, root: Rect, app: &App) {
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(8),
            Constraint::Length(3),
        ])
        .split(area);

    let data = app.usage();

    render_limits_header(frame, sections[0], app);
    let tool_sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Ratio(1, 4),
            Constraint::Ratio(1, 4),
            Constraint::Ratio(1, 4),
            Constraint::Ratio(1, 4),
        ])
        .split(sections[2]);

    for (idx, section) in data.sections.iter().enumerate().take(4) {
        render_tool_usage_section(frame, tool_sections[idx], section);
    }

    render_footer(frame, sections[3], app);
    render_project_modal(frame, root, app);
    render_currency_modal(frame, root, app);
}

fn render_limits_header(frame: &mut Frame<'_>, area: Rect, _app: &App) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    Paragraph::new(Line::from(vec![
        Span::styled("[ Usage ]", theme::key()),
        Span::raw("    "),
        Span::styled("Dashboard", theme::dim()),
        Span::raw("    "),
        Span::styled("Config", theme::dim()),
    ]))
    .style(theme::base())
    .render(columns[0], frame.buffer_mut());

    Paragraph::new(Line::from(vec![Span::styled(
        "sorted by 24h usage",
        theme::muted(),
    )]))
    .style(theme::base())
    .alignment(Alignment::Right)
    .render(columns[1], frame.buffer_mut());
}

fn render_tool_usage_section(frame: &mut Frame<'_>, area: Rect, section: &ToolLimitSection) {
    let mut rows = Vec::new();

    rows.push(Row::new(vec![
        Cell::from("usage").style(theme::key()),
        Cell::from("24h total").style(theme::muted()),
        usage_cell(&section.usage),
        Cell::from(section.usage.calls.to_string()).style(theme::base()),
        Cell::from(section.usage.tokens).style(theme::muted()),
        Cell::from(section.usage.cost).style(theme::money()),
        Cell::from(section.usage.last_seen).style(theme::muted()),
    ]));

    rows.extend(section.limits.iter().map(|limit| {
        Row::new(vec![
            Cell::from("limit").style(theme::base().fg(theme::CYAN)),
            Cell::from(format!("{} {}", limit.scope, limit.window)).style(theme::muted()),
            bar_cell(limit.used),
            Cell::from(limit.left).style(theme::base()),
            Cell::from(limit.reset).style(theme::muted()),
            Cell::from(limit.plan).style(theme::base().fg(theme::YELLOW_SOFT)),
            Cell::from(""),
        ])
    }));

    rows.extend(section.models.iter().map(model_row));

    let title = format!("{} · 24h usage + models", section.tool);
    let table = Table::new(
        rows,
        [
            Constraint::Length(7),
            Constraint::Min(22),
            Constraint::Length(24),
            Constraint::Length(10),
            Constraint::Length(14),
            Constraint::Length(12),
            Constraint::Length(8),
        ],
    )
    .header(Row::new(vec![
        Cell::from("kind").style(theme::dim()),
        Cell::from("name").style(theme::dim()),
        Cell::from("24h / used").style(theme::dim()),
        Cell::from("calls / left").style(theme::dim()),
        Cell::from("tokens / reset").style(theme::dim()),
        Cell::from("cost / plan").style(theme::dim()),
        Cell::from("seen").style(theme::dim()),
    ]))
    .column_spacing(1)
    .block(theme::panel_block(&title, theme::MAGENTA));

    frame.render_widget(table, area);
}

fn model_row(model: &RecentModelMetric) -> Row<'static> {
    Row::new(vec![
        Cell::from("model").style(theme::base().fg(theme::BLUE_SOFT)),
        Cell::from(model.name).style(theme::muted()),
        bar_cell(model.value),
        Cell::from(model.calls.to_string()).style(theme::base()),
        Cell::from(model.tokens).style(theme::muted()),
        Cell::from(model.cost).style(theme::money()),
        Cell::from(""),
    ])
}

fn usage_cell(usage: &RecentUsageMetric) -> Cell<'static> {
    let spans = usage
        .buckets
        .into_iter()
        .map(|value| {
            let color = if value == 0 {
                theme::BAR_EMPTY
            } else if value < 34 {
                theme::BLUE_SOFT
            } else if value < 67 {
                theme::CYAN
            } else {
                theme::PRIMARY
            };
            Span::styled(" ", theme::base().bg(color))
        })
        .collect::<Vec<_>>();

    Cell::from(Line::from(spans))
}

pub(super) fn render_config(frame: &mut Frame<'_>, area: Rect, root: Rect, app: &App) {
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(8),
            Constraint::Length(1),
            Constraint::Length(9),
            Constraint::Min(1),
            Constraint::Length(3),
        ])
        .split(area);

    Paragraph::new(Line::from(vec![
        Span::styled("[ Configuration ]", theme::key()),
        Span::raw("    "),
        Span::styled("Dashboard", theme::dim()),
    ]))
    .style(theme::base())
    .render(sections[0], frame.buffer_mut());

    render_config_rows(frame, sections[2], app);
    render_config_paths(frame, sections[4], app);
    render_footer(frame, sections[6], app);
    render_currency_modal(frame, root, app);
}

fn render_config_rows(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let rows_data = app.config_rows();
    let rows = rows_data.iter().enumerate().map(|(idx, row)| {
        let is_selected = idx == app.config_selected;
        let bg = if is_selected {
            theme::SURFACE
        } else {
            theme::BACKGROUND
        };
        let marker = if is_selected { ">" } else { " " };
        Row::new(vec![
            Cell::from(marker).style(theme::key().bg(bg)),
            Cell::from(row.name).style(if is_selected {
                theme::key().bg(bg)
            } else {
                theme::muted().bg(bg)
            }),
            Cell::from(row.value.as_str()).style(theme::base().bg(bg)),
            Cell::from(row.action).style(theme::money().bg(bg)),
        ])
    });

    let table = Table::new(
        rows,
        [
            Constraint::Length(2),
            Constraint::Length(22),
            Constraint::Min(36),
            Constraint::Length(8),
        ],
    )
    .header(Row::new(vec![
        Cell::from(""),
        Cell::from("setting").style(theme::dim()),
        Cell::from("value").style(theme::dim()),
        Cell::from("enter").style(theme::dim()),
    ]))
    .column_spacing(1)
    .block(theme::panel_block("Configuration", theme::PRIMARY));

    frame.render_widget(table, area);
}

fn render_config_paths(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let mut lines = Vec::new();
    if let Some(status) = app.status.as_ref() {
        lines.push(Line::from(vec![
            Span::styled("status ", theme::key()),
            Span::styled(status.as_str(), theme::muted()),
        ]));
    }
    lines.extend([
        path_line("config dir", app.paths.dir.display().to_string()),
        path_line("config file", app.paths.config_file.display().to_string()),
        path_line(
            "rates data",
            app.paths.currency_rates_file.display().to_string(),
        ),
        path_line(
            "pricing data",
            app.paths.pricing_snapshot_file.display().to_string(),
        ),
    ]);
    lines.push(Line::from(vec![
        Span::styled("rates source ", theme::key()),
        Span::styled(app.currency_table.source().label(), theme::muted()),
    ]));

    Paragraph::new(Text::from(lines))
        .block(theme::panel_block("Local Files", theme::CYAN))
        .style(theme::base())
        .render(area, frame.buffer_mut());
}

fn path_line(label: &'static str, value: String) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{label:<12}"), theme::key()),
        Span::styled(value, theme::muted()),
    ])
}

pub(super) fn render_footer(frame: &mut Frame<'_>, area: Rect, app: &App) {
    if app.page == Page::Config {
        let commands = Line::from(vec![
            Span::styled("q", theme::key()),
            Span::styled(" quit    ", theme::muted()),
            Span::styled("Esc", theme::key()),
            Span::styled(" dashboard    ", theme::muted()),
            Span::styled("Up/Down", theme::key()),
            Span::styled(" move    ", theme::muted()),
            Span::styled("Enter", theme::key()),
            Span::styled(" select", theme::muted()),
        ]);

        Paragraph::new(commands)
            .alignment(Alignment::Center)
            .block(theme::panel_block("", theme::DIM))
            .style(theme::base())
            .render(area, frame.buffer_mut());
        return;
    }

    if app.page == Page::Usage {
        let commands = Line::from(vec![
            Span::styled("q", theme::key()),
            Span::styled(" quit    ", theme::muted()),
            Span::styled("Esc", theme::key()),
            Span::styled(" dashboard    ", theme::muted()),
            Span::styled("u", theme::key()),
            Span::styled(" dashboard    ", theme::muted()),
            Span::styled("c", theme::key()),
            Span::styled(" config", theme::muted()),
        ]);

        Paragraph::new(commands)
            .alignment(Alignment::Center)
            .block(theme::panel_block("", theme::DIM))
            .style(theme::base())
            .render(area, frame.buffer_mut());
        return;
    }

    let commands = Line::from(vec![
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
        Span::styled("t", theme::key()),
        Span::styled(" tool    ", theme::muted()),
        Span::styled("p", theme::key()),
        Span::styled(" project    ", theme::muted()),
        Span::styled("c", theme::key()),
        Span::styled(" config    ", theme::muted()),
        Span::styled("u", theme::key()),
        Span::styled(" usage", theme::muted()),
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

pub(super) fn render_project_modal(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let Some(modal) = app.project_modal.as_ref() else {
        return;
    };

    let width = 76.min(area.width.saturating_sub(4)).max(48);
    let height = (modal.options.len() as u16 + 3)
        .min(area.height.saturating_sub(4))
        .max(7);
    let modal_area = centered_rect(width, height, area);
    Clear.render(modal_area, frame.buffer_mut());

    let title = format!(
        "Project {}/{}",
        modal.selected.saturating_add(1),
        modal.options.len().max(1)
    );
    let block = theme::panel_block(&title, theme::GREEN);
    let inner = block.inner(modal_area);
    block.render(modal_area, frame.buffer_mut());

    let row_capacity = inner.height.saturating_sub(1).max(1) as usize;
    let selected = modal.selected.min(modal.options.len().saturating_sub(1));
    let start = selected.saturating_add(1).saturating_sub(row_capacity);
    let end = (start + row_capacity).min(modal.options.len());

    let rows = modal.options[start..end]
        .iter()
        .enumerate()
        .map(|(offset, option)| {
            let idx = start + offset;
            let is_selected = idx == modal.selected;
            let bg = if is_selected {
                theme::SURFACE
            } else {
                theme::BACKGROUND
            };
            let marker = if is_selected { ">" } else { " " };

            Row::new(vec![
                Cell::from(marker).style(theme::key().bg(bg)),
                Cell::from(option.label.as_str()).style(if is_selected {
                    theme::key().bg(bg)
                } else {
                    theme::muted().bg(bg)
                }),
                Cell::from(option.cost.as_str()).style(theme::money().bg(bg)),
                Cell::from(option.calls.to_string()).style(theme::base().bg(bg)),
            ])
        });

    let table = Table::new(
        rows,
        [
            Constraint::Length(2),
            Constraint::Min(30),
            Constraint::Length(10),
            Constraint::Length(8),
        ],
    )
    .header(Row::new(vec![
        Cell::from(""),
        Cell::from("project").style(theme::dim()),
        Cell::from("cost").style(theme::dim()),
        Cell::from("calls").style(theme::dim()),
    ]))
    .column_spacing(1);

    frame.render_widget(table, inner);
}

pub(super) fn render_currency_modal(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let Some(modal) = app.currency_modal.as_ref() else {
        return;
    };

    let width = 58.min(area.width.saturating_sub(4)).max(40);
    let height = (modal.options.len() as u16 + 3)
        .min(area.height.saturating_sub(4))
        .max(9);
    let modal_area = centered_rect(width, height, area);
    Clear.render(modal_area, frame.buffer_mut());

    let title = format!(
        "Currency {}/{}",
        modal.selected.saturating_add(1),
        modal.options.len().max(1)
    );
    let block = theme::panel_block(&title, theme::PRIMARY);
    let inner = block.inner(modal_area);
    block.render(modal_area, frame.buffer_mut());

    let row_capacity = inner.height.saturating_sub(1).max(1) as usize;
    let selected = modal.selected.min(modal.options.len().saturating_sub(1));
    let start = selected.saturating_add(1).saturating_sub(row_capacity);
    let end = (start + row_capacity).min(modal.options.len());

    let rows = modal.options[start..end]
        .iter()
        .enumerate()
        .map(|(offset, code)| {
            let idx = start + offset;
            let is_selected = idx == modal.selected;
            let is_active = code == app.currency().code();
            let bg = if is_selected {
                theme::SURFACE
            } else {
                theme::BACKGROUND
            };
            let marker = if is_selected { ">" } else { " " };
            let active = if is_active { "active" } else { "" };
            let rate = app
                .currency_table
                .rate(code)
                .map(|rate| format!("{rate:.6}"))
                .unwrap_or_else(|| "-".into());

            Row::new(vec![
                Cell::from(marker).style(theme::key().bg(bg)),
                Cell::from(code.as_str()).style(if is_selected {
                    theme::key().bg(bg)
                } else {
                    theme::muted().bg(bg)
                }),
                Cell::from(rate).style(theme::base().bg(bg)),
                Cell::from(active).style(theme::money().bg(bg)),
            ])
        });

    let table = Table::new(
        rows,
        [
            Constraint::Length(2),
            Constraint::Length(8),
            Constraint::Length(14),
            Constraint::Length(8),
        ],
    )
    .header(Row::new(vec![
        Cell::from(""),
        Cell::from("code").style(theme::dim()),
        Cell::from("per USD").style(theme::dim()),
        Cell::from(""),
    ]))
    .column_spacing(1);

    frame.render_widget(table, inner);
}
