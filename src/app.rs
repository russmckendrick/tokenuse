use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::time::Duration;

use chrono::{DateTime, Utc};
use color_eyre::Result;
use crossterm::event::{
    KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::layout::{Constraint, Direction, Layout, Margin, Rect};

use crate::archive;
use crate::config::{ConfigPaths, UserConfig};
use crate::currency::{CurrencyFormatter, CurrencyTable};
use crate::data::{
    DashboardData, LimitsData, ProjectOption, SessionDetail, SessionDetailView, SessionOption,
};
use crate::export::{ExportContext, ExportFormat};
use crate::ingest::Ingested;
use crate::keymap;

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
            Self::Today => "24 Hours",
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
pub enum SortMode {
    Spend,
    Date,
    Tokens,
}

impl SortMode {
    pub const ALL: [Self; 3] = [Self::Spend, Self::Date, Self::Tokens];

    pub fn label(self) -> &'static str {
        match self {
            Self::Spend => "Spend",
            Self::Date => "Date",
            Self::Tokens => "Tokens",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Spend => Self::Date,
            Self::Date => Self::Tokens,
            Self::Tokens => Self::Spend,
        }
    }
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

fn session_calls_area(terminal_area: Rect) -> Rect {
    let area = terminal_area.inner(Margin {
        horizontal: 1,
        vertical: 1,
    });
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(4),
            Constraint::Length(1),
            Constraint::Min(8),
            Constraint::Length(3),
        ])
        .split(area);
    sections[3]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigDownload {
    CurrencyRates,
    PricingSnapshot,
}

impl ConfigDownload {
    pub fn title(self) -> &'static str {
        match self {
            Self::CurrencyRates => "Download rates.json?",
            Self::PricingSnapshot => "Download LiteLLM prices?",
        }
    }

    pub fn file_name(self) -> &'static str {
        match self {
            Self::CurrencyRates => "rates.json",
            Self::PricingSnapshot => "pricing-snapshot.json",
        }
    }

    pub fn source(self) -> &'static str {
        match self {
            Self::CurrencyRates => "published tokenuse currency snapshot",
            Self::PricingSnapshot => "LiteLLM model price table",
        }
    }

    pub fn effect(self) -> &'static str {
        match self {
            Self::CurrencyRates => "display rates update immediately",
            Self::PricingSnapshot => "new prices apply to newly imported calls",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FolderPickerEntryKind {
    UseCurrent,
    Parent,
    Directory,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderPickerEntry {
    pub kind: FolderPickerEntryKind,
    pub label: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct FolderPickerModal {
    pub current_dir: PathBuf,
    pub entries: Vec<FolderPickerEntry>,
    pub selected: usize,
    pub error: Option<String>,
}

impl FolderPickerModal {
    pub fn new(current_dir: PathBuf) -> Self {
        let mut modal = Self {
            current_dir,
            entries: Vec::new(),
            selected: 0,
            error: None,
        };
        modal.refresh();
        modal
    }

    pub fn refresh(&mut self) {
        let (entries, error) = folder_picker_entries(&self.current_dir);
        self.entries = entries;
        self.error = error;
        self.selected = self.selected.min(self.entries.len().saturating_sub(1));
    }

    pub fn current_entry(&self) -> Option<&FolderPickerEntry> {
        self.entries.get(self.selected)
    }

    pub fn move_by(&mut self, delta: isize) {
        let last = self.entries.len().saturating_sub(1);
        if delta.is_negative() {
            self.selected = self.selected.saturating_sub(delta.unsigned_abs());
        } else {
            self.selected = (self.selected + delta as usize).min(last);
        }
    }

    pub fn go_parent(&mut self) {
        let Some(parent) = self.current_dir.parent().map(Path::to_path_buf) else {
            return;
        };
        self.current_dir = parent;
        self.selected = 0;
        self.refresh();
    }

    pub fn activate(&mut self) -> Option<PathBuf> {
        let entry = self.current_entry()?.clone();
        match entry.kind {
            FolderPickerEntryKind::UseCurrent => Some(entry.path),
            FolderPickerEntryKind::Parent | FolderPickerEntryKind::Directory => {
                self.current_dir = entry.path;
                self.selected = 0;
                self.refresh();
                None
            }
        }
    }
}

fn folder_picker_entries(dir: &Path) -> (Vec<FolderPickerEntry>, Option<String>) {
    let mut entries = vec![FolderPickerEntry {
        kind: FolderPickerEntryKind::UseCurrent,
        label: "Use this folder".into(),
        path: dir.to_path_buf(),
    }];

    if let Some(parent) = dir.parent() {
        entries.push(FolderPickerEntry {
            kind: FolderPickerEntryKind::Parent,
            label: "..".into(),
            path: parent.to_path_buf(),
        });
    }

    let read = fs::read_dir(dir);
    let mut subdirs = Vec::new();
    let error = match read {
        Ok(read_dir) => {
            for entry in read_dir.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if is_hidden_dir_name(&name) {
                    continue;
                }
                let Ok(file_type) = entry.file_type() else {
                    continue;
                };
                if file_type.is_dir() {
                    subdirs.push(FolderPickerEntry {
                        kind: FolderPickerEntryKind::Directory,
                        label: name,
                        path: entry.path(),
                    });
                }
            }
            None
        }
        Err(e) => Some(format!("could not read folder · {e}")),
    };

    subdirs.sort_by(|a, b| {
        a.label
            .to_ascii_lowercase()
            .cmp(&b.label.to_ascii_lowercase())
            .then_with(|| a.label.cmp(&b.label))
    });
    entries.extend(subdirs);
    (entries, error)
}

fn is_hidden_dir_name(name: &str) -> bool {
    name.starts_with('.') && name != ".."
}

fn move_index(selected: &mut usize, len: usize, delta: isize) {
    if len == 0 {
        *selected = 0;
        return;
    }

    if delta.is_negative() {
        *selected = selected.saturating_sub(delta.unsigned_abs());
    } else {
        *selected = (*selected + delta as usize).min(len - 1);
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

#[derive(Debug, Clone, serde::Serialize)]
pub struct ConfigRowView {
    pub name: &'static str,
    pub value: String,
    pub action: &'static str,
}

pub enum DataSource {
    Live(Ingested),
    Sample,
}

fn cached_live_source(source: &DataSource) -> Option<Ingested> {
    match source {
        DataSource::Live(ingested) => Some(ingested.clone()),
        DataSource::Sample => None,
    }
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

#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct UsageTotals {
    calls: u64,
    tokens: u64,
    cost_usd: f64,
}

impl UsageTotals {
    fn from_ingested(ingested: &Ingested) -> Self {
        let mut totals = Self::default();
        for call in &ingested.calls {
            totals.calls += 1;
            totals.tokens = totals.tokens.saturating_add(call.input_tokens);
            totals.tokens = totals.tokens.saturating_add(call.output_tokens);
            totals.tokens = totals
                .tokens
                .saturating_add(call.cache_creation_input_tokens);
            totals.tokens = totals.tokens.saturating_add(call.cache_read_input_tokens);
            totals.cost_usd += call.cost_usd;
        }
        totals
    }

    fn delta_since(self, baseline: Self) -> Self {
        Self {
            calls: self.calls.saturating_sub(baseline.calls),
            tokens: self.tokens.saturating_sub(baseline.tokens),
            cost_usd: (self.cost_usd - baseline.cost_usd).max(0.0),
        }
    }

    fn is_less_than(self, other: Self) -> bool {
        self.calls < other.calls || self.tokens < other.tokens || self.cost_usd < other.cost_usd
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BackgroundUsageAlert {
    pub calls: u64,
    pub tokens: u64,
    pub cost_usd: f64,
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
    pub sort: SortMode,
    pub project_filter: ProjectFilter,
    pub project_modal: Option<ProjectModal>,
    pub currency_modal: Option<CurrencyModal>,
    pub download_confirm: Option<ConfigDownload>,
    pub session_modal: Option<SessionModal>,
    pub export_modal: Option<ExportModal>,
    pub export_dir_picker: Option<FolderPickerModal>,
    pub export_dir: PathBuf,
    pub session_view: Option<SessionDetailView>,
    pub session_scroll: usize,
    pub session_selected: usize,
    pub call_detail_index: Option<usize>,
    pub help_open: bool,
    pub refresher: Option<Refresher>,
    pub config_selected: usize,
    pub settings: UserConfig,
    pub paths: ConfigPaths,
    pub currency_table: CurrencyTable,
    pub source: DataSource,
    background_alert_baseline: Option<UsageTotals>,
    background_alert_last_sent: Option<DateTime<Utc>>,
    background_alerts: Vec<BackgroundUsageAlert>,
    live_source: Option<Ingested>,
    sample_forced: bool,
    pub status: Option<String>,
    should_quit: bool,
}

impl Default for App {
    fn default() -> Self {
        let paths = ConfigPaths::default();
        let export_dir = crate::export::default_export_dir(&paths);
        Self {
            page: Page::Overview,
            period: Period::Week,
            tool: Tool::All,
            sort: SortMode::Spend,
            project_filter: ProjectFilter::All,
            project_modal: None,
            currency_modal: None,
            download_confirm: None,
            session_modal: None,
            export_modal: None,
            export_dir_picker: None,
            export_dir,
            session_view: None,
            session_scroll: 0,
            session_selected: 0,
            call_detail_index: None,
            help_open: false,
            refresher: None,
            config_selected: 0,
            settings: UserConfig::default(),
            paths,
            currency_table: CurrencyTable::embedded()
                .expect("embedded currency rates must be valid JSON"),
            source: DataSource::Sample,
            background_alert_baseline: None,
            background_alert_last_sent: None,
            background_alerts: Vec::new(),
            live_source: None,
            sample_forced: false,
            status: None,
            should_quit: false,
        }
    }
}

impl App {
    pub fn with_source(source: DataSource, status: Option<String>) -> Self {
        let live_source = cached_live_source(&source);
        let background_alert_baseline = live_source.as_ref().map(UsageTotals::from_ingested);
        Self {
            source,
            live_source,
            background_alert_baseline,
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
        let export_dir = crate::export::default_export_dir(&paths);
        let live_source = cached_live_source(&source);
        let background_alert_baseline = live_source.as_ref().map(UsageTotals::from_ingested);
        Self {
            source,
            live_source,
            background_alert_baseline,
            status,
            settings,
            export_dir,
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
            DataSource::Live(ingested) => ingested.dashboard(
                self.period,
                self.tool,
                &self.project_filter,
                self.sort,
                &currency,
            ),
            DataSource::Sample => crate::data::dashboard_data(
                self.period,
                self.tool,
                &self.project_filter,
                self.sort,
                &currency,
            ),
        }
    }

    pub fn usage(&self) -> LimitsData {
        let currency = self.currency();
        match &self.source {
            DataSource::Live(ingested) => ingested.limits(self.tool, self.sort, &currency),
            DataSource::Sample => crate::data::limits_data(self.tool, self.sort, &currency),
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

    pub fn take_background_alerts(&mut self) -> Vec<BackgroundUsageAlert> {
        std::mem::take(&mut self.background_alerts)
    }

    pub fn toggle_data_source(&mut self) {
        match std::mem::replace(&mut self.source, DataSource::Sample) {
            DataSource::Live(ingested) => {
                self.live_source = Some(ingested);
                self.sample_forced = true;
                self.status = Some("sample data".into());
            }
            DataSource::Sample => {
                self.sample_forced = false;
                if let Some(ingested) = self.live_source.take() {
                    self.source = DataSource::Live(ingested);
                    self.status = Some("live data".into());
                } else {
                    self.source = DataSource::Sample;
                    self.status = Some("no local sessions found · sample data".into());
                }
            }
        }
        self.refresh_session_view();
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
                self.update_background_alerts(outcome.kind, &ingested);
                if self.sample_forced {
                    self.live_source = Some(ingested);
                } else {
                    self.source = DataSource::Live(ingested);
                    self.live_source = None;
                }
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

        self.refresh_session_view();
    }

    fn update_background_alerts(&mut self, kind: RefreshKind, ingested: &Ingested) {
        let totals = UsageTotals::from_ingested(ingested);
        if matches!(kind, RefreshKind::Manual) || !self.settings.background_alerts.enabled {
            self.background_alert_baseline = Some(totals);
            return;
        }

        let Some(baseline) = self.background_alert_baseline else {
            self.background_alert_baseline = Some(totals);
            return;
        };

        if totals.is_less_than(baseline) {
            self.background_alert_baseline = Some(totals);
            return;
        }

        let delta = totals.delta_since(baseline);
        let config = &self.settings.background_alerts;
        let crossed_threshold = delta.cost_usd >= config.min_cost_usd
            || delta.tokens >= config.min_tokens()
            || delta.calls >= config.min_calls();
        if !crossed_threshold {
            return;
        }

        let now = Utc::now();
        if let Some(last_sent) = self.background_alert_last_sent {
            let elapsed = (now - last_sent)
                .to_std()
                .unwrap_or_else(|_| Duration::from_secs(0));
            if elapsed < config.cooldown() {
                return;
            }
        }

        self.background_alerts.push(BackgroundUsageAlert {
            calls: delta.calls,
            tokens: delta.tokens,
            cost_usd: delta.cost_usd,
        });
        self.background_alert_last_sent = Some(now);
        self.background_alert_baseline = Some(totals);
    }

    fn refresh_session_view(&mut self) {
        if let Some(view) = self.session_view.as_ref() {
            let key = view.key.clone();
            self.session_view = self.lookup_session_view(&key);
            self.clamp_session_call_state();
        }
    }

    pub fn session_options(&self) -> Vec<SessionOption> {
        let currency = self.currency();
        match &self.source {
            DataSource::Live(ingested) => ingested.session_options(
                self.period,
                self.tool,
                &self.project_filter,
                self.sort,
                &currency,
            ),
            DataSource::Sample => {
                crate::data::session_options(self.period, self.tool, self.sort, &currency)
            }
        }
    }

    pub fn lookup_session_view(&self, key: &str) -> Option<SessionDetailView> {
        let currency = self.currency();
        match &self.source {
            DataSource::Live(ingested) => ingested.session_detail(key, self.sort, &currency),
            DataSource::Sample => crate::data::session_detail(key, self.sort, &currency),
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

    pub fn enter_session(&mut self, key: &str) {
        match self.lookup_session_view(key) {
            Some(view) => {
                self.session_view = Some(view);
                self.session_scroll = 0;
                self.session_selected = 0;
                self.call_detail_index = None;
                self.page = Page::Session;
            }
            None => {
                self.status = Some(format!("session not found · {key}"));
            }
        }
    }

    pub fn leave_session(&mut self) {
        self.page = Page::DeepDive;
        self.session_view = None;
        self.session_scroll = 0;
        self.session_selected = 0;
        self.call_detail_index = None;
    }

    pub fn selected_call_detail(&self) -> Option<&SessionDetail> {
        let view = self.session_view.as_ref()?;
        let idx = self.call_detail_index?;
        view.calls.get(idx)
    }

    pub fn open_session_call_detail(&mut self, idx: usize) {
        if self
            .session_view
            .as_ref()
            .is_some_and(|view| idx < view.calls.len())
        {
            self.session_selected = idx;
            self.call_detail_index = Some(idx);
        }
    }

    fn close_call_detail(&mut self) {
        self.call_detail_index = None;
    }

    fn clamp_session_call_state(&mut self) {
        let row_count = self
            .session_view
            .as_ref()
            .map(|view| view.calls.len())
            .unwrap_or(0);
        if row_count == 0 {
            self.session_scroll = 0;
            self.session_selected = 0;
            self.call_detail_index = None;
            return;
        }

        let last = row_count - 1;
        self.session_scroll = self.session_scroll.min(last);
        self.session_selected = self.session_selected.min(last);
        if self.call_detail_index.is_some_and(|idx| idx > last) {
            self.call_detail_index = None;
        }
    }

    fn select_session_call(&mut self, idx: usize) {
        let row_count = self
            .session_view
            .as_ref()
            .map(|view| view.calls.len())
            .unwrap_or(0);
        if row_count == 0 {
            self.session_scroll = 0;
            self.session_selected = 0;
            return;
        }

        let idx = idx.min(row_count - 1);
        self.session_selected = idx;
        self.session_scroll = idx;
    }

    fn open_export_modal(&mut self) {
        self.export_dir_picker = None;
        self.export_modal = Some(ExportModal::new());
    }

    fn open_export_dir_picker(&mut self) {
        self.export_dir_picker = Some(FolderPickerModal::new(self.export_dir.clone()));
    }

    pub fn export_current(
        &mut self,
        format: ExportFormat,
    ) -> color_eyre::Result<std::path::PathBuf> {
        let path = {
            let dashboard = self.dashboard();
            let currency = self.currency();
            let source_label = match &self.source {
                DataSource::Live(_) => "live",
                DataSource::Sample => "sample",
            };
            let context = ExportContext {
                dashboard: &dashboard,
                session: self.session_view.as_ref(),
                period: self.period,
                tool: self.tool,
                project_filter: &self.project_filter,
                sort: self.sort,
                currency_code: currency.code(),
                source_label,
            };
            crate::export::write_to_dir(&self.export_dir, format, &context)?
        };
        self.status = Some(format!("exported {} · {}", format.label(), path.display()));
        Ok(path)
    }

    fn run_export(&mut self, format: ExportFormat) {
        if let Err(e) = self.export_current(format) {
            self.status = Some(format!("export failed · {e}"));
        }
    }

    pub fn set_export_dir(&mut self, dir: PathBuf) {
        self.export_dir = dir;
        self.status = Some(format!("export folder · {}", self.export_dir.display()));
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
                action: "download",
            },
            ConfigRowView {
                name: "LiteLLM prices",
                value: pricing_value,
                action: "download",
            },
        ]
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        if key.kind != KeyEventKind::Press {
            return;
        }

        let context = self.shortcut_context();
        if let Some(action) = keymap::keymap().resolve_tui(context, key) {
            self.apply_shortcut_action(action);
            return;
        }

        self.handle_text_input_key(key);
    }

    fn shortcut_context(&self) -> &'static str {
        if self.help_open {
            return keymap::CONTEXT_HELP;
        }

        if self.call_detail_index.is_some() {
            return keymap::CONTEXT_CALL_DETAIL;
        }

        if self.currency_modal.is_some() {
            return keymap::CONTEXT_CURRENCY_PICKER;
        }

        if self.download_confirm.is_some() {
            return keymap::CONTEXT_DOWNLOAD_CONFIRM;
        }

        if self.project_modal.is_some() {
            return keymap::CONTEXT_PROJECT_PICKER;
        }

        if self.session_modal.is_some() {
            return keymap::CONTEXT_SESSION_PICKER;
        }

        if self.export_dir_picker.is_some() {
            return keymap::CONTEXT_EXPORT_FOLDER_PICKER;
        }

        if self.export_modal.is_some() {
            return keymap::CONTEXT_EXPORT_PICKER;
        }

        match self.page {
            Page::Config => keymap::CONTEXT_CONFIG_PAGE,
            Page::Usage => keymap::CONTEXT_USAGE_PAGE,
            Page::Session => keymap::CONTEXT_SESSION_PAGE,
            Page::Overview | Page::DeepDive => keymap::CONTEXT_DASHBOARD,
        }
    }

    pub fn apply_shortcut_action(&mut self, action: &str) -> bool {
        match action {
            keymap::ACTION_QUIT => self.should_quit = true,
            keymap::ACTION_OPEN_HELP => self.help_open = true,
            keymap::ACTION_CLOSE_HELP => self.help_open = false,
            keymap::ACTION_CLOSE_CALL_DETAIL => self.close_call_detail(),
            keymap::ACTION_CLOSE_MODAL | keymap::ACTION_CANCEL => self.cancel_active_context(),
            keymap::ACTION_CONFIRM => self.confirm_active_context(),
            keymap::ACTION_NEXT_TAB => self.page = self.page.next_tab(),
            keymap::ACTION_PREV_TAB => self.page = self.page.prev_tab(),
            keymap::ACTION_PERIOD_TODAY => self.period = Period::Today,
            keymap::ACTION_PERIOD_WEEK => self.period = Period::Week,
            keymap::ACTION_PERIOD_THIRTY_DAYS => self.period = Period::ThirtyDays,
            keymap::ACTION_PERIOD_MONTH => self.period = Period::Month,
            keymap::ACTION_PERIOD_ALL_TIME => self.period = Period::AllTime,
            keymap::ACTION_CYCLE_TOOL => self.tool = self.tool.next(),
            keymap::ACTION_CYCLE_SORT => self.cycle_sort(),
            keymap::ACTION_TOGGLE_DATA_SOURCE => self.toggle_data_source(),
            keymap::ACTION_OPEN_PROJECT_PICKER => self.open_project_modal(),
            keymap::ACTION_OPEN_SESSION_PICKER => self.open_session_modal(),
            keymap::ACTION_OPEN_EXPORT_PICKER => self.open_export_modal(),
            keymap::ACTION_OPEN_EXPORT_FOLDER_PICKER => self.open_export_dir_picker(),
            keymap::ACTION_PAGE_OVERVIEW => self.page = Page::Overview,
            keymap::ACTION_PAGE_DEEP_DIVE => self.page = Page::DeepDive,
            keymap::ACTION_PAGE_USAGE => self.page = Page::Usage,
            keymap::ACTION_PAGE_CONFIG => self.page = Page::Config,
            keymap::ACTION_CLOSE_SESSION => self.leave_session(),
            keymap::ACTION_RELOAD => self.reload(),
            keymap::ACTION_MOVE_UP => self.move_active_selection(-1),
            keymap::ACTION_MOVE_DOWN => self.move_active_selection(1),
            keymap::ACTION_MOVE_PAGE_UP => self.move_active_page(-10),
            keymap::ACTION_MOVE_PAGE_DOWN => self.move_active_page(10),
            keymap::ACTION_MOVE_HOME => self.move_active_to_start(),
            keymap::ACTION_MOVE_END => self.move_active_to_end(),
            keymap::ACTION_QUERY_BACKSPACE => self.backspace_active_query(),
            keymap::ACTION_QUERY_CLEAR => self.clear_active_query(),
            keymap::ACTION_GO_PARENT => self.go_active_parent(),
            _ => return false,
        }
        true
    }

    fn handle_text_input_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.push_active_query_char(c)
            }
            _ => false,
        }
    }

    fn cancel_active_context(&mut self) {
        if self.currency_modal.is_some() {
            self.currency_modal = None;
        } else if self.download_confirm.is_some() {
            self.download_confirm = None;
        } else if self.project_modal.is_some() {
            self.project_modal = None;
        } else if self.session_modal.is_some() {
            self.session_modal = None;
        } else if self.export_dir_picker.is_some() {
            self.export_dir_picker = None;
        } else if self.export_modal.is_some() {
            self.export_modal = None;
        }
    }

    fn confirm_active_context(&mut self) {
        if self.download_confirm.is_some() {
            self.confirm_download();
        } else if self.currency_modal.is_some() {
            self.confirm_currency_picker();
        } else if self.project_modal.is_some() {
            self.confirm_project_picker();
        } else if self.session_modal.is_some() {
            self.confirm_session_picker();
        } else if self.export_dir_picker.is_some() {
            self.confirm_export_dir_picker();
        } else if self.export_modal.is_some() {
            self.confirm_export_picker();
        } else if self.page == Page::Config {
            self.activate_config_row();
        } else if self.page == Page::Session {
            self.open_session_call_detail(self.session_selected);
        }
    }

    fn confirm_project_picker(&mut self) {
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

    fn confirm_session_picker(&mut self) {
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

    fn confirm_currency_picker(&mut self) {
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

    fn confirm_download(&mut self) {
        let target = self.download_confirm.take();
        match target {
            Some(ConfigDownload::CurrencyRates) => self.refresh_currency_rates(),
            Some(ConfigDownload::PricingSnapshot) => self.refresh_pricing_snapshot(),
            None => {}
        }
    }

    fn confirm_export_picker(&mut self) {
        let format = self
            .export_modal
            .as_ref()
            .and_then(|modal| modal.options.get(modal.selected).copied());
        self.export_modal = None;
        if let Some(format) = format {
            self.run_export(format);
        }
    }

    fn confirm_export_dir_picker(&mut self) {
        let picked = self
            .export_dir_picker
            .as_mut()
            .and_then(FolderPickerModal::activate);
        if let Some(dir) = picked {
            self.export_dir = dir;
            self.export_dir_picker = None;
            self.status = Some(format!("export folder · {}", self.export_dir.display()));
        }
    }

    fn move_active_selection(&mut self, delta: isize) {
        if let Some(modal) = self.currency_modal.as_mut() {
            move_index(&mut modal.selected, modal.filtered.len(), delta);
        } else if let Some(modal) = self.project_modal.as_mut() {
            move_index(&mut modal.selected, modal.filtered.len(), delta);
        } else if let Some(modal) = self.session_modal.as_mut() {
            move_index(&mut modal.selected, modal.filtered.len(), delta);
        } else if let Some(picker) = self.export_dir_picker.as_mut() {
            picker.move_by(delta);
        } else if let Some(modal) = self.export_modal.as_mut() {
            move_index(&mut modal.selected, modal.options.len(), delta);
        } else if self.page == Page::Config {
            let len = self.config_rows().len();
            move_index(&mut self.config_selected, len, delta);
        } else if self.page == Page::Session {
            let row_count = self
                .session_view
                .as_ref()
                .map(|view| view.calls.len())
                .unwrap_or(0);
            let next = if delta.is_negative() {
                self.session_selected.saturating_sub(delta.unsigned_abs())
            } else {
                let last = row_count.saturating_sub(1);
                (self.session_selected + delta as usize).min(last)
            };
            self.select_session_call(next);
        }
    }

    fn move_active_page(&mut self, delta: isize) {
        if let Some(picker) = self.export_dir_picker.as_mut() {
            picker.move_by(delta);
        } else if self.page == Page::Session {
            let row_count = self
                .session_view
                .as_ref()
                .map(|view| view.calls.len())
                .unwrap_or(0);
            let next = if delta.is_negative() {
                self.session_selected.saturating_sub(delta.unsigned_abs())
            } else {
                let last = row_count.saturating_sub(1);
                (self.session_selected + delta as usize).min(last)
            };
            self.select_session_call(next);
        }
    }

    fn move_active_to_start(&mut self) {
        if let Some(modal) = self.currency_modal.as_mut() {
            modal.selected = 0;
        } else if let Some(modal) = self.project_modal.as_mut() {
            modal.selected = 0;
        } else if let Some(modal) = self.session_modal.as_mut() {
            modal.selected = 0;
        } else if let Some(picker) = self.export_dir_picker.as_mut() {
            picker.selected = 0;
        } else if let Some(modal) = self.export_modal.as_mut() {
            modal.selected = 0;
        } else if self.page == Page::Config {
            self.config_selected = 0;
        } else if self.page == Page::Session {
            self.select_session_call(0);
        }
    }

    fn move_active_to_end(&mut self) {
        if let Some(modal) = self.currency_modal.as_mut() {
            modal.selected = modal.filtered.len().saturating_sub(1);
        } else if let Some(modal) = self.project_modal.as_mut() {
            modal.selected = modal.filtered.len().saturating_sub(1);
        } else if let Some(modal) = self.session_modal.as_mut() {
            modal.selected = modal.filtered.len().saturating_sub(1);
        } else if let Some(picker) = self.export_dir_picker.as_mut() {
            picker.selected = picker.entries.len().saturating_sub(1);
        } else if let Some(modal) = self.export_modal.as_mut() {
            modal.selected = modal.options.len().saturating_sub(1);
        } else if self.page == Page::Config {
            self.config_selected = self.config_rows().len().saturating_sub(1);
        } else if self.page == Page::Session {
            let row_count = self
                .session_view
                .as_ref()
                .map(|view| view.calls.len())
                .unwrap_or(0);
            self.select_session_call(row_count.saturating_sub(1));
        }
    }

    fn backspace_active_query(&mut self) {
        if let Some(modal) = self.currency_modal.as_mut() {
            modal.query.pop();
            modal.refilter();
        } else if let Some(modal) = self.project_modal.as_mut() {
            modal.query.pop();
            modal.refilter();
        } else if let Some(modal) = self.session_modal.as_mut() {
            modal.query.pop();
            modal.refilter();
        }
    }

    fn clear_active_query(&mut self) {
        if let Some(modal) = self.currency_modal.as_mut() {
            modal.query.clear();
            modal.refilter();
        } else if let Some(modal) = self.project_modal.as_mut() {
            modal.query.clear();
            modal.refilter();
        } else if let Some(modal) = self.session_modal.as_mut() {
            modal.query.clear();
            modal.refilter();
        }
    }

    fn push_active_query_char(&mut self, c: char) -> bool {
        if let Some(modal) = self.currency_modal.as_mut() {
            modal.query.push(c);
            modal.refilter();
            true
        } else if let Some(modal) = self.project_modal.as_mut() {
            modal.query.push(c);
            modal.refilter();
            true
        } else if let Some(modal) = self.session_modal.as_mut() {
            modal.query.push(c);
            modal.refilter();
            true
        } else {
            false
        }
    }

    fn go_active_parent(&mut self) {
        if let Some(picker) = self.export_dir_picker.as_mut() {
            picker.go_parent();
        }
    }

    pub fn handle_mouse(&mut self, mouse: MouseEvent, terminal_area: Rect) {
        if !matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
            return;
        }

        if self.call_detail_index.is_some() {
            self.close_call_detail();
            return;
        }

        if self.page != Page::Session {
            return;
        }

        if let Some(idx) = self.session_call_index_at(terminal_area, mouse.column, mouse.row) {
            self.open_session_call_detail(idx);
        }
    }

    pub fn session_call_index_at(
        &self,
        terminal_area: Rect,
        column: u16,
        row: u16,
    ) -> Option<usize> {
        let calls_len = self.session_view.as_ref()?.calls.len();
        if calls_len == 0 || terminal_area.width < 120 || terminal_area.height < 40 {
            return None;
        }

        let table_area = session_calls_area(terminal_area);
        let row_top = table_area.y.saturating_add(2);
        let row_bottom = table_area
            .y
            .saturating_add(table_area.height)
            .saturating_sub(1);

        if column < table_area.x
            || column >= table_area.x.saturating_add(table_area.width)
            || row < row_top
            || row >= row_bottom
        {
            return None;
        }

        let idx = self.session_scroll + usize::from(row - row_top);
        (idx < calls_len).then_some(idx)
    }

    pub fn project_options(&self) -> Vec<ProjectOption> {
        let currency = self.currency();
        match &self.source {
            DataSource::Live(ingested) => {
                ingested.project_options(self.period, self.tool, self.sort, &currency)
            }
            DataSource::Sample => {
                crate::data::project_options(self.period, self.tool, self.sort, &currency)
            }
        }
    }

    pub fn set_period(&mut self, period: Period) {
        self.period = period;
    }

    pub fn set_tool(&mut self, tool: Tool) {
        self.tool = tool;
    }

    pub fn set_sort(&mut self, sort: SortMode) {
        self.sort = sort;
        if let Some(view) = self.session_view.as_ref() {
            let key = view.key.clone();
            self.session_view = self.lookup_session_view(&key);
            self.clamp_session_call_state();
        }
    }

    fn cycle_sort(&mut self) {
        self.set_sort(self.sort.next());
    }

    pub fn set_project_by_identity(&mut self, identity: Option<&str>) {
        match identity {
            None => self.project_filter = ProjectFilter::All,
            Some(identity) => {
                if let Some(option) = self
                    .project_options()
                    .into_iter()
                    .find(|option| option.identity.as_deref() == Some(identity))
                {
                    self.project_filter = ProjectFilter::from_option(&option);
                } else {
                    self.status = Some(format!("project not found · {identity}"));
                }
            }
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

    fn activate_config_row(&mut self) {
        match self.config_selected {
            0 => self.open_currency_modal(),
            1 => self.open_download_confirm(ConfigDownload::CurrencyRates),
            2 => self.open_download_confirm(ConfigDownload::PricingSnapshot),
            _ => {}
        }
    }

    fn open_download_confirm(&mut self, target: ConfigDownload) {
        self.download_confirm = Some(target);
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

    pub fn set_currency(&mut self, code: &str) {
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
    pub fn refresh_currency_rates(&mut self) {
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
    pub fn refresh_currency_rates(&mut self) {
        self.status = Some("rates download unavailable in this build".into());
    }

    #[cfg(feature = "refresh-prices")]
    pub fn refresh_pricing_snapshot(&mut self) {
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
    pub fn refresh_pricing_snapshot(&mut self) {
        self.status = Some("LiteLLM download unavailable in this build".into());
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
    use ratatui::layout::Rect;

    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn shift_key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::SHIFT)
    }

    fn tempdir(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "tokenuse-app-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            name
        ));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    fn call(label: &str) -> SessionDetail {
        SessionDetail {
            timestamp: "04-29 12:00".into(),
            model: "gpt-5".into(),
            cost: "$0.12".into(),
            input_tokens: 100,
            output_tokens: 50,
            cache_read: 20,
            cache_write: 5,
            reasoning_tokens: 3,
            web_search_requests: 1,
            tools: "Bash".into(),
            bash_commands: vec!["cargo test".into()],
            prompt: label.into(),
            prompt_full: format!("{label} full prompt"),
        }
    }

    fn app_with_session_calls(count: usize) -> App {
        App {
            page: Page::Session,
            session_view: Some(SessionDetailView {
                key: "codex:s1".into(),
                session_id: "s1".into(),
                project: "tokens".into(),
                tool: "Codex",
                date_range: "2026-04-29".into(),
                total_cost: "$1.00".into(),
                total_calls: count as u64,
                total_input: "100".into(),
                total_output: "50".into(),
                total_cache_read: "20".into(),
                calls: (0..count).map(|idx| call(&format!("call {idx}"))).collect(),
                note: None,
            }),
            ..App::default()
        }
    }

    fn ingested_with_calls(count: usize) -> Ingested {
        Ingested {
            calls: (0..count)
                .map(|idx| crate::tools::ParsedCall {
                    tool: "codex",
                    model: "gpt-5".into(),
                    cost_usd: 0.12,
                    dedup_key: format!("call-{idx}"),
                    session_id: format!("session-{idx}"),
                    project: "fixture/project".into(),
                    ..Default::default()
                })
                .collect(),
            limits: Vec::new(),
        }
    }

    fn ingested_with_usage(count: usize, cost_usd: f64, input_tokens: u64) -> Ingested {
        Ingested {
            calls: (0..count)
                .map(|idx| crate::tools::ParsedCall {
                    tool: "codex",
                    model: "gpt-5".into(),
                    cost_usd,
                    input_tokens,
                    dedup_key: format!("usage-call-{idx}"),
                    session_id: format!("usage-session-{idx}"),
                    project: "fixture/project".into(),
                    ..Default::default()
                })
                .collect(),
            limits: Vec::new(),
        }
    }

    fn set_completed_refresh(app: &mut App, kind: RefreshKind, ingested: Ingested) {
        let (signal_tx, _signal_rx) = mpsc::channel::<()>();
        let (result_tx, result_rx) = mpsc::channel::<RefreshOutcome>();
        result_tx
            .send(RefreshOutcome {
                kind,
                result: Ok(ingested),
            })
            .unwrap();
        app.refresher = Some(Refresher {
            signal_tx,
            result_rx,
        });
        std::mem::forget(result_tx);
    }

    #[test]
    fn t_cycles_tool_filter() {
        let mut app = App::default();

        app.handle_key(key(KeyCode::Char('t')));

        assert_eq!(app.tool, Tool::ClaudeCode);
    }

    #[test]
    fn g_cycles_sort_mode() {
        let mut app = App::default();

        app.handle_key(key(KeyCode::Char('g')));
        assert_eq!(app.sort, SortMode::Date);

        app.handle_key(key(KeyCode::Char('g')));
        assert_eq!(app.sort, SortMode::Tokens);

        app.handle_key(key(KeyCode::Char('g')));
        assert_eq!(app.sort, SortMode::Spend);
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
    fn shift_d_toggles_live_and_sample_data() {
        let mut app = App::with_source(DataSource::Live(ingested_with_calls(1)), None);

        app.handle_key(shift_key(KeyCode::Char('D')));
        assert!(matches!(app.source, DataSource::Sample));
        assert!(app.sample_forced);
        assert_eq!(app.status.as_deref(), Some("sample data"));

        app.handle_key(shift_key(KeyCode::Char('D')));
        assert!(matches!(app.source, DataSource::Live(_)));
        assert!(!app.sample_forced);
        assert_eq!(app.status.as_deref(), Some("live data"));
    }

    #[test]
    fn shift_d_without_live_data_keeps_sample_fallback() {
        let mut app = App::default();

        app.handle_key(shift_key(KeyCode::Char('D')));

        assert!(matches!(app.source, DataSource::Sample));
        assert!(!app.sample_forced);
        assert_eq!(
            app.status.as_deref(),
            Some("no local sessions found · sample data")
        );
    }

    #[test]
    fn plain_d_keeps_navigation_instead_of_toggling_data() {
        let mut app = App::with_source(DataSource::Live(ingested_with_calls(1)), None);

        app.handle_key(key(KeyCode::Char('d')));

        assert_eq!(app.page, Page::DeepDive);
        assert!(matches!(app.source, DataSource::Live(_)));
        assert!(!app.sample_forced);
    }

    #[test]
    fn refresh_updates_cached_live_data_while_sample_is_forced() {
        let mut app = App::with_source(DataSource::Live(ingested_with_calls(1)), None);
        app.handle_key(shift_key(KeyCode::Char('D')));
        let (signal_tx, _signal_rx) = mpsc::channel::<()>();
        let (result_tx, result_rx) = mpsc::channel::<RefreshOutcome>();
        result_tx
            .send(RefreshOutcome {
                kind: RefreshKind::Manual,
                result: Ok(ingested_with_calls(2)),
            })
            .unwrap();
        app.refresher = Some(Refresher {
            signal_tx,
            result_rx,
        });

        app.poll_reload();

        assert!(matches!(app.source, DataSource::Sample));
        assert!(app.sample_forced);
        assert_eq!(
            app.live_source.as_ref().map(|live| live.calls.len()),
            Some(2)
        );

        app.handle_key(shift_key(KeyCode::Char('D')));
        match &app.source {
            DataSource::Live(ingested) => assert_eq!(ingested.calls.len(), 2),
            DataSource::Sample => panic!("expected cached live data"),
        }
    }

    #[test]
    fn session_enter_opens_and_escape_closes_call_detail() {
        let mut app = app_with_session_calls(3);

        app.handle_key(key(KeyCode::Down));
        assert_eq!(app.session_selected, 1);
        assert_eq!(app.session_scroll, 1);

        app.handle_key(key(KeyCode::Enter));
        assert_eq!(app.call_detail_index, Some(1));
        assert_eq!(
            app.selected_call_detail()
                .map(|call| call.prompt_full.as_str()),
            Some("call 1 full prompt")
        );

        app.handle_key(key(KeyCode::Esc));
        assert_eq!(app.call_detail_index, None);
        assert_eq!(app.page, Page::Session);
    }

    #[test]
    fn session_mouse_click_opens_visible_call_detail() {
        let mut app = app_with_session_calls(5);
        app.session_scroll = 1;
        let terminal_area = Rect::new(0, 0, 170, 64);
        let table_area = session_calls_area(terminal_area);
        let mouse = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: table_area.x + 2,
            row: table_area.y + 3,
            modifiers: KeyModifiers::NONE,
        };

        app.handle_mouse(mouse, terminal_area);

        assert_eq!(app.call_detail_index, Some(2));
        assert_eq!(app.session_selected, 2);
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
    fn usage_page_can_open_export_picker() {
        let mut app = App::default();

        app.handle_key(key(KeyCode::Char('u')));
        app.handle_key(key(KeyCode::Char('e')));

        assert_eq!(app.page, Page::Usage);
        assert!(app.export_modal.is_some());
    }

    #[test]
    fn session_page_can_open_export_picker() {
        let mut app = app_with_session_calls(1);

        app.handle_key(key(KeyCode::Char('e')));

        assert_eq!(app.page, Page::Session);
        assert!(app.export_modal.is_some());
    }

    #[test]
    fn config_rows_label_download_actions() {
        let app = App::default();
        let rows = app.config_rows();

        assert_eq!(rows[1].name, "rates.json");
        assert_eq!(rows[1].action, "download");
        assert_eq!(rows[2].name, "LiteLLM prices");
        assert_eq!(rows[2].action, "download");
    }

    #[test]
    fn config_download_confirmation_opens_and_cancels() {
        let mut app = App::default();

        app.handle_key(key(KeyCode::Char('c')));
        app.handle_key(key(KeyCode::Down));
        app.handle_key(key(KeyCode::Enter));

        assert_eq!(app.download_confirm, Some(ConfigDownload::CurrencyRates));

        app.handle_key(key(KeyCode::Char('q')));
        assert_eq!(app.download_confirm, Some(ConfigDownload::CurrencyRates));
        assert!(!app.should_quit());

        app.handle_key(key(KeyCode::Char('n')));
        assert_eq!(app.download_confirm, None);
        assert_eq!(app.status, None);
    }

    #[test]
    fn config_download_confirmation_escape_cancels_pricing_download() {
        let mut app = App::default();

        app.handle_key(key(KeyCode::Char('c')));
        app.handle_key(key(KeyCode::Down));
        app.handle_key(key(KeyCode::Down));
        app.handle_key(key(KeyCode::Enter));

        assert_eq!(app.download_confirm, Some(ConfigDownload::PricingSnapshot));

        app.handle_key(key(KeyCode::Esc));
        assert_eq!(app.download_confirm, None);
        assert_eq!(app.status, None);
    }

    #[cfg(not(feature = "refresh-currency"))]
    #[test]
    fn confirming_rates_download_reports_when_build_has_no_downloads() {
        let mut app = App::default();

        app.handle_key(key(KeyCode::Char('c')));
        app.handle_key(key(KeyCode::Down));
        app.handle_key(key(KeyCode::Enter));
        app.handle_key(key(KeyCode::Enter));

        assert_eq!(app.download_confirm, None);
        assert_eq!(
            app.status.as_deref(),
            Some("rates download unavailable in this build")
        );
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
    fn export_modal_opens_folder_picker_from_browse_key() {
        let mut app = App::default();
        let dir = tempdir("browse-open");
        app.export_dir = dir.clone();

        app.handle_key(key(KeyCode::Char('e')));
        app.handle_key(key(KeyCode::Char('f')));

        assert!(app.export_modal.is_some());
        assert_eq!(app.export_dir_picker.as_ref().unwrap().current_dir, dir);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn export_folder_picker_updates_session_destination() {
        let mut app = App::default();
        let dir = tempdir("browse-choose");
        let chosen = dir.join("chosen");
        std::fs::create_dir_all(&chosen).unwrap();
        app.export_dir = dir.clone();

        app.handle_key(key(KeyCode::Char('e')));
        app.handle_key(key(KeyCode::Char('f')));
        let chosen_idx = app
            .export_dir_picker
            .as_ref()
            .unwrap()
            .entries
            .iter()
            .position(|entry| entry.label == "chosen")
            .unwrap();
        app.export_dir_picker.as_mut().unwrap().selected = chosen_idx;
        app.handle_key(key(KeyCode::Enter));
        app.handle_key(key(KeyCode::Enter));

        assert_eq!(app.export_dir, chosen);
        assert!(app.export_dir_picker.is_none());
        assert!(app.export_modal.is_some());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn export_folder_picker_escape_keeps_existing_destination() {
        let mut app = App::default();
        let dir = tempdir("browse-cancel");
        app.export_dir = dir.clone();

        app.handle_key(key(KeyCode::Char('e')));
        app.handle_key(key(KeyCode::Char('b')));
        app.handle_key(key(KeyCode::Esc));

        assert_eq!(app.export_dir, dir);
        assert!(app.export_dir_picker.is_none());
        assert!(app.export_modal.is_some());
        let _ = std::fs::remove_dir_all(app.export_dir);
    }

    #[test]
    fn export_writes_to_selected_session_destination() {
        let mut app = App::default();
        let dir = tempdir("export-target");
        app.export_dir = dir.clone();

        app.handle_key(key(KeyCode::Char('e')));
        app.handle_key(key(KeyCode::Enter));

        let exported: Vec<_> = std::fs::read_dir(&dir).unwrap().flatten().collect();
        assert_eq!(exported.len(), 1);
        assert!(exported[0]
            .path()
            .extension()
            .is_some_and(|ext| ext == "json"));
        assert!(app.export_modal.is_none());
        assert!(app
            .status
            .as_deref()
            .is_some_and(|status| status.contains("exported JSON")));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn folder_picker_lists_sorted_subdirectories_and_hides_dot_dirs() {
        let dir = tempdir("picker-sort");
        std::fs::create_dir_all(dir.join("zeta")).unwrap();
        std::fs::create_dir_all(dir.join("alpha")).unwrap();
        std::fs::create_dir_all(dir.join(".hidden")).unwrap();

        let picker = FolderPickerModal::new(dir.clone());
        let labels: Vec<_> = picker
            .entries
            .iter()
            .filter(|entry| entry.kind == FolderPickerEntryKind::Directory)
            .map(|entry| entry.label.as_str())
            .collect();

        assert_eq!(labels, vec!["alpha", "zeta"]);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn folder_picker_parent_navigation_moves_up_one_directory() {
        let dir = tempdir("picker-parent");
        let child = dir.join("child");
        std::fs::create_dir_all(&child).unwrap();
        let mut picker = FolderPickerModal::new(child);

        picker.go_parent();

        assert_eq!(picker.current_dir, dir);
        assert_eq!(picker.selected, 0);
        let _ = std::fs::remove_dir_all(picker.current_dir);
    }

    #[test]
    fn folder_picker_keeps_use_current_available_on_read_error() {
        let dir = tempdir("picker-error");
        let missing = dir.join("missing");
        let picker = FolderPickerModal::new(missing.clone());

        assert!(picker.error.is_some());
        assert_eq!(picker.entries[0].kind, FolderPickerEntryKind::UseCurrent);
        assert_eq!(picker.entries[0].path, missing);
        let _ = std::fs::remove_dir_all(dir);
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

    #[test]
    fn auto_refresh_queues_background_alert_when_delta_crosses_threshold() {
        let mut app = App::with_source(DataSource::Live(ingested_with_usage(10, 0.0, 0)), None);
        set_completed_refresh(&mut app, RefreshKind::Auto, ingested_with_usage(35, 0.0, 0));

        app.poll_reload();

        assert_eq!(
            app.take_background_alerts(),
            vec![BackgroundUsageAlert {
                calls: 25,
                tokens: 0,
                cost_usd: 0.0
            }]
        );
    }

    #[test]
    fn below_threshold_auto_refreshes_accumulate_until_alert_fires() {
        let mut app = App::with_source(DataSource::Live(ingested_with_usage(10, 0.0, 0)), None);
        set_completed_refresh(&mut app, RefreshKind::Auto, ingested_with_usage(20, 0.0, 0));

        app.poll_reload();
        assert!(app.take_background_alerts().is_empty());

        set_completed_refresh(&mut app, RefreshKind::Auto, ingested_with_usage(35, 0.0, 0));
        app.poll_reload();

        assert_eq!(app.take_background_alerts()[0].calls, 25);
    }

    #[test]
    fn background_alert_cooldown_delays_repeated_notifications() {
        let mut app = App::with_source(DataSource::Live(ingested_with_usage(10, 0.0, 0)), None);
        set_completed_refresh(&mut app, RefreshKind::Auto, ingested_with_usage(35, 0.0, 0));
        app.poll_reload();
        assert_eq!(app.take_background_alerts().len(), 1);

        set_completed_refresh(&mut app, RefreshKind::Auto, ingested_with_usage(60, 0.0, 0));
        app.poll_reload();
        assert!(app.take_background_alerts().is_empty());

        app.background_alert_last_sent = Some(Utc::now() - chrono::Duration::minutes(31));
        set_completed_refresh(&mut app, RefreshKind::Auto, ingested_with_usage(60, 0.0, 0));
        app.poll_reload();

        assert_eq!(app.take_background_alerts()[0].calls, 25);
    }

    #[test]
    fn manual_refresh_resets_background_alert_baseline_without_notifying() {
        let mut app = App::with_source(DataSource::Live(ingested_with_usage(10, 0.0, 0)), None);
        set_completed_refresh(
            &mut app,
            RefreshKind::Manual,
            ingested_with_usage(40, 0.0, 0),
        );

        app.poll_reload();

        assert!(app.take_background_alerts().is_empty());

        set_completed_refresh(&mut app, RefreshKind::Auto, ingested_with_usage(64, 0.0, 0));
        app.poll_reload();
        assert!(app.take_background_alerts().is_empty());

        set_completed_refresh(&mut app, RefreshKind::Auto, ingested_with_usage(65, 0.0, 0));
        app.poll_reload();
        assert_eq!(app.take_background_alerts()[0].calls, 25);
    }

    #[test]
    fn first_live_auto_refresh_from_sample_sets_baseline_without_alerting() {
        let mut app = App::default();
        set_completed_refresh(
            &mut app,
            RefreshKind::Auto,
            ingested_with_usage(100, 10.0, 10_000),
        );

        app.poll_reload();

        assert!(app.take_background_alerts().is_empty());
    }

    #[test]
    fn disabled_background_alerts_never_queue_notifications() {
        let mut app = App::with_source(DataSource::Live(ingested_with_usage(10, 0.0, 0)), None);
        app.settings.background_alerts.enabled = false;
        set_completed_refresh(
            &mut app,
            RefreshKind::Auto,
            ingested_with_usage(100, 10.0, 10_000),
        );

        app.poll_reload();

        assert!(app.take_background_alerts().is_empty());
    }
}
