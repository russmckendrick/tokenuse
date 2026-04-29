use std::{
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
    ingest, ui,
};

fn main() -> Result<()> {
    color_eyre::install()?;

    if handle_subcommand()? {
        return Ok(());
    }

    let (source, status) = match ingest::load() {
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

    let mut session = TerminalSession::new()?;
    run(session.terminal(), App::with_source(source, status))
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

#[cfg(feature = "refresh-prices")]
fn handle_subcommand() -> Result<bool> {
    let mut args = std::env::args().skip(1);
    if let Some(arg) = args.next() {
        if arg == "--refresh-prices" {
            let target = std::path::PathBuf::from("src/pricing/snapshot.json");
            tokenuse::pricing::refresh::run(&target)?;
            println!("wrote {}", target.display());
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(not(feature = "refresh-prices"))]
fn handle_subcommand() -> Result<bool> {
    if std::env::args().any(|a| a == "--refresh-prices") {
        eprintln!("--refresh-prices requires building with --features refresh-prices");
        return Ok(true);
    }
    Ok(false)
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
