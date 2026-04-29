use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};

use crate::config::{ConfigPaths, UserConfig};
use crate::currency::{CurrencyFormatter, CurrencyTable};
use crate::data::{DashboardData, LimitsData, ProjectOption};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Dashboard,
    Config,
    Usage,
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

#[derive(Debug, Clone)]
pub struct CurrencyModal {
    pub options: Vec<String>,
    pub selected: usize,
}

#[derive(Debug, Clone)]
pub struct ConfigRowView {
    pub name: &'static str,
    pub value: String,
    pub action: &'static str,
}

pub enum DataSource {
    Live(Ingested),
    Sample,
}

pub struct App {
    pub page: Page,
    pub period: Period,
    pub tool: Tool,
    pub project_filter: ProjectFilter,
    pub project_modal: Option<ProjectModal>,
    pub currency_modal: Option<CurrencyModal>,
    pub config_selected: usize,
    pub settings: UserConfig,
    pub paths: ConfigPaths,
    pub currency_table: CurrencyTable,
    pub source: DataSource,
    pub status: Option<String>,
    should_quit: bool,
}

impl Default for App {
    fn default() -> Self {
        Self {
            page: Page::Dashboard,
            period: Period::Week,
            tool: Tool::All,
            project_filter: ProjectFilter::All,
            project_modal: None,
            currency_modal: None,
            config_selected: 0,
            settings: UserConfig::default(),
            paths: ConfigPaths::default(),
            currency_table: CurrencyTable::embedded()
                .expect("embedded currency rates must be valid JSON"),
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

    pub fn with_runtime(
        source: DataSource,
        status: Option<String>,
        settings: UserConfig,
        paths: ConfigPaths,
        currency_table: CurrencyTable,
    ) -> Self {
        Self {
            source,
            status,
            settings,
            paths,
            currency_table,
            ..Self::default()
        }
    }

    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    pub fn dashboard(&self) -> DashboardData {
        let currency = self.currency();
        match &self.source {
            DataSource::Live(ingested) => {
                ingested.dashboard(self.period, self.tool, &self.project_filter, &currency)
            }
            DataSource::Sample => {
                crate::data::dashboard_data(self.period, self.tool, &self.project_filter, &currency)
            }
        }
    }

    pub fn usage(&self) -> LimitsData {
        let currency = self.currency();
        match &self.source {
            DataSource::Live(ingested) => ingested.limits(self.tool, &currency),
            DataSource::Sample => crate::data::limits_data(self.tool),
        }
    }

    pub fn currency(&self) -> CurrencyFormatter {
        self.currency_table.formatter(&self.settings.currency)
    }

    pub fn config_rows(&self) -> Vec<ConfigRowView> {
        let currency = self.currency();
        let currency_value = if currency.is_usd() {
            "USD (default)".into()
        } else {
            format!(
                "{} · 1 USD = {:.6}",
                currency.code(),
                self.currency_table.rate(currency.code()).unwrap_or(1.0)
            )
        };

        let rates_value = format!(
            "{} · {} · {}",
            self.currency_table.source().short_label(),
            self.currency_table.source_name(),
            self.currency_table.date()
        );

        let pricing_value = if self.paths.pricing_snapshot_file.exists() {
            "local snapshot".into()
        } else {
            "embedded snapshot".into()
        };

        vec![
            ConfigRowView {
                name: "currency override",
                value: currency_value,
                action: "pick",
            },
            ConfigRowView {
                name: "rates.json",
                value: rates_value,
                action: "pull",
            },
            ConfigRowView {
                name: "LiteLLM prices",
                value: pricing_value,
                action: "pull",
            },
        ]
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        if key.kind != KeyEventKind::Press {
            return;
        }

        if self.currency_modal.is_some() {
            self.handle_currency_modal_key(key);
            return;
        }

        if self.project_modal.is_some() {
            self.handle_project_modal_key(key);
            return;
        }

        if key.code == KeyCode::Char('q') {
            self.should_quit = true;
            return;
        }

        if self.page == Page::Config {
            self.handle_config_key(key);
            return;
        }

        if self.page == Page::Usage {
            self.handle_usage_key(key);
            return;
        }

        match key.code {
            KeyCode::Esc => self.should_quit = true,
            KeyCode::Char('1') => self.period = Period::Today,
            KeyCode::Char('2') => self.period = Period::Week,
            KeyCode::Char('3') => self.period = Period::ThirtyDays,
            KeyCode::Char('4') => self.period = Period::Month,
            KeyCode::Char('5') => self.period = Period::AllTime,
            KeyCode::Char('t') => self.tool = self.tool.next(),
            KeyCode::Char('p') => self.open_project_modal(),
            KeyCode::Char('c') => self.page = Page::Config,
            KeyCode::Char('u') => self.page = Page::Usage,
            _ => {}
        }
    }

    fn project_options(&self) -> Vec<ProjectOption> {
        let currency = self.currency();
        match &self.source {
            DataSource::Live(ingested) => {
                ingested.project_options(self.period, self.tool, &currency)
            }
            DataSource::Sample => crate::data::project_options(self.period, self.tool, &currency),
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

    fn handle_config_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('d') | KeyCode::Char('c') => {
                self.page = Page::Dashboard;
            }
            KeyCode::Up => {
                self.config_selected = self.config_selected.saturating_sub(1);
            }
            KeyCode::Down => {
                let last = self.config_rows().len().saturating_sub(1);
                self.config_selected = (self.config_selected + 1).min(last);
            }
            KeyCode::Home => self.config_selected = 0,
            KeyCode::End => self.config_selected = self.config_rows().len().saturating_sub(1),
            KeyCode::Enter => self.activate_config_row(),
            _ => {}
        }
    }

    fn handle_usage_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('d') | KeyCode::Char('u') => {
                self.page = Page::Dashboard;
            }
            KeyCode::Char('c') => self.page = Page::Config,
            _ => {}
        }
    }

    fn activate_config_row(&mut self) {
        match self.config_selected {
            0 => self.open_currency_modal(),
            1 => self.refresh_currency_rates(),
            2 => self.refresh_pricing_snapshot(),
            _ => {}
        }
    }

    fn open_currency_modal(&mut self) {
        let options = self.currency_table.codes();
        let selected = options
            .iter()
            .position(|code| code == self.currency().code())
            .unwrap_or(0);
        self.currency_modal = Some(CurrencyModal { options, selected });
    }

    fn handle_currency_modal_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.currency_modal = None,
            KeyCode::Up => {
                if let Some(modal) = self.currency_modal.as_mut() {
                    modal.selected = modal.selected.saturating_sub(1);
                }
            }
            KeyCode::Down => {
                if let Some(modal) = self.currency_modal.as_mut() {
                    let last = modal.options.len().saturating_sub(1);
                    modal.selected = (modal.selected + 1).min(last);
                }
            }
            KeyCode::Home => {
                if let Some(modal) = self.currency_modal.as_mut() {
                    modal.selected = 0;
                }
            }
            KeyCode::End => {
                if let Some(modal) = self.currency_modal.as_mut() {
                    modal.selected = modal.options.len().saturating_sub(1);
                }
            }
            KeyCode::Enter => {
                let selected = self
                    .currency_modal
                    .as_ref()
                    .and_then(|modal| modal.options.get(modal.selected))
                    .cloned();
                if let Some(code) = selected {
                    self.set_currency(&code);
                }
                self.currency_modal = None;
            }
            _ => {}
        }
    }

    fn set_currency(&mut self, code: &str) {
        self.settings.set_currency(code);
        match self.settings.save(&self.paths) {
            Ok(()) => {
                self.status = Some(format!("currency set to {}", self.currency().code()));
            }
            Err(e) => {
                self.status = Some(format!("config save failed · {e}"));
            }
        }
    }

    #[cfg(feature = "refresh-currency")]
    fn refresh_currency_rates(&mut self) {
        match crate::currency::refresh::download_published_snapshot(&self.paths.currency_rates_file)
            .and_then(|_| CurrencyTable::load(&self.paths))
        {
            Ok(table) => {
                self.currency_table = table;
                self.status = Some(format!("rates refreshed · {}", self.currency_table.date()));
            }
            Err(e) => {
                self.status = Some(format!("rates refresh failed · {e}"));
            }
        }
    }

    #[cfg(not(feature = "refresh-currency"))]
    fn refresh_currency_rates(&mut self) {
        self.status = Some("rates refresh requires cargo run --features refresh-currency".into());
    }

    #[cfg(feature = "refresh-prices")]
    fn refresh_pricing_snapshot(&mut self) {
        match crate::pricing::refresh::run(&self.paths.pricing_snapshot_file) {
            Ok(()) => {
                self.status = Some("LiteLLM prices refreshed · restart to apply".into());
            }
            Err(e) => {
                self.status = Some(format!("LiteLLM refresh failed · {e}"));
            }
        }
    }

    #[cfg(not(feature = "refresh-prices"))]
    fn refresh_pricing_snapshot(&mut self) {
        self.status = Some("LiteLLM refresh requires cargo run --features refresh-prices".into());
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
    fn u_opens_usage_and_u_or_escape_returns_dashboard() {
        let mut app = App::default();

        app.handle_key(key(KeyCode::Char('u')));
        assert_eq!(app.page, Page::Usage);

        app.handle_key(key(KeyCode::Char('u')));
        assert_eq!(app.page, Page::Dashboard);

        app.handle_key(key(KeyCode::Char('u')));
        app.handle_key(key(KeyCode::Esc));
        assert_eq!(app.page, Page::Dashboard);
        assert!(!app.should_quit());
    }

    #[test]
    fn usage_page_keeps_config_shortcut() {
        let mut app = App::default();

        app.handle_key(key(KeyCode::Char('u')));
        assert_eq!(app.page, Page::Usage);

        app.handle_key(key(KeyCode::Char('c')));
        assert_eq!(app.page, Page::Config);
    }

    #[test]
    fn usage_ignores_period_and_project_filters() {
        let mut app = App::default();
        app.period = Period::Today;
        app.project_filter = ProjectFilter::Selected {
            identity: "missing".into(),
            label: "missing".into(),
        };

        app.handle_key(key(KeyCode::Char('u')));
        let data = app.usage();

        assert_eq!(app.page, Page::Usage);
        assert!(!data.sections.is_empty());
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
