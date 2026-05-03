mod components;
mod graphs;
mod sections;

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    prelude::{Frame, Line, Span},
    text::Text,
    widgets::{Block, Paragraph, Widget},
};

use crate::{
    app::{App, Page},
    copy::copy,
    data::DashboardData,
    theme,
};

use components::{centered_rect, weighted_columns};
use sections::{
    render_activity_pulse, render_config, render_counts, render_currency_modal, render_daily_trend,
    render_export_dir_picker_modal, render_export_modal, render_footer, render_help_modal,
    render_kpi_strip, render_limits, render_model_efficiency, render_models, render_project_modal,
    render_project_tools, render_projects, render_session_modal, render_session_page,
    render_sessions, render_title_bar,
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
            Constraint::Length(7),
            Constraint::Length(1),
            Constraint::Min(16),
            Constraint::Length(3),
        ])
        .split(area);

    let data = app.dashboard();

    render_title_bar(frame, sections[0], app);
    render_kpi_strip(frame, sections[1], app, &data.summary);
    render_activity_pulse(frame, sections[3], &data.activity_timeline);

    let lower = weighted_columns(sections[5], 58);
    render_project_tools(frame, lower[0], &data.project_tools);

    let right_split = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(42),
            Constraint::Length(1),
            Constraint::Percentage(29),
            Constraint::Percentage(29),
        ])
        .split(lower[1]);
    render_models(frame, right_split[0], &data.models);
    render_counts(
        frame,
        right_split[2],
        copy().panels.shell_commands.as_str(),
        theme::PRIMARY,
        &data.commands,
    );
    render_counts(
        frame,
        right_split[3],
        copy().panels.mcp_servers.as_str(),
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
    render_daily_trend(frame, top[0], &data.activity_timeline);
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
    render_model_efficiency(frame, right_stack[0], &data.models);
    render_counts(
        frame,
        right_stack[2],
        copy().panels.core_tools.as_str(),
        theme::CYAN,
        &data.tools,
    );

    let bottom = weighted_columns(sections[8], 70);
    render_counts(
        frame,
        bottom[0],
        copy().panels.shell_commands.as_str(),
        theme::PRIMARY,
        &data.commands,
    );
    render_counts(
        frame,
        bottom[1],
        copy().panels.mcp_servers.as_str(),
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
    let copy = copy();
    let block = theme::panel_block(copy.brand.command.as_str(), theme::PRIMARY);
    let text = Text::from(vec![
        Line::from(vec![
            Span::styled(copy.empty.terminal_too_small.as_str(), theme::key()),
            Span::styled(
                copy.empty.terminal_dashboard_suffix.as_str(),
                theme::muted(),
            ),
        ]),
        Line::from(vec![Span::styled(
            copy.empty.terminal_resize.as_str(),
            theme::muted(),
        )]),
        Line::from(vec![
            Span::styled("q", theme::key()),
            Span::styled(format!(" {}", copy.keymap.actions["quit"]), theme::muted()),
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

    fn session_app_with_call_detail() -> App {
        let mut app = App::default();
        app.page = Page::Session;
        app.session_view = Some(crate::data::SessionDetailView {
            key: "codex:s1".into(),
            session_id: "s1".into(),
            project: "tokens".into(),
            tool: "Codex",
            date_range: "2026-04-29".into(),
            total_cost: "$0.12".into(),
            total_calls: 1,
            total_input: "100".into(),
            total_output: "50".into(),
            total_cache_read: "20".into(),
            calls: vec![crate::data::SessionDetail {
                timestamp: "04-29 12:00".into(),
                model: "gpt-5".into(),
                cost: "$0.12".into(),
                input_tokens: 100,
                output_tokens: 50,
                cache_read: 20,
                cache_write: 5,
                reasoning_tokens: 7,
                web_search_requests: 2,
                tools: "exec_command".into(),
                bash_commands: vec!["cargo test".into()],
                prompt: "run checks".into(),
                prompt_full: "run the checks and show me failures".into(),
            }],
            note: None,
        });
        app.call_detail_index = Some(0);
        app
    }

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

        let copy = copy();
        assert!(rendered.contains(&copy.brand.name));
        assert!(rendered.contains(&copy.brand.mark));
        assert!(!rendered.contains("v0.0.2"));
        assert!(rendered.contains(&copy.panels.activity_trend));
        assert!(rendered.contains(&copy.panels.model_efficiency));
        assert!(rendered.contains(&copy.panels.project_spend_by_tool));
        assert!(rendered.contains("q quit"));
        let first_footer_hint = copy.footer("dashboard")[0].clone();
        assert!(rendered.contains(&format!(
            "{} {}",
            first_footer_hint.keys, first_footer_hint.label
        )));
        assert!(rendered.contains("h help"));
        assert!(rendered.contains("[t]"));
        assert!(rendered.contains("[p]"));
        assert!(rendered.contains("[g]"));
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
        app.status = Some(crate::app::AppStatus::info("auto-refreshed · 12399 calls"));
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

        let copy = copy();
        assert!(rendered.contains(&copy.brand.name));
        assert!(rendered.contains(&copy.nav.overview));
        assert!(rendered.contains(&copy.nav.deep_dive));
        assert!(rendered.contains(&copy.nav.usage));
        assert!(rendered.contains(&copy.metrics.cost.to_ascii_uppercase()));
        assert!(rendered.contains(&copy.metrics.calls.to_ascii_uppercase()));
        assert!(rendered.contains(&copy.metrics.cache_hit.to_ascii_uppercase()));
        assert!(rendered.contains(&copy.panels.activity_pulse));
        assert!(rendered.contains(&copy.panels.project_spend_by_tool));
        assert!(rendered.contains(&copy.panels.shell_commands));
        assert!(rendered.contains(&copy.panels.mcp_servers));
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
        let copy = copy();
        assert!(rendered.contains(&copy.modals.help_title));
        assert!(rendered.contains("keybindings"));
        assert!(rendered.contains(&copy.keymap.help[1].title));
        assert!(rendered.contains(&copy.keymap.help[6].title));
        let help_item = &copy.keymap.help[0].items[1];
        assert!(rendered.contains(&help_item.label));

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
        assert!(rendered.contains(&copy().tools.all));
        assert!(rendered.contains(&copy().tables.cost));
        assert!(rendered.contains(&copy().tables.calls));
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

        let copy = copy();
        assert!(rendered.contains(&copy.nav.configuration));
        assert!(rendered.contains(&copy.config.rows.currency_override.name));
        assert!(rendered.contains(&copy.config.rows.rates_json.name));
        assert!(rendered.contains(&copy.config.rows.litellm_prices.name));
        assert!(rendered.contains(&copy.config.rows.clear_data.name));
        assert!(rendered.contains(&copy.panels.local_files));
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

        let copy = copy();
        assert!(rendered.contains(&copy.modals.download_rates_title));
        assert!(rendered.contains(&copy.modals.rates_source));
        assert!(rendered.contains("Enter/y"));
        assert!(rendered.contains("Esc/n"));
    }

    #[test]
    fn config_clear_data_confirmation_modal_render_smoke_test() {
        let backend = TestBackend::new(170, 64);
        let mut terminal = Terminal::new(backend).expect("create terminal");
        let mut app = App::default();
        app.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE));
        app.handle_key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE));
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        terminal
            .draw(|frame| render(frame, &app))
            .expect("draw clear data confirmation modal");

        let buffer = terminal.backend().buffer();
        let rendered = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();

        let copy = copy();
        assert!(rendered.contains(&copy.modals.clear_data_question));
        assert!(rendered.contains(&copy.modals.delete));
        assert!(rendered.contains(&copy.modals.missing_source_files));
        assert!(rendered.contains(&copy.actions.clear_data_lower));
        assert!(rendered.contains("Esc/n"));
    }

    #[test]
    fn config_clear_data_running_modal_render_smoke_test() {
        let backend = TestBackend::new(170, 64);
        let mut terminal = Terminal::new(backend).expect("create terminal");
        let mut app = App::default();
        app.page = Page::Config;
        app.clear_data_modal = Some(crate::app::ClearDataModal::Running);

        terminal
            .draw(|frame| render(frame, &app))
            .expect("draw clear data running modal");

        let rendered = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();

        let copy = copy();
        assert!(rendered.contains(&copy.modals.clearing_data));
        assert!(rendered.contains(&copy.modals.rebuilding_archive));
        assert!(rendered.contains(&copy.modals.local_history));
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

        let copy = copy();
        assert!(rendered.contains(&copy.nav.usage));
        assert!(rendered.contains(&copy.tools.codex));
        assert!(rendered.contains("5h"));
        assert!(rendered.contains("weekly"));
        assert!(rendered.contains("% left"));
        assert!(rendered.contains("24h"));
        assert!(rendered.contains(&copy.usage.model));
        assert!(rendered.contains(&copy.tools.claude_code));
        assert!(rendered.contains(&copy.tools.cursor));
        assert!(rendered.contains(&copy.tools.copilot));
        assert!(rendered.contains(&copy.tools.gemini));
        assert!(rendered.contains("Console"));
        assert!(rendered.contains(&copy.usage.pulse));
        assert!(rendered.contains(&copy.metrics.tokens));
        assert!(rendered.contains(&crate::copy::template(
            &copy.filters.sorted_by_24h,
            &[("sort", app.sort.label().to_lowercase())],
        )));
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
        assert!(rendered.contains(&copy().tables.per_usd));
    }

    #[test]
    fn session_call_detail_modal_render_smoke_test() {
        let backend = TestBackend::new(170, 64);
        let mut terminal = Terminal::new(backend).expect("create terminal");
        let app = session_app_with_call_detail();

        terminal
            .draw(|frame| render(frame, &app))
            .expect("draw session call detail modal");

        let rendered = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();

        assert!(rendered.contains(&copy().session.call_detail));
        assert!(rendered.contains("gpt-5"));
        assert!(rendered.contains("cargo test"));
        assert!(rendered.contains("run the checks and show me failures"));
    }
}
