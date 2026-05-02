use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    prelude::{Color, Frame, Line, Modifier, Span, Style},
    text::Text,
    widgets::{Cell, Clear, Paragraph, Row, Table, Widget, Wrap},
};

use crate::{
    app::{App, ConfigDownload, FolderPickerEntryKind, Page, Period},
    data::{
        ActivityMetric, CountMetric, ModelMetric, ProjectMetric, ProjectToolMetric, SessionDetail,
        SessionMetric, Summary, ToolLimitSection,
    },
    keymap, theme,
};

use super::components::centered_rect;
use super::graphs;

pub(super) fn render_title_bar(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let block = theme::panel_block("", theme::PRIMARY);
    let inner = block.inner(area);
    block.render(area, frame.buffer_mut());

    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(28),
            Constraint::Min(20),
            Constraint::Length(58),
        ])
        .split(inner);

    Paragraph::new(Line::from(vec![
        Span::styled("▂▅█▆", theme::key().add_modifier(Modifier::BOLD)),
        Span::raw("  "),
        Span::styled("Token Use", theme::key().add_modifier(Modifier::BOLD)),
    ]))
    .style(theme::base())
    .render(columns[0], frame.buffer_mut());

    let mut tab_spans: Vec<Span<'_>> = Vec::new();
    for (i, tab) in Page::TABS.iter().enumerate() {
        if i > 0 {
            tab_spans.push(Span::raw("    "));
        }
        if *tab == app.page {
            tab_spans.push(Span::styled(format!("[ {} ]", tab.label()), theme::key()));
        } else {
            tab_spans.push(Span::styled(tab.label().to_string(), theme::dim()));
        }
    }
    Paragraph::new(Line::from(tab_spans))
        .alignment(Alignment::Center)
        .style(theme::base())
        .render(columns[1], frame.buffer_mut());

    Paragraph::new(Line::from(vec![
        Span::styled(app.period.label(), theme::muted()),
        Span::styled("  ·  ", theme::dim()),
        Span::styled("[t] ", theme::key()),
        Span::styled(app.tool.label(), theme::muted()),
        Span::styled("  ·  ", theme::dim()),
        Span::styled("[p] ", theme::key()),
        Span::styled(app.project_filter.label().to_string(), theme::muted()),
        Span::styled("  ·  ", theme::dim()),
        Span::styled("[g] ", theme::key()),
        Span::styled(app.sort.label(), theme::muted()),
    ]))
    .alignment(Alignment::Right)
    .style(theme::base())
    .render(columns[2], frame.buffer_mut());
}

pub(super) fn render_kpi_strip(frame: &mut Frame<'_>, area: Rect, app: &App, summary: &Summary) {
    let cells = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Ratio(1, 5),
            Constraint::Ratio(1, 5),
            Constraint::Ratio(1, 5),
            Constraint::Ratio(1, 5),
            Constraint::Ratio(1, 5),
        ])
        .split(area);

    let currency_code = app.currency().code().to_string();
    let period_label = app.period.label();

    render_kpi_card(
        frame,
        cells[0],
        "COST",
        summary.cost,
        theme::money().add_modifier(Modifier::BOLD),
        &format!("{currency_code} · {period_label}"),
    );
    render_kpi_card(
        frame,
        cells[1],
        "CALLS",
        summary.calls,
        theme::base().add_modifier(Modifier::BOLD),
        &format!("{} in", summary.input),
    );
    render_kpi_card(
        frame,
        cells[2],
        "SESSIONS",
        summary.sessions,
        theme::base().add_modifier(Modifier::BOLD),
        period_label,
    );
    render_kpi_card(
        frame,
        cells[3],
        "CACHE HIT",
        summary.cache_hit,
        theme::base()
            .fg(theme::PRIMARY)
            .add_modifier(Modifier::BOLD),
        &format!("{} cached", summary.cached),
    );
    render_kpi_card(
        frame,
        cells[4],
        "IN / OUT",
        summary.input,
        theme::base().add_modifier(Modifier::BOLD),
        &format!("/ {} out", summary.output),
    );
}

fn render_kpi_card(
    frame: &mut Frame<'_>,
    area: Rect,
    label: &str,
    value: &str,
    value_style: Style,
    sub: &str,
) {
    let block = theme::panel_block("", theme::PRIMARY);
    let text = Text::from(vec![
        Line::from(Span::styled(label.to_string(), theme::muted())),
        Line::from(Span::styled(value.to_string(), value_style)),
        Line::from(Span::styled(sub.to_string(), theme::dim())),
    ]);
    Paragraph::new(text)
        .block(block)
        .style(theme::base())
        .render(area, frame.buffer_mut());
}

pub(super) fn render_activity_pulse(frame: &mut Frame<'_>, area: Rect, rows: &[ActivityMetric]) {
    render_timeline_panel(frame, area, "Activity Pulse", theme::CYAN, rows, true);
}

pub(super) fn render_daily_trend(frame: &mut Frame<'_>, area: Rect, rows: &[ActivityMetric]) {
    render_timeline_panel(frame, area, "Activity Trend", theme::BLUE, rows, false);
}

fn render_timeline_panel(
    frame: &mut Frame<'_>,
    area: Rect,
    title: &str,
    color: Color,
    rows: &[ActivityMetric],
    compact: bool,
) {
    let block = theme::panel_block(title, color);
    let inner = block.inner(area);
    block.render(area, frame.buffer_mut());

    if rows.is_empty() {
        Paragraph::new(Text::from(vec![graphs::no_data_line("pulse")]))
            .style(theme::base())
            .render(inner, frame.buffer_mut());
        return;
    }

    let spark_width = inner.width.saturating_sub(11).max(8) as usize;
    let spend_values: Vec<u64> = rows.iter().map(|row| row.value).collect();
    let max_calls = rows.iter().map(|row| row.calls).max().unwrap_or(0);
    let call_values: Vec<u64> = rows
        .iter()
        .map(|row| {
            if max_calls == 0 {
                0
            } else {
                ((row.calls as f64 / max_calls as f64) * 100.0).round() as u64
            }
        })
        .collect();

    let first = rows.first().expect("rows is not empty");
    let latest = rows.last().expect("rows is not empty");
    let high = rows
        .iter()
        .max_by_key(|row| row.value)
        .expect("rows is not empty");
    let total_calls = rows.iter().map(|row| row.calls).sum::<u64>();

    let mut spend_line = vec![Span::styled("spend ", theme::key())];
    spend_line.extend(graphs::sparkline_spans(&spend_values, spark_width));

    let mut call_line = vec![Span::styled("calls ", theme::base().fg(theme::BLUE_SOFT))];
    call_line.extend(graphs::sparkline_spans(&call_values, spark_width));

    let mut lines = vec![Line::from(spend_line), Line::from(call_line)];
    lines.push(Line::from(vec![
        Span::styled("range ", theme::dim()),
        Span::styled(first.label, theme::muted()),
        Span::styled(" to ", theme::dim()),
        Span::styled(latest.label, theme::muted()),
        Span::styled("   high ", theme::dim()),
        Span::styled(high.label, theme::key()),
        Span::styled(" ", theme::dim()),
        Span::styled(high.cost, theme::money()),
        Span::styled("   latest ", theme::dim()),
        Span::styled(latest.cost, theme::money()),
        Span::styled("   calls ", theme::dim()),
        Span::styled(format_compact_u64(total_calls), theme::base()),
    ]));

    if !compact && inner.height > 4 {
        let recent = rows
            .iter()
            .rev()
            .take(3)
            .map(|row| {
                format!(
                    "{} {} {}",
                    row.label,
                    row.cost,
                    format_compact_u64(row.calls)
                )
            })
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("   ");
        lines.push(Line::from(vec![
            Span::styled("recent ", theme::dim()),
            Span::styled(recent, theme::muted()),
        ]));
    }

    Paragraph::new(Text::from(lines))
        .style(theme::base())
        .render(inner, frame.buffer_mut());
}

pub(super) fn render_projects(frame: &mut Frame<'_>, area: Rect, rows: &[ProjectMetric]) {
    let table_rows = rows.iter().map(|item| {
        Row::new(vec![
            graphs::rank_cell(item.value),
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
            Constraint::Length(graphs::RANK_WIDTH as u16),
            Constraint::Min(16),
            Constraint::Length(9),
            Constraint::Length(8),
            Constraint::Length(5),
            Constraint::Min(14),
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
            graphs::rank_cell(item.value),
            Cell::from(item.date).style(theme::muted()),
            Cell::from(item.project).style(theme::muted()),
            Cell::from(item.cost).style(theme::money()),
            Cell::from(item.calls.to_string()).style(theme::base()),
        ])
    });

    let table = Table::new(
        table_rows,
        [
            Constraint::Length(graphs::RANK_WIDTH as u16),
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
            graphs::rank_cell(item.value),
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
            Constraint::Length(graphs::RANK_WIDTH as u16),
            Constraint::Min(12),
            Constraint::Length(7),
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
    render_models_panel(frame, area, "By Model", rows);
}

pub(super) fn render_model_efficiency(frame: &mut Frame<'_>, area: Rect, rows: &[ModelMetric]) {
    render_models_panel(frame, area, "Model Efficiency", rows);
}

fn render_models_panel(frame: &mut Frame<'_>, area: Rect, title: &str, rows: &[ModelMetric]) {
    let table_rows = rows.iter().map(|item| {
        Row::new(vec![
            graphs::rank_cell(item.value),
            Cell::from(item.name).style(theme::base()),
            Cell::from(item.cost).style(theme::money()),
            Cell::from(item.cache).style(theme::base()),
            Cell::from(item.calls.to_string()).style(theme::base()),
        ])
    });

    let table = Table::new(
        table_rows,
        [
            Constraint::Length(graphs::RANK_WIDTH as u16),
            Constraint::Min(10),
            Constraint::Length(9),
            Constraint::Length(7),
            Constraint::Length(6),
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
    .block(theme::panel_block(title, theme::MAGENTA));

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
            graphs::rank_cell(item.value),
            Cell::from(item.name).style(theme::base()),
            Cell::from(item.calls.to_string()).style(theme::base()),
        ])
    });

    let table = Table::new(
        table_rows,
        [
            Constraint::Length(graphs::RANK_WIDTH as u16),
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
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Min(8),
            Constraint::Length(3),
        ])
        .split(area);

    let data = app.usage();

    render_title_bar(frame, sections[0], app);

    Paragraph::new(Line::from(Span::styled(
        format!("sorted by 24h {}", app.sort.label().to_lowercase()),
        theme::muted(),
    )))
    .style(theme::base())
    .alignment(Alignment::Right)
    .render(sections[1], frame.buffer_mut());

    let usage_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Ratio(1, 2),
            Constraint::Length(1),
            Constraint::Ratio(1, 2),
        ])
        .split(sections[2]);

    let top = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Ratio(1, 2),
            Constraint::Length(1),
            Constraint::Ratio(1, 2),
        ])
        .split(usage_rows[0]);
    let bottom = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Ratio(1, 2),
            Constraint::Length(1),
            Constraint::Ratio(1, 2),
        ])
        .split(usage_rows[2]);

    for (area, section) in [top[0], top[2], bottom[0], bottom[2]]
        .into_iter()
        .zip(data.sections.iter().take(4))
    {
        render_tool_usage_section(frame, area, section);
    }

    render_footer(frame, sections[3], app);
    render_project_modal(frame, root, app);
    render_currency_modal(frame, root, app);
}

fn render_tool_usage_section(frame: &mut Frame<'_>, area: Rect, section: &ToolLimitSection) {
    let title = format!("{} Console · 24h + models", section.tool);
    let block = theme::panel_block(&title, usage_tool_color(section.tool));
    let inner = block.inner(area);
    block.render(area, frame.buffer_mut());

    let split = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(2)])
        .split(inner);

    render_tool_usage_header(frame, split[0], section);
    render_tool_usage_rows(frame, split[1], section);
}

fn render_tool_usage_header(frame: &mut Frame<'_>, area: Rect, section: &ToolLimitSection) {
    let spark_width = area.width.saturating_sub(12).max(12) as usize;
    let mut pulse = vec![Span::styled("24h pulse ", theme::key())];
    pulse.extend(graphs::sparkline_spans(&section.usage.buckets, spark_width));

    let text = Text::from(vec![
        Line::from(pulse),
        Line::from(vec![
            Span::styled(
                section.usage.cost,
                theme::money().add_modifier(Modifier::BOLD),
            ),
            Span::styled(" cost   ", theme::dim()),
            Span::styled(section.usage.calls.to_string(), theme::base()),
            Span::styled(" calls   ", theme::dim()),
            Span::styled(section.usage.tokens, theme::muted()),
            Span::styled(" tokens   seen ", theme::dim()),
            Span::styled(section.usage.last_seen, theme::muted()),
        ]),
    ]);

    Paragraph::new(text)
        .style(theme::base())
        .render(area, frame.buffer_mut());
}

fn render_tool_usage_rows(frame: &mut Frame<'_>, area: Rect, section: &ToolLimitSection) {
    let mut rows: Vec<Row<'static>> = section
        .limits
        .iter()
        .map(|limit| {
            Row::new(vec![
                Cell::from("limit").style(theme::base().fg(theme::CYAN)),
                Cell::from(format!("{} {}", limit.scope, limit.window)).style(theme::muted()),
                graphs::gauge_cell(limit.used),
                Cell::from(limit.left).style(theme::base()),
                Cell::from(limit.reset).style(theme::muted()),
                Cell::from(limit.plan).style(theme::base().fg(theme::YELLOW_SOFT)),
            ])
        })
        .collect();

    rows.extend(section.models.iter().map(|model| {
        Row::new(vec![
            Cell::from("model").style(theme::base().fg(theme::BLUE_SOFT)),
            Cell::from(model.name).style(theme::muted()),
            graphs::rank_cell(model.value),
            Cell::from(model.calls.to_string()).style(theme::base()),
            Cell::from(model.tokens).style(theme::muted()),
            Cell::from(model.cost).style(theme::money()),
        ])
    }));

    let table = Table::new(
        rows,
        [
            Constraint::Length(6),
            Constraint::Min(14),
            Constraint::Length(8),
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Length(10),
        ],
    )
    .header(Row::new(vec![
        Cell::from("kind").style(theme::dim()),
        Cell::from("scope/model").style(theme::dim()),
        Cell::from("bar").style(theme::dim()),
        Cell::from("left/call").style(theme::dim()),
        Cell::from("reset/tok").style(theme::dim()),
        Cell::from("cost/plan").style(theme::dim()),
    ]))
    .column_spacing(1);

    frame.render_widget(table, area);
}

fn usage_tool_color(tool: &str) -> Color {
    match tool {
        "Codex" => theme::PRIMARY,
        "Claude Code" => theme::MAGENTA,
        "Cursor" => theme::BLUE,
        "Copilot" => theme::GREEN,
        _ => theme::CYAN,
    }
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
    render_download_confirm_modal(frame, root, app);
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
            "archive db",
            app.paths.archive_db_file.display().to_string(),
        ),
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

pub(super) fn render_session_page(frame: &mut Frame<'_>, area: Rect, root: Rect, app: &App) {
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(4),
            Constraint::Length(1),
            Constraint::Min(8),
            Constraint::Length(3),
        ])
        .split(area);

    render_session_header(frame, sections[0], app);
    render_session_summary(frame, sections[1], app);
    render_session_calls(frame, sections[3], app);
    render_footer(frame, sections[4], app);
    render_session_modal(frame, root, app);
    render_currency_modal(frame, root, app);
    render_project_modal(frame, root, app);
    render_call_detail_modal(frame, root, app);
}

fn render_session_header(frame: &mut Frame<'_>, area: Rect, _app: &App) {
    Paragraph::new(Line::from(vec![
        Span::styled("[ Session ]", theme::key()),
        Span::raw("    "),
        Span::styled("Dashboard", theme::dim()),
        Span::raw("    "),
        Span::styled("Config", theme::dim()),
    ]))
    .style(theme::base())
    .render(area, frame.buffer_mut());
}

fn render_session_summary(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let Some(view) = app.session_view.as_ref() else {
        let text = Text::from(vec![Line::from(vec![Span::styled(
            "no session loaded · press s to pick one",
            theme::muted(),
        )])]);
        Paragraph::new(text)
            .block(theme::panel_block("Session", theme::PRIMARY))
            .style(theme::base())
            .render(area, frame.buffer_mut());
        return;
    };

    let mut lines = vec![
        Line::from(vec![
            Span::styled(view.project.as_str(), theme::key()),
            Span::raw("  "),
            Span::styled(view.tool, theme::base().fg(theme::YELLOW_SOFT)),
            Span::raw("  "),
            Span::styled(view.session_id.as_str(), theme::muted()),
        ]),
        Line::from(vec![
            Span::styled(
                view.total_cost.as_str(),
                theme::money().add_modifier(Modifier::BOLD),
            ),
            Span::styled(" cost   ", theme::muted()),
            Span::styled(
                view.total_calls.to_string(),
                theme::base().add_modifier(Modifier::BOLD),
            ),
            Span::styled(" calls   ", theme::muted()),
            Span::styled(view.date_range.as_str(), theme::muted()),
        ]),
        Line::from(vec![
            Span::styled(view.total_input.as_str(), theme::muted()),
            Span::styled(" in   ", theme::dim()),
            Span::styled(view.total_output.as_str(), theme::muted()),
            Span::styled(" out   ", theme::dim()),
            Span::styled(view.total_cache_read.as_str(), theme::muted()),
            Span::styled(" cached", theme::dim()),
        ]),
    ];

    if let Some(note) = view.note.as_ref() {
        lines.push(Line::from(vec![Span::styled(note.as_str(), theme::dim())]));
    }

    Paragraph::new(Text::from(lines))
        .block(theme::panel_block("Session", theme::PRIMARY))
        .style(theme::base())
        .render(area, frame.buffer_mut());
}

fn render_session_calls(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let Some(view) = app.session_view.as_ref() else {
        return;
    };

    let inner_height = area.height.saturating_sub(3) as usize; // header + table header + 1 buffer
    let total = view.calls.len();
    let start = app.session_scroll.min(total.saturating_sub(1));
    let end = (start + inner_height.max(1)).min(total);

    let rows = view.calls[start..end]
        .iter()
        .enumerate()
        .map(|(offset, call)| {
            let idx = start + offset;
            let bg = if idx == app.session_selected {
                theme::SURFACE
            } else {
                theme::BACKGROUND
            };
            Row::new(vec![
                Cell::from(call.timestamp.clone()).style(theme::muted().bg(bg)),
                Cell::from(call.model.clone()).style(theme::base().bg(bg)),
                Cell::from(call.cost.clone()).style(theme::money().bg(bg)),
                Cell::from(format_compact_u64(call.input_tokens)).style(theme::base().bg(bg)),
                Cell::from(format_compact_u64(call.output_tokens)).style(theme::base().bg(bg)),
                Cell::from(format_compact_u64(call.cache_read)).style(theme::muted().bg(bg)),
                Cell::from(format_compact_u64(call.cache_write)).style(theme::muted().bg(bg)),
                Cell::from(call.tools.clone()).style(theme::base().fg(theme::BLUE_SOFT).bg(bg)),
                Cell::from(call.prompt.clone()).style(theme::muted().bg(bg)),
            ])
        });

    let title = format!(
        "Calls · {}–{} of {}",
        if total == 0 { 0 } else { start + 1 },
        end,
        total
    );
    let table = Table::new(
        rows,
        [
            Constraint::Length(11),
            Constraint::Length(16),
            Constraint::Length(9),
            Constraint::Length(7),
            Constraint::Length(7),
            Constraint::Length(7),
            Constraint::Length(7),
            Constraint::Length(22),
            Constraint::Min(20),
        ],
    )
    .header(Row::new(vec![
        Cell::from("time").style(theme::dim()),
        Cell::from("model").style(theme::dim()),
        Cell::from("cost").style(theme::dim()),
        Cell::from("in").style(theme::dim()),
        Cell::from("out").style(theme::dim()),
        Cell::from("cache r").style(theme::dim()),
        Cell::from("cache w").style(theme::dim()),
        Cell::from("tools").style(theme::dim()),
        Cell::from("prompt").style(theme::dim()),
    ]))
    .column_spacing(1)
    .block(theme::panel_block(&title, theme::CYAN));

    frame.render_widget(table, area);
}

fn render_call_detail_modal(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let Some(call) = app.selected_call_detail() else {
        return;
    };

    let width = 110.min(area.width.saturating_sub(4)).max(70);
    let height = 30.min(area.height.saturating_sub(4)).max(18);
    let modal_area = centered_rect(width, height, area);
    Clear.render(modal_area, frame.buffer_mut());

    let title = format!(
        "Call Detail · {}",
        app.call_detail_index
            .map(|idx| idx.saturating_add(1).to_string())
            .unwrap_or_else(|| "-".into())
    );
    let block = theme::panel_block(&title, theme::PRIMARY);
    let inner = block.inner(modal_area);
    block.render(modal_area, frame.buffer_mut());

    let lines = call_detail_lines(call);
    Paragraph::new(Text::from(lines))
        .style(theme::base())
        .wrap(Wrap { trim: false })
        .render(inner, frame.buffer_mut());
}

fn call_detail_lines(call: &SessionDetail) -> Vec<Line<'static>> {
    let mut lines = vec![
        Line::from(vec![
            Span::styled("time ", theme::dim()),
            Span::styled(call.timestamp.clone(), theme::muted()),
            Span::raw("   "),
            Span::styled("model ", theme::dim()),
            Span::styled(call.model.clone(), theme::base()),
            Span::raw("   "),
            Span::styled("cost ", theme::dim()),
            Span::styled(
                call.cost.clone(),
                theme::money().add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("in ", theme::dim()),
            Span::styled(format_compact_u64(call.input_tokens), theme::base()),
            Span::raw("   "),
            Span::styled("out ", theme::dim()),
            Span::styled(format_compact_u64(call.output_tokens), theme::base()),
            Span::raw("   "),
            Span::styled("cache r ", theme::dim()),
            Span::styled(format_compact_u64(call.cache_read), theme::muted()),
            Span::raw("   "),
            Span::styled("cache w ", theme::dim()),
            Span::styled(format_compact_u64(call.cache_write), theme::muted()),
            Span::raw("   "),
            Span::styled("reasoning ", theme::dim()),
            Span::styled(format_compact_u64(call.reasoning_tokens), theme::muted()),
            Span::raw("   "),
            Span::styled("web ", theme::dim()),
            Span::styled(format_compact_u64(call.web_search_requests), theme::muted()),
        ]),
        Line::from(vec![
            Span::styled("tools ", theme::dim()),
            Span::styled(call.tools.clone(), theme::base().fg(theme::BLUE_SOFT)),
        ]),
    ];

    if !call.bash_commands.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(vec![Span::styled("bash", theme::key())]));
        for command in &call.bash_commands {
            lines.push(Line::from(vec![
                Span::styled("$ ", theme::dim()),
                Span::styled(command.clone(), theme::muted()),
            ]));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![Span::styled("prompt", theme::key())]));
    lines.push(Line::from(Span::styled(
        if call.prompt_full.is_empty() {
            "-".to_string()
        } else {
            call.prompt_full.clone()
        },
        theme::muted(),
    )));
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("Esc/Enter", theme::key()),
        Span::styled(" close", theme::muted()),
    ]));

    lines
}

fn format_compact_u64(n: u64) -> String {
    if n >= 1_000_000_000 {
        format!("{:.1}B", n as f64 / 1_000_000_000.0)
    } else if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else if n == 0 {
        "-".into()
    } else {
        n.to_string()
    }
}

pub(super) fn render_help_modal(frame: &mut Frame<'_>, area: Rect, app: &App) {
    if !app.help_open {
        return;
    }

    let width = 80.min(area.width.saturating_sub(4)).max(60);
    let height = 38.min(area.height.saturating_sub(4)).max(24);
    let modal_area = centered_rect(width, height, area);
    Clear.render(modal_area, frame.buffer_mut());

    let block = theme::panel_block("Help · keybindings", theme::PRIMARY);
    let inner = block.inner(modal_area);
    block.render(modal_area, frame.buffer_mut());

    let mut lines: Vec<Line> = Vec::new();
    for (i, group) in keymap::keymap().help_groups().iter().enumerate() {
        if i > 0 {
            lines.push(Line::from(""));
        }
        lines.push(Line::from(vec![Span::styled(
            group.title.clone(),
            theme::base().fg(theme::CYAN).add_modifier(Modifier::BOLD),
        )]));
        for item in &group.items {
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(format!("{:<22}", item.keys), theme::key()),
                Span::styled(item.label.clone(), theme::muted()),
            ]));
        }
    }

    Paragraph::new(Text::from(lines))
        .style(theme::base())
        .render(inner, frame.buffer_mut());
}

pub(super) fn render_export_modal(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let Some(modal) = app.export_modal.as_ref() else {
        return;
    };

    let width = 84.min(area.width.saturating_sub(4)).max(56);
    let height = (modal.options.len() as u16 + 6)
        .min(area.height.saturating_sub(4))
        .max(10);
    let modal_area = centered_rect(width, height, area);
    Clear.render(modal_area, frame.buffer_mut());

    let title = format!(
        "Export {}/{}",
        modal.selected.saturating_add(1),
        modal.options.len().max(1)
    );
    let block = theme::panel_block(&title, theme::YELLOW);
    let inner = block.inner(modal_area);
    block.render(modal_area, frame.buffer_mut());

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(inner);

    Paragraph::new(Text::from(vec![
        Line::from(vec![
            Span::styled("Format ", theme::key()),
            Span::styled("current period & filters apply", theme::dim()),
        ]),
        Line::from(vec![
            Span::styled("Folder ", theme::key()),
            Span::styled(app.export_dir.display().to_string(), theme::muted()),
        ]),
    ]))
    .style(theme::base())
    .render(layout[0], frame.buffer_mut());

    let rows = modal.options.iter().enumerate().map(|(idx, option)| {
        let is_selected = idx == modal.selected;
        let bg = if is_selected {
            theme::SURFACE
        } else {
            theme::BACKGROUND
        };
        let marker = if is_selected { ">" } else { " " };
        Row::new(vec![
            Cell::from(marker).style(theme::key().bg(bg)),
            Cell::from(option.label()).style(if is_selected {
                theme::key().bg(bg)
            } else {
                theme::muted().bg(bg)
            }),
        ])
    });

    let table = Table::new(rows, [Constraint::Length(2), Constraint::Min(20)]).column_spacing(1);

    frame.render_widget(table, layout[1]);

    Paragraph::new(Line::from(vec![
        Span::styled("Enter", theme::key()),
        Span::styled(" export   ", theme::muted()),
        Span::styled("f/b", theme::key()),
        Span::styled(" browse folder   ", theme::muted()),
        Span::styled("Esc", theme::key()),
        Span::styled(" close", theme::muted()),
    ]))
    .style(theme::base())
    .render(layout[2], frame.buffer_mut());
}

pub(super) fn render_download_confirm_modal(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let Some(target) = app.download_confirm else {
        return;
    };

    let width = 88.min(area.width.saturating_sub(4)).max(58);
    let height = 11.min(area.height.saturating_sub(4)).max(9);
    let modal_area = centered_rect(width, height, area);
    Clear.render(modal_area, frame.buffer_mut());

    let block = theme::panel_block(target.title(), theme::YELLOW);
    let inner = block.inner(modal_area);
    block.render(modal_area, frame.buffer_mut());

    let output = match target {
        ConfigDownload::CurrencyRates => &app.paths.currency_rates_file,
        ConfigDownload::PricingSnapshot => &app.paths.pricing_snapshot_file,
    };

    let lines = vec![
        Line::from(vec![
            Span::styled("File   ", theme::key()),
            Span::styled(target.file_name(), theme::base()),
        ]),
        Line::from(vec![
            Span::styled("Source ", theme::key()),
            Span::styled(target.source(), theme::muted()),
        ]),
        Line::from(vec![
            Span::styled("Write  ", theme::key()),
            Span::styled(output.display().to_string(), theme::muted()),
        ]),
        Line::from(vec![
            Span::styled("After  ", theme::key()),
            Span::styled(target.effect(), theme::muted()),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Enter/y", theme::key()),
            Span::styled(" download    ", theme::muted()),
            Span::styled("Esc/n", theme::key()),
            Span::styled(" cancel", theme::muted()),
        ]),
    ];

    Paragraph::new(Text::from(lines))
        .style(theme::base())
        .render(inner, frame.buffer_mut());
}

pub(super) fn render_export_dir_picker_modal(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let Some(picker) = app.export_dir_picker.as_ref() else {
        return;
    };

    let width = 92.min(area.width.saturating_sub(4)).max(60);
    let height = (picker.entries.len() as u16 + 6)
        .min(area.height.saturating_sub(4))
        .max(12);
    let modal_area = centered_rect(width, height, area);
    Clear.render(modal_area, frame.buffer_mut());

    let title = format!(
        "Export Folder {}/{}",
        picker.selected.saturating_add(1),
        picker.entries.len().max(1)
    );
    let block = theme::panel_block(&title, theme::CYAN);
    let inner = block.inner(modal_area);
    block.render(modal_area, frame.buffer_mut());

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(1),
        ])
        .split(inner);

    Paragraph::new(Line::from(vec![
        Span::styled("Current ", theme::key()),
        Span::styled(picker.current_dir.display().to_string(), theme::muted()),
    ]))
    .style(theme::base())
    .render(layout[0], frame.buffer_mut());

    Paragraph::new(Line::from(vec![
        Span::styled("Enter", theme::key()),
        Span::styled(" select/open   ", theme::muted()),
        Span::styled("Backspace/Left", theme::key()),
        Span::styled(" parent   ", theme::muted()),
        Span::styled("Esc", theme::key()),
        Span::styled(" cancel", theme::muted()),
    ]))
    .style(theme::base())
    .render(layout[1], frame.buffer_mut());

    let visible = layout[2].height as usize;
    let start = picker.selected.saturating_sub(visible.saturating_sub(1));
    let rows = picker
        .entries
        .iter()
        .enumerate()
        .skip(start)
        .take(visible)
        .map(|(idx, entry)| {
            let is_selected = idx == picker.selected;
            let bg = if is_selected {
                theme::SURFACE
            } else {
                theme::BACKGROUND
            };
            let kind = match entry.kind {
                FolderPickerEntryKind::UseCurrent => "use",
                FolderPickerEntryKind::Parent => "up",
                FolderPickerEntryKind::Directory => "dir",
            };
            let name_style = if is_selected {
                theme::key().bg(bg)
            } else {
                theme::muted().bg(bg)
            };
            Row::new(vec![
                Cell::from(if is_selected { ">" } else { " " }).style(theme::key().bg(bg)),
                Cell::from(kind).style(theme::dim().bg(bg)),
                Cell::from(entry.label.as_str()).style(name_style),
                Cell::from(entry.path.display().to_string()).style(theme::dim().bg(bg)),
            ])
        });

    let table = Table::new(
        rows,
        [
            Constraint::Length(2),
            Constraint::Length(4),
            Constraint::Length(24),
            Constraint::Min(20),
        ],
    )
    .column_spacing(1);

    frame.render_widget(table, layout[2]);

    let footer = match picker.error.as_ref() {
        Some(error) => Line::from(Span::styled(error.as_str(), theme::base().fg(theme::RED))),
        None => Line::from(Span::styled("hidden folders are not shown", theme::dim())),
    };
    Paragraph::new(footer)
        .style(theme::base())
        .render(layout[3], frame.buffer_mut());
}

pub(super) fn render_session_modal(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let Some(modal) = app.session_modal.as_ref() else {
        return;
    };

    let width = 92.min(area.width.saturating_sub(4)).max(60);
    let height = (modal.filtered.len() as u16 + 4)
        .min(area.height.saturating_sub(4))
        .max(10);
    let modal_area = centered_rect(width, height, area);
    Clear.render(modal_area, frame.buffer_mut());

    let title = if modal.query.is_empty() {
        format!(
            "Session {}/{}",
            modal.selected.saturating_add(1),
            modal.filtered.len().max(1)
        )
    } else {
        format!(
            "Session {}/{} (of {})",
            modal.selected.saturating_add(1),
            modal.filtered.len().max(1),
            modal.options.len()
        )
    };
    let block = theme::panel_block(&title, theme::RED);
    let inner = block.inner(modal_area);
    block.render(modal_area, frame.buffer_mut());

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(inner);

    render_filter_input(frame, layout[0], &modal.query);

    let table_area = layout[1];
    let row_capacity = table_area.height.saturating_sub(1).max(1) as usize;
    let selected = modal.selected.min(modal.filtered.len().saturating_sub(1));
    let start = selected.saturating_add(1).saturating_sub(row_capacity);
    let end = (start + row_capacity).min(modal.filtered.len());

    let rows = modal.filtered[start..end]
        .iter()
        .enumerate()
        .map(|(offset, &option_idx)| {
            let idx = start + offset;
            let option = &modal.options[option_idx];
            let is_selected = idx == modal.selected;
            let bg = if is_selected {
                theme::SURFACE
            } else {
                theme::BACKGROUND
            };
            let marker = if is_selected { ">" } else { " " };

            Row::new(vec![
                Cell::from(marker).style(theme::key().bg(bg)),
                Cell::from(option.date.as_str()).style(theme::muted().bg(bg)),
                Cell::from(option.tool).style(theme::base().fg(theme::YELLOW_SOFT).bg(bg)),
                Cell::from(option.project.as_str()).style(if is_selected {
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
            Constraint::Length(11),
            Constraint::Length(8),
            Constraint::Min(20),
            Constraint::Length(10),
            Constraint::Length(8),
        ],
    )
    .header(Row::new(vec![
        Cell::from(""),
        Cell::from("date").style(theme::dim()),
        Cell::from("tool").style(theme::dim()),
        Cell::from("project").style(theme::dim()),
        Cell::from("cost").style(theme::dim()),
        Cell::from("calls").style(theme::dim()),
    ]))
    .column_spacing(1);

    frame.render_widget(table, table_area);
}

pub(super) fn render_footer(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let footer = match app.page {
        Page::Config => "config",
        Page::Session => "session",
        Page::Usage => "usage",
        Page::Overview | Page::DeepDive => "dashboard",
    };
    let commands = footer_line(keymap::keymap().footer(footer), app);

    Paragraph::new(commands)
        .alignment(Alignment::Center)
        .block(theme::panel_block("", theme::DIM))
        .style(theme::base())
        .render(area, frame.buffer_mut());
}

fn footer_line(hints: &[keymap::KeyHint], app: &App) -> Line<'static> {
    let mut spans = Vec::new();
    for (idx, hint) in hints.iter().enumerate() {
        if idx > 0 {
            spans.push(Span::raw("    "));
        }
        if let Some(period) = footer_period_action(&hint.action) {
            let style = if app.period == period {
                theme::key()
            } else {
                theme::muted()
            };
            spans.push(Span::styled(format!("{} {}", hint.keys, hint.label), style));
        } else {
            spans.push(Span::styled(hint.keys.clone(), theme::key()));
            spans.push(Span::styled(format!(" {}", hint.label), theme::muted()));
        }
    }
    Line::from(spans)
}

fn footer_period_action(action: &str) -> Option<Period> {
    match action {
        keymap::ACTION_PERIOD_TODAY => Some(Period::Today),
        keymap::ACTION_PERIOD_WEEK => Some(Period::Week),
        keymap::ACTION_PERIOD_THIRTY_DAYS => Some(Period::ThirtyDays),
        keymap::ACTION_PERIOD_MONTH => Some(Period::Month),
        keymap::ACTION_PERIOD_ALL_TIME => Some(Period::AllTime),
        _ => None,
    }
}

pub(super) fn render_project_modal(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let Some(modal) = app.project_modal.as_ref() else {
        return;
    };

    let width = 76.min(area.width.saturating_sub(4)).max(48);
    let height = (modal.filtered.len() as u16 + 4)
        .min(area.height.saturating_sub(4))
        .max(8);
    let modal_area = centered_rect(width, height, area);
    Clear.render(modal_area, frame.buffer_mut());

    let title = if modal.query.is_empty() {
        format!(
            "Project {}/{}",
            modal.selected.saturating_add(1),
            modal.filtered.len().max(1)
        )
    } else {
        format!(
            "Project {}/{} (of {})",
            modal.selected.saturating_add(1),
            modal.filtered.len().max(1),
            modal.options.len()
        )
    };
    let block = theme::panel_block(&title, theme::GREEN);
    let inner = block.inner(modal_area);
    block.render(modal_area, frame.buffer_mut());

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(inner);

    render_filter_input(frame, layout[0], &modal.query);

    let table_area = layout[1];
    let row_capacity = table_area.height.saturating_sub(1).max(1) as usize;
    let selected = modal.selected.min(modal.filtered.len().saturating_sub(1));
    let start = selected.saturating_add(1).saturating_sub(row_capacity);
    let end = (start + row_capacity).min(modal.filtered.len());

    let rows = modal.filtered[start..end]
        .iter()
        .enumerate()
        .map(|(offset, &option_idx)| {
            let idx = start + offset;
            let option = &modal.options[option_idx];
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

    frame.render_widget(table, table_area);
}

fn render_filter_input(frame: &mut Frame<'_>, area: Rect, query: &str) {
    let line = if query.is_empty() {
        Line::from(vec![
            Span::styled("Filter ", theme::key()),
            Span::styled(
                "type to search · Backspace to delete · Ctrl-U clear",
                theme::dim(),
            ),
        ])
    } else {
        Line::from(vec![
            Span::styled("Filter ", theme::key()),
            Span::styled(
                query.to_string(),
                theme::base().add_modifier(Modifier::BOLD),
            ),
            Span::styled("_", theme::muted()),
        ])
    };
    Paragraph::new(line)
        .style(theme::base())
        .render(area, frame.buffer_mut());
}

pub(super) fn render_currency_modal(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let Some(modal) = app.currency_modal.as_ref() else {
        return;
    };

    let width = 58.min(area.width.saturating_sub(4)).max(40);
    let height = (modal.filtered.len() as u16 + 4)
        .min(area.height.saturating_sub(4))
        .max(10);
    let modal_area = centered_rect(width, height, area);
    Clear.render(modal_area, frame.buffer_mut());

    let title = if modal.query.is_empty() {
        format!(
            "Currency {}/{}",
            modal.selected.saturating_add(1),
            modal.filtered.len().max(1)
        )
    } else {
        format!(
            "Currency {}/{} (of {})",
            modal.selected.saturating_add(1),
            modal.filtered.len().max(1),
            modal.options.len()
        )
    };
    let block = theme::panel_block(&title, theme::PRIMARY);
    let inner = block.inner(modal_area);
    block.render(modal_area, frame.buffer_mut());

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(inner);

    render_filter_input(frame, layout[0], &modal.query);

    let table_area = layout[1];
    let row_capacity = table_area.height.saturating_sub(1).max(1) as usize;
    let selected = modal.selected.min(modal.filtered.len().saturating_sub(1));
    let start = selected.saturating_add(1).saturating_sub(row_capacity);
    let end = (start + row_capacity).min(modal.filtered.len());

    let rows = modal.filtered[start..end]
        .iter()
        .enumerate()
        .map(|(offset, &code_idx)| {
            let idx = start + offset;
            let code = &modal.options[code_idx];
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

    frame.render_widget(table, table_area);
}
