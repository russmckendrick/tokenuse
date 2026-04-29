use std::{
    collections::BTreeSet,
    io::{self, Stdout},
    time::Duration,
};

use color_eyre::Result;
use crossterm::{
    event::{self, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use tokenuse::{
    app::{App, DataSource},
    config::{ConfigPaths, UserConfig},
    currency::CurrencyTable,
    ingest, ui,
};

fn main() -> Result<()> {
    color_eyre::install()?;

    if handle_subcommand()? {
        return Ok(());
    }

    let paths = ConfigPaths::default();
    let mut status_messages = Vec::new();
    let settings = match UserConfig::load_or_create(&paths) {
        Ok(settings) => settings,
        Err(e) => {
            status_messages.push(format!("config failed · defaults ({e})"));
            UserConfig::default()
        }
    };
    let currency_table = match CurrencyTable::load(&paths) {
        Ok(table) => table,
        Err(e) => {
            status_messages.push(format!("currency rates failed · embedded ({e})"));
            CurrencyTable::embedded().expect("embedded currency rates must be valid JSON")
        }
    };

    let (source, ingest_status) = match ingest::load() {
        Ok(ingested) if !ingested.is_empty() => (DataSource::Live(ingested), None),
        Ok(_) => (
            DataSource::Sample,
            Some("no local sessions found · sample data".into()),
        ),
        Err(e) => (
            DataSource::Sample,
            Some(format!("ingest failed · sample data ({e})")),
        ),
    };
    if let Some(status) = ingest_status {
        status_messages.push(status);
    }
    let status = if status_messages.is_empty() {
        None
    } else {
        Some(status_messages.join(" · "))
    };

    let mut session = TerminalSession::new()?;
    run(
        session.terminal(),
        App::with_runtime(source, status, settings, paths, currency_table),
    )
}

fn run(terminal: &mut Terminal<CrosstermBackend<Stdout>>, mut app: App) -> Result<()> {
    loop {
        terminal.draw(|frame| ui::render(frame, &app))?;

        if app.should_quit() {
            break;
        }

        if event::poll(Duration::from_millis(200))? {
            if let Event::Key(key) = event::read()? {
                app.handle_key(key);
            }
        }
    }

    Ok(())
}

fn handle_subcommand() -> Result<bool> {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.iter().any(|arg| arg == "--list-projects") {
        print_project_inventory()?;
        return Ok(true);
    }

    if args.iter().any(|arg| arg == "--refresh-prices") {
        refresh_prices()?;
        return Ok(true);
    }

    if args.iter().any(|arg| arg == "--generate-currency-json") {
        refresh_currency()?;
        return Ok(true);
    }

    Ok(false)
}

fn print_project_inventory() -> Result<()> {
    let ingested = ingest::load()?;
    if ingested.is_empty() {
        println!("no local sessions found");
        return Ok(());
    }

    let rows = ingested.project_inventory();
    let projects: BTreeSet<&str> = rows.iter().map(|row| row.project.as_str()).collect();
    let raw_projects: BTreeSet<&str> = rows.iter().map(|row| row.raw_project.as_str()).collect();

    println!(
        "{} projects, {} raw variants, {} project-agent rows, {} calls",
        projects.len(),
        raw_projects.len(),
        rows.len(),
        ingested.calls.len()
    );
    println!();

    let project_w = rows
        .iter()
        .map(|row| row.project.len())
        .chain(std::iter::once("project".len()))
        .max()
        .unwrap_or("project".len());
    let agent_w = rows
        .iter()
        .map(|row| row.tool.len())
        .chain(std::iter::once("agent".len()))
        .max()
        .unwrap_or("agent".len());
    let calls_w = rows
        .iter()
        .map(|row| row.calls.to_string().len())
        .chain(std::iter::once("calls".len()))
        .max()
        .unwrap_or("calls".len());
    let sessions_w = rows
        .iter()
        .map(|row| row.sessions.to_string().len())
        .chain(std::iter::once("sess".len()))
        .max()
        .unwrap_or("sess".len());
    let cost_w = rows
        .iter()
        .map(|row| row.cost.len())
        .chain(std::iter::once("cost".len()))
        .max()
        .unwrap_or("cost".len());

    println!(
        "{:<project_w$}  {:<agent_w$}  {:>calls_w$}  {:>sessions_w$}  {:>cost_w$}  raw project",
        "project",
        "agent",
        "calls",
        "sess",
        "cost",
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
    let target = std::path::PathBuf::from("src/pricing/snapshot.json");
    tokenuse::pricing::refresh::run(&target)?;
    println!("wrote {}", target.display());
    Ok(())
}

#[cfg(not(feature = "refresh-prices"))]
fn refresh_prices() -> Result<()> {
    eprintln!("--refresh-prices requires building with --features refresh-prices");
    Ok(())
}

#[cfg(feature = "refresh-currency")]
fn refresh_currency() -> Result<()> {
    let target = std::path::PathBuf::from("currency/rates.json");
    tokenuse::currency::refresh::run(&target)?;
    println!("wrote {}", target.display());
    Ok(())
}

#[cfg(not(feature = "refresh-currency"))]
fn refresh_currency() -> Result<()> {
    eprintln!("--generate-currency-json requires building with --features refresh-currency");
    Ok(())
}

struct TerminalSession {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl TerminalSession {
    fn new() -> Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
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
        let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
        let _ = self.terminal.show_cursor();
    }
}
