use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread;
use std::time::Duration;

use chrono::{DateTime, Datelike, Local, Utc};
use color_eyre::Result;
use crossterm::event::{
    KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::layout::{Constraint, Direction, Layout, Margin, Rect};

use crate::advice::{AdviceDataScope, AdviceHistory, AdviceItemStatus, AdviceTool};
use crate::archive;
use crate::config::{ConfigPaths, UserConfig};
use crate::copy::{self, copy, CopyDeck};
use crate::currency::{CurrencyFormatter, CurrencyTable};
use crate::data::{
    DashboardData, LimitsData, ProjectOption, SessionDetail, SessionDetailView, SessionOption,
};
use crate::ingest::Ingested;
use crate::keymap;
use crate::reports::{ReportFormat, ReportRequest, ReportScope};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Period {
    Today,
    Week,
    ThirtyDays,
    Month,
    AllTime,
}

impl Period {
    const MONTH_DAILY_START_DAY: u32 = 15;

    pub const ALL: [Self; 5] = [
        Self::Today,
        Self::Week,
        Self::ThirtyDays,
        Self::Month,
        Self::AllTime,
    ];

    pub fn label(self) -> &'static str {
        let copy = copy();
        match self {
            Self::Today => copy.periods.today.as_str(),
            Self::Week => copy.periods.week.as_str(),
            Self::ThirtyDays => copy.periods.thirty_days.as_str(),
            Self::Month => copy.periods.month.as_str(),
            Self::AllTime => copy.periods.all_time.as_str(),
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

    pub fn uses_hourly_activity_timeline(self, now: DateTime<Local>) -> bool {
        matches!(self, Self::Today | Self::Week)
            || (self == Self::Month && now.day() < Self::MONTH_DAILY_START_DAY)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    All,
    ClaudeCode,
    Cursor,
    Codex,
    Copilot,
    Gemini,
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
        let copy = copy();
        match self {
            Self::Spend => copy.sorts.spend.as_str(),
            Self::Date => copy.sorts.date.as_str(),
            Self::Tokens => copy.sorts.tokens.as_str(),
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
    Insights,
    Session,
}

impl Page {
    pub const TABS: [Page; 4] = [Page::Overview, Page::DeepDive, Page::Usage, Page::Insights];

    pub fn label(self) -> &'static str {
        let copy = copy();
        match self {
            Self::Overview => copy.nav.overview.as_str(),
            Self::DeepDive => copy.nav.deep_dive.as_str(),
            Self::Usage => copy.nav.usage.as_str(),
            Self::Insights => copy.nav.insights.as_str(),
            Self::Config => copy.nav.config.as_str(),
            Self::Session => copy.nav.session.as_str(),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InsightsTab {
    Advice,
    Signals,
}

impl InsightsTab {
    fn next(self) -> Self {
        match self {
            Self::Advice => Self::Signals,
            Self::Signals => Self::Advice,
        }
    }

    fn prev(self) -> Self {
        self.next()
    }
}

impl Tool {
    pub fn label(self) -> &'static str {
        let copy = copy();
        match self {
            Self::All => copy.tools.all.as_str(),
            Self::ClaudeCode => copy.tools.claude_code.as_str(),
            Self::Cursor => copy.tools.cursor.as_str(),
            Self::Codex => copy.tools.codex.as_str(),
            Self::Copilot => copy.tools.copilot.as_str(),
            Self::Gemini => copy.tools.gemini.as_str(),
        }
    }

    fn next(self) -> Self {
        match self {
            Self::All => Self::ClaudeCode,
            Self::ClaudeCode => Self::Cursor,
            Self::Cursor => Self::Codex,
            Self::Codex => Self::Copilot,
            Self::Copilot => Self::Gemini,
            Self::Gemini => Self::All,
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
            Self::All => copy().tools.all.as_str(),
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
    pub options: Vec<ReportFormat>,
    pub selected: usize,
    pub period: Period,
    pub project_options: Vec<ProjectOption>,
    pub project_selected: usize,
    pub redacted: bool,
}

impl Default for ExportModal {
    fn default() -> Self {
        Self::new(
            Period::Week,
            vec![ProjectOption::all("$0.00".into(), 0)],
            &ProjectFilter::All,
        )
    }
}

impl ExportModal {
    pub fn new(
        period: Period,
        project_options: Vec<ProjectOption>,
        project_filter: &ProjectFilter,
    ) -> Self {
        let project_selected = project_filter
            .identity()
            .and_then(|identity| {
                project_options
                    .iter()
                    .position(|option| option.identity.as_deref() == Some(identity))
            })
            .unwrap_or(0);
        Self {
            options: ReportFormat::ALL.to_vec(),
            selected: 0,
            period,
            project_options,
            project_selected,
            redacted: false,
        }
    }

    pub fn scope(&self) -> ReportScope {
        self.project_options
            .get(self.project_selected)
            .map(|option| match &option.identity {
                Some(identity) => ReportScope::Project {
                    identity: identity.clone(),
                    label: option.label.clone(),
                },
                None => ReportScope::AllProjects,
            })
            .unwrap_or(ReportScope::AllProjects)
    }

    pub fn current_project_label(&self) -> &str {
        self.project_options
            .get(self.project_selected)
            .map(|option| option.label.as_str())
            .unwrap_or_else(|| copy().reports.all_projects.as_str())
    }

    fn cycle_period(&mut self, delta: isize) {
        let periods = Period::ALL;
        let idx = periods
            .iter()
            .position(|period| *period == self.period)
            .unwrap_or(0);
        let next = if delta.is_negative() {
            (idx + periods.len() - 1) % periods.len()
        } else {
            (idx + 1) % periods.len()
        };
        self.period = periods[next];
    }

    fn cycle_project(&mut self) {
        let len = self.project_options.len().max(1);
        self.project_selected = (self.project_selected + 1) % len;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigDownload {
    CurrencyRates,
    PricingSnapshot,
    CopilotLimits,
}

impl ConfigDownload {
    pub fn title(self) -> &'static str {
        let copy = copy();
        match self {
            Self::CurrencyRates => copy.modals.download_rates_title.as_str(),
            Self::PricingSnapshot => copy.modals.download_prices_title.as_str(),
            Self::CopilotLimits => copy.modals.sync_copilot_limits_title.as_str(),
        }
    }

    pub fn file_name(self) -> &'static str {
        let copy = copy();
        match self {
            Self::CurrencyRates => copy.modals.rates_file.as_str(),
            Self::PricingSnapshot => copy.modals.pricing_file.as_str(),
            Self::CopilotLimits => copy.modals.copilot_limits_file.as_str(),
        }
    }

    pub fn source(self) -> &'static str {
        let copy = copy();
        match self {
            Self::CurrencyRates => copy.modals.rates_source.as_str(),
            Self::PricingSnapshot => copy.modals.prices_source.as_str(),
            Self::CopilotLimits => copy.modals.copilot_limits_source.as_str(),
        }
    }

    pub fn effect(self) -> &'static str {
        let copy = copy();
        match self {
            Self::CurrencyRates => copy.modals.rates_effect.as_str(),
            Self::PricingSnapshot => copy.modals.prices_effect.as_str(),
            Self::CopilotLimits => copy.modals.copilot_limits_effect.as_str(),
        }
    }

    pub fn confirm_action_lower(self) -> &'static str {
        let copy = copy();
        match self {
            Self::CurrencyRates | Self::PricingSnapshot => copy.actions.download_lower.as_str(),
            Self::CopilotLimits => copy.actions.sync_lower.as_str(),
        }
    }

    pub fn links(self) -> Vec<ConfigLinkView> {
        match self {
            Self::CurrencyRates => rates_download_links(),
            Self::PricingSnapshot => pricing_download_links(),
            Self::CopilotLimits => Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CookieModalKind {
    Claude,
    Codex,
}

impl CookieModalKind {
    pub fn keyring_account(self) -> &'static str {
        match self {
            Self::Claude => crate::tools::claude_subscription::config::KEYRING_ACCOUNT,
            Self::Codex => crate::tools::codex_subscription::config::KEYRING_ACCOUNT,
        }
    }

    pub fn config_row_id(self) -> &'static str {
        match self {
            Self::Claude => "claude_subscription_limits",
            Self::Codex => "codex_subscription_limits",
        }
    }

    pub fn title(self) -> &'static str {
        let copy = copy();
        match self {
            Self::Claude => copy.modals.subscription_cookie.title_claude.as_str(),
            Self::Codex => copy.modals.subscription_cookie.title_codex.as_str(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CookieField {
    Primary,
    ShardOne,
    Extras,
    ActionRow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CookieAction {
    SaveAndSync,
    SyncStored,
    Clear,
}

#[derive(Debug, Clone)]
pub struct SubscriptionCookieModal {
    pub kind: CookieModalKind,
    pub primary: String,
    pub shard_one: String,
    pub extras: String,
    pub focus: CookieField,
    pub action: CookieAction,
    pub busy: bool,
    pub error: Option<String>,
    pub has_stored: bool,
}

impl SubscriptionCookieModal {
    pub fn new(kind: CookieModalKind, has_stored: bool) -> Self {
        Self {
            kind,
            primary: String::new(),
            shard_one: String::new(),
            extras: String::new(),
            focus: CookieField::Primary,
            action: CookieAction::SaveAndSync,
            busy: false,
            error: None,
            has_stored,
        }
    }

    pub fn fields(&self) -> &'static [CookieField] {
        match self.kind {
            CookieModalKind::Claude => &[CookieField::Primary, CookieField::ActionRow],
            CookieModalKind::Codex => &[
                CookieField::Primary,
                CookieField::ShardOne,
                CookieField::Extras,
                CookieField::ActionRow,
            ],
        }
    }

    pub fn actions(&self) -> &'static [CookieAction] {
        &[
            CookieAction::SaveAndSync,
            CookieAction::SyncStored,
            CookieAction::Clear,
        ]
    }

    pub fn cycle_field(&mut self, delta: isize) {
        let fields = self.fields();
        let Some(pos) = fields.iter().position(|f| *f == self.focus) else {
            self.focus = fields[0];
            return;
        };
        let len = fields.len() as isize;
        let next = ((pos as isize + delta).rem_euclid(len)) as usize;
        self.focus = fields[next];
    }

    pub fn cycle_action(&mut self, delta: isize) {
        let actions = self.actions();
        let pos = actions.iter().position(|a| *a == self.action).unwrap_or(0) as isize;
        let len = actions.len() as isize;
        let next = ((pos + delta).rem_euclid(len)) as usize;
        self.action = actions[next];
        self.focus = CookieField::ActionRow;
    }

    pub fn field_value_mut(&mut self, field: CookieField) -> Option<&mut String> {
        match field {
            CookieField::Primary => Some(&mut self.primary),
            CookieField::ShardOne if self.kind == CookieModalKind::Codex => {
                Some(&mut self.shard_one)
            }
            CookieField::Extras if self.kind == CookieModalKind::Codex => Some(&mut self.extras),
            _ => None,
        }
    }

    pub fn focused_field_value_mut(&mut self) -> Option<&mut String> {
        let focus = self.focus;
        self.field_value_mut(focus)
    }

    pub fn has_input(&self) -> bool {
        match self.kind {
            CookieModalKind::Claude => !self.primary.trim().is_empty(),
            CookieModalKind::Codex => {
                !self.primary.trim().is_empty() && !self.shard_one.trim().is_empty()
            }
        }
    }

    pub fn save_and_sync_disabled(&self) -> bool {
        self.busy || !self.has_input()
    }

    pub fn sync_stored_disabled(&self) -> bool {
        self.busy || !self.has_stored
    }

    pub fn clear_disabled(&self) -> bool {
        self.busy || !self.has_stored
    }

    pub fn action_disabled(&self, action: CookieAction) -> bool {
        match action {
            CookieAction::SaveAndSync => self.save_and_sync_disabled(),
            CookieAction::SyncStored => self.sync_stored_disabled(),
            CookieAction::Clear => self.clear_disabled(),
        }
    }

    pub fn compose_cookie(&self) -> String {
        match self.kind {
            CookieModalKind::Claude => self.primary.trim().to_string(),
            CookieModalKind::Codex => {
                let mut parts: Vec<String> = Vec::new();
                let p0 = self.primary.trim();
                let p1 = self.shard_one.trim();
                if !p0.is_empty() {
                    parts.push(format!("__Secure-next-auth.session-token.0={p0}"));
                }
                if !p1.is_empty() {
                    parts.push(format!("__Secure-next-auth.session-token.1={p1}"));
                }
                let extras = self.extras.trim();
                if !extras.is_empty() {
                    parts.push(extras.to_string());
                }
                parts.join("; ")
            }
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
        label: copy().modals.use_this_folder.clone(),
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
        Err(e) => Some(copy::template(
            &copy().modals.could_not_read_folder,
            &[("error", e.to_string())],
        )),
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

fn estimated_wrapped_lines(text: &str) -> usize {
    text.chars().count().div_ceil(100).max(1)
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
pub struct ConfigLinkView {
    pub label: String,
    pub url: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ConfigRowView {
    pub id: &'static str,
    pub name: &'static str,
    pub value: String,
    pub action: &'static str,
    pub links: Vec<ConfigLinkView>,
}

fn rates_download_links() -> Vec<ConfigLinkView> {
    vec![ConfigLinkView {
        label: copy().config.links.published_rates.clone(),
        url: crate::config::CURRENCY_RATES_URL.to_string(),
    }]
}

fn render_statusline_value(
    copy: &CopyDeck,
    state: Option<&crate::tools::claude_code::statusline::InstallState>,
) -> String {
    use crate::tools::claude_code::statusline::InstallState;
    match state {
        None | Some(InstallState::NotInstalled) => {
            copy.config.values.statusline_not_installed.clone()
        }
        Some(InstallState::InstalledWrapping(inner)) => crate::copy::template(
            &copy.config.values.statusline_installed_wrapping,
            &[("inner", inner.clone())],
        ),
        Some(InstallState::InstalledPassthrough) => {
            copy.config.values.statusline_installed_passthrough.clone()
        }
        Some(InstallState::External(cmd)) => crate::copy::template(
            &copy.config.values.statusline_external,
            &[("command", cmd.clone())],
        ),
    }
}

fn pricing_download_links() -> Vec<ConfigLinkView> {
    let Ok(urls) = crate::pricing::published_book_urls() else {
        return Vec::new();
    };
    vec![
        ConfigLinkView {
            label: copy().config.links.pricing_upstream.clone(),
            url: urls.upstream,
        },
        ConfigLinkView {
            label: copy().config.links.pricing_overrides.clone(),
            url: urls.overrides,
        },
    ]
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

fn load_advice_history(paths: &ConfigPaths) -> AdviceHistory {
    archive::Archive::open(paths)
        .and_then(|archive| archive.advice_history())
        .unwrap_or_default()
}

fn run_advice_job(
    ingested: Ingested,
    paths: ConfigPaths,
    tool: AdviceTool,
    data_scope: AdviceDataScope,
) -> AdviceJobResult {
    let run = crate::advice::generate_advice_run(&ingested, &paths, tool, data_scope);
    let item_count = run.items.len();
    let run_error = run.error.clone();
    let mut archive = archive::Archive::open(&paths).map_err(|e| e.to_string())?;
    let usage_sync_status = match archive.sync() {
        Ok(stats) => format!(
            "{} calls · {} limits",
            stats.calls_inserted, stats.limits_inserted
        ),
        Err(e) => format!("sync failed: {e}"),
    };
    let ingested = archive.load().ok();

    archive
        .insert_advice_run(&run, &usage_sync_status)
        .map_err(|e| e.to_string())?;

    Ok(AdviceJobOutcome {
        item_count,
        run_error,
        usage_sync_status,
        ingested,
    })
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
    Archive(Box<ConfigPaths>),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClearDataModal {
    Confirm,
    Running,
}

type ClearDataResult = std::result::Result<(Ingested, archive::SyncStats), String>;

struct ClearDataJob {
    result_rx: Receiver<ClearDataResult>,
}

type AdviceJobResult = std::result::Result<AdviceJobOutcome, String>;

struct AdviceJob {
    result_rx: Receiver<AdviceJobResult>,
}

struct AdviceJobOutcome {
    item_count: usize,
    run_error: Option<String>,
    usage_sync_status: String,
    ingested: Option<Ingested>,
}

#[cfg(feature = "quota-sync")]
type SubscriptionSyncResult = std::result::Result<(usize, Ingested), String>;

#[cfg(feature = "quota-sync")]
struct SubscriptionSyncJob {
    kind: CookieModalKind,
    result_rx: Receiver<SubscriptionSyncResult>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusTone {
    Info,
    Busy,
    Success,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppStatus {
    pub text: String,
    pub tone: StatusTone,
}

impl AppStatus {
    pub fn new(text: impl Into<String>, tone: StatusTone) -> Self {
        Self {
            text: text.into(),
            tone,
        }
    }

    pub fn info(text: impl Into<String>) -> Self {
        Self::new(text, StatusTone::Info)
    }
}

impl From<String> for AppStatus {
    fn from(text: String) -> Self {
        Self::info(text)
    }
}

impl From<&str> for AppStatus {
    fn from(text: &str) -> Self {
        Self::info(text)
    }
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
                    RefreshSource::Archive(paths) => {
                        #[cfg(feature = "quota-sync")]
                        crate::quota_sync::auto_refresh(paths.as_ref());
                        crate::archive::sync_and_load(paths.as_ref())
                    }
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
    pub subscription_cookie_modal: Option<SubscriptionCookieModal>,
    pub download_confirm: Option<ConfigDownload>,
    pub clear_data_modal: Option<ClearDataModal>,
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
    clear_data_job: Option<ClearDataJob>,
    advice_job: Option<AdviceJob>,
    #[cfg(feature = "quota-sync")]
    subscription_sync_job: Option<SubscriptionSyncJob>,
    clear_data_tick: usize,
    background_alert_baseline: Option<UsageTotals>,
    background_alert_last_sent: Option<DateTime<Utc>>,
    background_alerts: Vec<BackgroundUsageAlert>,
    live_source: Option<Ingested>,
    sample_forced: bool,
    advice_history: AdviceHistory,
    insights_tab: InsightsTab,
    insights_advice_scroll: usize,
    insights_signals_scroll: usize,
    pub status: Option<AppStatus>,
    should_quit: bool,
}

impl Default for App {
    fn default() -> Self {
        let paths = ConfigPaths::default();
        let export_dir = crate::reports::default_report_dir(&paths);
        Self {
            page: Page::Overview,
            period: Period::Week,
            tool: Tool::All,
            sort: SortMode::Spend,
            project_filter: ProjectFilter::All,
            project_modal: None,
            currency_modal: None,
            subscription_cookie_modal: None,
            download_confirm: None,
            clear_data_modal: None,
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
            clear_data_job: None,
            advice_job: None,
            #[cfg(feature = "quota-sync")]
            subscription_sync_job: None,
            clear_data_tick: 0,
            background_alert_baseline: None,
            background_alert_last_sent: None,
            background_alerts: Vec::new(),
            live_source: None,
            sample_forced: false,
            advice_history: AdviceHistory::default(),
            insights_tab: InsightsTab::Advice,
            insights_advice_scroll: 0,
            insights_signals_scroll: 0,
            status: None,
            should_quit: false,
        }
    }
}

impl App {
    pub fn with_source(source: DataSource, status: Option<AppStatus>) -> Self {
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
        status: Option<AppStatus>,
        settings: UserConfig,
        paths: ConfigPaths,
        currency_table: CurrencyTable,
        initial_refresh_delay: Duration,
        refresh_source: RefreshSource,
    ) -> Self {
        let refresher = Some(Refresher::spawn(initial_refresh_delay, refresh_source));
        let export_dir = crate::reports::default_report_dir(&paths);
        let live_source = cached_live_source(&source);
        let background_alert_baseline = live_source.as_ref().map(UsageTotals::from_ingested);
        let advice_history = load_advice_history(&paths);
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
            advice_history,
            ..Self::default()
        }
    }

    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    pub fn dashboard(&self) -> DashboardData {
        self.dashboard_for(self.period, self.tool, &self.project_filter, self.sort)
    }

    pub fn dashboard_for(
        &self,
        period: Period,
        tool: Tool,
        project_filter: &ProjectFilter,
        sort: SortMode,
    ) -> DashboardData {
        let currency = self.currency();
        match &self.source {
            DataSource::Live(ingested) => {
                ingested.dashboard(period, tool, project_filter, sort, &currency)
            }
            DataSource::Sample => {
                crate::data::dashboard_data(period, tool, project_filter, sort, &currency)
            }
        }
    }

    pub fn usage(&self) -> LimitsData {
        self.usage_for(self.tool, self.sort)
    }

    pub fn usage_for(&self, tool: Tool, sort: SortMode) -> LimitsData {
        let currency = self.currency();
        match &self.source {
            DataSource::Live(ingested) => ingested.limits(tool, sort, &currency),
            DataSource::Sample => crate::data::limits_data(tool, sort, &currency),
        }
    }

    pub fn insights(&self) -> crate::insights::InsightsView {
        match &self.source {
            DataSource::Live(ingested) => ingested.insights(),
            DataSource::Sample => crate::data::insights_view(),
        }
    }

    pub fn advice_history(&self) -> AdviceHistory {
        self.advice_history.clone()
    }

    pub fn advice_running(&self) -> bool {
        self.advice_job.is_some()
    }

    pub fn insights_tab(&self) -> InsightsTab {
        self.insights_tab
    }

    pub fn insights_scroll(&self) -> usize {
        match self.insights_tab {
            InsightsTab::Advice => self.insights_advice_scroll,
            InsightsTab::Signals => self.insights_signals_scroll,
        }
    }

    pub fn set_insights_tab(&mut self, tab: InsightsTab) {
        self.insights_tab = tab;
        self.clamp_insights_scroll();
    }

    pub fn currency(&self) -> CurrencyFormatter {
        self.currency_table.formatter(&self.settings.currency)
    }

    pub fn status_tone(&self) -> StatusTone {
        self.status
            .as_ref()
            .map(|status| status.tone)
            .unwrap_or(StatusTone::Info)
    }

    pub fn status_text(&self) -> Option<&str> {
        self.status.as_ref().map(|status| status.text.as_str())
    }

    fn set_status(&mut self, text: impl Into<String>, tone: StatusTone) {
        self.status = Some(AppStatus::new(text, tone));
    }

    pub fn clear_data_spinner_frame(&self) -> usize {
        self.clear_data_tick
    }

    /// Ask the background refresher to run ingest now (out-of-cycle). The
    /// thread does one ingest at a time, so a queued signal just runs after
    /// the in-flight one finishes - no dedup needed here.
    pub fn reload(&mut self) {
        let Some(refresher) = self.refresher.as_ref() else {
            return;
        };
        if refresher.signal_tx.send(()).is_ok() {
            self.set_status(copy().status.reloading.clone(), StatusTone::Busy);
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
                self.set_status(copy().status.sample_data.clone(), StatusTone::Info);
            }
            DataSource::Sample => {
                self.sample_forced = false;
                if let Some(ingested) = self.live_source.take() {
                    self.source = DataSource::Live(ingested);
                    self.set_status(copy().status.live_data.clone(), StatusTone::Info);
                } else {
                    self.source = DataSource::Sample;
                    self.set_status(
                        copy().status.no_local_sessions_sample_data.clone(),
                        StatusTone::Warning,
                    );
                }
            }
        }
        self.refresh_session_view();
    }

    /// Drain any results the refresher has produced and apply the most recent
    /// successful one. Called every tick from the main loop.
    pub fn poll_reload(&mut self) {
        self.poll_clear_data();
        self.poll_advice_job();
        self.poll_subscription_sync();

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
                    self.set_status(
                        copy().status.refresher_stopped_prior_data_kept.clone(),
                        StatusTone::Warning,
                    );
                    return;
                }
            }
        }
        let Some(outcome) = latest else {
            return;
        };

        let manual = matches!(outcome.kind, RefreshKind::Manual);
        let keep_busy_status = self.advice_job.is_some() && !manual;
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
                if !keep_busy_status {
                    let template = if manual {
                        &copy().status.reloaded_calls
                    } else {
                        &copy().status.auto_refreshed_calls
                    };
                    self.set_status(
                        copy::template(template, &[("calls", n.to_string())]),
                        StatusTone::Success,
                    );
                }
            }
            Ok(_) => {
                if !keep_busy_status {
                    let text = if manual {
                        copy().status.reload_no_sessions_prior_data_kept.clone()
                    } else {
                        copy()
                            .status
                            .auto_refresh_no_sessions_prior_data_kept
                            .clone()
                    };
                    self.set_status(text, StatusTone::Warning);
                }
            }
            Err(e) => {
                if !keep_busy_status {
                    let template = if manual {
                        &copy().status.reload_failed_prior_data_kept
                    } else {
                        &copy().status.auto_refresh_failed_prior_data_kept
                    };
                    self.set_status(
                        copy::template(template, &[("error", e.to_string())]),
                        StatusTone::Error,
                    );
                }
            }
        }

        self.refresh_session_view();
    }

    fn poll_clear_data(&mut self) {
        if self.clear_data_job.is_some() {
            self.clear_data_tick = self.clear_data_tick.wrapping_add(1);
        }

        let result = match self
            .clear_data_job
            .as_ref()
            .map(|job| job.result_rx.try_recv())
        {
            Some(Ok(result)) => Some(result),
            Some(Err(TryRecvError::Disconnected)) => {
                Some(Err("clear data worker stopped before reporting".into()))
            }
            Some(Err(TryRecvError::Empty)) | None => None,
        };

        if let Some(result) = result {
            self.clear_data_job = None;
            self.clear_data_modal = None;
            self.apply_clear_data_result(result);
        }
    }

    fn poll_advice_job(&mut self) {
        let result = match self.advice_job.as_ref().map(|job| job.result_rx.try_recv()) {
            Some(Ok(result)) => Some(result),
            Some(Err(TryRecvError::Disconnected)) => {
                Some(Err("advice worker stopped before reporting".into()))
            }
            Some(Err(TryRecvError::Empty)) | None => None,
        };

        let Some(result) = result else {
            return;
        };

        self.advice_job = None;
        match result {
            Ok(outcome) => self.apply_advice_job_result(outcome),
            Err(error) => self.set_status(
                copy::template(&copy().status.advice_failed, &[("error", error)]),
                StatusTone::Error,
            ),
        }
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
            self.set_status(
                copy().status.no_sessions_to_drill_into.clone(),
                StatusTone::Warning,
            );
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
                self.set_status(
                    copy::template(
                        &copy().status.session_not_found,
                        &[("key", key.to_string())],
                    ),
                    StatusTone::Warning,
                );
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
        let mut project_options = self.report_project_options(self.period);
        if project_options.is_empty() {
            project_options.push(ProjectOption::all("$0.00".into(), 0));
        }
        self.export_modal = Some(ExportModal::new(
            self.period,
            project_options,
            &self.project_filter,
        ));
    }

    fn open_export_dir_picker(&mut self) {
        self.export_dir_picker = Some(FolderPickerModal::new(self.export_dir.clone()));
    }

    pub fn generate_report(
        &mut self,
        request: ReportRequest,
    ) -> color_eyre::Result<std::path::PathBuf> {
        let path = {
            let currency = self.currency();
            let source_label = match &self.source {
                DataSource::Live(_) => "live",
                DataSource::Sample => "sample",
            };
            match &self.source {
                DataSource::Live(ingested) => {
                    crate::reports::write_ingested_to_dir(
                        &self.export_dir,
                        &request,
                        ingested,
                        &currency,
                        source_label,
                    )?
                    .path
                }
                DataSource::Sample => {
                    let dashboard = self.dashboard_for(
                        request.period,
                        Tool::All,
                        &request.scope.project_filter(),
                        SortMode::Spend,
                    );
                    crate::reports::write_sample_to_dir(
                        &self.export_dir,
                        &request,
                        &dashboard,
                        &currency,
                        source_label,
                    )?
                    .path
                }
            }
        };
        self.set_status(
            copy::template(
                &copy().status.report_generated,
                &[
                    ("format", request.format.label().to_string()),
                    ("path", path.display().to_string()),
                ],
            ),
            StatusTone::Success,
        );
        Ok(path)
    }

    fn run_report(&mut self, request: ReportRequest) {
        if let Err(e) = self.generate_report(request) {
            self.set_status(
                copy::template(&copy().status.report_failed, &[("error", e.to_string())]),
                StatusTone::Error,
            );
        }
    }

    pub fn set_export_dir(&mut self, dir: PathBuf) {
        self.export_dir = dir;
        self.set_status(
            copy::template(
                &copy().status.report_folder,
                &[("path", self.export_dir.display().to_string())],
            ),
            StatusTone::Info,
        );
    }

    pub fn config_rows(&self) -> Vec<ConfigRowView> {
        let copy = copy();
        let currency = self.currency();
        let currency_value = if currency.is_usd() {
            copy.metrics.usd_default.clone()
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

        let pricing_status = crate::pricing::configured_book_status(&self.paths);
        let pricing_source = match pricing_status.source {
            crate::pricing::PricingBookSource::LocalBooks => &copy.config.values.local_snapshot,
            crate::pricing::PricingBookSource::LegacySnapshot => {
                &copy.config.values.legacy_snapshot
            }
            crate::pricing::PricingBookSource::EmbeddedBooks => {
                &copy.config.values.embedded_snapshot
            }
        };
        let pricing_value = if let Some(date) = pricing_status.date {
            format!("{pricing_source} · {date}")
        } else {
            pricing_source.clone()
        };
        let clear_value = if self.paths.archive_db_file.exists() {
            copy.config.values.delete_archive_then_rebuild.clone()
        } else {
            copy.config.values.build_archive_from_history.clone()
        };
        let claude_limits_value = format!(
            "{} · {}",
            if self.paths.claude_code_limits_file.exists() {
                &copy.config.values.sidecar_found
            } else {
                &copy.config.values.sidecar_missing
            },
            self.paths.claude_code_limits_file.display()
        );
        let statusline_state = crate::tools::claude_code::statusline::detect()
            .ok()
            .map(|d| d.state);
        let statusline_value = render_statusline_value(copy, statusline_state.as_ref());
        let copilot_limits_value = format!(
            "{} · {}",
            if self.paths.copilot_limits_file.exists() {
                &copy.config.values.quota_snapshot_found
            } else {
                &copy.config.values.quota_snapshot_missing
            },
            self.paths.copilot_limits_file.display()
        );
        let claude_subscription_limits_value = format!(
            "{} · {}",
            if self.paths.claude_subscription_limits_file.exists() {
                &copy.config.values.quota_snapshot_found
            } else {
                &copy.config.values.quota_snapshot_missing
            },
            self.paths.claude_subscription_limits_file.display()
        );
        let codex_subscription_limits_value = format!(
            "{} · {}",
            if self.paths.codex_subscription_limits_file.exists() {
                &copy.config.values.quota_snapshot_found
            } else {
                &copy.config.values.quota_snapshot_missing
            },
            self.paths.codex_subscription_limits_file.display()
        );
        let advice_tool = AdviceTool::from_config(&self.settings.insights.advice_tool);
        let advice_tool_value = copy::template(
            if crate::advice::tool_available(advice_tool) {
                &copy.config.values.advice_tool_available
            } else {
                &copy.config.values.advice_tool_missing
            },
            &[("tool", advice_tool.label().to_string())],
        );
        let advice_prompt_status = crate::advice::prompt_file_status(&self.paths);
        let advice_prompts_value = if advice_prompt_status.ready {
            copy::template(
                &copy.config.values.advice_prompts_ready,
                &[("path", advice_prompt_status.dir.display().to_string())],
            )
        } else {
            copy::template(
                &copy.config.values.advice_prompts_missing,
                &[
                    ("files", advice_prompt_status.missing.join(", ")),
                    ("path", advice_prompt_status.dir.display().to_string()),
                ],
            )
        };

        vec![
            ConfigRowView {
                id: "currency_override",
                name: copy.config.rows.currency_override.name.as_str(),
                value: currency_value,
                action: copy.config.rows.currency_override.action.as_str(),
                links: Vec::new(),
            },
            ConfigRowView {
                id: "rates_json",
                name: copy.config.rows.rates_json.name.as_str(),
                value: rates_value,
                action: copy.config.rows.rates_json.action.as_str(),
                links: rates_download_links(),
            },
            ConfigRowView {
                id: "litellm_prices",
                name: copy.config.rows.litellm_prices.name.as_str(),
                value: pricing_value,
                action: copy.config.rows.litellm_prices.action.as_str(),
                links: pricing_download_links(),
            },
            ConfigRowView {
                id: "claude_statusline",
                name: copy.config.rows.claude_statusline.name.as_str(),
                value: statusline_value,
                action: copy.config.rows.claude_statusline.action.as_str(),
                links: Vec::new(),
            },
            ConfigRowView {
                id: "claude_limits",
                name: copy.config.rows.claude_limits.name.as_str(),
                value: claude_limits_value,
                action: copy.config.rows.claude_limits.action.as_str(),
                links: Vec::new(),
            },
            ConfigRowView {
                id: "copilot_limits",
                name: copy.config.rows.copilot_limits.name.as_str(),
                value: copilot_limits_value,
                action: copy.config.rows.copilot_limits.action.as_str(),
                links: Vec::new(),
            },
            ConfigRowView {
                id: "claude_subscription_limits",
                name: copy.config.rows.claude_subscription_limits.name.as_str(),
                value: claude_subscription_limits_value,
                action: copy.config.rows.claude_subscription_limits.action.as_str(),
                links: Vec::new(),
            },
            ConfigRowView {
                id: "codex_subscription_limits",
                name: copy.config.rows.codex_subscription_limits.name.as_str(),
                value: codex_subscription_limits_value,
                action: copy.config.rows.codex_subscription_limits.action.as_str(),
                links: Vec::new(),
            },
            ConfigRowView {
                id: "advice_tool",
                name: copy.config.rows.advice_tool.name.as_str(),
                value: advice_tool_value,
                action: copy.config.rows.advice_tool.action.as_str(),
                links: Vec::new(),
            },
            ConfigRowView {
                id: "advice_prompts",
                name: copy.config.rows.advice_prompts.name.as_str(),
                value: advice_prompts_value,
                action: copy.config.rows.advice_prompts.action.as_str(),
                links: Vec::new(),
            },
            ConfigRowView {
                id: "clear_data",
                name: copy.config.rows.clear_data.name.as_str(),
                value: clear_value,
                action: copy.config.rows.clear_data.action.as_str(),
                links: Vec::new(),
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

        if self.subscription_cookie_modal.is_some() {
            return keymap::CONTEXT_COOKIE_MODAL;
        }

        if let Some(clear_modal) = self.clear_data_modal {
            return match clear_modal {
                ClearDataModal::Confirm => keymap::CONTEXT_DOWNLOAD_CONFIRM,
                ClearDataModal::Running => keymap::CONTEXT_BUSY_MODAL,
            };
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
            Page::Insights => keymap::CONTEXT_INSIGHTS_PAGE,
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
            keymap::ACTION_NEXT_TAB => self.set_page(self.page.next_tab()),
            keymap::ACTION_PREV_TAB => self.set_page(self.page.prev_tab()),
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
            keymap::ACTION_PAGE_OVERVIEW => self.set_page(Page::Overview),
            keymap::ACTION_PAGE_DEEP_DIVE => self.set_page(Page::DeepDive),
            keymap::ACTION_PAGE_USAGE => self.set_page(Page::Usage),
            keymap::ACTION_PAGE_INSIGHTS => self.set_page(Page::Insights),
            keymap::ACTION_PAGE_CONFIG => self.set_page(Page::Config),
            keymap::ACTION_CLOSE_SESSION => self.leave_session(),
            keymap::ACTION_RELOAD => self.reload(),
            keymap::ACTION_INSIGHTS_NEXT_TAB => self.cycle_insights_tab(1),
            keymap::ACTION_INSIGHTS_PREV_TAB => self.cycle_insights_tab(-1),
            keymap::ACTION_GENERATE_ADVICE_REDACTED => {
                self.generate_advice_from_tui(AdviceDataScope::Redacted)
            }
            keymap::ACTION_GENERATE_ADVICE_SNIPPETS => {
                self.generate_advice_from_tui(AdviceDataScope::PromptSnippets)
            }
            keymap::ACTION_MOVE_UP => self.move_active_selection(-1),
            keymap::ACTION_MOVE_DOWN => self.move_active_selection(1),
            keymap::ACTION_MOVE_PAGE_UP => self.move_active_page(-10),
            keymap::ACTION_MOVE_PAGE_DOWN => self.move_active_page(10),
            keymap::ACTION_MOVE_HOME => self.move_active_to_start(),
            keymap::ACTION_MOVE_END => self.move_active_to_end(),
            keymap::ACTION_QUERY_BACKSPACE => self.backspace_active_query(),
            keymap::ACTION_QUERY_CLEAR => self.clear_active_query(),
            keymap::ACTION_GO_PARENT => self.go_active_parent(),
            keymap::ACTION_COOKIE_FIELD_NEXT => self.cycle_cookie_field(1),
            keymap::ACTION_COOKIE_FIELD_PREV => self.cycle_cookie_field(-1),
            keymap::ACTION_COOKIE_ACTION_NEXT => self.cycle_cookie_action(1),
            keymap::ACTION_COOKIE_ACTION_PREV => self.cycle_cookie_action(-1),
            _ => return false,
        }
        true
    }

    fn handle_text_input_key(&mut self, key: KeyEvent) -> bool {
        if let Some(modal) = self.export_modal.as_mut() {
            match key.code {
                KeyCode::Left | KeyCode::Right => {
                    let period = {
                        let delta = if matches!(key.code, KeyCode::Left) {
                            -1
                        } else {
                            1
                        };
                        modal.cycle_period(delta);
                        modal.period
                    };
                    let project_options = self.report_project_options(period);
                    if let Some(modal) = self.export_modal.as_mut() {
                        modal.project_options = project_options;
                        modal.project_selected = modal
                            .project_selected
                            .min(modal.project_options.len().saturating_sub(1));
                    }
                    return true;
                }
                KeyCode::Char('p') | KeyCode::Char('P') => {
                    if let Some(modal) = self.export_modal.as_mut() {
                        modal.cycle_project();
                    }
                    return true;
                }
                KeyCode::Char('x')
                | KeyCode::Char('X')
                | KeyCode::Char('r')
                | KeyCode::Char('R') => {
                    if let Some(modal) = self.export_modal.as_mut() {
                        modal.redacted = !modal.redacted;
                    }
                    return true;
                }
                _ => {}
            }
        }
        match key.code {
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.push_active_query_char(c)
            }
            _ => false,
        }
    }

    fn cancel_active_context(&mut self) {
        if self.subscription_cookie_modal.is_some() {
            self.close_subscription_cookie_modal();
        } else if self.currency_modal.is_some() {
            self.currency_modal = None;
        } else if self.clear_data_modal == Some(ClearDataModal::Confirm) {
            self.clear_data_modal = None;
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
        if self.subscription_cookie_modal.is_some() {
            self.confirm_subscription_cookie_modal();
        } else if self.clear_data_modal == Some(ClearDataModal::Confirm) {
            self.confirm_clear_data();
        } else if self.download_confirm.is_some() {
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
            Some(ConfigDownload::CopilotLimits) => self.sync_copilot_limits(),
            None => {}
        }
    }

    fn confirm_export_picker(&mut self) {
        let request = self.export_modal.as_ref().and_then(|modal| {
            modal
                .options
                .get(modal.selected)
                .copied()
                .map(|format| ReportRequest {
                    format,
                    period: modal.period,
                    scope: modal.scope(),
                    redacted: modal.redacted,
                })
        });
        self.export_modal = None;
        if let Some(request) = request {
            self.run_report(request);
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
            self.set_status(
                copy::template(
                    &copy().status.report_folder,
                    &[("path", self.export_dir.display().to_string())],
                ),
                StatusTone::Info,
            );
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
        } else if self.page == Page::Insights {
            self.scroll_insights(delta);
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
        } else if self.page == Page::Insights {
            self.scroll_insights(delta);
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
        } else if self.page == Page::Insights {
            self.set_insights_scroll(0);
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
        } else if self.page == Page::Insights {
            self.set_insights_scroll(self.insights_scroll_limit());
        } else if self.page == Page::Session {
            let row_count = self
                .session_view
                .as_ref()
                .map(|view| view.calls.len())
                .unwrap_or(0);
            self.select_session_call(row_count.saturating_sub(1));
        }
    }

    fn cycle_insights_tab(&mut self, delta: isize) {
        self.insights_tab = if delta.is_negative() {
            self.insights_tab.prev()
        } else {
            self.insights_tab.next()
        };
        self.clamp_insights_scroll();
    }

    pub fn handle_paste(&mut self, text: String) {
        if let Some(modal) = self.subscription_cookie_modal.as_mut() {
            modal.error = None;
            if let Some(field) = modal.focused_field_value_mut() {
                for ch in text.chars() {
                    if ch == '\r' || ch == '\n' || ch == '\t' {
                        continue;
                    }
                    if ch.is_control() {
                        continue;
                    }
                    field.push(ch);
                }
            }
            return;
        }
        // Fall back to per-character routing for other modals that accept text.
        for ch in text.chars() {
            if ch.is_control() {
                continue;
            }
            self.push_active_query_char(ch);
        }
    }

    fn cycle_cookie_field(&mut self, delta: isize) {
        if let Some(modal) = self.subscription_cookie_modal.as_mut() {
            modal.cycle_field(delta);
        }
    }

    fn cycle_cookie_action(&mut self, delta: isize) {
        if let Some(modal) = self.subscription_cookie_modal.as_mut() {
            modal.cycle_action(delta);
        }
    }

    fn open_subscription_cookie_modal(&mut self, kind: CookieModalKind) {
        #[cfg(feature = "quota-sync")]
        {
            let has_stored = crate::secrets::read(kind.keyring_account())
                .ok()
                .flatten()
                .is_some();
            self.subscription_cookie_modal = Some(SubscriptionCookieModal::new(kind, has_stored));
        }
        #[cfg(not(feature = "quota-sync"))]
        {
            let status = match kind {
                CookieModalKind::Claude => {
                    copy().status.claude_subscription_sync_unavailable.clone()
                }
                CookieModalKind::Codex => copy().status.codex_subscription_sync_unavailable.clone(),
            };
            self.set_status(status, StatusTone::Warning);
        }
    }

    fn close_subscription_cookie_modal(&mut self) {
        self.subscription_cookie_modal = None;
    }

    #[cfg(feature = "quota-sync")]
    fn confirm_subscription_cookie_modal(&mut self) {
        let Some(modal) = self.subscription_cookie_modal.as_ref() else {
            return;
        };
        if modal.busy {
            return;
        }
        let kind = modal.kind;
        let action = modal.action;
        match action {
            CookieAction::SaveAndSync => {
                if !modal.has_input() {
                    if let Some(modal) = self.subscription_cookie_modal.as_mut() {
                        modal.error =
                            Some(copy().modals.subscription_cookie.empty_value_error.clone());
                    }
                    return;
                }
                let cookie = modal.compose_cookie();
                if let Err(error) = crate::secrets::store(kind.keyring_account(), &cookie) {
                    let message = copy::template(
                        &copy().modals.subscription_cookie.save_failed,
                        &[("error", error.to_string())],
                    );
                    if let Some(modal) = self.subscription_cookie_modal.as_mut() {
                        modal.error = Some(message);
                    }
                    return;
                }
                if let Some(modal) = self.subscription_cookie_modal.as_mut() {
                    modal.has_stored = true;
                    modal.busy = true;
                    modal.error = None;
                    modal.primary.clear();
                    modal.shard_one.clear();
                    modal.extras.clear();
                }
                self.spawn_subscription_sync(kind);
            }
            CookieAction::SyncStored => {
                if !modal.has_stored {
                    return;
                }
                if let Some(modal) = self.subscription_cookie_modal.as_mut() {
                    modal.busy = true;
                    modal.error = None;
                }
                self.spawn_subscription_sync(kind);
            }
            CookieAction::Clear => {
                if !modal.has_stored {
                    return;
                }
                match crate::secrets::delete(kind.keyring_account()) {
                    Ok(()) => {
                        self.set_status(
                            copy().modals.subscription_cookie.cleared_status.clone(),
                            StatusTone::Info,
                        );
                        self.subscription_cookie_modal = None;
                    }
                    Err(error) => {
                        let message = copy::template(
                            &copy().modals.subscription_cookie.clear_failed,
                            &[("error", error.to_string())],
                        );
                        if let Some(modal) = self.subscription_cookie_modal.as_mut() {
                            modal.error = Some(message);
                        }
                    }
                }
            }
        }
    }

    #[cfg(not(feature = "quota-sync"))]
    fn confirm_subscription_cookie_modal(&mut self) {
        // Without the feature the modal cannot be opened, but keep the symbol
        // resolvable so the keymap dispatch table compiles uniformly.
        self.subscription_cookie_modal = None;
    }

    #[cfg(feature = "quota-sync")]
    fn spawn_subscription_sync(&mut self, kind: CookieModalKind) {
        if self.subscription_sync_job.is_some() {
            return;
        }
        let cookie = match crate::secrets::read(kind.keyring_account()) {
            Ok(Some(value)) => value,
            Ok(None) => {
                let status = match kind {
                    CookieModalKind::Claude => {
                        copy().status.claude_subscription_cookie_missing.clone()
                    }
                    CookieModalKind::Codex => {
                        copy().status.codex_subscription_cookie_missing.clone()
                    }
                };
                self.set_status(status, StatusTone::Warning);
                if let Some(modal) = self.subscription_cookie_modal.as_mut() {
                    modal.busy = false;
                }
                return;
            }
            Err(error) => {
                let template_str = match kind {
                    CookieModalKind::Claude => {
                        copy().status.claude_subscription_sync_failed.clone()
                    }
                    CookieModalKind::Codex => copy().status.codex_subscription_sync_failed.clone(),
                };
                self.set_status(
                    copy::template(&template_str, &[("error", error.to_string())]),
                    StatusTone::Error,
                );
                if let Some(modal) = self.subscription_cookie_modal.as_mut() {
                    modal.busy = false;
                }
                return;
            }
        };

        let paths = self.paths.clone();
        let (result_tx, result_rx) = mpsc::channel::<SubscriptionSyncResult>();
        thread::spawn(move || {
            let result = match kind {
                CookieModalKind::Claude => {
                    crate::tools::claude_subscription::limits::refresh_sidecar(
                        &paths.claude_subscription_limits_file,
                        &cookie,
                    )
                }
                CookieModalKind::Codex => {
                    crate::tools::codex_subscription::limits::refresh_sidecar(
                        &paths.codex_subscription_limits_file,
                        &cookie,
                    )
                }
            }
            .and_then(|snapshots| {
                crate::archive::sync_and_load(&paths).map(|ingested| (snapshots, ingested))
            })
            .map_err(|e| e.to_string());
            let _ = result_tx.send(result);
        });
        self.subscription_sync_job = Some(SubscriptionSyncJob { kind, result_rx });
        let status = match kind {
            CookieModalKind::Claude => copy().modals.subscription_cookie.busy.clone(),
            CookieModalKind::Codex => copy().modals.subscription_cookie.busy.clone(),
        };
        self.set_status(status, StatusTone::Busy);
    }

    #[cfg(feature = "quota-sync")]
    fn poll_subscription_sync(&mut self) {
        let result = match self
            .subscription_sync_job
            .as_ref()
            .map(|job| job.result_rx.try_recv())
        {
            Some(Ok(result)) => Some(result),
            Some(Err(TryRecvError::Disconnected)) => Some(Err(
                "subscription sync worker stopped before reporting".into(),
            )),
            Some(Err(TryRecvError::Empty)) | None => None,
        };

        let Some(result) = result else {
            return;
        };

        let kind = self
            .subscription_sync_job
            .as_ref()
            .map(|job| job.kind)
            .unwrap_or(CookieModalKind::Claude);
        self.subscription_sync_job = None;
        if let Some(modal) = self.subscription_cookie_modal.as_mut() {
            modal.busy = false;
        }

        match result {
            Ok((snapshots, ingested)) => {
                let limits = ingested.limits.len();
                self.apply_synced_archive(ingested);
                let template_str = match kind {
                    CookieModalKind::Claude => copy().status.claude_subscription_synced.clone(),
                    CookieModalKind::Codex => copy().status.codex_subscription_synced.clone(),
                };
                self.set_status(
                    copy::template(
                        &template_str,
                        &[
                            ("snapshots", snapshots.to_string()),
                            ("limits", limits.to_string()),
                        ],
                    ),
                    StatusTone::Success,
                );
                self.subscription_cookie_modal = None;
            }
            Err(error) => {
                let template_str = match kind {
                    CookieModalKind::Claude => {
                        copy().status.claude_subscription_sync_failed.clone()
                    }
                    CookieModalKind::Codex => copy().status.codex_subscription_sync_failed.clone(),
                };
                self.set_status(
                    copy::template(&template_str, &[("error", error.clone())]),
                    StatusTone::Error,
                );
                if let Some(modal) = self.subscription_cookie_modal.as_mut() {
                    modal.error = Some(error);
                }
            }
        }
    }

    #[cfg(not(feature = "quota-sync"))]
    fn poll_subscription_sync(&mut self) {}

    fn scroll_insights(&mut self, delta: isize) {
        let current = self.insights_scroll();
        let next = if delta.is_negative() {
            current.saturating_sub(delta.unsigned_abs())
        } else {
            current.saturating_add(delta as usize)
        };
        self.set_insights_scroll(next);
    }

    fn set_insights_scroll(&mut self, offset: usize) {
        let offset = offset.min(self.insights_scroll_limit());
        match self.insights_tab {
            InsightsTab::Advice => self.insights_advice_scroll = offset,
            InsightsTab::Signals => self.insights_signals_scroll = offset,
        }
    }

    fn clamp_insights_scroll(&mut self) {
        self.set_insights_scroll(self.insights_scroll());
    }

    fn insights_scroll_limit(&self) -> usize {
        let line_count = match self.insights_tab {
            InsightsTab::Advice => self.advice_line_count(),
            InsightsTab::Signals => self.signals_line_count(),
        };
        line_count.saturating_sub(1)
    }

    fn advice_line_count(&self) -> usize {
        if self.advice_history.runs.is_empty() {
            return 1;
        }
        self.advice_history
            .runs
            .iter()
            .map(|run| {
                1 + run
                    .summary
                    .as_ref()
                    .map(|summary| estimated_wrapped_lines(summary))
                    .unwrap_or(0)
                    + usize::from(run.status == "failed")
                    + if run.items.is_empty() {
                        1
                    } else {
                        run.items
                            .iter()
                            .map(|item| {
                                2 + estimated_wrapped_lines(&item.body)
                                    + estimated_wrapped_lines(&item.impact)
                                    + if item.evidence.is_empty() {
                                        0
                                    } else {
                                        estimated_wrapped_lines(&item.evidence.join(" · "))
                                    }
                                    + estimated_wrapped_lines(&item.next_step)
                            })
                            .sum::<usize>()
                    }
            })
            .sum()
    }

    fn signals_line_count(&self) -> usize {
        let recs = self.insights().recommendations;
        if recs.is_empty() {
            1
        } else {
            recs.iter()
                .map(|rec| {
                    2 + estimated_wrapped_lines(&rec.body)
                        + rec
                            .silenced_reason
                            .as_ref()
                            .map(|line| estimated_wrapped_lines(line))
                            .unwrap_or(0)
                        + rec
                            .assumption
                            .as_ref()
                            .map(|line| estimated_wrapped_lines(line))
                            .unwrap_or(0)
                })
                .sum()
        }
    }

    fn backspace_active_query(&mut self) {
        if let Some(modal) = self.subscription_cookie_modal.as_mut() {
            modal.error = None;
            if let Some(field) = modal.focused_field_value_mut() {
                field.pop();
            }
            return;
        }
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
        if let Some(modal) = self.subscription_cookie_modal.as_mut() {
            modal.error = None;
            if let Some(field) = modal.focused_field_value_mut() {
                field.clear();
            }
            return;
        }
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
        if let Some(modal) = self.subscription_cookie_modal.as_mut() {
            modal.error = None;
            if let Some(field) = modal.focused_field_value_mut() {
                field.push(c);
                return true;
            }
            return false;
        }
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
        match mouse.kind {
            MouseEventKind::ScrollUp if self.page == Page::Insights => {
                self.scroll_insights(-3);
                return;
            }
            MouseEventKind::ScrollDown if self.page == Page::Insights => {
                self.scroll_insights(3);
                return;
            }
            MouseEventKind::Down(MouseButton::Left) => {}
            _ => return,
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

    pub fn report_project_options(&self, period: Period) -> Vec<ProjectOption> {
        let currency = self.currency();
        match &self.source {
            DataSource::Live(ingested) => {
                ingested.project_options(period, Tool::All, SortMode::Spend, &currency)
            }
            DataSource::Sample => {
                crate::data::project_options(period, Tool::All, SortMode::Spend, &currency)
            }
        }
    }

    pub fn set_tool(&mut self, tool: Tool) {
        self.tool = tool;
    }

    pub fn set_page(&mut self, page: Page) {
        self.page = page;
        if page == Page::Usage {
            self.period = Period::Today;
        }
    }

    pub fn set_period(&mut self, period: Period) {
        self.period = period;
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
                    self.set_status(
                        copy::template(
                            &copy().status.project_not_found,
                            &[("identity", identity.to_string())],
                        ),
                        StatusTone::Warning,
                    );
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
        match self
            .config_rows()
            .get(self.config_selected)
            .map(|row| row.id)
        {
            Some("currency_override") => self.open_currency_modal(),
            Some("rates_json") => self.open_download_confirm(ConfigDownload::CurrencyRates),
            Some("litellm_prices") => self.open_download_confirm(ConfigDownload::PricingSnapshot),
            Some("claude_statusline") => self.install_claude_statusline(),
            Some("claude_limits") => self.sync_claude_limits(),
            Some("copilot_limits") => self.open_download_confirm(ConfigDownload::CopilotLimits),
            Some("claude_subscription_limits") => {
                self.open_subscription_cookie_modal(CookieModalKind::Claude)
            }
            Some("codex_subscription_limits") => {
                self.open_subscription_cookie_modal(CookieModalKind::Codex)
            }
            Some("advice_tool") => self.cycle_advice_tool(),
            Some("advice_prompts") => self.prepare_advice_prompts(),
            Some("clear_data") => self.open_clear_data_confirm(),
            _ => {}
        }
    }

    fn open_download_confirm(&mut self, target: ConfigDownload) {
        self.download_confirm = Some(target);
    }

    fn open_clear_data_confirm(&mut self) {
        if self.clear_data_job.is_none() {
            self.clear_data_modal = Some(ClearDataModal::Confirm);
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

    pub fn set_currency(&mut self, code: &str) {
        self.settings.set_currency(code);
        match self.settings.save(&self.paths) {
            Ok(()) => {
                self.set_status(
                    copy::template(
                        &copy().status.currency_set,
                        &[("code", self.currency().code().to_string())],
                    ),
                    StatusTone::Success,
                );
            }
            Err(e) => {
                self.set_status(
                    copy::template(
                        &copy().status.config_save_failed,
                        &[("error", e.to_string())],
                    ),
                    StatusTone::Error,
                );
            }
        }
    }

    pub fn set_advice_tool(&mut self, tool: AdviceTool) {
        self.settings.insights.advice_tool = tool.id().to_string();
        match self.settings.save(&self.paths) {
            Ok(()) => self.set_status(
                copy::template(
                    &copy().status.advice_tool_set,
                    &[("tool", tool.label().to_string())],
                ),
                StatusTone::Success,
            ),
            Err(e) => self.set_status(
                copy::template(
                    &copy().status.advice_tool_config_failed,
                    &[("error", e.to_string())],
                ),
                StatusTone::Error,
            ),
        }
    }

    fn cycle_advice_tool(&mut self) {
        let current = AdviceTool::from_config(&self.settings.insights.advice_tool);
        let next = AdviceTool::ALL
            .iter()
            .position(|tool| *tool == current)
            .map(|idx| AdviceTool::ALL[(idx + 1) % AdviceTool::ALL.len()])
            .unwrap_or(AdviceTool::Codex);
        self.set_advice_tool(next);
    }

    pub fn prepare_advice_prompts(&mut self) {
        match crate::advice::ensure_prompt_files(&self.paths) {
            Ok(()) => {
                let status = crate::advice::prompt_file_status(&self.paths);
                self.set_status(
                    copy::template(
                        &copy().config.values.advice_prompts_ready,
                        &[("path", status.dir.display().to_string())],
                    ),
                    StatusTone::Success,
                );
            }
            Err(e) => self.set_status(
                copy::template(&copy().status.advice_failed, &[("error", e.to_string())]),
                StatusTone::Error,
            ),
        }
    }

    pub fn generate_advice(&mut self, data_scope: AdviceDataScope) -> Result<(), String> {
        if self.advice_job.is_some() {
            self.set_status(
                copy().status.advice_already_running.clone(),
                StatusTone::Busy,
            );
            return Ok(());
        }

        let ingested = match &self.source {
            DataSource::Live(ingested) => ingested.clone(),
            DataSource::Sample => {
                self.set_status(
                    copy().status.advice_requires_live_data.clone(),
                    StatusTone::Warning,
                );
                return Ok(());
            }
        };

        let tool = AdviceTool::from_config(&self.settings.insights.advice_tool);
        self.set_status(
            copy::template(
                &copy().status.advice_running,
                &[("tool", tool.label().to_string())],
            ),
            StatusTone::Busy,
        );

        let paths = self.paths.clone();
        let (result_tx, result_rx) = mpsc::channel::<AdviceJobResult>();
        thread::spawn(move || {
            let result = run_advice_job(ingested, paths, tool, data_scope);
            let _ = result_tx.send(result);
        });
        self.advice_job = Some(AdviceJob { result_rx });
        Ok(())
    }

    fn generate_advice_from_tui(&mut self, data_scope: AdviceDataScope) {
        if let Err(error) = self.generate_advice(data_scope) {
            self.set_status(
                copy::template(&copy().status.advice_failed, &[("error", error)]),
                StatusTone::Error,
            );
        } else {
            self.set_insights_tab(InsightsTab::Advice);
            self.set_insights_scroll(0);
        }
    }

    pub fn update_advice_item_status(
        &mut self,
        item_id: i64,
        status: AdviceItemStatus,
        notes: Option<String>,
    ) -> Result<(), String> {
        let mut archive = archive::Archive::open(&self.paths).map_err(|e| e.to_string())?;
        match archive.update_advice_item_status(item_id, status, notes) {
            Ok(true) => {
                self.refresh_advice_history();
                self.set_status(
                    copy().status.advice_item_updated.clone(),
                    StatusTone::Success,
                );
                Ok(())
            }
            Ok(false) => {
                self.set_status(
                    copy::template(
                        &copy().status.advice_item_not_found,
                        &[("id", item_id.to_string())],
                    ),
                    StatusTone::Warning,
                );
                Ok(())
            }
            Err(e) => Err(e.to_string()),
        }
    }

    pub fn install_claude_statusline(&mut self) {
        match crate::tools::claude_code::statusline::install() {
            Ok(report) => {
                if report.already_installed {
                    self.set_status(
                        copy().status.claude_statusline_already_installed.clone(),
                        StatusTone::Info,
                    );
                } else if let Some(inner) = &report.previous_inner {
                    self.set_status(
                        copy::template(
                            &copy().status.claude_statusline_installed_wrapping,
                            &[("inner", inner.clone())],
                        ),
                        StatusTone::Success,
                    );
                } else {
                    self.set_status(
                        copy::template(
                            &copy().status.claude_statusline_installed,
                            &[("path", report.wrapper_path.display().to_string())],
                        ),
                        StatusTone::Success,
                    );
                }
            }
            Err(e) => self.set_status(
                copy::template(
                    &copy().status.claude_statusline_failed,
                    &[("error", e.to_string())],
                ),
                StatusTone::Error,
            ),
        }
    }

    pub fn install_claude_statusline_manual(&mut self) {
        match crate::tools::claude_code::statusline::install_manual() {
            Ok(path) => self.set_status(
                copy::template(
                    &copy().status.claude_statusline_installed_manual,
                    &[("path", path.display().to_string())],
                ),
                StatusTone::Success,
            ),
            Err(e) => self.set_status(
                copy::template(
                    &copy().status.claude_statusline_failed,
                    &[("error", e.to_string())],
                ),
                StatusTone::Error,
            ),
        }
    }

    pub fn uninstall_claude_statusline(&mut self) {
        match crate::tools::claude_code::statusline::uninstall() {
            Ok(_) => self.set_status(
                copy().status.claude_statusline_uninstalled.clone(),
                StatusTone::Success,
            ),
            Err(e) => self.set_status(
                copy::template(
                    &copy().status.claude_statusline_failed,
                    &[("error", e.to_string())],
                ),
                StatusTone::Error,
            ),
        }
    }

    pub fn sync_claude_limits(&mut self) {
        if !self.paths.claude_code_limits_file.exists() {
            self.set_status(
                copy::template(
                    &copy().status.claude_limits_sidecar_missing,
                    &[(
                        "path",
                        self.paths.claude_code_limits_file.display().to_string(),
                    )],
                ),
                StatusTone::Warning,
            );
            return;
        }

        match crate::archive::sync_and_load(&self.paths) {
            Ok(ingested) => {
                let limits = ingested.limits.len();
                self.apply_synced_archive(ingested);
                self.set_status(
                    copy::template(
                        &copy().status.claude_limits_synced,
                        &[("limits", limits.to_string())],
                    ),
                    StatusTone::Success,
                );
            }
            Err(e) => self.set_status(
                copy::template(
                    &copy().status.reload_failed_prior_data_kept,
                    &[("error", e.to_string())],
                ),
                StatusTone::Error,
            ),
        }
    }

    #[cfg(feature = "quota-sync")]
    pub fn sync_copilot_limits(&mut self) {
        match crate::tools::copilot::limits::refresh_sidecar(&self.paths.copilot_limits_file)
            .and_then(|snapshots| {
                crate::archive::sync_and_load(&self.paths).map(|ingested| (snapshots, ingested))
            }) {
            Ok((snapshots, ingested)) => {
                let limits = ingested.limits.len();
                self.apply_synced_archive(ingested);
                self.set_status(
                    copy::template(
                        &copy().status.copilot_limits_synced,
                        &[
                            ("snapshots", snapshots.to_string()),
                            ("limits", limits.to_string()),
                        ],
                    ),
                    StatusTone::Success,
                );
            }
            Err(e) => self.set_status(
                copy::template(
                    &copy().status.copilot_limits_sync_failed,
                    &[("error", e.to_string())],
                ),
                StatusTone::Error,
            ),
        }
    }

    #[cfg(not(feature = "quota-sync"))]
    pub fn sync_copilot_limits(&mut self) {
        self.set_status(
            copy().status.copilot_limits_sync_unavailable.clone(),
            StatusTone::Warning,
        );
    }

    #[cfg(feature = "quota-sync")]
    pub fn sync_claude_subscription_limits(&mut self) {
        let session_key = match crate::secrets::read(
            crate::tools::claude_subscription::config::KEYRING_ACCOUNT,
        ) {
            Ok(Some(value)) => value,
            Ok(None) => {
                self.set_status(
                    copy().status.claude_subscription_cookie_missing.clone(),
                    StatusTone::Warning,
                );
                return;
            }
            Err(e) => {
                self.set_status(
                    copy::template(
                        &copy().status.claude_subscription_sync_failed,
                        &[("error", e.to_string())],
                    ),
                    StatusTone::Error,
                );
                return;
            }
        };
        match crate::tools::claude_subscription::limits::refresh_sidecar(
            &self.paths.claude_subscription_limits_file,
            &session_key,
        )
        .and_then(|snapshots| {
            crate::archive::sync_and_load(&self.paths).map(|ingested| (snapshots, ingested))
        }) {
            Ok((snapshots, ingested)) => {
                let limits = ingested.limits.len();
                self.apply_synced_archive(ingested);
                self.set_status(
                    copy::template(
                        &copy().status.claude_subscription_synced,
                        &[
                            ("snapshots", snapshots.to_string()),
                            ("limits", limits.to_string()),
                        ],
                    ),
                    StatusTone::Success,
                );
            }
            Err(e) => self.set_status(
                copy::template(
                    &copy().status.claude_subscription_sync_failed,
                    &[("error", e.to_string())],
                ),
                StatusTone::Error,
            ),
        }
    }

    #[cfg(not(feature = "quota-sync"))]
    pub fn sync_claude_subscription_limits(&mut self) {
        self.set_status(
            copy().status.claude_subscription_sync_unavailable.clone(),
            StatusTone::Warning,
        );
    }

    #[cfg(feature = "quota-sync")]
    pub fn sync_codex_subscription_limits(&mut self) {
        let session_token =
            match crate::secrets::read(crate::tools::codex_subscription::config::KEYRING_ACCOUNT) {
                Ok(Some(value)) => value,
                Ok(None) => {
                    self.set_status(
                        copy().status.codex_subscription_cookie_missing.clone(),
                        StatusTone::Warning,
                    );
                    return;
                }
                Err(e) => {
                    self.set_status(
                        copy::template(
                            &copy().status.codex_subscription_sync_failed,
                            &[("error", e.to_string())],
                        ),
                        StatusTone::Error,
                    );
                    return;
                }
            };
        match crate::tools::codex_subscription::limits::refresh_sidecar(
            &self.paths.codex_subscription_limits_file,
            &session_token,
        )
        .and_then(|snapshots| {
            crate::archive::sync_and_load(&self.paths).map(|ingested| (snapshots, ingested))
        }) {
            Ok((snapshots, ingested)) => {
                let limits = ingested.limits.len();
                self.apply_synced_archive(ingested);
                self.set_status(
                    copy::template(
                        &copy().status.codex_subscription_synced,
                        &[
                            ("snapshots", snapshots.to_string()),
                            ("limits", limits.to_string()),
                        ],
                    ),
                    StatusTone::Success,
                );
            }
            Err(e) => self.set_status(
                copy::template(
                    &copy().status.codex_subscription_sync_failed,
                    &[("error", e.to_string())],
                ),
                StatusTone::Error,
            ),
        }
    }

    #[cfg(not(feature = "quota-sync"))]
    pub fn sync_codex_subscription_limits(&mut self) {
        self.set_status(
            copy().status.codex_subscription_sync_unavailable.clone(),
            StatusTone::Warning,
        );
    }

    fn confirm_clear_data(&mut self) {
        self.start_clear_data();
    }

    pub fn clear_data(&mut self) {
        self.refresher = None;
        let result = crate::archive::reset_and_load(&self.paths).map_err(|e| e.to_string());
        self.apply_clear_data_result(result);
    }

    fn start_clear_data(&mut self) {
        if self.clear_data_job.is_some() {
            return;
        }

        self.refresher = None;
        self.clear_data_modal = Some(ClearDataModal::Running);
        self.clear_data_tick = 0;
        self.set_status(
            copy().status.clearing_data_reimporting.clone(),
            StatusTone::Busy,
        );
        let paths = self.paths.clone();
        let (result_tx, result_rx) = mpsc::channel::<ClearDataResult>();
        std::thread::spawn(move || {
            let result = crate::archive::reset_and_load(&paths).map_err(|e| e.to_string());
            let _ = result_tx.send(result);
        });
        self.clear_data_job = Some(ClearDataJob { result_rx });
    }

    fn apply_clear_data_result(&mut self, result: ClearDataResult) {
        match result {
            Ok((ingested, _stats)) => {
                let calls = ingested.calls.len();
                let limits = ingested.limits.len();
                self.project_filter = ProjectFilter::All;
                self.session_view = None;
                self.session_scroll = 0;
                self.session_selected = 0;
                self.call_detail_index = None;
                self.background_alert_last_sent = None;
                self.background_alerts.clear();
                self.sample_forced = false;

                if ingested.is_empty() {
                    self.source = DataSource::Sample;
                    self.live_source = None;
                    self.background_alert_baseline = None;
                    self.set_status(
                        copy().status.data_cleared_no_sessions_sample_data.clone(),
                        StatusTone::Warning,
                    );
                } else {
                    self.background_alert_baseline = Some(UsageTotals::from_ingested(&ingested));
                    self.source = DataSource::Live(ingested);
                    self.live_source = None;
                    self.set_status(
                        copy::template(
                            &copy().status.data_cleared_counts,
                            &[("calls", calls.to_string()), ("limits", limits.to_string())],
                        ),
                        StatusTone::Success,
                    );
                }

                self.refresher = Some(Refresher::spawn(
                    archive::SYNC_INTERVAL,
                    RefreshSource::Archive(Box::new(self.paths.clone())),
                ));
            }
            Err(e) => {
                self.set_status(
                    copy::template(&copy().status.clear_data_failed, &[("error", e)]),
                    StatusTone::Error,
                );
                self.refresher = Some(Refresher::spawn(
                    archive::SYNC_INTERVAL,
                    RefreshSource::Archive(Box::new(self.paths.clone())),
                ));
            }
        }
        self.refresh_session_view();
    }

    fn refresh_advice_history(&mut self) {
        self.advice_history = load_advice_history(&self.paths);
    }

    fn apply_advice_job_result(&mut self, outcome: AdviceJobOutcome) {
        if let Some(ingested) = outcome.ingested {
            self.apply_synced_archive(ingested);
        }
        self.refresh_advice_history();
        self.set_insights_tab(InsightsTab::Advice);
        self.set_insights_scroll(0);

        if let Some(error) = outcome.run_error {
            self.set_status(
                copy::template(&copy().status.advice_failed, &[("error", error)]),
                StatusTone::Error,
            );
        } else {
            self.set_status(
                copy::template(
                    &copy().status.advice_run_saved,
                    &[
                        ("items", outcome.item_count.to_string()),
                        ("usage_sync", outcome.usage_sync_status),
                    ],
                ),
                StatusTone::Success,
            );
        }
    }

    fn apply_synced_archive(&mut self, ingested: Ingested) {
        self.background_alert_baseline =
            (!ingested.is_empty()).then(|| UsageTotals::from_ingested(&ingested));
        if ingested.is_empty() {
            if !self.sample_forced {
                self.source = DataSource::Sample;
                self.live_source = None;
            }
        } else if self.sample_forced {
            self.live_source = Some(ingested);
        } else {
            self.source = DataSource::Live(ingested);
            self.live_source = None;
        }
        self.refresh_session_view();
    }

    #[cfg(feature = "refresh-currency")]
    pub fn refresh_currency_rates(&mut self) {
        match crate::currency::refresh::download_published_snapshot(&self.paths.currency_rates_file)
            .and_then(|_| CurrencyTable::load(&self.paths))
        {
            Ok(table) => {
                self.currency_table = table;
                self.set_status(
                    copy::template(
                        &copy().status.rates_refreshed,
                        &[("date", self.currency_table.date().to_string())],
                    ),
                    StatusTone::Success,
                );
            }
            Err(e) => {
                self.set_status(
                    copy::template(
                        &copy().status.rates_refresh_failed,
                        &[("error", e.to_string())],
                    ),
                    StatusTone::Error,
                );
            }
        }
    }

    #[cfg(not(feature = "refresh-currency"))]
    pub fn refresh_currency_rates(&mut self) {
        self.set_status(
            copy().status.rates_download_unavailable.clone(),
            StatusTone::Warning,
        );
    }

    #[cfg(feature = "refresh-prices")]
    pub fn refresh_pricing_snapshot(&mut self) {
        match crate::pricing::refresh::download_published_books(&self.paths)
            .map_err(|e| e.to_string())
            .and_then(|_| crate::pricing::PriceTable::reload_configured())
        {
            Ok(()) => {
                self.set_status(
                    copy().status.litellm_prices_refreshed.clone(),
                    StatusTone::Success,
                );
            }
            Err(e) => {
                self.set_status(
                    copy::template(
                        &copy().status.litellm_refresh_failed,
                        &[("error", e.to_string())],
                    ),
                    StatusTone::Error,
                );
            }
        }
    }

    #[cfg(not(feature = "refresh-prices"))]
    pub fn refresh_pricing_snapshot(&mut self) {
        self.set_status(
            copy().status.litellm_download_unavailable.clone(),
            StatusTone::Warning,
        );
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
            cache_read_rate: "10%".into(),
            cache_write_rate: "-".into(),
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

        app.handle_key(key(KeyCode::Char('t')));
        app.handle_key(key(KeyCode::Char('t')));
        app.handle_key(key(KeyCode::Char('t')));
        app.handle_key(key(KeyCode::Char('t')));
        assert_eq!(app.tool, Tool::Gemini);

        app.handle_key(key(KeyCode::Char('t')));
        assert_eq!(app.tool, Tool::All);
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
        app.set_period(Period::AllTime);
        assert_eq!(app.page, Page::Overview);

        app.handle_key(key(KeyCode::Char('u')));
        assert_eq!(app.page, Page::Usage);
        assert_eq!(app.period, Period::Today);

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
        app.set_period(Period::AllTime);
        assert_eq!(app.page, Page::Overview);

        app.handle_key(key(KeyCode::Tab));
        assert_eq!(app.page, Page::DeepDive);
        app.handle_key(key(KeyCode::Tab));
        assert_eq!(app.page, Page::Usage);
        assert_eq!(app.period, Period::Today);
        app.handle_key(key(KeyCode::Tab));
        assert_eq!(app.page, Page::Insights);
        app.handle_key(key(KeyCode::Tab));
        assert_eq!(app.page, Page::Overview);

        app.set_period(Period::AllTime);
        app.handle_key(key(KeyCode::BackTab));
        assert_eq!(app.page, Page::Insights);
        app.handle_key(key(KeyCode::BackTab));
        assert_eq!(app.page, Page::Usage);
        assert_eq!(app.period, Period::Today);
    }

    #[test]
    fn insights_tab_scroll_and_generate_keys_are_manual() {
        let mut app = App::default();
        app.set_page(Page::Insights);
        assert_eq!(app.insights_tab(), InsightsTab::Advice);

        app.handle_key(key(KeyCode::Right));
        assert_eq!(app.insights_tab(), InsightsTab::Signals);

        app.handle_key(key(KeyCode::Down));
        assert_eq!(app.insights_scroll(), 1);

        app.handle_key(key(KeyCode::PageDown));
        assert!(app.insights_scroll() > 1);
        let scrolled_by_key = app.insights_scroll();

        app.handle_mouse(
            MouseEvent {
                kind: MouseEventKind::ScrollDown,
                column: 20,
                row: 20,
                modifiers: KeyModifiers::NONE,
            },
            Rect::new(0, 0, 170, 64),
        );
        assert!(app.insights_scroll() > scrolled_by_key);

        app.handle_key(key(KeyCode::Left));
        assert_eq!(app.insights_tab(), InsightsTab::Advice);

        app.handle_key(key(KeyCode::Char('a')));
        assert_eq!(
            app.status_text(),
            Some(copy().status.advice_requires_live_data.as_str())
        );

        app.handle_key(shift_key(KeyCode::Char('A')));
        assert_eq!(
            app.status_text(),
            Some(copy().status.advice_requires_live_data.as_str())
        );
    }

    #[test]
    fn advice_job_completion_updates_status_after_poll() {
        let (result_tx, result_rx) = mpsc::channel::<AdviceJobResult>();
        result_tx
            .send(Ok(AdviceJobOutcome {
                item_count: 2,
                run_error: None,
                usage_sync_status: "1 calls · 0 limits".into(),
                ingested: None,
            }))
            .unwrap();
        let mut app = App {
            page: Page::Insights,
            paths: ConfigPaths::new(tempdir("advice-job")),
            advice_job: Some(AdviceJob { result_rx }),
            status: Some(AppStatus::new("Generating advice", StatusTone::Busy)),
            ..App::default()
        };

        app.poll_reload();

        assert!(!app.advice_running());
        assert_eq!(app.status_tone(), StatusTone::Success);
        assert_eq!(
            app.status_text(),
            Some("Advice saved · 2 items · usage 1 calls · 0 limits")
        );
    }

    #[test]
    fn direct_keys_jump_between_tabs() {
        let mut app = App::default();
        app.set_period(Period::AllTime);

        app.handle_key(key(KeyCode::Char('d')));
        assert_eq!(app.page, Page::DeepDive);

        app.handle_key(key(KeyCode::Char('u')));
        assert_eq!(app.page, Page::Usage);
        assert_eq!(app.period, Period::Today);

        app.handle_key(key(KeyCode::Char('o')));
        assert_eq!(app.page, Page::Overview);
    }

    #[test]
    fn set_page_usage_selects_rolling_24_hour_period() {
        let mut app = App::default();

        app.set_period(Period::Month);
        app.set_page(Page::Usage);
        assert_eq!(app.page, Page::Usage);
        assert_eq!(app.period, Period::Today);

        app.set_period(Period::AllTime);
        app.set_page(Page::DeepDive);
        assert_eq!(app.page, Page::DeepDive);
        assert_eq!(app.period, Period::AllTime);
    }

    #[test]
    fn shift_d_toggles_live_and_sample_data() {
        let mut app = App::with_source(DataSource::Live(ingested_with_calls(1)), None);

        app.handle_key(shift_key(KeyCode::Char('D')));
        assert!(matches!(app.source, DataSource::Sample));
        assert!(app.sample_forced);
        assert_eq!(app.status_text(), Some(copy().status.sample_data.as_str()));

        app.handle_key(shift_key(KeyCode::Char('D')));
        assert!(matches!(app.source, DataSource::Live(_)));
        assert!(!app.sample_forced);
        assert_eq!(app.status_text(), Some(copy().status.live_data.as_str()));
    }

    #[test]
    fn shift_d_without_live_data_keeps_sample_fallback() {
        let mut app = App::default();

        app.handle_key(shift_key(KeyCode::Char('D')));

        assert!(matches!(app.source, DataSource::Sample));
        assert!(!app.sample_forced);
        assert_eq!(
            app.status_text(),
            Some(copy().status.no_local_sessions_sample_data.as_str())
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
            period: Period::AllTime,
            project_filter: ProjectFilter::Selected {
                identity: "missing".into(),
                label: "missing".into(),
            },
            ..App::default()
        };

        app.handle_key(key(KeyCode::Char('u')));
        let data = app.usage();

        assert_eq!(app.page, Page::Usage);
        assert_eq!(app.period, Period::Today);
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
        let app = App {
            paths: ConfigPaths::new(tempdir("config-rows")),
            ..App::default()
        };
        let rows = app.config_rows();

        assert_eq!(rows[1].id, "rates_json");
        assert_eq!(rows[1].name, copy().config.rows.rates_json.name);
        assert_eq!(rows[1].action, copy().config.rows.rates_json.action);
        assert_eq!(rows[1].links.len(), 1);
        assert_eq!(rows[1].links[0].url, crate::config::CURRENCY_RATES_URL);
        assert_eq!(rows[2].id, "litellm_prices");
        assert_eq!(rows[2].name, copy().config.rows.litellm_prices.name);
        assert_eq!(rows[2].action, copy().config.rows.litellm_prices.action);
        assert!(rows[2]
            .value
            .starts_with(&copy().config.values.embedded_snapshot));
        assert!(rows[2].value.contains(" · "));
        assert_eq!(rows[2].links.len(), 2);
        assert_eq!(rows[3].id, "claude_statusline");
        assert_eq!(rows[3].name, copy().config.rows.claude_statusline.name);
        assert_eq!(rows[3].action, copy().config.rows.claude_statusline.action);
        assert_eq!(rows[4].id, "claude_limits");
        assert_eq!(rows[4].name, copy().config.rows.claude_limits.name);
        assert_eq!(rows[4].action, copy().config.rows.claude_limits.action);
        assert!(rows[4]
            .value
            .starts_with(&copy().config.values.sidecar_missing));
        assert_eq!(rows[5].id, "copilot_limits");
        assert_eq!(rows[5].name, copy().config.rows.copilot_limits.name);
        assert_eq!(rows[5].action, copy().config.rows.copilot_limits.action);
        assert!(rows[5]
            .value
            .starts_with(&copy().config.values.quota_snapshot_missing));
        assert_eq!(rows[6].id, "claude_subscription_limits");
        assert_eq!(
            rows[6].name,
            copy().config.rows.claude_subscription_limits.name
        );
        assert_eq!(
            rows[6].action,
            copy().config.rows.claude_subscription_limits.action
        );
        assert!(rows[6]
            .value
            .starts_with(&copy().config.values.quota_snapshot_missing));
        assert_eq!(rows[7].id, "codex_subscription_limits");
        assert_eq!(
            rows[7].name,
            copy().config.rows.codex_subscription_limits.name
        );
        assert_eq!(
            rows[7].action,
            copy().config.rows.codex_subscription_limits.action
        );
        assert_eq!(rows[8].id, "advice_tool");
        assert_eq!(rows[8].name, copy().config.rows.advice_tool.name);
        assert_eq!(rows[8].action, copy().config.rows.advice_tool.action);
        assert!(rows[8].value.contains("Codex"));
        assert_eq!(rows[9].id, "advice_prompts");
        assert_eq!(rows[9].name, copy().config.rows.advice_prompts.name);
        assert_eq!(rows[9].action, copy().config.rows.advice_prompts.action);
        assert_eq!(rows[10].id, "clear_data");
        assert_eq!(rows[10].name, copy().config.rows.clear_data.name);
        assert_eq!(rows[10].action, copy().config.rows.clear_data.action);
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

    #[test]
    fn config_clear_data_confirmation_opens_and_cancels() {
        let mut app = App::default();

        app.handle_key(key(KeyCode::Char('c')));
        app.handle_key(key(KeyCode::End));
        app.handle_key(key(KeyCode::Enter));

        assert_eq!(app.clear_data_modal, Some(ClearDataModal::Confirm));

        app.handle_key(key(KeyCode::Char('q')));
        assert_eq!(app.clear_data_modal, Some(ClearDataModal::Confirm));
        assert!(!app.should_quit());

        app.handle_key(key(KeyCode::Char('n')));
        assert_eq!(app.clear_data_modal, None);
        assert_eq!(app.status, None);
    }

    #[test]
    fn clear_data_statuses_have_distinct_tones() {
        let mut app = App {
            status: Some(AppStatus::new(
                copy().status.clearing_data_reimporting.clone(),
                StatusTone::Busy,
            )),
            ..Default::default()
        };
        assert_eq!(app.status_tone(), StatusTone::Busy);

        app.status = Some(AppStatus::new(
            copy::template(
                &copy().status.data_cleared_counts,
                &[("calls", "10".into()), ("limits", "2".into())],
            ),
            StatusTone::Success,
        ));
        assert_eq!(app.status_tone(), StatusTone::Success);

        app.status = Some(AppStatus::new(
            copy::template(
                &copy().status.clear_data_failed,
                &[("error", "locked".into())],
            ),
            StatusTone::Error,
        ));
        assert_eq!(app.status_tone(), StatusTone::Error);
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
            app.status_text(),
            Some(copy().status.rates_download_unavailable.as_str())
        );
    }

    #[test]
    fn project_modal_selects_project() {
        let mut app = App::default();

        app.handle_key(key(KeyCode::Char('p')));
        assert!(app.project_modal.is_some());
        assert_eq!(
            app.project_modal.as_ref().unwrap().options[0].label,
            copy().tools.all
        );

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
    fn report_writes_to_selected_session_destination() {
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
            .is_some_and(|ext| ext == "html"));
        assert!(app.export_modal.is_none());
        assert!(app
            .status_text()
            .is_some_and(|status| status.contains("Report generated")));
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
            status: Some(AppStatus::info("untouched")),
            ..App::default()
        };
        app.reload();
        assert_eq!(app.status_text(), Some("untouched"));
        app.poll_reload();
        assert_eq!(app.status_text(), Some("untouched"));
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
            app.status_text(),
            Some(copy().status.reload_no_sessions_prior_data_kept.as_str())
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
            app.status_text(),
            Some(
                copy()
                    .status
                    .auto_refresh_no_sessions_prior_data_kept
                    .as_str()
            )
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
