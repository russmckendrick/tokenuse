mod components;
mod sections;

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    prelude::{Frame, Line, Span},
    text::Text,
    widgets::{Block, Paragraph, Widget},
};

use crate::{
    app::{App, Page},
    data::DashboardData,
    theme,
};

use components::{centered_rect, two_columns, weighted_columns};
use sections::{
    render_config, render_counts, render_currency_modal, render_daily,
    render_export_dir_picker_modal, render_export_modal, render_footer, render_help_modal,
    render_kpi_strip, render_limits, render_models, render_project_modal, render_project_tools,
    render_projects, render_session_modal, render_session_page, render_sessions, render_title_bar,
};

pub fn render(frame: &mut Frame<'_>, app: &App) {
    let root = frame.area();
    Block::default()
        .style(theme::base())
        .render(root, frame.buffer_mut());

    if root.width < 120 || root.height < 40 {
        render_small_terminal_notice(frame, root);
        return;
    }

    let area = root.inner(Margin {
        horizontal: 1,
        vertical: 1,
    });

    match app.page {
        Page::Overview => render_overview(frame, area, root, app),
        Page::DeepDive => render_dashboard(frame, area, root, app),
        Page::Config => render_config(frame, area, root, app),
        Page::Usage => render_limits(frame, area, root, app),
        Page::Session => render_session_page(frame, area, root, app),
    }

    render_help_modal(frame, root, app);
}

fn render_overview(frame: &mut Frame<'_>, area: Rect, root: Rect, app: &App) {
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(5),
            Constraint::Length(1),
            Constraint::Min(11),
            Constraint::Length(1),
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .split(area);

    let data = app.dashboard();

    render_title_bar(frame, sections[0], app);
    render_kpi_strip(frame, sections[1], app, &data.summary);

    let middle = two_columns(sections[3]);
    render_daily(frame, middle[0], &data.daily);
    render_models(frame, middle[1], &data.models);

    let lower = two_columns(sections[5]);
    render_project_tools(frame, lower[0], &data.project_tools);

    let right_split = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(lower[1]);
    render_counts(
        frame,
        right_split[0],
        "Shell Commands",
        theme::PRIMARY,
        &data.commands,
    );
    render_counts(
        frame,
        right_split[1],
        "MCP Servers",
        theme::MAGENTA,
        &data.mcp_servers,
    );

    render_footer(frame, sections[6], app);
    render_project_modal(frame, root, app);
    render_currency_modal(frame, root, app);
    render_session_modal(frame, root, app);
    render_export_modal(frame, root, app);
    render_export_dir_picker_modal(frame, root, app);
}

fn render_dashboard(frame: &mut Frame<'_>, area: Rect, root: Rect, app: &App) {
    let data = app.dashboard();
    let heights = deep_dive_panel_heights(area.height, &data);
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Length(heights[0]),
            Constraint::Length(1),
            Constraint::Length(heights[1]),
            Constraint::Length(1),
            Constraint::Length(heights[2]),
            Constraint::Length(1),
            Constraint::Length(heights[3]),
            Constraint::Fill(1),
            Constraint::Length(3),
        ])
        .split(area);

    render_title_bar(frame, sections[0], app);

    let top = weighted_columns(sections[2], 35);
    render_daily(frame, top[0], &data.daily);
    render_projects(frame, top[1], &data.projects);

    render_sessions(frame, sections[4], &data.sessions);

    let middle = weighted_columns(sections[6], 58);
    render_project_tools(frame, middle[0], &data.project_tools);

    let model_height = table_panel_height(data.models.len(), 5, 7)
        .min(middle[1].height.saturating_sub(4))
        .max(3)
        .min(middle[1].height);
    let right_stack = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(model_height),
            Constraint::Length(u16::from(middle[1].height > model_height + 1)),
            Constraint::Min(0),
        ])
        .split(middle[1]);
    render_models(frame, right_stack[0], &data.models);
    render_counts(
        frame,
        right_stack[2],
        "Core Tools",
        theme::CYAN,
        &data.tools,
    );

    let bottom = weighted_columns(sections[8], 70);
    render_counts(
        frame,
        bottom[0],
        "Shell Commands",
        theme::PRIMARY,
        &data.commands,
    );
    render_counts(
        frame,
        bottom[1],
        "MCP Servers",
        theme::MAGENTA,
        &data.mcp_servers,
    );
    render_footer(frame, sections[10], app);
    render_project_modal(frame, root, app);
    render_currency_modal(frame, root, app);
    render_session_modal(frame, root, app);
    render_export_modal(frame, root, app);
    render_export_dir_picker_modal(frame, root, app);
}

fn deep_dive_panel_heights(area_height: u16, data: &DashboardData) -> [u16; 4] {
    let top = table_panel_height(data.daily.len().max(data.projects.len()), 6, 10);
    let sessions = table_panel_height(data.sessions.len(), 6, 13);
    let project_tools = table_panel_height(data.project_tools.len(), 8, 15);
    let models = table_panel_height(data.models.len(), 5, 7);
    let tools = table_panel_height(data.tools.len(), 6, 13);
    let main = project_tools
        .max(models.saturating_add(1).saturating_add(tools))
        .min(21);
    let bottom = table_panel_height(data.commands.len().max(data.mcp_servers.len()), 5, 13);
    let mut heights = [top, sessions, main, bottom];

    let reserved = 3 + 3 + 4; // title, footer, and the gaps between content bands
    let available = area_height.saturating_sub(reserved);
    let mut overflow = heights.iter().sum::<u16>().saturating_sub(available);
    let minimums = [6, 6, 8, 5];

    for idx in [2, 1, 3, 0] {
        if overflow == 0 {
            break;
        }
        let shrink = heights[idx].saturating_sub(minimums[idx]).min(overflow);
        heights[idx] -= shrink;
        overflow -= shrink;
    }

    heights
}

fn table_panel_height(rows: usize, min: u16, max: u16) -> u16 {
    (rows as u16).saturating_add(3).clamp(min, max)
}

fn render_small_terminal_notice(frame: &mut Frame<'_>, area: Rect) {
    let block = theme::panel_block("tokenuse", theme::PRIMARY);
    let text = Text::from(vec![
        Line::from(vec![
            Span::styled("terminal too small", theme::key()),
            Span::styled(" for the dashboard", theme::muted()),
        ]),
        Line::from(vec![Span::styled(
            "resize to at least 120x40 for the full MVP layout",
            theme::muted(),
        )]),
        Line::from(vec![
            Span::styled("q", theme::key()),
            Span::styled(" quit", theme::muted()),
        ]),
    ]);

    let notice = centered_rect(72, 7, area);
    Paragraph::new(text)
        .alignment(Alignment::Center)
        .block(block)
        .style(theme::base())
        .render(notice, frame.buffer_mut());
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::{backend::TestBackend, Terminal};

    use super::*;

    #[test]
    fn dashboard_render_smoke_test() {
        let backend = TestBackend::new(170, 64);
        let mut terminal = Terminal::new(backend).expect("create terminal");
        let mut app = App::default();
        app.handle_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE));

        terminal
            .draw(|frame| render(frame, &app))
            .expect("draw dashboard");

        let buffer = terminal.backend().buffer();
        let rendered = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();

        assert!(rendered.contains("tokenuse"));
        assert!(rendered.contains("Daily Activity"));
        assert!(rendered.contains("Project Spend by Tool"));
        assert!(rendered.contains("q quit"));
        assert!(rendered.contains("h help"));
        assert!(rendered.contains("[t]"));
        assert!(rendered.contains("[p]"));
        assert!(rendered.contains("Tab"));
        assert!(!rendered.contains("p tool"));
        assert!(!rendered.contains("switch"));
        assert!(!rendered.contains("optimize"));
        assert!(!rendered.contains("compare"));
    }

    #[test]
    fn dashboard_omits_redundant_status_summary_strip() {
        let backend = TestBackend::new(170, 64);
        let mut terminal = Terminal::new(backend).expect("create terminal");
        let mut app = App::default();
        app.status = Some("auto-refreshed · 12399 calls".into());
        app.handle_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE));

        terminal
            .draw(|frame| render(frame, &app))
            .expect("draw dashboard");

        let rendered = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();

        assert!(!rendered.contains("auto-refreshed"));
        assert!(!rendered.contains("12399 calls"));
    }

    #[test]
    fn overview_render_smoke_test() {
        let backend = TestBackend::new(170, 64);
        let mut terminal = Terminal::new(backend).expect("create terminal");
        let app = App::default();

        terminal
            .draw(|frame| render(frame, &app))
            .expect("draw overview");

        let buffer = terminal.backend().buffer();
        let rendered = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();

        assert!(rendered.contains("tokenuse"));
        assert!(rendered.contains("Overview"));
        assert!(rendered.contains("Deep Dive"));
        assert!(rendered.contains("Usage"));
        assert!(rendered.contains("COST"));
        assert!(rendered.contains("CALLS"));
        assert!(rendered.contains("CACHE HIT"));
        assert!(rendered.contains("Daily Activity"));
        assert!(rendered.contains("Project Spend by Tool"));
        assert!(rendered.contains("Shell Commands"));
        assert!(rendered.contains("MCP Servers"));
        assert!(rendered.contains("Tab"));
    }

    #[test]
    fn h_opens_help_modal_and_h_or_escape_closes_it() {
        let backend = TestBackend::new(170, 64);
        let mut terminal = Terminal::new(backend).expect("create terminal");
        let mut app = App::default();

        app.handle_key(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE));
        assert!(app.help_open);

        terminal
            .draw(|frame| render(frame, &app))
            .expect("draw help modal");
        let rendered = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(rendered.contains("Help"));
        assert!(rendered.contains("keybindings"));
        assert!(rendered.contains("Period"));
        assert!(rendered.contains("Pickers"));

        app.handle_key(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE));
        assert!(!app.help_open);

        app.handle_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE));
        assert!(app.help_open);
        app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(!app.help_open);
    }

    #[test]
    fn project_modal_render_smoke_test() {
        let backend = TestBackend::new(170, 64);
        let mut terminal = Terminal::new(backend).expect("create terminal");
        let mut app = App::default();
        app.handle_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE));

        terminal
            .draw(|frame| render(frame, &app))
            .expect("draw dashboard");

        let buffer = terminal.backend().buffer();
        let rendered = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();

        assert!(rendered.contains("Project 1/"));
        assert!(rendered.contains("All"));
        assert!(rendered.contains("cost"));
        assert!(rendered.contains("calls"));
    }

    #[test]
    fn config_page_render_smoke_test() {
        let backend = TestBackend::new(170, 64);
        let mut terminal = Terminal::new(backend).expect("create terminal");
        let mut app = App::default();
        app.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE));

        terminal
            .draw(|frame| render(frame, &app))
            .expect("draw config page");

        let buffer = terminal.backend().buffer();
        let rendered = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();

        assert!(rendered.contains("Configuration"));
        assert!(rendered.contains("currency override"));
        assert!(rendered.contains("rates.json"));
        assert!(rendered.contains("LiteLLM prices"));
        assert!(rendered.contains("Local Files"));
        assert!(rendered.contains("Esc dashboard"));
    }

    #[test]
    fn config_download_confirmation_modal_render_smoke_test() {
        let backend = TestBackend::new(170, 64);
        let mut terminal = Terminal::new(backend).expect("create terminal");
        let mut app = App::default();
        app.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE));
        app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        terminal
            .draw(|frame| render(frame, &app))
            .expect("draw download confirmation modal");

        let buffer = terminal.backend().buffer();
        let rendered = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();

        assert!(rendered.contains("Download rates.json?"));
        assert!(rendered.contains("published tokenuse currency snapshot"));
        assert!(rendered.contains("Enter/y"));
        assert!(rendered.contains("Esc/n"));
    }

    #[test]
    fn usage_page_render_smoke_test() {
        let backend = TestBackend::new(170, 64);
        let mut terminal = Terminal::new(backend).expect("create terminal");
        let mut app = App::default();
        app.handle_key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::NONE));

        terminal
            .draw(|frame| render(frame, &app))
            .expect("draw usage page");

        let buffer = terminal.backend().buffer();
        let rendered = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();

        assert!(rendered.contains("Usage"));
        assert!(rendered.contains("Codex"));
        assert!(rendered.contains("5h"));
        assert!(rendered.contains("weekly"));
        assert!(rendered.contains("% left"));
        assert!(rendered.contains("24h"));
        assert!(rendered.contains("models"));
        assert!(rendered.contains("Claude Code"));
        assert!(rendered.contains("Cursor"));
        assert!(rendered.contains("Copilot"));
        assert!(rendered.contains("tokens"));
        assert!(rendered.contains("sorted by 24h usage"));
        assert!(rendered.contains("c config"));
    }

    #[test]
    fn currency_modal_render_smoke_test() {
        let backend = TestBackend::new(170, 64);
        let mut terminal = Terminal::new(backend).expect("create terminal");
        let mut app = App::default();
        app.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE));
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        terminal
            .draw(|frame| render(frame, &app))
            .expect("draw currency modal");

        let buffer = terminal.backend().buffer();
        let rendered = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();

        assert!(rendered.contains("Currency 1/"));
        assert!(rendered.contains("USD"));
        assert!(rendered.contains("per USD"));
    }
}
