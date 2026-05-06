use std::{
    collections::BTreeSet,
    io::{self, Stdout},
    time::Duration,
};

use color_eyre::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, layout::Rect, Terminal};
use tokenuse::{
    app::App,
    archive,
    config::ConfigPaths,
    copy::{copy, template},
    ingest, runtime, ui,
};

mod report_cli;

fn main() -> Result<()> {
    color_eyre::install()?;

    if handle_subcommand()? {
        return Ok(());
    }

    let startup = runtime::load_startup()?;

    let mut session = TerminalSession::new()?;
    run(
        session.terminal(),
        App::with_runtime(
            startup.source,
            startup.status,
            startup.settings,
            startup.paths,
            startup.currency_table,
            startup.initial_refresh_delay,
            startup.refresh_source,
        ),
    )
}

fn run(terminal: &mut Terminal<CrosstermBackend<Stdout>>, mut app: App) -> Result<()> {
    loop {
        terminal.draw(|frame| ui::render(frame, &app))?;

        if app.should_quit() {
            break;
        }

        // Short timeout so reload completion shows up promptly even when the
        // user isn't pressing keys. Archive sync/load runs on a background
        // thread and surfaces its result via App::poll_reload.
        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) => app.handle_key(key),
                Event::Mouse(mouse) => {
                    let size = terminal.size()?;
                    let area = Rect::new(0, 0, size.width, size.height);
                    app.handle_mouse(mouse, area);
                }
                _ => {}
            }
        }
        app.poll_reload();
    }

    Ok(())
}

fn handle_subcommand() -> Result<bool> {
    let args: Vec<String> = std::env::args().skip(1).collect();

    match cli_action(&args) {
        CliAction::Dashboard => Ok(false),
        CliAction::Version => {
            println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
            Ok(true)
        }
        CliAction::Help => {
            print_help();
            Ok(true)
        }
        CliAction::ListProjects => {
            print_project_inventory()?;
            Ok(true)
        }
        CliAction::RefreshPrices => {
            refresh_prices()?;
            Ok(true)
        }
        CliAction::GenerateCurrencyJson => {
            refresh_currency()?;
            Ok(true)
        }
        CliAction::Report => {
            report_cli::run()?;
            Ok(true)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CliAction {
    Dashboard,
    Help,
    Version,
    ListProjects,
    RefreshPrices,
    GenerateCurrencyJson,
    Report,
}

fn cli_action(args: &[String]) -> CliAction {
    if args.first().is_some_and(|arg| arg == "report") {
        if args.iter().skip(1).any(|arg| is_help_arg(arg)) {
            return CliAction::Help;
        }
        return CliAction::Report;
    }

    if args.iter().any(|arg| arg == "--version" || arg == "-V") {
        return CliAction::Version;
    }

    if args.iter().any(|arg| is_help_arg(arg)) {
        return CliAction::Help;
    }

    if args.iter().any(|arg| arg == "--list-projects") {
        return CliAction::ListProjects;
    }

    if args.iter().any(|arg| arg == "--refresh-prices") {
        return CliAction::RefreshPrices;
    }

    if args.iter().any(|arg| arg == "--generate-currency-json") {
        return CliAction::GenerateCurrencyJson;
    }

    CliAction::Dashboard
}

fn is_help_arg(arg: &str) -> bool {
    arg == "--help" || arg == "-h"
}

fn print_help() {
    let copy = copy();
    println!(
        "{name} {version}
{description}

{usage}
    {name} [FLAGS]
    {name} report

{commands}
    report                         {report_command}

{flags}
    -h, --help                     {help_flag}
    -V, --version                  {version_flag}
        --list-projects            {list_projects_flag}
        --refresh-prices           {refresh_prices_flag}
        --generate-currency-json   {generate_currency_flag}

{launch_dashboard}",
        name = env!("CARGO_PKG_NAME"),
        version = env!("CARGO_PKG_VERSION"),
        description = env!("CARGO_PKG_DESCRIPTION"),
        usage = copy.cli.usage,
        commands = copy.cli.commands,
        flags = copy.cli.flags,
        report_command = copy.cli.report_command,
        help_flag = copy.cli.help_flag,
        version_flag = copy.cli.version_flag,
        list_projects_flag = copy.cli.list_projects_flag,
        refresh_prices_flag = copy.cli.refresh_prices_flag,
        generate_currency_flag = copy.cli.generate_currency_flag,
        launch_dashboard = copy.cli.launch_dashboard,
    );
}

fn print_project_inventory() -> Result<()> {
    let paths = ConfigPaths::default();
    let ingested = match archive::sync_and_load(&paths) {
        Ok(ingested) => ingested,
        Err(e) => {
            eprintln!(
                "{}",
                template(
                    &copy().cli.archive_failed_raw_ingest,
                    &[("error", e.to_string())]
                )
            );
            ingest::load()?
        }
    };
    if ingested.is_empty() {
        println!("{}", copy().cli.no_local_sessions_found);
        return Ok(());
    }

    let rows = ingested.project_inventory();
    let projects: BTreeSet<&str> = rows.iter().map(|row| row.project.as_str()).collect();
    let raw_projects: BTreeSet<&str> = rows.iter().map(|row| row.raw_project.as_str()).collect();

    println!(
        "{}",
        template(
            &copy().cli.project_inventory_summary,
            &[
                ("projects", projects.len().to_string()),
                ("raw_projects", raw_projects.len().to_string()),
                ("rows", rows.len().to_string()),
                ("calls", ingested.calls.len().to_string())
            ]
        )
    );
    println!();

    let project_w = rows
        .iter()
        .map(|row| row.project.len())
        .chain(std::iter::once(copy().tables.project.len()))
        .max()
        .unwrap_or(copy().tables.project.len());
    let agent_w = rows
        .iter()
        .map(|row| row.tool.len())
        .chain(std::iter::once(copy().tables.agent.len()))
        .max()
        .unwrap_or(copy().tables.agent.len());
    let calls_w = rows
        .iter()
        .map(|row| row.calls.to_string().len())
        .chain(std::iter::once(copy().tables.calls.len()))
        .max()
        .unwrap_or(copy().tables.calls.len());
    let sessions_w = rows
        .iter()
        .map(|row| row.sessions.to_string().len())
        .chain(std::iter::once(copy().tables.sess.len()))
        .max()
        .unwrap_or(copy().tables.sess.len());
    let cost_w = rows
        .iter()
        .map(|row| row.cost.len())
        .chain(std::iter::once(copy().tables.cost.len()))
        .max()
        .unwrap_or(copy().tables.cost.len());

    println!(
        "{:<project_w$}  {:<agent_w$}  {:>calls_w$}  {:>sessions_w$}  {:>cost_w$}  {}",
        copy().tables.project,
        copy().tables.agent,
        copy().tables.calls,
        copy().tables.sess,
        copy().tables.cost,
        copy().tables.raw_project,
        project_w = project_w,
        agent_w = agent_w,
        calls_w = calls_w,
        sessions_w = sessions_w,
        cost_w = cost_w
    );

    for row in rows {
        println!(
            "{:<project_w$}  {:<agent_w$}  {:>calls_w$}  {:>sessions_w$}  {:>cost_w$}  {}",
            row.project,
            row.tool,
            row.calls,
            row.sessions,
            row.cost,
            row.raw_project,
            project_w = project_w,
            agent_w = agent_w,
            calls_w = calls_w,
            sessions_w = sessions_w,
            cost_w = cost_w
        );
    }

    Ok(())
}

#[cfg(feature = "refresh-prices")]
fn refresh_prices() -> Result<()> {
    let target = std::path::PathBuf::from("src/pricing/books");
    let output = tokenuse::pricing::refresh::run(&target)?;
    println!(
        "{}",
        template(
            &copy().cli.wrote_path,
            &[(
                "path",
                format!(
                    "{}, {}",
                    output.upstream.display(),
                    output.overrides.display()
                )
            )]
        )
    );
    Ok(())
}

#[cfg(not(feature = "refresh-prices"))]
fn refresh_prices() -> Result<()> {
    eprintln!("{}", copy().cli.refresh_prices_requires_feature);
    Ok(())
}

#[cfg(feature = "refresh-currency")]
fn refresh_currency() -> Result<()> {
    let target = std::path::PathBuf::from("currency/rates.json");
    tokenuse::currency::refresh::run(&target)?;
    println!(
        "{}",
        template(
            &copy().cli.wrote_path,
            &[("path", target.display().to_string())]
        )
    );
    Ok(())
}

#[cfg(not(feature = "refresh-currency"))]
fn refresh_currency() -> Result<()> {
    eprintln!("{}", copy().cli.generate_currency_requires_feature);
    Ok(())
}

struct TerminalSession {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl TerminalSession {
    fn new() -> Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;

        Ok(Self { terminal })
    }

    fn terminal(&mut self) -> &mut Terminal<CrosstermBackend<Stdout>> {
        &mut self.terminal
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(
            self.terminal.backend_mut(),
            DisableMouseCapture,
            LeaveAlternateScreen
        );
        let _ = self.terminal.show_cursor();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn report_command_routes_to_guided_wizard() {
        assert_eq!(cli_action(&args(&["report"])), CliAction::Report);
    }

    #[test]
    fn report_help_uses_normal_help_output() {
        assert_eq!(cli_action(&args(&["report", "--help"])), CliAction::Help);
        assert_eq!(cli_action(&args(&["report", "-h"])), CliAction::Help);
    }

    #[test]
    fn existing_flags_still_route_to_their_handlers() {
        assert_eq!(cli_action(&args(&["--help"])), CliAction::Help);
        assert_eq!(cli_action(&args(&["-V"])), CliAction::Version);
        assert_eq!(
            cli_action(&args(&["--list-projects"])),
            CliAction::ListProjects
        );
        assert_eq!(
            cli_action(&args(&["--refresh-prices"])),
            CliAction::RefreshPrices
        );
        assert_eq!(
            cli_action(&args(&["--generate-currency-json"])),
            CliAction::GenerateCurrencyJson
        );
        assert_eq!(cli_action(&[]), CliAction::Dashboard);
    }
}
