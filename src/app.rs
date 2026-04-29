use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Period {
    Today,
    Week,
    ThirtyDays,
    Month,
    AllTime,
}

impl Period {
    pub const ALL: [Self; 5] = [
        Self::Today,
        Self::Week,
        Self::ThirtyDays,
        Self::Month,
        Self::AllTime,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Today => "Today",
            Self::Week => "7 Days",
            Self::ThirtyDays => "30 Days",
            Self::Month => "This Month",
            Self::AllTime => "All Time",
        }
    }

    pub fn key(self) -> char {
        match self {
            Self::Today => '1',
            Self::Week => '2',
            Self::ThirtyDays => '3',
            Self::Month => '4',
            Self::AllTime => '5',
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    Claude,
    All,
}

impl Provider {
    pub fn label(self) -> &'static str {
        match self {
            Self::Claude => "Claude",
            Self::All => "All",
        }
    }

    fn next(self) -> Self {
        match self {
            Self::Claude => Self::All,
            Self::All => Self::Claude,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Dashboard,
    Optimize,
    Compare,
}

impl View {
    pub fn label(self) -> &'static str {
        match self {
            Self::Dashboard => "Dashboard",
            Self::Optimize => "Optimize",
            Self::Compare => "Compare",
        }
    }

    fn next(self) -> Self {
        match self {
            Self::Dashboard => Self::Optimize,
            Self::Optimize => Self::Compare,
            Self::Compare => Self::Dashboard,
        }
    }
}

#[derive(Debug, Clone)]
pub struct App {
    pub period: Period,
    pub provider: Provider,
    pub view: View,
    should_quit: bool,
}

impl Default for App {
    fn default() -> Self {
        Self {
            period: Period::Week,
            provider: Provider::Claude,
            view: View::Dashboard,
            should_quit: false,
        }
    }
}

impl App {
    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        if key.kind != KeyEventKind::Press {
            return;
        }

        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Char('1') => self.period = Period::Today,
            KeyCode::Char('2') => self.period = Period::Week,
            KeyCode::Char('3') => self.period = Period::ThirtyDays,
            KeyCode::Char('4') => self.period = Period::Month,
            KeyCode::Char('5') => self.period = Period::AllTime,
            KeyCode::Char('p') => self.provider = self.provider.next(),
            KeyCode::Char('o') => self.view = View::Optimize,
            KeyCode::Char('c') => self.view = View::Compare,
            KeyCode::Char('<') | KeyCode::Char('>') | KeyCode::Tab => self.view = self.view.next(),
            _ => {}
        }
    }
}
