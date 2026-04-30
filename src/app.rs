use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::time::Duration;

use color_eyre::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::archive;
use crate::config::{ConfigPaths, UserConfig};
use crate::currency::{CurrencyFormatter, CurrencyTable};
use crate::data::{DashboardData, LimitsData, ProjectOption, SessionDetailView, SessionOption};
use crate::export::ExportFormat;
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
    Overview,
    DeepDive,
    Config,
    Usage,
    Session,
}

impl Page {
    pub const TABS: [Page; 3] = [Page::Overview, Page::DeepDive, Page::Usage];

    pub fn label(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::DeepDive => "Deep Dive",
            Self::Usage => "Usage",
            Self::Config => "Config",
            Self::Session => "Session",
        }
    }

    pub fn next_tab(self) -> Page {
        let tabs = Self::TABS;
        let idx = tabs.iter().position(|p| *p == self).unwrap_or(0);
        tabs[(idx + 1) % tabs.len()]
    }

    pub fn prev_tab(self) -> Page {
        let tabs = Self::TABS;
        let idx = tabs.iter().position(|p| *p == self).unwrap_or(0);
        tabs[(idx + tabs.len() - 1) % tabs.len()]
    }
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

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ProjectFilter {
    #[default]
    All,
    Selected {
        identity: String,
        label: String,
    },
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
    pub filtered: Vec<usize>,
    pub query: String,
    pub selected: usize,
}

impl ProjectModal {
    pub fn refilter(&mut self) {
        let needle = self.query.to_lowercase();
        self.filtered = self
            .options
            .iter()
            .enumerate()
            .filter(|(_, option)| {
                if option.identity.is_none() {
                    return true;
                }
                if needle.is_empty() {
                    return true;
                }
                option.label.to_lowercase().contains(&needle)
            })
            .map(|(idx, _)| idx)
            .collect();
        if self.filtered.is_empty() {
            self.selected = 0;
        } else {
            let last = self.filtered.len() - 1;
            self.selected = self.selected.min(last);
        }
    }

    pub fn current_option(&self) -> Option<&ProjectOption> {
        let idx = *self.filtered.get(self.selected)?;
        self.options.get(idx)
    }
}

#[derive(Debug, Clone)]
pub struct CurrencyModal {
    pub options: Vec<String>,
    pub filtered: Vec<usize>,
    pub query: String,
    pub selected: usize,
}

impl CurrencyModal {
    pub fn refilter(&mut self) {
        let needle = self.query.to_lowercase();
        self.filtered = self
            .options
            .iter()
            .enumerate()
            .filter(|(_, code)| needle.is_empty() || code.to_lowercase().contains(&needle))
            .map(|(idx, _)| idx)
            .collect();
        if self.filtered.is_empty() {
            self.selected = 0;
        } else {
            let last = self.filtered.len() - 1;
            self.selected = self.selected.min(last);
        }
    }

    pub fn current_code(&self) -> Option<&str> {
        let idx = *self.filtered.get(self.selected)?;
        self.options.get(idx).map(|s| s.as_str())
    }
}

#[derive(Debug, Clone)]
pub struct ExportModal {
    pub options: Vec<ExportFormat>,
    pub selected: usize,
}

impl Default for ExportModal {
    fn default() -> Self {
        Self::new()
    }
}

impl ExportModal {
    pub fn new() -> Self {
        Self {
            options: ExportFormat::ALL.to_vec(),
            selected: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SessionModal {
    pub options: Vec<SessionOption>,
    pub filtered: Vec<usize>,
    pub query: String,
    pub selected: usize,
}

impl SessionModal {
    pub fn refilter(&mut self) {
        let needle = self.query.to_lowercase();
        self.filtered = self
            .options
            .iter()
            .enumerate()
            .filter(|(_, option)| {
                if needle.is_empty() {
                    return true;
                }
                option.project.to_lowercase().contains(&needle)
                    || option.date.to_lowercase().contains(&needle)
                    || option.key.to_lowercase().contains(&needle)
                    || option.tool.to_lowercase().contains(&needle)
            })
            .map(|(idx, _)| idx)
            .collect();
        if self.filtered.is_empty() {
            self.selected = 0;
        } else {
            let last = self.filtered.len() - 1;
            self.selected = self.selected.min(last);
        }
    }

    pub fn current_option(&self) -> Option<&SessionOption> {
        let idx = *self.filtered.get(self.selected)?;
        self.options.get(idx)
    }
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

/// Channel pair used to talk to the long-lived refresher thread. The thread
/// sleeps for `archive::SYNC_INTERVAL`, then syncs the local archive and sends
/// the resulting in-memory view. `signal_tx` lets the UI request an out-of-cycle
/// refresh (e.g. the 'r' key); `result_rx` receives finished snapshots tagged
/// with whether they came from a manual or timer-driven trigger.
pub struct Refresher {
    signal_tx: Sender<()>,
    result_rx: Receiver<RefreshOutcome>,
}

#[derive(Clone)]
pub enum RefreshSource {
    Archive(ConfigPaths),
    RawIngest,
}

#[derive(Clone, Copy)]
enum RefreshKind {
    Manual,
    Auto,
}

struct RefreshOutcome {
    kind: RefreshKind,
    result: Result<Ingested>,
}

impl Refresher {
    /// Spawn the refresher thread. `initial_delay` sets how long to wait
    /// before the first auto refresh. Existing archives pass zero so startup
    /// can render immediately and sync in the background; cold starts pass the
    /// regular interval because they just ran a synchronous sync.
    pub fn spawn(initial_delay: Duration, source: RefreshSource) -> Self {
        let (signal_tx, signal_rx) = mpsc::channel::<()>();
        let (result_tx, result_rx) = mpsc::channel::<RefreshOutcome>();

        std::thread::spawn(move || {
            let mut next_delay = initial_delay;
            loop {
                let kind = match signal_rx.recv_timeout(next_delay) {
                    Ok(()) => RefreshKind::Manual,
                    Err(mpsc::RecvTimeoutError::Timeout) => RefreshKind::Auto,
                    Err(mpsc::RecvTimeoutError::Disconnected) => return,
                };
                let result = match &source {
                    RefreshSource::Archive(paths) => crate::archive::sync_and_load(paths),
                    RefreshSource::RawIngest => crate::ingest::load(),
                };
                if result_tx.send(RefreshOutcome { kind, result }).is_err() {
                    return;
                }
                next_delay = archive::SYNC_INTERVAL;
            }
        });

        Self {
            signal_tx,
            result_rx,
        }
    }
}

pub struct App {
    pub page: Page,
    pub period: Period,
    pub tool: Tool,
    pub project_filter: ProjectFilter,
    pub project_modal: Option<ProjectModal>,
    pub currency_modal: Option<CurrencyModal>,
    pub session_modal: Option<SessionModal>,
    pub export_modal: Option<ExportModal>,
    pub session_view: Option<SessionDetailView>,
    pub session_scroll: usize,
    pub help_open: bool,
    pub refresher: Option<Refresher>,
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
            page: Page::Overview,
            period: Period::Week,
            tool: Tool::All,
            project_filter: ProjectFilter::All,
            project_modal: None,
            currency_modal: None,
            session_modal: None,
            export_modal: None,
            session_view: None,
            session_scroll: 0,
            help_open: false,
            refresher: None,
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
        initial_refresh_delay: Duration,
        refresh_source: RefreshSource,
    ) -> Self {
        let refresher = Some(Refresher::spawn(initial_refresh_delay, refresh_source));
        Self {
            source,
            status,
            settings,
            paths,
            currency_table,
            refresher,
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

    /// Ask the background refresher to run ingest now (out-of-cycle). The
    /// thread does one ingest at a time, so a queued signal just runs after
    /// the in-flight one finishes - no dedup needed here.
    pub fn reload(&mut self) {
        let Some(refresher) = self.refresher.as_ref() else {
            return;
        };
        if refresher.signal_tx.send(()).is_ok() {
            self.status = Some("reloading…".into());
        }
    }

    /// Drain any results the refresher has produced and apply the most recent
    /// successful one. Called every tick from the main loop.
    pub fn poll_reload(&mut self) {
        let Some(refresher) = self.refresher.as_ref() else {
            return;
        };
        let mut latest: Option<RefreshOutcome> = None;
        loop {
            match refresher.result_rx.try_recv() {
                Ok(outcome) => latest = Some(outcome),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.refresher = None;
                    self.status = Some("refresher stopped · prior data kept".into());
                    return;
                }
            }
        }
        let Some(outcome) = latest else {
            return;
        };

        let manual = matches!(outcome.kind, RefreshKind::Manual);
        match outcome.result {
            Ok(ingested) if !ingested.is_empty() => {
                let n = ingested.calls.len();
                self.source = DataSource::Live(ingested);
                self.status = Some(if manual {
                    format!("reloaded · {n} calls")
                } else {
                    format!("auto-refreshed · {n} calls")
                });
            }
            Ok(_) => {
                self.status = Some(if manual {
                    "reload · no sessions found · prior data kept".into()
                } else {
                    "auto-refresh · no sessions found · prior data kept".into()
                });
            }
            Err(e) => {
                self.status = Some(if manual {
                    format!("reload failed · prior data kept ({e})")
                } else {
                    format!("auto-refresh failed · prior data kept ({e})")
                });
            }
        }

        if let Some(view) = self.session_view.as_ref() {
            let key = view.key.clone();
            self.session_view = self.lookup_session_view(&key);
        }
    }

    fn session_options(&self) -> Vec<SessionOption> {
        let currency = self.currency();
        match &self.source {
            DataSource::Live(ingested) => {
                ingested.session_options(self.period, self.tool, &self.project_filter, &currency)
            }
            DataSource::Sample => crate::data::session_options(self.period, self.tool, &currency),
        }
    }

    fn lookup_session_view(&self, key: &str) -> Option<SessionDetailView> {
        let currency = self.currency();
        match &self.source {
            DataSource::Live(ingested) => ingested.session_detail(key, &currency),
            DataSource::Sample => crate::data::session_detail(key, &currency),
        }
    }

    fn open_session_modal(&mut self) {
        let options = self.session_options();
        if options.is_empty() {
            self.status = Some("no sessions to drill into".into());
            return;
        }
        let filtered: Vec<usize> = (0..options.len()).collect();
        self.session_modal = Some(SessionModal {
            options,
            filtered,
            query: String::new(),
            selected: 0,
        });
    }

    fn handle_session_modal_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.session_modal = None,
            KeyCode::Up => {
                if let Some(modal) = self.session_modal.as_mut() {
                    modal.selected = modal.selected.saturating_sub(1);
                }
            }
            KeyCode::Down => {
                if let Some(modal) = self.session_modal.as_mut() {
                    let last = modal.filtered.len().saturating_sub(1);
                    modal.selected = (modal.selected + 1).min(last);
                }
            }
            KeyCode::Home => {
                if let Some(modal) = self.session_modal.as_mut() {
                    modal.selected = 0;
                }
            }
            KeyCode::End => {
                if let Some(modal) = self.session_modal.as_mut() {
                    modal.selected = modal.filtered.len().saturating_sub(1);
                }
            }
            KeyCode::Backspace => {
                if let Some(modal) = self.session_modal.as_mut() {
                    modal.query.pop();
                    modal.refilter();
                }
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if let Some(modal) = self.session_modal.as_mut() {
                    modal.query.clear();
                    modal.refilter();
                }
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                if let Some(modal) = self.session_modal.as_mut() {
                    modal.query.push(c);
                    modal.refilter();
                }
            }
            KeyCode::Enter => {
                let key = self
                    .session_modal
                    .as_ref()
                    .and_then(|modal| modal.current_option())
                    .map(|option| option.key.clone());
                self.session_modal = None;
                if let Some(key) = key {
                    self.enter_session(&key);
                }
            }
            _ => {}
        }
    }

    fn enter_session(&mut self, key: &str) {
        match self.lookup_session_view(key) {
            Some(view) => {
                self.session_view = Some(view);
                self.session_scroll = 0;
                self.page = Page::Session;
            }
            None => {
                self.status = Some(format!("session not found · {key}"));
            }
        }
    }

    fn handle_session_page_key(&mut self, key: KeyEvent) {
        let row_count = self
            .session_view
            .as_ref()
            .map(|view| view.calls.len())
            .unwrap_or(0);
        match key.code {
            KeyCode::Esc | KeyCode::Char('d') => {
                self.page = Page::DeepDive;
                self.session_view = None;
                self.session_scroll = 0;
            }
            KeyCode::Up => {
                self.session_scroll = self.session_scroll.saturating_sub(1);
            }
            KeyCode::Down => {
                let last = row_count.saturating_sub(1);
                self.session_scroll = (self.session_scroll + 1).min(last);
            }
            KeyCode::PageUp => {
                self.session_scroll = self.session_scroll.saturating_sub(10);
            }
            KeyCode::PageDown => {
                let last = row_count.saturating_sub(1);
                self.session_scroll = (self.session_scroll + 10).min(last);
            }
            KeyCode::Home => self.session_scroll = 0,
            KeyCode::End => self.session_scroll = row_count.saturating_sub(1),
            KeyCode::Char('r') => self.reload(),
            KeyCode::Char('s') => self.open_session_modal(),
            _ => {}
        }
    }

    fn open_export_modal(&mut self) {
        self.export_modal = Some(ExportModal::new());
    }

    fn handle_export_modal_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.export_modal = None,
            KeyCode::Up => {
                if let Some(modal) = self.export_modal.as_mut() {
                    modal.selected = modal.selected.saturating_sub(1);
                }
            }
            KeyCode::Down => {
                if let Some(modal) = self.export_modal.as_mut() {
                    let last = modal.options.len().saturating_sub(1);
                    modal.selected = (modal.selected + 1).min(last);
                }
            }
            KeyCode::Home => {
                if let Some(modal) = self.export_modal.as_mut() {
                    modal.selected = 0;
                }
            }
            KeyCode::End => {
                if let Some(modal) = self.export_modal.as_mut() {
                    modal.selected = modal.options.len().saturating_sub(1);
                }
            }
            KeyCode::Enter => {
                let format = self
                    .export_modal
                    .as_ref()
                    .and_then(|modal| modal.options.get(modal.selected).copied());
                self.export_modal = None;
                if let Some(format) = format {
                    self.run_export(format);
                }
            }
            _ => {}
        }
    }

    fn run_export(&mut self, format: ExportFormat) {
        let data = self.dashboard();
        match crate::export::write(
            &self.paths,
            format,
            &data,
            self.period,
            self.tool,
            &self.project_filter,
        ) {
            Ok(path) => {
                self.status = Some(format!("exported {} · {}", format.label(), path.display()));
            }
            Err(e) => {
                self.status = Some(format!("export failed · {e}"));
            }
        }
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

        if self.help_open {
            match key.code {
                KeyCode::Esc | KeyCode::Char('h') | KeyCode::Char('?') => self.help_open = false,
                KeyCode::Char('q') => self.should_quit = true,
                _ => {}
            }
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

        if self.session_modal.is_some() {
            self.handle_session_modal_key(key);
            return;
        }

        if self.export_modal.is_some() {
            self.handle_export_modal_key(key);
            return;
        }

        if key.code == KeyCode::Char('q') {
            self.should_quit = true;
            return;
        }

        if matches!(key.code, KeyCode::Char('h') | KeyCode::Char('?')) {
            self.help_open = true;
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

        if self.page == Page::Session {
            self.handle_session_page_key(key);
            return;
        }

        match key.code {
            KeyCode::Esc => self.should_quit = true,
            KeyCode::Tab => self.page = self.page.next_tab(),
            KeyCode::BackTab => self.page = self.page.prev_tab(),
            KeyCode::Char('1') => self.period = Period::Today,
            KeyCode::Char('2') => self.period = Period::Week,
            KeyCode::Char('3') => self.period = Period::ThirtyDays,
            KeyCode::Char('4') => self.period = Period::Month,
            KeyCode::Char('5') => self.period = Period::AllTime,
            KeyCode::Char('t') => self.tool = self.tool.next(),
            KeyCode::Char('p') => self.open_project_modal(),
            KeyCode::Char('c') => self.page = Page::Config,
            KeyCode::Char('o') => self.page = Page::Overview,
            KeyCode::Char('d') => self.page = Page::DeepDive,
            KeyCode::Char('u') => self.page = Page::Usage,
            KeyCode::Char('r') => self.reload(),
            KeyCode::Char('s') => self.open_session_modal(),
            KeyCode::Char('e') => self.open_export_modal(),
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

        let initial = self
            .project_filter
            .identity()
            .and_then(|identity| {
                options
                    .iter()
                    .position(|option| option.identity.as_deref() == Some(identity))
            })
            .unwrap_or(0);

        let filtered: Vec<usize> = (0..options.len()).collect();
        let selected = filtered.iter().position(|&i| i == initial).unwrap_or(0);

        self.project_modal = Some(ProjectModal {
            options,
            filtered,
            query: String::new(),
            selected,
        });
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
                    let last = modal.filtered.len().saturating_sub(1);
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
                    modal.selected = modal.filtered.len().saturating_sub(1);
                }
            }
            KeyCode::Backspace => {
                if let Some(modal) = self.project_modal.as_mut() {
                    modal.query.pop();
                    modal.refilter();
                }
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                if let Some(modal) = self.project_modal.as_mut() {
                    modal.query.push(c);
                    modal.refilter();
                }
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if let Some(modal) = self.project_modal.as_mut() {
                    modal.query.clear();
                    modal.refilter();
                }
            }
            KeyCode::Enter => {
                let selected = self
                    .project_modal
                    .as_ref()
                    .and_then(|modal| modal.current_option())
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
                self.page = Page::DeepDive;
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
            KeyCode::Esc | KeyCode::Char('d') => self.page = Page::DeepDive,
            KeyCode::Char('o') => self.page = Page::Overview,
            KeyCode::Char('u') => self.page = Page::Overview,
            KeyCode::Tab => self.page = self.page.next_tab(),
            KeyCode::BackTab => self.page = self.page.prev_tab(),
            KeyCode::Char('1') => self.period = Period::Today,
            KeyCode::Char('2') => self.period = Period::Week,
            KeyCode::Char('3') => self.period = Period::ThirtyDays,
            KeyCode::Char('4') => self.period = Period::Month,
            KeyCode::Char('5') => self.period = Period::AllTime,
            KeyCode::Char('t') => self.tool = self.tool.next(),
            KeyCode::Char('c') => self.page = Page::Config,
            KeyCode::Char('r') => self.reload(),
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
        let initial = options
            .iter()
            .position(|code| code == self.currency().code())
            .unwrap_or(0);
        let filtered: Vec<usize> = (0..options.len()).collect();
        let selected = filtered.iter().position(|&i| i == initial).unwrap_or(0);
        self.currency_modal = Some(CurrencyModal {
            options,
            filtered,
            query: String::new(),
            selected,
        });
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
                    let last = modal.filtered.len().saturating_sub(1);
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
                    modal.selected = modal.filtered.len().saturating_sub(1);
                }
            }
            KeyCode::Backspace => {
                if let Some(modal) = self.currency_modal.as_mut() {
                    modal.query.pop();
                    modal.refilter();
                }
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                if let Some(modal) = self.currency_modal.as_mut() {
                    modal.query.push(c);
                    modal.refilter();
                }
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if let Some(modal) = self.currency_modal.as_mut() {
                    modal.query.clear();
                    modal.refilter();
                }
            }
            KeyCode::Enter => {
                let selected = self
                    .currency_modal
                    .as_ref()
                    .and_then(|modal| modal.current_code())
                    .map(|s| s.to_string());
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
    fn u_opens_usage_and_u_or_escape_returns_to_a_main_tab() {
        let mut app = App::default();
        assert_eq!(app.page, Page::Overview);

        app.handle_key(key(KeyCode::Char('u')));
        assert_eq!(app.page, Page::Usage);

        // From Usage, `u` returns to Overview (the home tab).
        app.handle_key(key(KeyCode::Char('u')));
        assert_eq!(app.page, Page::Overview);

        // From Usage, Esc routes to Deep Dive (preserves the historical
        // "esc returns to dashboard" behaviour).
        app.handle_key(key(KeyCode::Char('u')));
        app.handle_key(key(KeyCode::Esc));
        assert_eq!(app.page, Page::DeepDive);
        assert!(!app.should_quit());
    }

    #[test]
    fn tab_cycles_between_main_tabs() {
        let mut app = App::default();
        assert_eq!(app.page, Page::Overview);

        app.handle_key(key(KeyCode::Tab));
        assert_eq!(app.page, Page::DeepDive);
        app.handle_key(key(KeyCode::Tab));
        assert_eq!(app.page, Page::Usage);
        app.handle_key(key(KeyCode::Tab));
        assert_eq!(app.page, Page::Overview);

        app.handle_key(key(KeyCode::BackTab));
        assert_eq!(app.page, Page::Usage);
    }

    #[test]
    fn direct_keys_jump_between_tabs() {
        let mut app = App::default();

        app.handle_key(key(KeyCode::Char('d')));
        assert_eq!(app.page, Page::DeepDive);

        app.handle_key(key(KeyCode::Char('u')));
        assert_eq!(app.page, Page::Usage);

        app.handle_key(key(KeyCode::Char('o')));
        assert_eq!(app.page, Page::Overview);
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
        let mut app = App {
            period: Period::Today,
            project_filter: ProjectFilter::Selected {
                identity: "missing".into(),
                label: "missing".into(),
            },
            ..App::default()
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

    #[test]
    fn reload_without_refresher_is_a_noop() {
        // App::default() doesn't spawn a refresher (only with_runtime does),
        // so reload + poll_reload should leave state untouched.
        let mut app = App {
            status: Some("untouched".into()),
            ..App::default()
        };
        app.reload();
        assert_eq!(app.status.as_deref(), Some("untouched"));
        app.poll_reload();
        assert_eq!(app.status.as_deref(), Some("untouched"));
    }

    #[test]
    fn poll_reload_drains_a_pre_completed_channel() {
        // Synthesise "background refresher already produced a result" without
        // spawning a thread or walking the filesystem, so the test is fast
        // and deterministic. We hand-build a Refresher whose result_rx is
        // already loaded.
        let mut app = App::default();
        let (signal_tx, _signal_rx) = mpsc::channel::<()>();
        let (result_tx, result_rx) = mpsc::channel::<RefreshOutcome>();
        let ingested = crate::ingest::Ingested {
            calls: Vec::new(),
            limits: Vec::new(),
        };
        result_tx
            .send(RefreshOutcome {
                kind: RefreshKind::Manual,
                result: Ok(ingested),
            })
            .unwrap();
        app.refresher = Some(Refresher {
            signal_tx,
            result_rx,
        });

        app.poll_reload();

        assert_eq!(
            app.status.as_deref(),
            Some("reload · no sessions found · prior data kept")
        );
    }

    #[test]
    fn poll_reload_keeps_only_the_latest_result() {
        // When several refreshes have completed between polls, only the last
        // one's status message and data should win.
        let mut app = App::default();
        let (signal_tx, _signal_rx) = mpsc::channel::<()>();
        let (result_tx, result_rx) = mpsc::channel::<RefreshOutcome>();
        for _ in 0..3 {
            result_tx
                .send(RefreshOutcome {
                    kind: RefreshKind::Auto,
                    result: Ok(crate::ingest::Ingested {
                        calls: Vec::new(),
                        limits: Vec::new(),
                    }),
                })
                .unwrap();
        }
        app.refresher = Some(Refresher {
            signal_tx,
            result_rx,
        });

        app.poll_reload();

        assert_eq!(
            app.status.as_deref(),
            Some("auto-refresh · no sessions found · prior data kept")
        );
    }
}
