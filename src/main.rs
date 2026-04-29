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
use tokenuse::{app::App, ui};

fn main() -> Result<()> {
    color_eyre::install()?;

    let mut session = TerminalSession::new()?;
    let result = run(session.terminal());
    result
}

fn run(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    let mut app = App::default();

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
