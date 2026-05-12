use std::collections::{BTreeMap, HashSet};
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CopyDeck {
    pub brand: BrandCopy,
    pub nav: NavCopy,
    pub periods: PeriodCopy,
    pub sorts: SortCopy,
    pub tools: ToolCopy,
    pub metrics: MetricCopy,
    pub filters: FilterCopy,
    pub panels: PanelCopy,
    pub tables: TableCopy,
    pub timeline: TimelineCopy,
    pub usage: UsageCopy,
    pub config: ConfigCopy,
    pub session: SessionCopy,
    pub modals: ModalCopy,
    pub actions: ActionCopy,
    pub desktop: DesktopCopy,
    pub updates: UpdateCopy,
    pub tray: TrayCopy,
    pub empty: EmptyCopy,
    pub export: ExportCopy,
    pub reports: ReportCopy,
    pub report_cli: ReportCliCopy,
    pub cli: CliCopy,
    pub insights: InsightsCopy,
    pub keymap: KeymapCopy,
    pub status: StatusCopy,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct InsightsCopy {
    pub title: String,
    pub subtitle: String,
    pub signals_title: String,
    pub advice_title: String,
    pub advice_empty: String,
    pub advice_failed: String,
    pub advice_scope_title: String,
    pub advice_scope_redacted: String,
    pub advice_scope_redacted_detail: String,
    pub advice_scope_snippets: String,
    pub advice_scope_snippets_detail: String,
    pub advice_generate_button: String,
    pub advice_scope_running: String,
    pub advice_latest: String,
    pub advice_run: String,
    pub advice_confidence: String,
    pub advice_next_step: String,
    pub advice_evidence: String,
    pub dashboard_label: String,
    pub screen_nav_label: String,
    pub screen_overview: String,
    pub screen_actions: String,
    pub screen_signals: String,
    pub screen_runs: String,
    pub overview_digest_title: String,
    pub overview_digest_empty: String,
    pub top_actions_title: String,
    pub top_actions_detail: String,
    pub actions_title: String,
    pub actions_subtitle: String,
    pub actions_empty_title: String,
    pub actions_empty_detail: String,
    pub signals_subtitle: String,
    pub signals_empty_title: String,
    pub signals_empty_detail: String,
    pub runs_title: String,
    pub runs_subtitle: String,
    pub local_badge: String,
    pub scope_badge: String,
    pub latest_run_title: String,
    pub estimated_savings: String,
    pub source_advice: String,
    pub source_signal: String,
    pub status_signal: String,
    pub status_open: String,
    pub status_done: String,
    pub status_dismissed: String,
    pub status_succeeded: String,
    pub status_failed: String,
    pub scope_all: String,
    pub filter_title: String,
    pub filter_all: String,
    pub filter_open: String,
    pub filter_done: String,
    pub filter_dismissed: String,
    pub filter_advice: String,
    pub filter_signals: String,
    pub detail_open: String,
    pub detail_back: String,
    pub detail_observation: String,
    pub detail_impact: String,
    pub detail_next_step: String,
    pub detail_guardrails: String,
    pub detail_evidence: String,
    pub detail_scope: String,
    pub detail_status: String,
    pub detail_savings: String,
    pub detail_rule: String,
    pub detail_empty_title: String,
    pub detail_empty_detail: String,
    pub latest_issue_title: String,
    pub no_failed_runs_title: String,
    pub no_failed_runs_detail: String,
    pub signal_map_title: String,
    pub run_history_title: String,
    pub data_context_title: String,
    pub generated_at_label: String,
    pub generated_at_value: String,
    pub baseline_window_label: String,
    pub baseline_window_value: String,
    pub archive_source_label: String,
    pub advice_tool_label: String,
    pub run_items: String,
    pub kpi_savings: String,
    pub kpi_risks: String,
    pub kpi_warns: String,
    pub kpi_infos: String,
    pub empty: String,
    pub categories: BTreeMap<String, String>,
    pub severity: BTreeMap<String, String>,
    pub savings: InsightsSavingsCopy,
    pub silenced: BTreeMap<String, String>,
    pub rules: BTreeMap<String, InsightsRuleCopy>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct InsightsSavingsCopy {
    pub per_week: String,
    pub per_month: String,
    pub one_off: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct InsightsRuleCopy {
    pub title: String,
    pub body: String,
    #[serde(default)]
    pub assumption: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BrandCopy {
    pub name: String,
    pub mark: String,
    pub command: String,
    pub website_label: String,
    pub about_title: String,
    pub comments: String,
    pub usage_alert_title: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NavCopy {
    pub overview: String,
    pub deep_dive: String,
    pub usage: String,
    pub insights: String,
    pub config: String,
    pub configuration: String,
    pub session: String,
    pub dashboard: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PeriodCopy {
    pub today: String,
    pub week: String,
    pub thirty_days: String,
    pub month: String,
    pub all_time: String,
    pub today_short: String,
    pub week_short: String,
    pub thirty_days_short: String,
    pub month_short: String,
    pub all_time_short: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SortCopy {
    pub spend: String,
    pub date: String,
    pub tokens: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ToolCopy {
    pub all: String,
    pub claude_code: String,
    pub cursor: String,
    pub codex: String,
    pub copilot: String,
    pub gemini: String,
    pub sample: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MetricCopy {
    pub cost: String,
    pub calls: String,
    pub sessions: String,
    pub cache_hit: String,
    pub cache: String,
    pub cache_read: String,
    pub cache_write: String,
    pub cache_read_price: String,
    pub cache_write_price: String,
    pub input: String,
    pub output: String,
    pub r#in: String,
    pub out: String,
    pub cached: String,
    pub written: String,
    pub tokens: String,
    pub active_set: String,
    pub mixed: String,
    pub gbp: String,
    pub usd_default: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FilterCopy {
    pub tool: String,
    pub project: String,
    pub sort: String,
    pub period: String,
    pub all: String,
    pub sorted_by_24h: String,
    pub filter: String,
    pub filter_help: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PanelCopy {
    pub activity_pulse: String,
    pub activity_trend: String,
    pub activity_timeline: String,
    pub by_project: String,
    pub by_model: String,
    pub top_sessions: String,
    pub project_spend_by_tool: String,
    pub model_efficiency: String,
    pub core_tools: String,
    pub shell_commands: String,
    pub mcp_servers: String,
    pub daily_activity: String,
    pub selected_session: String,
    pub calls: String,
    pub desktop: String,
    pub local_data: String,
    pub local_files: String,
    pub money_and_data: String,
    pub tool_integrations: String,
    pub advice_engine: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TableCopy {
    pub blank: String,
    pub name: String,
    pub date: String,
    pub day: String,
    pub project: String,
    pub tool: String,
    pub tools: String,
    pub model: String,
    pub cost: String,
    pub calls: String,
    pub sessions: String,
    pub sess: String,
    pub avg_per_session: String,
    pub cache: String,
    pub cache_rate: String,
    pub time: String,
    pub r#in: String,
    pub out: String,
    pub cache_r: String,
    pub cache_w: String,
    pub prompt: String,
    pub activity: String,
    pub tool_mix: String,
    pub cache_hit: String,
    pub setting: String,
    pub value: String,
    pub enter: String,
    pub status: String,
    pub code: String,
    pub per_usd: String,
    pub kind: String,
    pub scope_model: String,
    pub scope_model_spaced: String,
    pub bar: String,
    pub used: String,
    pub left_call: String,
    pub left_calls_spaced: String,
    pub reset_tok: String,
    pub reset_tokens_spaced: String,
    pub cost_plan: String,
    pub cost_plan_spaced: String,
    pub raw_project: String,
    pub agent: String,
    pub archive: String,
    pub currency: String,
    pub exports: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TimelineCopy {
    pub spend: String,
    pub calls: String,
    pub range: String,
    pub to: String,
    pub high: String,
    pub latest: String,
    pub recent: String,
    pub pulse: String,
    pub no_data: String,
    pub activity_aria: String,
    pub activity_export_aria: String,
    pub relative_rank: String,
    pub no_activity: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UsageCopy {
    pub console_title: String,
    pub pulse: String,
    pub models: String,
    pub seen: String,
    pub limit: String,
    pub model: String,
    pub idle: String,
    pub used_suffix: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ConfigCopy {
    pub rows: ConfigRowsCopy,
    pub values: ConfigValuesCopy,
    pub paths: ConfigPathsCopy,
    pub links: ConfigLinksCopy,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ConfigRowsCopy {
    pub currency_override: ConfigRowCopy,
    pub rates_json: ConfigRowCopy,
    pub litellm_prices: ConfigRowCopy,
    pub claude_statusline: ConfigRowCopy,
    pub claude_limits: ConfigRowCopy,
    pub copilot_limits: ConfigRowCopy,
    pub claude_subscription_limits: ConfigRowCopy,
    pub codex_subscription_limits: ConfigRowCopy,
    pub advice_tool: ConfigRowCopy,
    pub advice_prompts: ConfigRowCopy,
    pub clear_data: ConfigRowCopy,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ConfigRowCopy {
    pub name: String,
    pub action: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ConfigValuesCopy {
    pub local_snapshot: String,
    pub legacy_snapshot: String,
    pub embedded_snapshot: String,
    pub sidecar_found: String,
    pub sidecar_missing: String,
    pub statusline_not_installed: String,
    pub statusline_installed_wrapping: String,
    pub statusline_installed_passthrough: String,
    pub statusline_external: String,
    pub quota_snapshot_found: String,
    pub quota_snapshot_missing: String,
    pub advice_tool_available: String,
    pub advice_tool_missing: String,
    pub advice_prompts_ready: String,
    pub advice_prompts_missing: String,
    pub delete_archive_then_rebuild: String,
    pub build_archive_from_history: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ConfigPathsCopy {
    pub config_dir: String,
    pub config_file: String,
    pub archive_db: String,
    pub rates_data: String,
    pub pricing_data: String,
    pub claude_limits: String,
    pub copilot_limits: String,
    pub rates_source: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ConfigLinksCopy {
    pub published_rates: String,
    pub pricing_upstream: String,
    pub pricing_overrides: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SessionCopy {
    pub no_session_loaded: String,
    pub no_session_selected: String,
    pub calls_title: String,
    pub call_detail: String,
    pub call_detail_title: String,
    pub bash: String,
    pub reasoning: String,
    pub web_search: String,
    pub web: String,
    pub close: String,
    pub deep_dive: String,
    pub sample_project: String,
    pub sample_date_range: String,
    pub sample_note: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ModalCopy {
    pub help_title: String,
    pub project: String,
    pub currency: String,
    pub session: String,
    pub export: String,
    pub export_folder: String,
    pub selection_title: String,
    pub filtered_selection_title: String,
    pub of: String,
    pub current: String,
    pub format: String,
    pub folder: String,
    pub file: String,
    pub source: String,
    pub url: String,
    pub write: String,
    pub after: String,
    pub delete: String,
    pub rebuild: String,
    pub keep: String,
    pub note: String,
    pub archive_db: String,
    pub from_local_history: String,
    pub config_rates_pricing_exports: String,
    pub missing_source_files: String,
    pub clearing_data: String,
    pub clear_data_question: String,
    pub rebuilding_archive: String,
    pub local_history: String,
    pub reset: String,
    pub reimporting: String,
    pub download_rates_title: String,
    pub download_prices_title: String,
    pub sync_copilot_limits_title: String,
    pub sync_claude_subscription_limits_title: String,
    pub sync_codex_subscription_limits_title: String,
    pub install_claude_statusline_title: String,
    pub install_claude_statusline_message: String,
    pub install_claude_statusline_manual_title: String,
    pub install_claude_statusline_manual_message: String,
    pub uninstall_claude_statusline_title: String,
    pub uninstall_claude_statusline_message: String,
    pub rates_file: String,
    pub pricing_file: String,
    pub copilot_limits_file: String,
    pub claude_subscription_limits_file: String,
    pub codex_subscription_limits_file: String,
    pub rates_source: String,
    pub prices_source: String,
    pub copilot_limits_source: String,
    pub claude_subscription_limits_source: String,
    pub codex_subscription_limits_source: String,
    pub rates_effect: String,
    pub prices_effect: String,
    pub copilot_limits_effect: String,
    pub claude_subscription_limits_effect: String,
    pub codex_subscription_limits_effect: String,
    pub download_latest_rates_message: String,
    pub download_latest_prices_message: String,
    pub sync_copilot_limits_message: String,
    pub clear_data_message: String,
    pub current_period_filters_apply: String,
    pub hidden_folders: String,
    pub use_this_folder: String,
    pub r#use: String,
    pub up: String,
    pub dir: String,
    pub active: String,
    pub could_not_read_folder: String,
    pub subscription_cookie: SubscriptionCookieCopy,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SubscriptionCookieCopy {
    pub title_claude: String,
    pub title_codex: String,
    pub claude_help: String,
    pub codex_help: String,
    pub claude_field_label: String,
    pub claude_field_placeholder: String,
    pub shard_zero_label: String,
    pub shard_zero_placeholder: String,
    pub shard_one_label: String,
    pub shard_one_placeholder: String,
    pub extras_label: String,
    pub extras_help: String,
    pub extras_placeholder: String,
    pub stored_yes: String,
    pub stored_no: String,
    pub save_and_sync: String,
    pub sync_stored: String,
    pub clear_stored: String,
    pub empty_value_error: String,
    pub save_failed: String,
    pub clear_failed: String,
    pub cleared_status: String,
    pub field_hint: String,
    pub action_hint: String,
    pub keychain_note: String,
    pub busy: String,
    pub chars_suffix: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ActionCopy {
    pub download: String,
    pub download_lower: String,
    pub sync: String,
    pub sync_lower: String,
    pub install: String,
    pub install_manual: String,
    pub uninstall: String,
    pub cancel: String,
    pub cancel_lower: String,
    pub clear_data: String,
    pub clear_data_lower: String,
    pub refresh: String,
    pub folder: String,
    pub export: String,
    pub export_lower: String,
    pub open: String,
    pub open_lower: String,
    pub open_session_picker: String,
    pub generate_advice: String,
    pub mark_done: String,
    pub mark_open: String,
    pub dismiss: String,
    pub close: String,
    pub close_lower: String,
    pub select_open: String,
    pub parent: String,
    pub select: String,
    pub browse_folder: String,
    pub refresh_archive: String,
    pub export_current_view: String,
    pub close_dialog: String,
    pub close_call_detail: String,
    pub show_app: String,
    pub quit_app: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DesktopCopy {
    pub sections_aria: String,
    pub period_aria: String,
    pub tool_aria: String,
    pub sort_aria: String,
    pub project_aria: String,
    pub open_at_login: String,
    pub dock_taskbar_icon: String,
    pub enabled: String,
    pub disabled: String,
    pub shown: String,
    pub hidden: String,
    pub filter_projects: String,
    pub filter_sessions: String,
    pub filter_currencies: String,
    pub filter_advice_tools: String,
    pub rank: String,
    pub session_rank: String,
    pub model_usage: String,
    pub loading_label: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UpdateCopy {
    pub title: String,
    pub description: String,
    pub unsupported: String,
    pub current_version: String,
    pub check: String,
    pub checking: String,
    pub install: String,
    pub installing: String,
    pub available: String,
    pub up_to_date: String,
    pub download_started: String,
    pub download_progress: String,
    pub download_progress_unknown: String,
    pub download_finished: String,
    pub installed_restarting: String,
    pub failed: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TrayCopy {
    pub summary_aria: String,
    pub hours_24: String,
    pub activity: String,
    pub models: String,
    pub tokens: String,
    pub high: String,
    pub no_model_rows: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EmptyCopy {
    pub terminal_too_small: String,
    pub terminal_dashboard_suffix: String,
    pub terminal_resize: String,
    pub no_project_rows: String,
    pub no_project_tool_rows: String,
    pub no_sessions: String,
    pub no_models: String,
    pub no_rows: String,
    pub no_data: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExportCopy {
    pub json: String,
    pub csv: String,
    pub svg: String,
    pub png: String,
    pub html: String,
    pub pdf: String,
    pub report_title: String,
    pub full_workbook_report: String,
    pub summary_metrics_aria: String,
    pub dashboard_workbook_aria: String,
    pub generated: String,
    pub export_id: String,
    pub source: String,
    pub currency: String,
    pub period: String,
    pub tool: String,
    pub project: String,
    pub sort: String,
    pub date_range: String,
    pub calendar_weekdays: Vec<String>,
    pub csv_files: CsvFilesCopy,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ReportCopy {
    pub json: String,
    pub csv: String,
    pub svg: String,
    pub png: String,
    pub html: String,
    pub pdf: String,
    pub xlsx: String,
    pub report: String,
    pub modal_title: String,
    pub report_title: String,
    pub all_projects: String,
    pub format: String,
    pub folder: String,
    pub period: String,
    pub project: String,
    pub redaction: String,
    pub redaction_on: String,
    pub redaction_off: String,
    pub sample_no_raw_archive: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ReportCliCopy {
    pub title: String,
    pub loading: String,
    pub select_period: String,
    pub select_project: String,
    pub select_reports: String,
    pub select_folder: String,
    pub select_redaction: String,
    pub confirm: String,
    pub generated: String,
    pub cancelled: String,
    pub all: String,
    pub yes: String,
    pub no: String,
    pub on: String,
    pub off: String,
    pub summary: String,
    pub prompt: String,
    pub invalid_number: String,
    pub invalid_multi: String,
    pub invalid_yes_no: String,
    pub invalid_folder: String,
    pub wrote: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CsvFilesCopy {
    pub summary_file: String,
    pub daily_file: String,
    pub projects_file: String,
    pub project_tools_file: String,
    pub sessions_file: String,
    pub models_file: String,
    pub tools_file: String,
    pub commands_file: String,
    pub mcp_servers_file: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CliCopy {
    pub usage: String,
    pub commands: String,
    pub flags: String,
    pub report_command: String,
    pub help_flag: String,
    pub version_flag: String,
    pub list_projects_flag: String,
    pub refresh_prices_flag: String,
    pub generate_currency_flag: String,
    pub launch_dashboard: String,
    pub archive_failed_raw_ingest: String,
    pub no_local_sessions_found: String,
    pub project_inventory_summary: String,
    pub wrote_path: String,
    pub refresh_prices_requires_feature: String,
    pub generate_currency_requires_feature: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KeymapCopy {
    pub actions: BTreeMap<String, String>,
    pub help: Vec<CopyHintGroup>,
    pub footers: BTreeMap<String, Vec<CopyKeyHint>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CopyHintGroup {
    pub title: String,
    pub items: Vec<CopyKeyHint>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CopyKeyHint {
    pub keys: String,
    pub label: String,
    pub action: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StatusCopy {
    pub reloading: String,
    pub sample_data: String,
    pub live_data: String,
    pub no_local_sessions_sample_data: String,
    pub refresher_stopped_prior_data_kept: String,
    pub reloaded_calls: String,
    pub auto_refreshed_calls: String,
    pub reload_no_sessions_prior_data_kept: String,
    pub auto_refresh_no_sessions_prior_data_kept: String,
    pub reload_failed_prior_data_kept: String,
    pub auto_refresh_failed_prior_data_kept: String,
    pub no_sessions_to_drill_into: String,
    pub session_not_found: String,
    pub exported: String,
    pub export_failed: String,
    pub export_folder: String,
    pub report_generated: String,
    pub report_failed: String,
    pub report_folder: String,
    pub project_not_found: String,
    pub currency_set: String,
    pub config_save_failed: String,
    pub clearing_data_reimporting: String,
    pub data_cleared_no_sessions_sample_data: String,
    pub data_cleared_counts: String,
    pub clear_data_failed: String,
    pub rates_refreshed: String,
    pub rates_refresh_failed: String,
    pub rates_download_unavailable: String,
    pub litellm_prices_refreshed: String,
    pub litellm_refresh_failed: String,
    pub litellm_download_unavailable: String,
    pub claude_limits_sidecar_missing: String,
    pub claude_limits_synced: String,
    pub claude_statusline_installed: String,
    pub claude_statusline_installed_wrapping: String,
    pub claude_statusline_installed_manual: String,
    pub claude_statusline_already_installed: String,
    pub claude_statusline_uninstalled: String,
    pub claude_statusline_settings_backed_up: String,
    pub claude_statusline_failed: String,
    pub copilot_limits_synced: String,
    pub copilot_limits_sync_failed: String,
    pub copilot_limits_sync_unavailable: String,
    pub claude_subscription_synced: String,
    pub claude_subscription_sync_failed: String,
    pub claude_subscription_sync_unavailable: String,
    pub claude_subscription_cookie_missing: String,
    pub codex_subscription_synced: String,
    pub codex_subscription_sync_failed: String,
    pub codex_subscription_sync_unavailable: String,
    pub codex_subscription_cookie_missing: String,
    pub advice_tool_set: String,
    pub advice_tool_config_failed: String,
    pub advice_requires_live_data: String,
    pub advice_running: String,
    pub advice_already_running: String,
    pub advice_run_saved: String,
    pub advice_item_updated: String,
    pub advice_item_not_found: String,
    pub advice_failed: String,
    pub config_failed_defaults: String,
    pub currency_rates_failed_embedded: String,
    pub legacy_cache_imported_records: String,
    pub archive_synced_counts: String,
    pub archive_failed_raw_ingest: String,
    pub archive_failed_raw_ingest_no_sessions_sample_data: String,
    pub archive_failed_raw_ingest_ingest_failed_sample_data: String,
    pub open_at_login_state: String,
    pub dock_taskbar_icon_state: String,
    pub open_at_login_failed: String,
    pub dock_visibility_failed: String,
    pub taskbar_visibility_failed: String,
    pub export_folder_path_empty: String,
    pub background_usage_changed: String,
    pub background_usage_body: String,
}

pub fn copy() -> &'static CopyDeck {
    static COPY: OnceLock<CopyDeck> = OnceLock::new();
    COPY.get_or_init(|| {
        CopyDeck::from_json(include_str!("copy.json"))
            .unwrap_or_else(|err| panic!("invalid embedded copy deck: {err}"))
    })
}

pub fn template(template: &str, values: &[(&str, String)]) -> String {
    let mut out = template.to_string();
    for (key, value) in values {
        out = out.replace(&format!("{{{key}}}"), value);
    }
    out
}

impl CopyDeck {
    pub fn from_json(input: &str) -> Result<Self, String> {
        let deck: Self =
            serde_json::from_str(input).map_err(|err| format!("parse copy json: {err}"))?;
        deck.validate()?;
        Ok(deck)
    }

    pub fn footer(&self, name: &str) -> &[CopyKeyHint] {
        self.keymap
            .footers
            .get(name)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub fn action_label(&self, action: &str) -> Option<&str> {
        self.keymap.actions.get(action).map(String::as_str)
    }

    fn validate(&self) -> Result<(), String> {
        let value =
            serde_json::to_value(self).map_err(|err| format!("serialize copy json: {err}"))?;
        validate_non_empty_strings("$", &value)?;
        ensure_unique_table_labels(self)?;
        ensure_unique_footer_labels(self)?;
        ensure_template(&self.status.reloaded_calls, &["calls"])?;
        ensure_template(&self.status.exported, &["format", "path"])?;
        ensure_template(&self.status.report_generated, &["format", "path"])?;
        ensure_template(&self.status.report_failed, &["error"])?;
        ensure_template(&self.status.report_folder, &["path"])?;
        ensure_template(&self.status.clear_data_failed, &["error"])?;
        ensure_template(&self.status.claude_limits_sidecar_missing, &["path"])?;
        ensure_template(&self.status.claude_limits_synced, &["limits"])?;
        ensure_template(&self.status.claude_statusline_installed, &["path"])?;
        ensure_template(
            &self.status.claude_statusline_installed_wrapping,
            &["inner"],
        )?;
        ensure_template(&self.status.claude_statusline_installed_manual, &["path"])?;
        ensure_template(&self.status.claude_statusline_settings_backed_up, &["path"])?;
        ensure_template(&self.status.claude_statusline_failed, &["error"])?;
        ensure_template(
            &self.config.values.statusline_installed_wrapping,
            &["inner"],
        )?;
        ensure_template(&self.config.values.statusline_external, &["command"])?;
        ensure_template(&self.status.copilot_limits_synced, &["snapshots", "limits"])?;
        ensure_template(&self.status.copilot_limits_sync_failed, &["error"])?;
        ensure_template(
            &self.status.claude_subscription_synced,
            &["snapshots", "limits"],
        )?;
        ensure_template(&self.status.claude_subscription_sync_failed, &["error"])?;
        ensure_template(
            &self.status.codex_subscription_synced,
            &["snapshots", "limits"],
        )?;
        ensure_template(&self.status.codex_subscription_sync_failed, &["error"])?;
        ensure_template(&self.modals.subscription_cookie.save_failed, &["error"])?;
        ensure_template(&self.modals.subscription_cookie.clear_failed, &["error"])?;
        ensure_template(&self.status.background_usage_body, &["summary"])?;
        ensure_template(&self.report_cli.prompt, &["label", "default"])?;
        ensure_template(&self.report_cli.invalid_number, &["max"])?;
        ensure_template(&self.report_cli.wrote, &["format", "path"])?;
        ensure_template(&self.updates.current_version, &["version"])?;
        ensure_template(&self.updates.available, &["version"])?;
        ensure_template(&self.updates.download_progress, &["downloaded", "total"])?;
        ensure_template(&self.updates.download_progress_unknown, &["downloaded"])?;
        ensure_template(&self.updates.failed, &["error"])?;
        ensure_template(&self.modals.selection_title, &["name", "index", "total"])?;
        ensure_template(
            &self.modals.filtered_selection_title,
            &["name", "index", "count", "total"],
        )?;
        ensure_template(&self.reports.report_title, &["period", "project"])?;
        ensure_template(&self.timeline.activity_aria, &["first", "last"])?;
        ensure_template(&self.timeline.relative_rank, &["value"])?;
        ensure_template(&self.usage.console_title, &["tool"])?;
        ensure_template(&self.export.report_title, &["period", "tool"])?;
        if self.export.calendar_weekdays.len() != 7 {
            return Err("export.calendar_weekdays must contain seven labels".into());
        }
        ensure_template(&self.insights.savings.per_week, &["amount"])?;
        ensure_template(&self.insights.savings.per_month, &["amount"])?;
        ensure_template(&self.insights.savings.one_off, &["amount"])?;
        for category in ["model_rightsizing", "cache", "anomalies", "quota"] {
            if !self.insights.categories.contains_key(category) {
                return Err(format!(
                    "insights.categories.{category} missing — every Category must have a label"
                ));
            }
        }
        for severity in ["risk", "warn", "info"] {
            if !self.insights.severity.contains_key(severity) {
                return Err(format!(
                    "insights.severity.{severity} missing — every Severity must have a label"
                ));
            }
        }
        Ok(())
    }
}

fn validate_non_empty_strings(path: &str, value: &serde_json::Value) -> Result<(), String> {
    match value {
        serde_json::Value::String(s) if s.trim().is_empty() && path != "$.tables.blank" => {
            Err(format!("{path} cannot be empty"))
        }
        serde_json::Value::Array(items) => {
            for (idx, item) in items.iter().enumerate() {
                validate_non_empty_strings(&format!("{path}[{idx}]"), item)?;
            }
            Ok(())
        }
        serde_json::Value::Object(map) => {
            for (key, item) in map {
                validate_non_empty_strings(&format!("{path}.{key}"), item)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn ensure_unique_table_labels(deck: &CopyDeck) -> Result<(), String> {
    let table = serde_json::to_value(&deck.tables).map_err(|err| err.to_string())?;
    let serde_json::Value::Object(map) = table else {
        return Err("tables must serialize to an object".into());
    };
    let mut seen = HashSet::new();
    for (key, value) in map {
        let Some(label) = value.as_str() else {
            continue;
        };
        if label.is_empty() {
            continue;
        }
        if !seen.insert(label.to_string()) {
            return Err(format!("duplicate table label {label:?} at tables.{key}"));
        }
    }
    Ok(())
}

fn ensure_unique_footer_labels(deck: &CopyDeck) -> Result<(), String> {
    for (name, hints) in &deck.keymap.footers {
        let mut seen = HashSet::new();
        for hint in hints {
            let signature = format!("{} {}", hint.keys, hint.label);
            if !seen.insert(signature.clone()) {
                return Err(format!("duplicate footer hint {signature:?} in {name}"));
            }
        }
    }
    Ok(())
}

fn ensure_template(template: &str, placeholders: &[&str]) -> Result<(), String> {
    for placeholder in placeholders {
        let token = format!("{{{placeholder}}}");
        if !template.contains(&token) {
            return Err(format!("template {template:?} must contain {token}"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_copy_deck_is_valid() {
        let deck = copy();

        assert_eq!(deck.brand.name, "Token Use");
        assert_eq!(deck.export.calendar_weekdays.len(), 7);
        assert!(deck
            .footer("dashboard")
            .iter()
            .any(|hint| hint.label == "quit"));
    }

    #[test]
    fn keymap_copy_references_supported_actions() {
        let deck = copy();
        let actions: HashSet<&str> = crate::keymap::keymap()
            .actions()
            .iter()
            .map(|action| action.id.as_str())
            .collect();

        for action in deck.keymap.actions.keys() {
            assert!(actions.contains(action.as_str()), "{action}");
        }
        for group in &deck.keymap.help {
            for item in &group.items {
                assert!(actions.contains(item.action.as_str()), "{}", item.action);
            }
        }
        for hints in deck.keymap.footers.values() {
            for item in hints {
                assert!(actions.contains(item.action.as_str()), "{}", item.action);
            }
        }
    }

    #[test]
    fn keymap_json_is_behavior_only() {
        let raw = include_str!("../keymap/keymap.json");

        for forbidden in ["\"label\"", "\"footers\"", "\"title\"", "\"items\""] {
            assert!(
                !raw.contains(forbidden),
                "keymap.json should keep labels in copy.json, found {forbidden}"
            );
        }
    }

    #[test]
    fn surface_files_do_not_reintroduce_known_copy_literals() {
        let export_source = include_str!("../export/workbook.rs")
            .split("\n#[cfg(test)]")
            .next()
            .expect("export source has a non-test section");
        let files = [
            ("src/ui/mod.rs", include_str!("../ui/mod.rs")),
            ("src/ui/sections.rs", include_str!("../ui/sections.rs")),
            ("src/main.rs", include_str!("../main.rs")),
            ("src/export/workbook.rs", export_source),
            ("src/export/chart.rs", include_str!("../export/chart.rs")),
            ("src/export/csv.rs", include_str!("../export/csv.rs")),
            ("src/export/labels.rs", include_str!("../export/labels.rs")),
            ("src/export/report.rs", include_str!("../export/report.rs")),
            (
                "desktop/src/App.svelte",
                include_str!("../../desktop/src/App.svelte"),
            ),
            (
                "desktop/src/views/ConfigView.svelte",
                include_str!("../../desktop/src/views/ConfigView.svelte"),
            ),
            (
                "desktop/src/views/DeepDiveView.svelte",
                include_str!("../../desktop/src/views/DeepDiveView.svelte"),
            ),
            (
                "desktop/src/views/OverviewView.svelte",
                include_str!("../../desktop/src/views/OverviewView.svelte"),
            ),
            (
                "desktop/src/views/SessionView.svelte",
                include_str!("../../desktop/src/views/SessionView.svelte"),
            ),
            (
                "desktop/src/views/UsageView.svelte",
                include_str!("../../desktop/src/views/UsageView.svelte"),
            ),
            (
                "desktop/src/components/tables/CountTable.svelte",
                include_str!("../../desktop/src/components/tables/CountTable.svelte"),
            ),
            (
                "desktop/src/components/tables/KpiStrip.svelte",
                include_str!("../../desktop/src/components/tables/KpiStrip.svelte"),
            ),
            (
                "desktop/src/components/tables/ModelTable.svelte",
                include_str!("../../desktop/src/components/tables/ModelTable.svelte"),
            ),
            (
                "desktop/src/components/tables/ProjectTable.svelte",
                include_str!("../../desktop/src/components/tables/ProjectTable.svelte"),
            ),
            (
                "desktop/src/components/tables/ProjectToolTable.svelte",
                include_str!("../../desktop/src/components/tables/ProjectToolTable.svelte"),
            ),
            (
                "desktop/src/components/tables/SessionTable.svelte",
                include_str!("../../desktop/src/components/tables/SessionTable.svelte"),
            ),
            (
                "desktop/src/TrayPopover.svelte",
                include_str!("../../desktop/src/TrayPopover.svelte"),
            ),
            (
                "desktop/src/components/ActivityPulse.svelte",
                include_str!("../../desktop/src/components/ActivityPulse.svelte"),
            ),
            (
                "desktop/src/components/UsageConsole.svelte",
                include_str!("../../desktop/src/components/UsageConsole.svelte"),
            ),
        ];
        let forbidden = [
            "Token Use",
            "Activity Pulse",
            "Project Spend by Tool",
            "Open Session Picker",
            "Filter projects",
            "Clear data?",
            "Full workbook report",
            "Selected Session",
            "no activity in this view",
        ];

        for (file, source) in files {
            for literal in forbidden {
                assert!(
                    !source.contains(literal),
                    "{file} should reference src/copy/copy.json instead of {literal:?}"
                );
            }
        }
    }
}
