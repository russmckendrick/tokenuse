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
    theme,
};

use components::{centered_rect, two_columns};
use sections::{
    render_config, render_counts, render_currency_modal, render_daily, render_footer,
    render_models, render_nav, render_project_modal, render_project_tools, render_projects,
    render_sessions, render_summary,
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
        Page::Dashboard => render_dashboard(frame, area, root, app),
        Page::Config => render_config(frame, area, root, app),
    }
}

fn render_dashboard(frame: &mut Frame<'_>, area: Rect, root: Rect, app: &App) {
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(4),
            Constraint::Length(1),
            Constraint::Length(9),
            Constraint::Length(1),
            Constraint::Length(8),
            Constraint::Length(1),
            Constraint::Min(10),
            Constraint::Length(1),
            Constraint::Length(10),
            Constraint::Length(1),
            Constraint::Length(5),
            Constraint::Length(3),
        ])
        .split(area);

    let data = app.dashboard();

    render_nav(frame, sections[0], app);
    render_summary(frame, sections[1], app, &data.summary);

    let top = two_columns(sections[3]);
    render_daily(frame, top[0], &data.daily);
    render_projects(frame, top[1], &data.projects);

    render_sessions(frame, sections[5], &data.sessions);

    let middle = two_columns(sections[7]);
    render_project_tools(frame, middle[0], &data.project_tools);
    render_models(frame, middle[1], &data.models);

    let lower = two_columns(sections[9]);
    render_counts(frame, lower[0], "Core Tools", theme::CYAN, &data.tools);
    render_counts(
        frame,
        lower[1],
        "Shell Commands",
        theme::PRIMARY,
        &data.commands,
    );

    render_counts(
        frame,
        sections[11],
        "MCP Servers",
        theme::MAGENTA,
        &data.mcp_servers,
    );
    render_footer(frame, sections[12], app);
    render_project_modal(frame, root, app);
    render_currency_modal(frame, root, app);
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
        let app = App::default();

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
        assert!(rendered.contains("t tool"));
        assert!(rendered.contains("p project"));
        assert!(rendered.contains("c config"));
        assert!(!rendered.contains("p tool"));
        assert!(!rendered.contains("switch"));
        assert!(!rendered.contains("optimize"));
        assert!(!rendered.contains("compare"));
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
