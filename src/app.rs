use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};

use crate::data::{DashboardData, ProjectOption};
use crate::ingest::Ingested;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
pub enum Tool {
    All,
    ClaudeCode,
    Cursor,
    Codex,
    Copilot,
}

impl Tool {
    pub fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::ClaudeCode => "Claude Code",
            Self::Cursor => "Cursor",
            Self::Codex => "Codex",
            Self::Copilot => "Copilot",
        }
    }

    fn next(self) -> Self {
        match self {
            Self::All => Self::ClaudeCode,
            Self::ClaudeCode => Self::Cursor,
            Self::Cursor => Self::Codex,
            Self::Codex => Self::Copilot,
            Self::Copilot => Self::All,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectFilter {
    All,
    Selected { identity: String, label: String },
}

impl Default for ProjectFilter {
    fn default() -> Self {
        Self::All
    }
}

impl ProjectFilter {
    pub fn label(&self) -> &str {
        match self {
            Self::All => "All",
            Self::Selected { label, .. } => label,
        }
    }

    pub fn identity(&self) -> Option<&str> {
        match self {
            Self::All => None,
            Self::Selected { identity, .. } => Some(identity),
        }
    }

    fn from_option(option: &ProjectOption) -> Self {
        match &option.identity {
            Some(identity) => Self::Selected {
                identity: identity.clone(),
                label: option.label.clone(),
            },
            None => Self::All,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProjectModal {
    pub options: Vec<ProjectOption>,
    pub selected: usize,
}

pub enum DataSource {
    Live(Ingested),
    Sample,
}

pub struct App {
    pub period: Period,
    pub tool: Tool,
    pub project_filter: ProjectFilter,
    pub project_modal: Option<ProjectModal>,
    pub source: DataSource,
    pub status: Option<String>,
    should_quit: bool,
}

impl Default for App {
    fn default() -> Self {
        Self {
            period: Period::Week,
            tool: Tool::All,
            project_filter: ProjectFilter::All,
            project_modal: None,
            source: DataSource::Sample,
            status: None,
            should_quit: false,
        }
    }
}

impl App {
    pub fn with_source(source: DataSource, status: Option<String>) -> Self {
        Self {
            source,
            status,
            ..Self::default()
        }
    }

    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    pub fn dashboard(&self) -> DashboardData {
        match &self.source {
            DataSource::Live(ingested) => {
                ingested.dashboard(self.period, self.tool, &self.project_filter)
            }
            DataSource::Sample => {
                crate::data::dashboard_data(self.period, self.tool, &self.project_filter)
            }
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        if key.kind != KeyEventKind::Press {
            return;
        }

        if self.project_modal.is_some() {
            self.handle_project_modal_key(key);
            return;
        }

        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Char('1') => self.period = Period::Today,
            KeyCode::Char('2') => self.period = Period::Week,
            KeyCode::Char('3') => self.period = Period::ThirtyDays,
            KeyCode::Char('4') => self.period = Period::Month,
            KeyCode::Char('5') => self.period = Period::AllTime,
            KeyCode::Char('t') => self.tool = self.tool.next(),
            KeyCode::Char('p') => self.open_project_modal(),
            _ => {}
        }
    }

    fn project_options(&self) -> Vec<ProjectOption> {
        match &self.source {
            DataSource::Live(ingested) => ingested.project_options(self.period, self.tool),
            DataSource::Sample => crate::data::project_options(self.period, self.tool),
        }
    }

    fn open_project_modal(&mut self) {
        let mut options = self.project_options();
        if options.is_empty() {
            options.push(ProjectOption::all("$0.00".into(), 0));
        }

        let selected = self
            .project_filter
            .identity()
            .and_then(|identity| {
                options
                    .iter()
                    .position(|option| option.identity.as_deref() == Some(identity))
            })
            .unwrap_or(0);

        self.project_modal = Some(ProjectModal { options, selected });
    }

    fn handle_project_modal_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.project_modal = None,
            KeyCode::Up => {
                if let Some(modal) = self.project_modal.as_mut() {
                    modal.selected = modal.selected.saturating_sub(1);
                }
            }
            KeyCode::Down => {
                if let Some(modal) = self.project_modal.as_mut() {
                    let last = modal.options.len().saturating_sub(1);
                    modal.selected = (modal.selected + 1).min(last);
                }
            }
            KeyCode::Home => {
                if let Some(modal) = self.project_modal.as_mut() {
                    modal.selected = 0;
                }
            }
            KeyCode::End => {
                if let Some(modal) = self.project_modal.as_mut() {
                    modal.selected = modal.options.len().saturating_sub(1);
                }
            }
            KeyCode::Enter => {
                let selected = self
                    .project_modal
                    .as_ref()
                    .and_then(|modal| modal.options.get(modal.selected))
                    .cloned();
                if let Some(option) = selected {
                    self.project_filter = ProjectFilter::from_option(&option);
                }
                self.project_modal = None;
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::KeyModifiers;

    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn t_cycles_tool_filter() {
        let mut app = App::default();

        app.handle_key(key(KeyCode::Char('t')));

        assert_eq!(app.tool, Tool::ClaudeCode);
    }

    #[test]
    fn project_modal_selects_project() {
        let mut app = App::default();

        app.handle_key(key(KeyCode::Char('p')));
        assert!(app.project_modal.is_some());
        assert_eq!(app.project_modal.as_ref().unwrap().options[0].label, "All");

        app.handle_key(key(KeyCode::Down));
        app.handle_key(key(KeyCode::Enter));

        assert!(app.project_modal.is_none());
        assert!(matches!(app.project_filter, ProjectFilter::Selected { .. }));
        assert!(!app.should_quit());
    }

    #[test]
    fn project_modal_escape_keeps_existing_filter() {
        let mut app = App::default();

        app.handle_key(key(KeyCode::Char('p')));
        app.handle_key(key(KeyCode::Down));
        app.handle_key(key(KeyCode::Esc));

        assert!(app.project_modal.is_none());
        assert_eq!(app.project_filter, ProjectFilter::All);
        assert!(!app.should_quit());
    }

    #[test]
    fn q_only_quits_when_project_modal_is_closed() {
        let mut app = App::default();

        app.handle_key(key(KeyCode::Char('p')));
        app.handle_key(key(KeyCode::Char('q')));

        assert!(app.project_modal.is_some());
        assert!(!app.should_quit());

        app.handle_key(key(KeyCode::Esc));
        app.handle_key(key(KeyCode::Char('q')));

        assert!(app.should_quit());
    }
}
