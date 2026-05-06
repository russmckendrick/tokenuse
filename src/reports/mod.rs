use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Datelike, Duration, Local, NaiveDate, Utc};
use color_eyre::{eyre::WrapErr, Result};
use fulgur::{Engine, Margin, PageSize};
use plotters::prelude::*;
use rust_xlsxwriter::{Color, Format, FormatBorder, Workbook, Worksheet};
use serde::Serialize;

use crate::app::{Period, ProjectFilter};
use crate::config::ConfigPaths;
use crate::copy::{copy, template};
use crate::currency::CurrencyFormatter;
use crate::data::DashboardData;
use crate::ingest::projects::{
    project_identity, project_label, project_label_lookup, raw_project_display, tool_short_label,
};
use crate::ingest::Ingested;
use crate::pricing;
use crate::tools::{LimitSnapshot, ParsedCall};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportFormat {
    Json,
    Csv,
    Svg,
    Png,
    Html,
    Pdf,
    Xlsx,
}

impl ReportFormat {
    pub const ALL: [Self; 7] = [
        Self::Html,
        Self::Pdf,
        Self::Svg,
        Self::Png,
        Self::Json,
        Self::Xlsx,
        Self::Csv,
    ];

    pub fn label(self) -> &'static str {
        let copy = copy();
        match self {
            Self::Json => copy.reports.json.as_str(),
            Self::Csv => copy.reports.csv.as_str(),
            Self::Svg => copy.reports.svg.as_str(),
            Self::Png => copy.reports.png.as_str(),
            Self::Html => copy.reports.html.as_str(),
            Self::Pdf => copy.reports.pdf.as_str(),
            Self::Xlsx => copy.reports.xlsx.as_str(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ReportScope {
    AllProjects,
    Project { identity: String, label: String },
}

impl ReportScope {
    pub fn label(&self) -> &str {
        match self {
            Self::AllProjects => copy().reports.all_projects.as_str(),
            Self::Project { label, .. } => label,
        }
    }

    pub fn project_filter(&self) -> ProjectFilter {
        match self {
            Self::AllProjects => ProjectFilter::All,
            Self::Project { identity, label } => ProjectFilter::Selected {
                identity: identity.clone(),
                label: label.clone(),
            },
        }
    }

    fn identity(&self) -> Option<&str> {
        match self {
            Self::AllProjects => None,
            Self::Project { identity, .. } => Some(identity),
        }
    }

    fn redacted_label(&self, redacted: bool) -> String {
        match self {
            Self::AllProjects => self.label().to_string(),
            Self::Project { label, .. } if !redacted => label.clone(),
            Self::Project { .. } => "Project 1".into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ReportRequest {
    pub format: ReportFormat,
    pub period: Period,
    pub scope: ReportScope,
    pub redacted: bool,
}

#[derive(Debug, Clone)]
pub struct ReportBatchRequest {
    pub formats: Vec<ReportFormat>,
    pub period: Period,
    pub scope: ReportScope,
    pub redacted: bool,
}

impl ReportBatchRequest {
    fn dataset_request(&self) -> ReportRequest {
        ReportRequest {
            format: self.formats.first().copied().unwrap_or(ReportFormat::Html),
            period: self.period,
            scope: self.scope.clone(),
            redacted: self.redacted,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ReportResponse {
    pub path: PathBuf,
    pub format: ReportFormat,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReportDataset {
    pub metadata: ReportMetadata,
    pub summary: ReportSummary,
    pub activity: Vec<ReportActivity>,
    pub projects: Vec<ReportProject>,
    pub project_tools: Vec<ReportProjectTool>,
    pub sessions: Vec<ReportSession>,
    pub calls: Vec<ReportCall>,
    pub models: Vec<ReportModel>,
    pub tools: Vec<ReportCount>,
    pub commands: Vec<ReportCount>,
    pub mcp_servers: Vec<ReportCount>,
    pub limits_latest: Vec<ReportLimitLatest>,
    pub limits_raw: Vec<ReportLimitRaw>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReportMetadata {
    pub report_id: String,
    pub generated_at: String,
    pub source: String,
    pub currency: String,
    pub period: String,
    pub project: String,
    pub redacted: bool,
    pub sample_note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct ReportSummary {
    pub cost: String,
    pub cost_usd: f64,
    pub calls: u64,
    pub sessions: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub cached_input_tokens: u64,
    pub reasoning_tokens: u64,
    pub web_search_requests: u64,
    pub total_tokens: u64,
    pub cache_hit_rate: String,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct ReportActivity {
    pub date: String,
    pub cost: String,
    pub cost_usd: f64,
    pub calls: u64,
    pub tokens: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub intensity: u64,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct ReportProject {
    pub project: String,
    pub project_identity: String,
    pub raw_projects: String,
    pub cost: String,
    pub cost_usd: f64,
    pub calls: u64,
    pub sessions: u64,
    pub tokens: u64,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct ReportProjectTool {
    pub project: String,
    pub tool: String,
    pub cost: String,
    pub cost_usd: f64,
    pub calls: u64,
    pub sessions: u64,
    pub tokens: u64,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct ReportSession {
    pub session_key: String,
    pub session_id: String,
    pub project: String,
    pub tool: String,
    pub started_at: String,
    pub ended_at: String,
    pub duration_minutes: i64,
    pub cost: String,
    pub cost_usd: f64,
    pub calls: u64,
    pub tokens: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub reasoning_tokens: u64,
    pub web_search_requests: u64,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct ReportCall {
    pub timestamp: String,
    pub tool: String,
    pub model: String,
    pub project: String,
    pub raw_project: String,
    pub session_key: String,
    pub session_id: String,
    pub dedup_key: String,
    pub cost: String,
    pub cost_usd: f64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub cache_read_rate: String,
    pub cache_write_rate: String,
    pub cached_input_tokens: u64,
    pub reasoning_tokens: u64,
    pub web_search_requests: u64,
    pub tools: String,
    pub bash_commands: String,
    pub prompt_preview: String,
    pub prompt_full: String,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct ReportModel {
    pub model: String,
    pub tool: String,
    pub cost: String,
    pub cost_usd: f64,
    pub calls: u64,
    pub tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_read_rate: String,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct ReportCount {
    pub name: String,
    pub calls: u64,
    pub tokens: u64,
    pub cost: String,
    pub cost_usd: f64,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct ReportLimitLatest {
    pub tool: String,
    pub limit_id: String,
    pub limit_name: String,
    pub plan_type: String,
    pub observed_at: String,
    pub primary_used_percent: Option<f64>,
    pub secondary_used_percent: Option<f64>,
    pub credits_balance: Option<f64>,
    pub rate_limit_reached_type: String,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct ReportLimitRaw {
    pub tool: String,
    pub limit_id: String,
    pub limit_name: String,
    pub plan_type: String,
    pub observed_at: String,
    pub primary_json: String,
    pub secondary_json: String,
    pub credits_json: String,
    pub rate_limit_reached_type: String,
}

pub fn default_report_dir(paths: &ConfigPaths) -> PathBuf {
    default_report_dir_from(paths, dirs::download_dir(), dirs::home_dir())
}

fn default_report_dir_from(
    paths: &ConfigPaths,
    download_dir: Option<PathBuf>,
    home_dir: Option<PathBuf>,
) -> PathBuf {
    download_dir
        .or_else(|| home_dir.map(|home| home.join("Downloads")))
        .unwrap_or_else(|| paths.dir.join("reports"))
}

pub fn write_ingested_to_dir(
    reports_root: &Path,
    request: &ReportRequest,
    ingested: &Ingested,
    currency: &CurrencyFormatter,
    source_label: &str,
) -> Result<ReportResponse> {
    let stamp = Local::now().format("%Y%m%dT%H%M%S").to_string();
    let dataset = build_ingested_dataset(request, ingested, currency, source_label, &stamp);
    write_dataset_to_dir(reports_root, request.format, &dataset, &stamp)
}

pub fn write_sample_to_dir(
    reports_root: &Path,
    request: &ReportRequest,
    dashboard: &DashboardData,
    currency: &CurrencyFormatter,
    source_label: &str,
) -> Result<ReportResponse> {
    let stamp = Local::now().format("%Y%m%dT%H%M%S").to_string();
    let dataset = build_sample_dataset(request, dashboard, currency, source_label, &stamp);
    write_dataset_to_dir(reports_root, request.format, &dataset, &stamp)
}

pub fn write_ingested_batch_to_dir(
    reports_root: &Path,
    request: &ReportBatchRequest,
    ingested: &Ingested,
    currency: &CurrencyFormatter,
    source_label: &str,
) -> Result<Vec<ReportResponse>> {
    let stamp = Local::now().format("%Y%m%dT%H%M%S").to_string();
    let dataset = build_ingested_dataset(
        &request.dataset_request(),
        ingested,
        currency,
        source_label,
        &stamp,
    );
    write_dataset_formats_to_dir(reports_root, &request.formats, &dataset, &stamp)
}

pub fn write_sample_batch_to_dir(
    reports_root: &Path,
    request: &ReportBatchRequest,
    dashboard: &DashboardData,
    currency: &CurrencyFormatter,
    source_label: &str,
) -> Result<Vec<ReportResponse>> {
    let stamp = Local::now().format("%Y%m%dT%H%M%S").to_string();
    let dataset = build_sample_dataset(
        &request.dataset_request(),
        dashboard,
        currency,
        source_label,
        &stamp,
    );
    write_dataset_formats_to_dir(reports_root, &request.formats, &dataset, &stamp)
}

fn write_dataset_formats_to_dir(
    reports_root: &Path,
    formats: &[ReportFormat],
    dataset: &ReportDataset,
    stamp: &str,
) -> Result<Vec<ReportResponse>> {
    formats
        .iter()
        .map(|format| write_dataset_to_dir(reports_root, *format, dataset, stamp))
        .collect()
}

fn write_dataset_to_dir(
    reports_root: &Path,
    format: ReportFormat,
    dataset: &ReportDataset,
    stamp: &str,
) -> Result<ReportResponse> {
    fs::create_dir_all(reports_root)
        .wrap_err_with(|| format!("create {}", reports_root.display()))?;
    let slug = report_slug(dataset);
    let base = format!("tokenuse-report-{stamp}-{slug}");
    let path = match format {
        ReportFormat::Json => {
            let path = reports_root.join(format!("{base}.json"));
            let text = serde_json::to_string_pretty(dataset).wrap_err("serialize report json")?;
            fs::write(&path, text).wrap_err_with(|| format!("write {}", path.display()))?;
            path
        }
        ReportFormat::Csv => {
            let path = reports_root.join(format!("{base}-csv"));
            fs::create_dir_all(&path).wrap_err_with(|| format!("create {}", path.display()))?;
            write_csv_report_dir(&path, dataset)?;
            path
        }
        ReportFormat::Svg => {
            let path = reports_root.join(format!("{base}.svg"));
            write_summary_svg(&path, dataset)?;
            path
        }
        ReportFormat::Png => {
            let path = reports_root.join(format!("{base}.png"));
            write_summary_png(&path, dataset)?;
            path
        }
        ReportFormat::Html => {
            let path = reports_root.join(format!("{base}.html"));
            fs::write(&path, build_html_report(dataset))
                .wrap_err_with(|| format!("write {}", path.display()))?;
            path
        }
        ReportFormat::Pdf => {
            let path = reports_root.join(format!("{base}.pdf"));
            write_pdf_report(&path, dataset)?;
            path
        }
        ReportFormat::Xlsx => {
            let path = reports_root.join(format!("{base}.xlsx"));
            write_xlsx_report(&path, dataset)?;
            path
        }
    };
    Ok(ReportResponse { path, format })
}

fn build_ingested_dataset(
    request: &ReportRequest,
    ingested: &Ingested,
    currency: &CurrencyFormatter,
    source_label: &str,
    stamp: &str,
) -> ReportDataset {
    let now = Local::now();
    let filtered: Vec<&ParsedCall> = ingested
        .calls
        .iter()
        .filter(|call| in_report_period(call.timestamp, request.period, now))
        .filter(|call| {
            request
                .scope
                .identity()
                .is_none_or(|identity| project_identity(&call.project) == identity)
        })
        .collect();

    let labels = project_label_lookup(filtered.iter().map(|call| call.project.as_str()));
    let mut redactor = Redactor::new(request.redacted);
    let metadata = report_metadata(request, currency, source_label, stamp, None);
    let summary = summarize_calls(&filtered, currency);
    let activity = aggregate_report_activity(&filtered, currency);
    let projects = aggregate_report_projects(&filtered, &labels, currency, &mut redactor);
    let project_tools = aggregate_report_project_tools(&filtered, &labels, currency, &mut redactor);
    let sessions = aggregate_report_sessions(&filtered, &labels, currency, &mut redactor);
    let calls = report_calls(&filtered, &labels, currency, &mut redactor);
    let models = aggregate_report_models(&filtered, currency);
    let tools = aggregate_report_tools(&filtered, currency);
    let commands = aggregate_report_commands(&filtered, currency);
    let mcp_servers = aggregate_report_mcp_servers(&filtered, currency);
    let period_limits = filter_limits(&ingested.limits, request.period, now);
    let limits_latest = latest_limits(&period_limits);
    let limits_raw = raw_limits(&period_limits);

    ReportDataset {
        metadata,
        summary,
        activity,
        projects,
        project_tools,
        sessions,
        calls,
        models,
        tools,
        commands,
        mcp_servers,
        limits_latest,
        limits_raw,
    }
}

fn build_sample_dataset(
    request: &ReportRequest,
    dashboard: &DashboardData,
    currency: &CurrencyFormatter,
    source_label: &str,
    stamp: &str,
) -> ReportDataset {
    let note = Some(copy().reports.sample_no_raw_archive.clone());
    let metadata = report_metadata(request, currency, source_label, stamp, note);
    let projects = dashboard
        .projects
        .iter()
        .map(|row| ReportProject {
            project: row.name.to_string(),
            project_identity: if request.redacted {
                "Project 1".into()
            } else {
                row.name.to_string()
            },
            raw_projects: if request.redacted {
                "Project 1".into()
            } else {
                row.name.to_string()
            },
            cost: row.cost.to_string(),
            cost_usd: parse_display_money_value(row.cost),
            sessions: row.sessions,
            tokens: row.value,
            ..Default::default()
        })
        .collect();
    let project_tools = dashboard
        .project_tools
        .iter()
        .map(|row| ReportProjectTool {
            project: row.project.to_string(),
            tool: row.tool.to_string(),
            cost: row.cost.to_string(),
            cost_usd: parse_display_money_value(row.cost),
            calls: row.calls,
            sessions: row.sessions,
            tokens: row.value,
        })
        .collect();
    let sessions = dashboard
        .sessions
        .iter()
        .enumerate()
        .map(|(idx, row)| ReportSession {
            session_key: format!("sample-session-{}", idx + 1),
            project: row.project.to_string(),
            started_at: row.date.to_string(),
            ended_at: row.date.to_string(),
            cost: row.cost.to_string(),
            cost_usd: parse_display_money_value(row.cost),
            calls: row.calls,
            tokens: row.value,
            ..Default::default()
        })
        .collect();
    let activity = dashboard
        .daily
        .iter()
        .enumerate()
        .map(|(idx, row)| ReportActivity {
            date: sample_report_date(request.period, idx, dashboard.daily.len()),
            cost: row.cost.to_string(),
            cost_usd: parse_display_money_value(row.cost),
            calls: row.calls,
            tokens: row.value,
            intensity: row.value,
            ..Default::default()
        })
        .collect();
    let models = dashboard
        .models
        .iter()
        .map(|row| ReportModel {
            model: row.name.to_string(),
            cost: row.cost.to_string(),
            cost_usd: parse_display_money_value(row.cost),
            calls: row.calls,
            tokens: row.value,
            cache_read_rate: row.cache_rate.to_string(),
            ..Default::default()
        })
        .collect();
    let tools = dashboard
        .tools
        .iter()
        .map(|row| ReportCount {
            name: row.name.to_string(),
            calls: row.calls,
            tokens: row.value,
            ..Default::default()
        })
        .collect();
    let commands = dashboard
        .commands
        .iter()
        .map(|row| ReportCount {
            name: row.name.to_string(),
            calls: row.calls,
            tokens: row.value,
            ..Default::default()
        })
        .collect();
    let mcp_servers = dashboard
        .mcp_servers
        .iter()
        .map(|row| ReportCount {
            name: row.name.to_string(),
            calls: row.calls,
            tokens: row.value,
            ..Default::default()
        })
        .collect();

    ReportDataset {
        metadata,
        summary: ReportSummary {
            cost: dashboard.summary.cost.to_string(),
            cost_usd: parse_display_money_value(dashboard.summary.cost),
            calls: parse_display_u64(dashboard.summary.calls),
            sessions: parse_display_u64(dashboard.summary.sessions),
            input_tokens: parse_compact(dashboard.summary.input),
            output_tokens: parse_compact(dashboard.summary.output),
            cache_read_tokens: parse_compact(dashboard.summary.cached),
            cache_write_tokens: parse_compact(dashboard.summary.written),
            total_tokens: parse_compact(dashboard.summary.input)
                .saturating_add(parse_compact(dashboard.summary.output))
                .saturating_add(parse_compact(dashboard.summary.cached))
                .saturating_add(parse_compact(dashboard.summary.written)),
            cache_hit_rate: dashboard.summary.cache_hit.to_string(),
            ..Default::default()
        },
        activity,
        projects,
        project_tools,
        sessions,
        calls: Vec::new(),
        models,
        tools,
        commands,
        mcp_servers,
        limits_latest: Vec::new(),
        limits_raw: Vec::new(),
    }
}

fn report_metadata(
    request: &ReportRequest,
    currency: &CurrencyFormatter,
    source_label: &str,
    stamp: &str,
    sample_note: Option<String>,
) -> ReportMetadata {
    ReportMetadata {
        report_id: stamp.to_string(),
        generated_at: Local::now().format("%Y-%m-%d %H:%M:%S %Z").to_string(),
        source: source_label.to_string(),
        currency: currency.code().to_string(),
        period: period_label(request.period).to_string(),
        project: request.scope.redacted_label(request.redacted),
        redacted: request.redacted,
        sample_note,
    }
}

fn summarize_calls(calls: &[&ParsedCall], currency: &CurrencyFormatter) -> ReportSummary {
    let cost_usd = calls.iter().map(|call| call.cost_usd).sum::<f64>();
    let input_tokens = calls.iter().map(|call| call.input_tokens).sum();
    let output_tokens = calls.iter().map(|call| call.output_tokens).sum();
    let cache_read_tokens = calls.iter().map(|call| call.cache_read_input_tokens).sum();
    let cache_write_tokens = calls
        .iter()
        .map(|call| call.cache_creation_input_tokens)
        .sum();
    let cached_input_tokens = calls.iter().map(|call| call.cached_input_tokens).sum();
    let reasoning_tokens = calls.iter().map(|call| call.reasoning_tokens).sum();
    let web_search_requests = calls.iter().map(|call| call.web_search_requests).sum();
    let sessions = calls
        .iter()
        .filter_map(|call| session_key(call))
        .collect::<HashSet<_>>()
        .len() as u64;
    let cache_denom = input_tokens + cache_read_tokens + cache_write_tokens;
    let cache_hit_rate = if cache_denom == 0 {
        "-".into()
    } else {
        format!(
            "{:.1}%",
            (cache_read_tokens as f64 / cache_denom as f64) * 100.0
        )
    };

    ReportSummary {
        cost: currency.format_money(cost_usd),
        cost_usd,
        calls: calls.len() as u64,
        sessions,
        input_tokens,
        output_tokens,
        cache_read_tokens,
        cache_write_tokens,
        cached_input_tokens,
        reasoning_tokens,
        web_search_requests,
        total_tokens: input_tokens
            .saturating_add(output_tokens)
            .saturating_add(cache_read_tokens)
            .saturating_add(cache_write_tokens)
            .saturating_add(reasoning_tokens),
        cache_hit_rate,
    }
}

#[derive(Default)]
struct Totals {
    cost_usd: f64,
    calls: u64,
    sessions: HashSet<String>,
    tokens: u64,
    input_tokens: u64,
    output_tokens: u64,
    cache_read_tokens: u64,
    cache_write_tokens: u64,
    cached_input_tokens: u64,
    reasoning_tokens: u64,
    web_search_requests: u64,
    first: Option<DateTime<Utc>>,
    last: Option<DateTime<Utc>>,
}

impl Totals {
    fn add_call(&mut self, call: &ParsedCall) {
        self.cost_usd += call.cost_usd;
        self.calls += 1;
        self.input_tokens = self.input_tokens.saturating_add(call.input_tokens);
        self.output_tokens = self.output_tokens.saturating_add(call.output_tokens);
        self.cache_read_tokens = self
            .cache_read_tokens
            .saturating_add(call.cache_read_input_tokens);
        self.cache_write_tokens = self
            .cache_write_tokens
            .saturating_add(call.cache_creation_input_tokens);
        self.cached_input_tokens = self
            .cached_input_tokens
            .saturating_add(call.cached_input_tokens);
        self.reasoning_tokens = self.reasoning_tokens.saturating_add(call.reasoning_tokens);
        self.web_search_requests = self
            .web_search_requests
            .saturating_add(call.web_search_requests);
        self.tokens = self.tokens.saturating_add(call_tokens(call));
        if let Some(key) = session_key(call) {
            self.sessions.insert(key);
        }
        if let Some(ts) = call.timestamp {
            self.first = Some(self.first.map(|first| first.min(ts)).unwrap_or(ts));
            self.last = Some(self.last.map(|last| last.max(ts)).unwrap_or(ts));
        }
    }
}

fn aggregate_report_activity(
    calls: &[&ParsedCall],
    currency: &CurrencyFormatter,
) -> Vec<ReportActivity> {
    let mut by_day: BTreeMap<NaiveDate, Totals> = BTreeMap::new();
    for call in calls {
        let Some(ts) = call.timestamp else {
            continue;
        };
        let date = ts.with_timezone(&Local).date_naive();
        by_day.entry(date).or_default().add_call(call);
    }
    let max_tokens = by_day
        .values()
        .map(|totals| totals.tokens)
        .max()
        .unwrap_or(0);
    by_day
        .into_iter()
        .map(|(date, totals)| ReportActivity {
            date: date.format("%Y-%m-%d").to_string(),
            cost: currency.format_money(totals.cost_usd),
            cost_usd: totals.cost_usd,
            calls: totals.calls,
            tokens: totals.tokens,
            input_tokens: totals.input_tokens,
            output_tokens: totals.output_tokens,
            cache_read_tokens: totals.cache_read_tokens,
            cache_write_tokens: totals.cache_write_tokens,
            intensity: scale_u64(totals.tokens, max_tokens),
        })
        .collect()
}

fn aggregate_report_projects(
    calls: &[&ParsedCall],
    labels: &HashMap<String, String>,
    currency: &CurrencyFormatter,
    redactor: &mut Redactor,
) -> Vec<ReportProject> {
    let mut by_project: HashMap<String, (Totals, HashSet<String>)> = HashMap::new();
    for call in calls {
        let identity = project_identity(&call.project);
        let entry = by_project.entry(identity).or_default();
        entry.0.add_call(call);
        entry.1.insert(raw_project_display(&call.project));
    }
    let mut rows: Vec<_> = by_project.into_iter().collect();
    rows.sort_by(|a, b| {
        b.1 .0
            .cost_usd
            .partial_cmp(&a.1 .0.cost_usd)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| project_label(labels, &a.0).cmp(&project_label(labels, &b.0)))
    });
    rows.into_iter()
        .map(|(identity, (totals, raw_projects))| ReportProject {
            project: redactor.project_label(project_label(labels, &identity)),
            project_identity: redactor.project_path(&identity),
            raw_projects: raw_projects
                .into_iter()
                .map(|raw| redactor.project_path(&raw))
                .collect::<Vec<_>>()
                .join("; "),
            cost: currency.format_money(totals.cost_usd),
            cost_usd: totals.cost_usd,
            calls: totals.calls,
            sessions: totals.sessions.len() as u64,
            tokens: totals.tokens,
        })
        .collect()
}

fn aggregate_report_project_tools(
    calls: &[&ParsedCall],
    labels: &HashMap<String, String>,
    currency: &CurrencyFormatter,
    redactor: &mut Redactor,
) -> Vec<ReportProjectTool> {
    let mut by_pair: HashMap<(String, &'static str), Totals> = HashMap::new();
    for call in calls {
        by_pair
            .entry((project_identity(&call.project), call.tool))
            .or_default()
            .add_call(call);
    }
    let mut rows: Vec<_> = by_pair.into_iter().collect();
    rows.sort_by(|a, b| {
        b.1.cost_usd
            .partial_cmp(&a.1.cost_usd)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    rows.into_iter()
        .map(|((identity, tool), totals)| ReportProjectTool {
            project: redactor.project_label(project_label(labels, &identity)),
            tool: tool_short_label(tool).to_string(),
            cost: currency.format_money(totals.cost_usd),
            cost_usd: totals.cost_usd,
            calls: totals.calls,
            sessions: totals.sessions.len() as u64,
            tokens: totals.tokens,
        })
        .collect()
}

fn aggregate_report_sessions(
    calls: &[&ParsedCall],
    labels: &HashMap<String, String>,
    currency: &CurrencyFormatter,
    redactor: &mut Redactor,
) -> Vec<ReportSession> {
    let mut by_session: HashMap<String, (Totals, &'static str, String, String)> = HashMap::new();
    for call in calls {
        let Some(key) = session_key(call) else {
            continue;
        };
        let entry = by_session.entry(key).or_insert_with(|| {
            (
                Totals::default(),
                call.tool,
                call.session_id.clone(),
                project_identity(&call.project),
            )
        });
        entry.0.add_call(call);
    }
    let mut rows: Vec<_> = by_session.into_iter().collect();
    rows.sort_by_key(|(_, (totals, _, _, _))| std::cmp::Reverse(totals.last));
    rows.into_iter()
        .map(|(key, (totals, tool, session_id, project))| {
            let duration_minutes = match (totals.first, totals.last) {
                (Some(first), Some(last)) => (last - first).num_minutes().max(0),
                _ => 0,
            };
            ReportSession {
                session_key: redactor.session_id(&key),
                session_id: redactor.session_id(&session_id),
                project: redactor.project_label(project_label(labels, &project)),
                tool: tool_short_label(tool).to_string(),
                started_at: format_ts(totals.first),
                ended_at: format_ts(totals.last),
                duration_minutes,
                cost: currency.format_money(totals.cost_usd),
                cost_usd: totals.cost_usd,
                calls: totals.calls,
                tokens: totals.tokens,
                input_tokens: totals.input_tokens,
                output_tokens: totals.output_tokens,
                cache_read_tokens: totals.cache_read_tokens,
                cache_write_tokens: totals.cache_write_tokens,
                reasoning_tokens: totals.reasoning_tokens,
                web_search_requests: totals.web_search_requests,
            }
        })
        .collect()
}

fn report_calls(
    calls: &[&ParsedCall],
    labels: &HashMap<String, String>,
    currency: &CurrencyFormatter,
    redactor: &mut Redactor,
) -> Vec<ReportCall> {
    let mut rows = calls.to_vec();
    rows.sort_by_key(|call| call.timestamp);
    rows.into_iter()
        .map(|call| {
            let project = project_identity(&call.project);
            let prompt = clean_text(&call.user_message);
            ReportCall {
                timestamp: format_ts(call.timestamp),
                tool: tool_short_label(call.tool).to_string(),
                model: call.model.clone(),
                project: redactor.project_label(project_label(labels, &project)),
                raw_project: redactor.project_path(&raw_project_display(&call.project)),
                session_key: session_key(call)
                    .map(|key| redactor.session_id(&key))
                    .unwrap_or_default(),
                session_id: redactor.session_id(&call.session_id),
                dedup_key: redactor.dedup_key(&call.dedup_key),
                cost: currency.format_money(call.cost_usd),
                cost_usd: call.cost_usd,
                input_tokens: call.input_tokens,
                output_tokens: call.output_tokens,
                cache_read_tokens: call.cache_read_input_tokens,
                cache_write_tokens: call.cache_creation_input_tokens,
                cache_read_rate: pricing::cache_read_rate_label(&call.model),
                cache_write_rate: pricing::cache_write_rate_label(&call.model),
                cached_input_tokens: call.cached_input_tokens,
                reasoning_tokens: call.reasoning_tokens,
                web_search_requests: call.web_search_requests,
                tools: call.tools.join(", "),
                bash_commands: redactor.bash_commands(&call.bash_commands),
                prompt_preview: redactor.prompt_preview(&prompt),
                prompt_full: redactor.prompt(&prompt),
            }
        })
        .collect()
}

fn aggregate_report_models(
    calls: &[&ParsedCall],
    currency: &CurrencyFormatter,
) -> Vec<ReportModel> {
    let mut by_model: HashMap<(String, &'static str), Totals> = HashMap::new();
    for call in calls {
        by_model
            .entry((call.model.clone(), call.tool))
            .or_default()
            .add_call(call);
    }
    let mut rows: Vec<_> = by_model.into_iter().collect();
    rows.sort_by(|a, b| {
        b.1.cost_usd
            .partial_cmp(&a.1.cost_usd)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0 .0.cmp(&b.0 .0))
    });
    rows.into_iter()
        .map(|((model, tool), totals)| {
            let cache_read_rate = pricing::cache_read_rate_label(&model);
            ReportModel {
                model,
                tool: tool_short_label(tool).to_string(),
                cost: currency.format_money(totals.cost_usd),
                cost_usd: totals.cost_usd,
                calls: totals.calls,
                tokens: totals.tokens,
                cache_read_tokens: totals.cache_read_tokens,
                cache_read_rate,
            }
        })
        .collect()
}

fn aggregate_report_tools(calls: &[&ParsedCall], currency: &CurrencyFormatter) -> Vec<ReportCount> {
    let mut counts: HashMap<String, Totals> = HashMap::new();
    for call in calls {
        counts
            .entry(tool_short_label(call.tool).to_string())
            .or_default()
            .add_call(call);
    }
    report_count_rows(counts, currency)
}

fn aggregate_report_commands(
    calls: &[&ParsedCall],
    currency: &CurrencyFormatter,
) -> Vec<ReportCount> {
    let mut counts: HashMap<String, Totals> = HashMap::new();
    for call in calls {
        for command in &call.bash_commands {
            counts
                .entry(first_word(command).to_string())
                .or_default()
                .add_call(call);
        }
    }
    report_count_rows(counts, currency)
}

fn aggregate_report_mcp_servers(
    calls: &[&ParsedCall],
    currency: &CurrencyFormatter,
) -> Vec<ReportCount> {
    let mut counts: HashMap<String, Totals> = HashMap::new();
    for call in calls {
        for tool in &call.tools {
            let Some(rest) = tool.strip_prefix("mcp__") else {
                continue;
            };
            let server = rest.split("__").next().unwrap_or(rest);
            counts.entry(server.to_string()).or_default().add_call(call);
        }
    }
    report_count_rows(counts, currency)
}

fn report_count_rows(
    counts: HashMap<String, Totals>,
    currency: &CurrencyFormatter,
) -> Vec<ReportCount> {
    let mut rows: Vec<_> = counts.into_iter().collect();
    rows.sort_by(|a, b| {
        b.1.calls
            .cmp(&a.1.calls)
            .then_with(|| b.1.tokens.cmp(&a.1.tokens))
            .then_with(|| a.0.cmp(&b.0))
    });
    rows.into_iter()
        .map(|(name, totals)| ReportCount {
            name,
            calls: totals.calls,
            tokens: totals.tokens,
            cost: currency.format_money(totals.cost_usd),
            cost_usd: totals.cost_usd,
        })
        .collect()
}

fn filter_limits(
    limits: &[LimitSnapshot],
    period: Period,
    now: DateTime<Local>,
) -> Vec<&LimitSnapshot> {
    limits
        .iter()
        .filter(|limit| in_report_period(limit.observed_at, period, now))
        .collect()
}

fn latest_limits(limits: &[&LimitSnapshot]) -> Vec<ReportLimitLatest> {
    let mut latest: HashMap<(&'static str, String), &LimitSnapshot> = HashMap::new();
    for limit in limits {
        let key = (limit.tool, limit.limit_id.clone());
        match latest.get(&key) {
            Some(existing) if existing.observed_at >= limit.observed_at => {}
            _ => {
                latest.insert(key, limit);
            }
        }
    }
    let mut rows: Vec<_> = latest.into_values().collect();
    rows.sort_by(|a, b| {
        tool_short_label(a.tool)
            .cmp(tool_short_label(b.tool))
            .then_with(|| a.limit_id.cmp(&b.limit_id))
    });
    rows.into_iter()
        .map(|limit| ReportLimitLatest {
            tool: tool_short_label(limit.tool).to_string(),
            limit_id: limit.limit_id.clone(),
            limit_name: limit.limit_name.clone().unwrap_or_default(),
            plan_type: limit.plan_type.clone().unwrap_or_default(),
            observed_at: format_ts(limit.observed_at),
            primary_used_percent: limit.primary.map(|window| window.used_percent),
            secondary_used_percent: limit.secondary.map(|window| window.used_percent),
            credits_balance: limit.credits.as_ref().and_then(|credits| credits.balance),
            rate_limit_reached_type: limit.rate_limit_reached_type.clone().unwrap_or_default(),
        })
        .collect()
}

fn raw_limits(limits: &[&LimitSnapshot]) -> Vec<ReportLimitRaw> {
    let mut rows: Vec<_> = limits.to_vec();
    rows.sort_by_key(|limit| limit.observed_at);
    rows.into_iter()
        .map(|limit| ReportLimitRaw {
            tool: tool_short_label(limit.tool).to_string(),
            limit_id: limit.limit_id.clone(),
            limit_name: limit.limit_name.clone().unwrap_or_default(),
            plan_type: limit.plan_type.clone().unwrap_or_default(),
            observed_at: format_ts(limit.observed_at),
            primary_json: json_string(&limit.primary),
            secondary_json: json_string(&limit.secondary),
            credits_json: json_string(&limit.credits),
            rate_limit_reached_type: limit.rate_limit_reached_type.clone().unwrap_or_default(),
        })
        .collect()
}

fn json_string<T: Serialize>(value: &Option<T>) -> String {
    value
        .as_ref()
        .and_then(|value| serde_json::to_string(value).ok())
        .unwrap_or_default()
}

fn write_csv_report_dir(dir: &Path, dataset: &ReportDataset) -> Result<()> {
    write_csv(
        dir,
        "summary.csv",
        &[
            "cost",
            "cost_usd",
            "calls",
            "sessions",
            "input_tokens",
            "output_tokens",
            "cache_read_tokens",
            "cache_write_tokens",
            "reasoning_tokens",
            "web_search_requests",
            "total_tokens",
            "cache_hit_rate",
        ],
        &[vec![
            dataset.summary.cost.clone(),
            dataset.summary.cost_usd.to_string(),
            dataset.summary.calls.to_string(),
            dataset.summary.sessions.to_string(),
            dataset.summary.input_tokens.to_string(),
            dataset.summary.output_tokens.to_string(),
            dataset.summary.cache_read_tokens.to_string(),
            dataset.summary.cache_write_tokens.to_string(),
            dataset.summary.reasoning_tokens.to_string(),
            dataset.summary.web_search_requests.to_string(),
            dataset.summary.total_tokens.to_string(),
            dataset.summary.cache_hit_rate.clone(),
        ]],
    )?;
    write_csv_rows(dir, "activity.csv", activity_csv(&dataset.activity))?;
    write_csv_rows(dir, "projects.csv", projects_csv(&dataset.projects))?;
    write_csv_rows(
        dir,
        "project_tools.csv",
        project_tools_csv(&dataset.project_tools),
    )?;
    write_csv_rows(dir, "sessions.csv", sessions_csv(&dataset.sessions))?;
    write_csv_rows(dir, "calls.csv", calls_csv(&dataset.calls))?;
    write_csv_rows(dir, "models.csv", models_csv(&dataset.models))?;
    write_csv_rows(dir, "tools.csv", counts_csv(&dataset.tools))?;
    write_csv_rows(dir, "commands.csv", counts_csv(&dataset.commands))?;
    write_csv_rows(dir, "mcp_servers.csv", counts_csv(&dataset.mcp_servers))?;
    write_csv_rows(
        dir,
        "limits_latest.csv",
        limits_latest_csv(&dataset.limits_latest),
    )?;
    write_csv_rows(dir, "limits_raw.csv", limits_raw_csv(&dataset.limits_raw))?;
    write_csv_rows(dir, "metadata.csv", metadata_csv(&dataset.metadata))?;
    Ok(())
}

fn write_csv_rows(dir: &Path, name: &str, rows: CsvRows) -> Result<()> {
    write_csv(dir, name, &rows.headers, &rows.rows)
}

struct CsvRows {
    headers: Vec<&'static str>,
    rows: Vec<Vec<String>>,
}

fn activity_csv(rows: &[ReportActivity]) -> CsvRows {
    CsvRows {
        headers: vec![
            "date",
            "cost",
            "cost_usd",
            "calls",
            "tokens",
            "input_tokens",
            "output_tokens",
            "cache_read_tokens",
            "cache_write_tokens",
            "intensity",
        ],
        rows: rows
            .iter()
            .map(|row| {
                vec![
                    row.date.clone(),
                    row.cost.clone(),
                    row.cost_usd.to_string(),
                    row.calls.to_string(),
                    row.tokens.to_string(),
                    row.input_tokens.to_string(),
                    row.output_tokens.to_string(),
                    row.cache_read_tokens.to_string(),
                    row.cache_write_tokens.to_string(),
                    row.intensity.to_string(),
                ]
            })
            .collect(),
    }
}

fn projects_csv(rows: &[ReportProject]) -> CsvRows {
    CsvRows {
        headers: vec![
            "project",
            "project_identity",
            "raw_projects",
            "cost",
            "cost_usd",
            "calls",
            "sessions",
            "tokens",
        ],
        rows: rows
            .iter()
            .map(|row| {
                vec![
                    row.project.clone(),
                    row.project_identity.clone(),
                    row.raw_projects.clone(),
                    row.cost.clone(),
                    row.cost_usd.to_string(),
                    row.calls.to_string(),
                    row.sessions.to_string(),
                    row.tokens.to_string(),
                ]
            })
            .collect(),
    }
}

fn project_tools_csv(rows: &[ReportProjectTool]) -> CsvRows {
    CsvRows {
        headers: vec![
            "project", "tool", "cost", "cost_usd", "calls", "sessions", "tokens",
        ],
        rows: rows
            .iter()
            .map(|row| {
                vec![
                    row.project.clone(),
                    row.tool.clone(),
                    row.cost.clone(),
                    row.cost_usd.to_string(),
                    row.calls.to_string(),
                    row.sessions.to_string(),
                    row.tokens.to_string(),
                ]
            })
            .collect(),
    }
}

fn sessions_csv(rows: &[ReportSession]) -> CsvRows {
    CsvRows {
        headers: vec![
            "session_key",
            "session_id",
            "project",
            "tool",
            "started_at",
            "ended_at",
            "duration_minutes",
            "cost",
            "cost_usd",
            "calls",
            "tokens",
            "input_tokens",
            "output_tokens",
            "cache_read_tokens",
            "cache_write_tokens",
            "reasoning_tokens",
            "web_search_requests",
        ],
        rows: rows
            .iter()
            .map(|row| {
                vec![
                    row.session_key.clone(),
                    row.session_id.clone(),
                    row.project.clone(),
                    row.tool.clone(),
                    row.started_at.clone(),
                    row.ended_at.clone(),
                    row.duration_minutes.to_string(),
                    row.cost.clone(),
                    row.cost_usd.to_string(),
                    row.calls.to_string(),
                    row.tokens.to_string(),
                    row.input_tokens.to_string(),
                    row.output_tokens.to_string(),
                    row.cache_read_tokens.to_string(),
                    row.cache_write_tokens.to_string(),
                    row.reasoning_tokens.to_string(),
                    row.web_search_requests.to_string(),
                ]
            })
            .collect(),
    }
}

fn calls_csv(rows: &[ReportCall]) -> CsvRows {
    CsvRows {
        headers: vec![
            "timestamp",
            "tool",
            "model",
            "project",
            "raw_project",
            "session_key",
            "session_id",
            "dedup_key",
            "cost",
            "cost_usd",
            "input_tokens",
            "output_tokens",
            "cache_read_tokens",
            "cache_write_tokens",
            "cache_read_rate",
            "cache_write_rate",
            "cached_input_tokens",
            "reasoning_tokens",
            "web_search_requests",
            "tools",
            "bash_commands",
            "prompt_preview",
            "prompt_full",
        ],
        rows: rows
            .iter()
            .map(|row| {
                vec![
                    row.timestamp.clone(),
                    row.tool.clone(),
                    row.model.clone(),
                    row.project.clone(),
                    row.raw_project.clone(),
                    row.session_key.clone(),
                    row.session_id.clone(),
                    row.dedup_key.clone(),
                    row.cost.clone(),
                    row.cost_usd.to_string(),
                    row.input_tokens.to_string(),
                    row.output_tokens.to_string(),
                    row.cache_read_tokens.to_string(),
                    row.cache_write_tokens.to_string(),
                    row.cache_read_rate.clone(),
                    row.cache_write_rate.clone(),
                    row.cached_input_tokens.to_string(),
                    row.reasoning_tokens.to_string(),
                    row.web_search_requests.to_string(),
                    row.tools.clone(),
                    row.bash_commands.clone(),
                    row.prompt_preview.clone(),
                    row.prompt_full.clone(),
                ]
            })
            .collect(),
    }
}

fn models_csv(rows: &[ReportModel]) -> CsvRows {
    CsvRows {
        headers: vec![
            "model",
            "tool",
            "cost",
            "cost_usd",
            "calls",
            "tokens",
            "cache_read_tokens",
            "cache_read_rate",
        ],
        rows: rows
            .iter()
            .map(|row| {
                vec![
                    row.model.clone(),
                    row.tool.clone(),
                    row.cost.clone(),
                    row.cost_usd.to_string(),
                    row.calls.to_string(),
                    row.tokens.to_string(),
                    row.cache_read_tokens.to_string(),
                    row.cache_read_rate.clone(),
                ]
            })
            .collect(),
    }
}

fn counts_csv(rows: &[ReportCount]) -> CsvRows {
    CsvRows {
        headers: vec!["name", "calls", "tokens", "cost", "cost_usd"],
        rows: rows
            .iter()
            .map(|row| {
                vec![
                    row.name.clone(),
                    row.calls.to_string(),
                    row.tokens.to_string(),
                    row.cost.clone(),
                    row.cost_usd.to_string(),
                ]
            })
            .collect(),
    }
}

fn limits_latest_csv(rows: &[ReportLimitLatest]) -> CsvRows {
    CsvRows {
        headers: vec![
            "tool",
            "limit_id",
            "limit_name",
            "plan_type",
            "observed_at",
            "primary_used_percent",
            "secondary_used_percent",
            "credits_balance",
            "rate_limit_reached_type",
        ],
        rows: rows
            .iter()
            .map(|row| {
                vec![
                    row.tool.clone(),
                    row.limit_id.clone(),
                    row.limit_name.clone(),
                    row.plan_type.clone(),
                    row.observed_at.clone(),
                    opt_f64(row.primary_used_percent),
                    opt_f64(row.secondary_used_percent),
                    opt_f64(row.credits_balance),
                    row.rate_limit_reached_type.clone(),
                ]
            })
            .collect(),
    }
}

fn limits_raw_csv(rows: &[ReportLimitRaw]) -> CsvRows {
    CsvRows {
        headers: vec![
            "tool",
            "limit_id",
            "limit_name",
            "plan_type",
            "observed_at",
            "primary_json",
            "secondary_json",
            "credits_json",
            "rate_limit_reached_type",
        ],
        rows: rows
            .iter()
            .map(|row| {
                vec![
                    row.tool.clone(),
                    row.limit_id.clone(),
                    row.limit_name.clone(),
                    row.plan_type.clone(),
                    row.observed_at.clone(),
                    row.primary_json.clone(),
                    row.secondary_json.clone(),
                    row.credits_json.clone(),
                    row.rate_limit_reached_type.clone(),
                ]
            })
            .collect(),
    }
}

fn metadata_csv(metadata: &ReportMetadata) -> CsvRows {
    CsvRows {
        headers: vec!["name", "value"],
        rows: vec![
            vec!["report_id".into(), metadata.report_id.clone()],
            vec!["generated_at".into(), metadata.generated_at.clone()],
            vec!["source".into(), metadata.source.clone()],
            vec!["currency".into(), metadata.currency.clone()],
            vec!["period".into(), metadata.period.clone()],
            vec!["project".into(), metadata.project.clone()],
            vec!["redacted".into(), metadata.redacted.to_string()],
            vec![
                "sample_note".into(),
                metadata.sample_note.clone().unwrap_or_default(),
            ],
        ],
    }
}

fn write_csv(dir: &Path, name: &str, header: &[&str], rows: &[Vec<String>]) -> Result<()> {
    let path = dir.join(name);
    let mut out = String::with_capacity(rows.len() * 96);
    push_csv_row(&mut out, header.iter().copied());
    for row in rows {
        push_csv_row(&mut out, row.iter().map(String::as_str));
    }
    fs::write(&path, out).wrap_err_with(|| format!("write {}", path.display()))
}

fn push_csv_row<'a, I>(out: &mut String, cells: I)
where
    I: IntoIterator<Item = &'a str>,
{
    for (idx, cell) in cells.into_iter().enumerate() {
        if idx > 0 {
            out.push(',');
        }
        out.push_str(&csv_escape(cell));
    }
    out.push('\n');
}

fn csv_escape(value: &str) -> String {
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

fn write_xlsx_report(path: &Path, dataset: &ReportDataset) -> Result<()> {
    let mut workbook = Workbook::new();
    let header = Format::new()
        .set_bold()
        .set_font_color(Color::White)
        .set_background_color(Color::RGB(0x202438))
        .set_border(FormatBorder::Thin);
    let money = Format::new().set_num_format("$#,##0.00");
    let integer = Format::new().set_num_format("#,##0");
    let title = Format::new()
        .set_bold()
        .set_font_size(16.0)
        .set_font_color(Color::RGB(0xFF8F40));

    write_summary_sheet(&mut workbook, dataset, &title, &header, &money, &integer)?;
    write_sheet(
        &mut workbook,
        "Activity",
        activity_csv(&dataset.activity),
        &header,
    )?;
    write_sheet(
        &mut workbook,
        "Projects",
        projects_csv(&dataset.projects),
        &header,
    )?;
    write_sheet(
        &mut workbook,
        "Project Tools",
        project_tools_csv(&dataset.project_tools),
        &header,
    )?;
    write_sheet(
        &mut workbook,
        "Sessions",
        sessions_csv(&dataset.sessions),
        &header,
    )?;
    write_sheet(&mut workbook, "Calls", calls_csv(&dataset.calls), &header)?;
    write_sheet(
        &mut workbook,
        "Models",
        models_csv(&dataset.models),
        &header,
    )?;
    write_sheet(&mut workbook, "Tools", counts_csv(&dataset.tools), &header)?;
    write_sheet(
        &mut workbook,
        "Commands",
        counts_csv(&dataset.commands),
        &header,
    )?;
    write_sheet(
        &mut workbook,
        "MCP Servers",
        counts_csv(&dataset.mcp_servers),
        &header,
    )?;
    write_sheet(
        &mut workbook,
        "Limits Latest",
        limits_latest_csv(&dataset.limits_latest),
        &header,
    )?;
    write_sheet(
        &mut workbook,
        "Limits Raw",
        limits_raw_csv(&dataset.limits_raw),
        &header,
    )?;
    write_sheet(
        &mut workbook,
        "Metadata",
        metadata_csv(&dataset.metadata),
        &header,
    )?;
    workbook
        .save(path)
        .wrap_err_with(|| format!("write {}", path.display()))
}

fn write_summary_sheet(
    workbook: &mut Workbook,
    dataset: &ReportDataset,
    title: &Format,
    header: &Format,
    money: &Format,
    integer: &Format,
) -> Result<()> {
    let worksheet = workbook.add_worksheet();
    worksheet.set_name("Summary")?;
    worksheet.write_with_format(0, 0, "Token Use Report", title)?;
    worksheet.write_string(1, 0, &dataset.metadata.period)?;
    worksheet.write_string(1, 1, &dataset.metadata.project)?;
    worksheet.write_with_format(3, 0, "Metric", header)?;
    worksheet.write_with_format(3, 1, "Value", header)?;
    let metrics = [
        ("Cost", dataset.summary.cost.clone()),
        ("Calls", dataset.summary.calls.to_string()),
        ("Sessions", dataset.summary.sessions.to_string()),
        ("Total tokens", dataset.summary.total_tokens.to_string()),
        ("Input tokens", dataset.summary.input_tokens.to_string()),
        ("Output tokens", dataset.summary.output_tokens.to_string()),
        (
            "Cache read tokens",
            dataset.summary.cache_read_tokens.to_string(),
        ),
        (
            "Cache write tokens",
            dataset.summary.cache_write_tokens.to_string(),
        ),
        ("Cache hit rate", dataset.summary.cache_hit_rate.clone()),
    ];
    for (idx, (name, value)) in metrics.iter().enumerate() {
        let row = (idx + 4) as u32;
        worksheet.write_string(row, 0, *name)?;
        worksheet.write_string(row, 1, value)?;
    }
    worksheet.write_with_format(14, 0, "Cost USD", header)?;
    worksheet.write_with_format(14, 1, dataset.summary.cost_usd, money)?;
    worksheet.write_with_format(15, 0, "Raw calls", header)?;
    worksheet.write_with_format(15, 1, dataset.calls.len() as u64, integer)?;
    worksheet.set_column_width(0, 24.0)?;
    worksheet.set_column_width(1, 28.0)?;
    worksheet.set_freeze_panes(4, 0)?;
    Ok(())
}

fn write_sheet(workbook: &mut Workbook, name: &str, rows: CsvRows, header: &Format) -> Result<()> {
    let worksheet = workbook.add_worksheet();
    worksheet.set_name(name)?;
    write_row(worksheet, 0, &rows.headers, Some(header))?;
    for (idx, row) in rows.rows.iter().enumerate() {
        write_row(worksheet, (idx + 1) as u32, row, None)?;
    }
    worksheet.set_freeze_panes(1, 0)?;
    worksheet.autofit();
    Ok(())
}

fn write_row<S: AsRef<str>>(
    worksheet: &mut Worksheet,
    row: u32,
    cells: &[S],
    format: Option<&Format>,
) -> Result<()> {
    for (col, cell) in cells.iter().enumerate() {
        match format {
            Some(format) => worksheet.write_with_format(row, col as u16, cell.as_ref(), format)?,
            None => worksheet.write_string(row, col as u16, cell.as_ref())?,
        };
    }
    Ok(())
}

const REPORT_MARK_SVG: &str = r##"<svg class="brand-mark" viewBox="0 0 440 560" aria-hidden="true" xmlns="http://www.w3.org/2000/svg"><rect x="0" y="280" width="80" height="280" rx="14" fill="#f2b15d"/><rect x="120" y="160" width="80" height="400" rx="14" fill="#ed8a47"/><rect x="240" y="0" width="80" height="560" rx="14" fill="#df6f3f"/><rect x="360" y="120" width="80" height="440" rx="14" fill="#c95b44"/></svg>"##;

const REPORT_CSS: &str = r#"
:root {
  color-scheme: light;
  --page: #f5f2ec;
  --paper: #ffffff;
  --ink: #252b37;
  --muted: #667085;
  --quiet: #98a2b3;
  --line: #d9dee8;
  --soft: #f7f8fb;
  --accent: #df6f3f;
  --accent-soft: #fff0e8;
  --teal: #168a7a;
  --teal-soft: #e4f4ef;
  --blue: #3478c7;
  --blue-soft: #e8f1fb;
  --coral: #d95f68;
  --coral-soft: #fdebed;
  --gold: #c9971f;
  --gold-soft: #fff5d7;
}
@page { size: A4 landscape; margin: 10mm; }
* { box-sizing: border-box; }
body {
  margin: 0;
  background: var(--page);
  color: var(--ink);
  font: 14px/1.45 "Aptos", "Avenir Next", "Helvetica Neue", "Segoe UI", sans-serif;
}
.report-deck { padding: 10px 0; }
.report-page {
  width: min(1080px, calc(100% - 36px));
  min-height: 640px;
  margin: 0 auto 16px;
  padding: 28px 34px;
  background: var(--paper);
  border: 1px solid rgba(37, 43, 55, .08);
  border-radius: 8px;
  box-shadow: 0 20px 42px rgba(37, 43, 55, .08);
  break-after: page;
  break-inside: avoid;
  overflow: hidden;
}
.report-page:last-child { break-after: auto; }
.deck-header,
.brand-lockup,
.section-title,
.rank-row,
.note-row {
  display: flex;
  align-items: center;
}
.deck-header {
  justify-content: space-between;
  gap: 24px;
  padding-bottom: 14px;
  border-bottom: 1px solid var(--line);
}
.brand-lockup { gap: 10px; color: var(--muted); font-weight: 700; letter-spacing: .02em; }
.brand-mark { width: 20px; height: 26px; display: block; }
.deck-kicker,
.meta-label,
.kpi-label,
.insight-label,
.panel-label,
.rank-value,
.footer-note {
  color: var(--muted);
  font-size: 11px;
  font-weight: 800;
  letter-spacing: .12em;
  text-transform: uppercase;
}
.deck-meta { color: var(--muted); font-size: 12px; text-align: right; }
h1, h2, h3, p { margin: 0; }
h1 {
  max-width: 690px;
  margin-top: 16px;
  color: var(--ink);
  font-size: 34px;
  line-height: 1.02;
  letter-spacing: -.03em;
}
h2 {
  color: var(--ink);
  font-size: 28px;
  line-height: 1.1;
  letter-spacing: -.02em;
}
h3 { color: var(--ink); font-size: 17px; line-height: 1.2; }
.deck-subtitle {
  max-width: 640px;
  margin-top: 10px;
  color: var(--muted);
  font-size: 16px;
}
.cover-grid {
  display: grid;
  grid-template-columns: minmax(0, 1.28fr) minmax(340px, .72fr);
  align-items: stretch;
  gap: 18px;
  margin-top: 18px;
}
.overview-kpi-area {
  display: flex;
  flex-direction: column;
  min-height: 300px;
}
.meta-panel,
.kpi-card,
.insight-card,
.report-panel,
.raw-note {
  background: var(--paper);
  border: 1px solid var(--line);
  border-radius: 8px;
}
.meta-panel {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 12px 14px;
  height: 100%;
  padding: 15px;
  background: linear-gradient(180deg, #fff, var(--soft));
}
.meta-item + .meta-item { margin-top: 0; padding-top: 0; border-top: 0; }
.meta-value {
  display: block;
  margin-top: 3px;
  color: var(--ink);
  font-size: 13px;
  font-weight: 750;
  overflow-wrap: anywhere;
}
.kpi-ribbon {
  display: grid;
  grid-template-columns: repeat(6, minmax(0, 1fr));
  gap: 10px;
  margin-top: 22px;
}
.overview-kpis {
  grid-template-columns: repeat(3, minmax(0, 1fr));
  grid-template-rows: repeat(2, minmax(0, 1fr));
  gap: 12px;
  flex: 1;
  min-height: 300px;
  margin-top: 0;
}
.kpi-card {
  min-height: 78px;
  padding: 12px;
  background: var(--soft);
  border-top: 4px solid var(--accent);
}
.overview-kpis .kpi-card {
  display: grid;
  grid-template-rows: 44px minmax(0, 1fr);
  min-height: 0;
  padding: 0;
  overflow: hidden;
  border: 0;
}
.kpi-card:nth-child(2) { border-top-color: var(--blue); }
.kpi-card:nth-child(3) { border-top-color: var(--teal); }
.kpi-card:nth-child(4) { border-top-color: var(--gold); }
.kpi-card:nth-child(5) { border-top-color: var(--coral); }
.overview-kpis .kpi-label {
  display: flex;
  align-items: center;
  min-height: 44px;
  padding: 0 18px;
  background: var(--accent);
  color: #fff;
}
.overview-kpis .kpi-card:nth-child(2) .kpi-label { background: var(--blue); }
.overview-kpis .kpi-card:nth-child(3) .kpi-label { background: var(--teal); }
.overview-kpis .kpi-card:nth-child(4) .kpi-label { background: var(--gold); }
.overview-kpis .kpi-card:nth-child(5) .kpi-label { background: var(--coral); }
.kpi-value {
  display: block;
  margin-top: 8px;
  color: var(--ink);
  font: 750 19px/1.05 "SFMono-Regular", "Cascadia Mono", "JetBrains Mono", monospace;
  letter-spacing: -.03em;
  overflow-wrap: anywhere;
}
.overview-kpis .kpi-value {
  display: flex;
  align-items: center;
  justify-content: center;
  margin: 0;
  padding: 18px;
  text-align: center;
  font-size: 32px;
}
.insight-grid {
  display: grid;
  grid-template-columns: 1.1fr .9fr .9fr .9fr;
  gap: 14px;
  margin-top: 18px;
}
.insight-card { padding: 15px; background: #fffdf9; }
.insight-card.primary { background: var(--accent-soft); border-color: #efc9b7; }
.insight-value {
  display: block;
  margin-top: 8px;
  color: var(--ink);
  font-size: 24px;
  font-weight: 780;
  letter-spacing: -.03em;
}
.insight-detail { margin-top: 6px; color: var(--muted); }
.sample-note {
  margin-top: 18px;
  padding: 10px 12px;
  color: #8a4b18;
  background: var(--gold-soft);
  border: 1px solid #ebd59a;
  border-radius: 8px;
  font-weight: 700;
}
.section-title { justify-content: space-between; gap: 20px; margin-bottom: 18px; }
.page-number { color: var(--quiet); font-weight: 800; letter-spacing: .16em; }
.activity-layout {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 270px;
  gap: 18px;
}
.report-panel { padding: 18px; background: #fff; }
.panel-label { display: block; margin-bottom: 12px; color: var(--accent); }
.heatmap-viewport { overflow-x: auto; padding-bottom: 4px; }
.heatmap-grid {
  display: grid;
  grid-template-rows: repeat(7, 14px);
  grid-auto-flow: column;
  grid-auto-columns: 14px;
  gap: 4px;
  width: max-content;
  min-height: 126px;
}
.heat-cell {
  display: block;
  width: 14px;
  height: 14px;
  border-radius: 3px;
  background: #edf0f5;
  border: 1px solid #dfe4ec;
}
.heatmap-strip {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(82px, 1fr));
  gap: 8px;
}
.heat-tile {
  min-height: 76px;
  padding: 10px;
  border-radius: 8px;
}
.heat-tile strong,
.heat-tile span {
  display: block;
}
.heat-tile strong {
  color: var(--ink);
  font-size: 13px;
}
.heat-tile span {
  margin-top: 8px;
  color: var(--muted);
  font-size: 12px;
}
.heat-legend {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-top: 12px;
  color: var(--muted);
  font-size: 12px;
}
.trend-panel { margin-top: 18px; }
.trend-svg { display: block; width: 100%; height: auto; }
.activity-facts {
  display: grid;
  gap: 12px;
}
.fact-card {
  min-height: 112px;
  padding: 17px;
  border-radius: 8px;
  border: 1px solid var(--line);
  background: var(--soft);
}
.fact-card strong {
  display: block;
  margin-top: 8px;
  color: var(--ink);
  font: 760 24px/1.05 "SFMono-Regular", "Cascadia Mono", "JetBrains Mono", monospace;
}
.fact-card span { display: block; margin-top: 8px; color: var(--muted); }
.breakdown-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 16px;
}
.signal-strip {
  grid-column: 1 / -1;
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 16px;
}
.signal-card {
  min-height: 116px;
  padding: 14px;
  border-radius: 8px;
  border: 1px solid var(--line);
  background: var(--soft);
}
.signal-card h3 { margin-bottom: 10px; }
.signal-row {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 12px;
  padding: 7px 0;
  border-top: 1px solid #e7ebf2;
}
.signal-row:first-of-type { border-top: 0; }
.signal-row span {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-weight: 720;
}
.signal-row strong {
  color: var(--muted);
  font: 750 13px/1 "SFMono-Regular", "Cascadia Mono", "JetBrains Mono", monospace;
  white-space: nowrap;
}
.rank-card { min-height: 250px; padding: 16px; background: var(--paper); border: 1px solid var(--line); border-radius: 8px; }
.rank-card.compact { min-height: 210px; }
.rank-card h2 { margin-bottom: 14px; font-size: 18px; letter-spacing: -.01em; }
.rank-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: 12px;
  padding: 9px 0;
  border-top: 1px solid #edf0f5;
}
.rank-row:first-of-type { border-top: 0; }
.rank-name { min-width: 0; color: var(--ink); font-weight: 720; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.rank-value { font-family: "SFMono-Regular", "Cascadia Mono", "JetBrains Mono", monospace; letter-spacing: 0; text-transform: none; }
.rank-track {
  grid-column: 1 / -1;
  height: 8px;
  overflow: hidden;
  border-radius: 999px;
  background: #edf0f5;
}
.rank-fill { display: block; height: 100%; min-width: 6px; border-radius: inherit; }
.report-page[data-report-page="breakdown"] h2 {
  font-weight: 620;
}
.report-page[data-report-page="breakdown"] .rank-card h2,
.report-page[data-report-page="breakdown"] .signal-card h3 {
  font-weight: 600;
}
.report-page[data-report-page="breakdown"] .rank-name,
.report-page[data-report-page="breakdown"] .signal-row span {
  font-weight: 500;
}
.report-page[data-report-page="breakdown"] .rank-value,
.report-page[data-report-page="breakdown"] .signal-row strong {
  font-weight: 500;
}
.report-page[data-report-page="breakdown"] .deck-kicker,
.report-page[data-report-page="breakdown"] .page-number {
  font-weight: 650;
}
.raw-note {
  margin-top: 9px;
  padding: 0;
  background: transparent;
  border: 0;
  color: #31516e;
  font-size: 12px;
  line-height: 1.35;
}
.raw-note strong { color: var(--ink); }
.i0 { background: #edf0f5; border-color: #dfe4ec; }
.i1 { background: #dbe9f8; border-color: #bfd6f0; }
.i2 { background: #cfeee7; border-color: #a9ddd3; }
.i3 { background: #f8e6a7; border-color: #e6ce71; }
.i4 { background: #f4bd8f; border-color: #df9a67; }
.i5 { background: #e8797f; border-color: #d86169; }
.fill-0 { background: #dfe4ec; }
.fill-1 { background: var(--blue); }
.fill-2 { background: var(--teal); }
.fill-3 { background: var(--gold); }
.fill-4 { background: var(--accent); }
.fill-5 { background: var(--coral); }
@media (max-width: 900px) {
  .report-page { width: calc(100% - 24px); min-height: 0; padding: 24px; }
  .cover-grid, .activity-layout, .breakdown-grid, .insight-grid { grid-template-columns: 1fr; }
  .kpi-ribbon { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  .overview-kpi-area,
  .overview-kpis { min-height: 0; }
  .overview-kpis { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  .overview-kpis .kpi-card { min-height: 120px; }
}
@media print {
  body { background: #fff; }
  .report-deck { padding: 0; }
  .report-page {
    width: 100%;
    min-height: 180mm;
    height: 180mm;
    margin: 0;
    padding: 6mm;
    border: 0;
    border-radius: 0;
    box-shadow: none;
    overflow: hidden;
    page-break-after: always;
  }
  .report-page:last-child { page-break-after: auto; }
  .report-page + .report-page { padding-top: 0; }
  h1 { font-size: 28px; margin-top: 8px; max-width: 620px; }
  h2 { font-size: 20px; }
  .deck-header { padding-bottom: 10px; }
  .deck-subtitle { max-width: 560px; font-size: 13px; margin-top: 7px; }
  .cover-grid { grid-template-columns: minmax(0, 1.25fr) minmax(330px, .75fr); margin-top: 12px; gap: 14px; }
  .overview-kpi-area { min-height: 235px; }
  .meta-panel { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 8px 12px; padding: 10px; }
  .meta-item + .meta-item { margin-top: 0; padding-top: 0; border-top: 0; }
  .meta-label { font-size: 9px; }
  .meta-value { margin-top: 2px; font-size: 11px; }
  .kpi-ribbon { gap: 7px; margin-top: 12px; }
  .kpi-card { min-height: 58px; padding: 7px; border-top-width: 3px; }
  .overview-kpis { grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 8px; min-height: 235px; margin-top: 0; }
  .overview-kpis .kpi-card { min-height: 0; padding: 0; grid-template-rows: 31px minmax(0, 1fr); border: 0; }
  .kpi-label { font-size: 9px; }
  .overview-kpis .kpi-label { min-height: 31px; padding: 0 10px; font-size: 9px; }
  .kpi-value { margin-top: 5px; font-size: 13px; }
  .overview-kpis .kpi-value { margin: 0; padding: 8px; font-size: 22px; }
  .insight-grid { gap: 9px; margin-top: 14px; }
  .insight-card { padding: 12px; }
  .insight-value { margin-top: 6px; font-size: 20px; }
  .insight-detail { margin-top: 5px; font-size: 12px; }
  .section-title { margin-bottom: 12px; }
  .activity-layout { gap: 10px; grid-template-columns: minmax(0, 1fr) 245px; }
  .report-panel { padding: 12px; }
  .heat-tile { min-height: 58px; padding: 7px; }
  .heat-tile span { margin-top: 4px; }
  .trend-panel { margin-top: 10px; }
  .fact-card { min-height: 82px; padding: 10px; }
  .fact-card strong { margin-top: 5px; font-size: 18px; }
  .fact-card span { margin-top: 5px; }
  .breakdown-grid { gap: 8px; }
  .rank-card { min-height: 150px; padding: 9px; }
  .rank-card h2 { margin-bottom: 8px; font-size: 15px; }
  .rank-row { gap: 8px; padding: 4px 0; }
  .rank-name { font-size: 11px; }
  .rank-value { font-size: 9px; }
  .rank-track { height: 5px; }
  .signal-strip { gap: 8px; }
  .signal-card { min-height: 82px; padding: 8px; }
  .signal-card h3 { margin-bottom: 4px; font-size: 13px; }
  .signal-row { padding: 4px 0; }
  .signal-row span { font-size: 10px; }
  .signal-row strong { font-size: 9px; }
  .raw-note { margin-top: 6px; padding: 0; font-size: 10px; }
  .heatmap-viewport { overflow: visible; }
}
"#;

fn build_html_report(dataset: &ReportDataset) -> String {
    let insights = report_insights(dataset);
    let mut out = String::with_capacity(128 * 1024);
    out.push_str("<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">");
    out.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">");
    out.push_str("<title>");
    out.push_str(&escape_html(&report_title(dataset)));
    out.push_str("</title><style>");
    out.push_str(REPORT_CSS);
    out.push_str("</style></head><body><main class=\"report-deck\">");
    push_overview_page(&mut out, dataset, &insights);
    push_activity_page(&mut out, dataset, &insights);
    push_breakdown_page(&mut out, dataset);
    out.push_str("</main></body></html>");
    out
}

fn write_pdf_report(path: &Path, dataset: &ReportDataset) -> Result<()> {
    let bytes = Engine::builder()
        .page_size(PageSize::A4)
        .landscape(true)
        .margin(Margin::uniform_mm(10.0))
        .title(report_title(dataset))
        .build()
        .render_html(&build_html_report(dataset))
        .wrap_err("render report PDF")?;
    fs::write(path, bytes).wrap_err_with(|| format!("write {}", path.display()))
}

fn push_deck_header(out: &mut String, dataset: &ReportDataset, page: &str) {
    out.push_str("<header class=\"deck-header\"><div class=\"brand-lockup\">");
    out.push_str(REPORT_MARK_SVG);
    out.push_str("<span>Token Use</span></div><div class=\"deck-meta\">");
    let _ = write!(
        out,
        "{} | {} | {}",
        escape_html(page),
        escape_html(&dataset.metadata.period),
        escape_html(&dataset.metadata.project)
    );
    out.push_str("</div></header>");
}

fn push_overview_page(out: &mut String, dataset: &ReportDataset, insights: &ReportInsights) {
    out.push_str("<section class=\"report-page\" data-report-page=\"overview\">");
    push_deck_header(out, dataset, "Overview");
    out.push_str("<div class=\"cover-grid\"><div class=\"overview-kpi-area\">");
    push_kpi_ribbon_html(out, dataset, " overview-kpis");
    if let Some(note) = &dataset.metadata.sample_note {
        out.push_str("<p class=\"sample-note\">");
        out.push_str(&escape_html(note));
        out.push_str("</p>");
    }
    out.push_str("</div><aside class=\"meta-panel\">");
    for (label, value) in [
        ("Generated", dataset.metadata.generated_at.as_str()),
        ("Report ID", dataset.metadata.report_id.as_str()),
        ("Source", dataset.metadata.source.as_str()),
        ("Currency", dataset.metadata.currency.as_str()),
        ("Period", dataset.metadata.period.as_str()),
        ("Project", dataset.metadata.project.as_str()),
        (
            "Redaction",
            if dataset.metadata.redacted {
                "On"
            } else {
                "Off"
            },
        ),
    ] {
        let _ = write!(
            out,
            "<div class=\"meta-item\"><span class=\"meta-label\">{}</span><strong class=\"meta-value\">{}</strong></div>",
            escape_html(label),
            escape_html(value)
        );
    }
    out.push_str("</aside></div>");
    out.push_str("<section class=\"insight-grid\" aria-label=\"Executive snapshot\">");
    push_insight_card(
        out,
        "Executive snapshot",
        &insights.primary_value,
        &insights.primary_detail,
        true,
    );
    push_insight_card(
        out,
        "Model concentration",
        &insights.top_model_share,
        &insights.top_model_detail,
        false,
    );
    push_insight_card(
        out,
        "Average session cost",
        &insights.avg_cost_per_session,
        "Mean spend across sessions in this report scope.",
        false,
    );
    push_insight_card(
        out,
        "Calls per active day",
        &insights.calls_per_active_day,
        "Average call volume on days with recorded usage.",
        false,
    );
    out.push_str("</section></section>");
}

fn push_kpi_ribbon_html(out: &mut String, dataset: &ReportDataset, extra_class: &str) {
    let _ = write!(
        out,
        "<section class=\"kpi-ribbon{}\" aria-label=\"Key metrics\">",
        escape_html(extra_class)
    );
    for (label, value) in [
        ("Cost", dataset.summary.cost.clone()),
        ("Calls", format_int(dataset.summary.calls)),
        ("Sessions", format_int(dataset.summary.sessions)),
        (
            "Total tokens",
            format_compact_u64(dataset.summary.total_tokens),
        ),
        ("Cache hit", dataset.summary.cache_hit_rate.clone()),
        (
            "Web search",
            format_int(dataset.summary.web_search_requests),
        ),
    ] {
        let _ = write!(
            out,
            "<article class=\"kpi-card\"><span class=\"kpi-label\">{}</span><strong class=\"kpi-value\">{}</strong></article>",
            escape_html(label),
            escape_html(&value)
        );
    }
    out.push_str("</section>");
}

fn push_insight_card(out: &mut String, label: &str, value: &str, detail: &str, primary: bool) {
    let _ = write!(
        out,
        "<article class=\"insight-card{}\"><span class=\"insight-label\">{}</span><strong class=\"insight-value\">{}</strong><p class=\"insight-detail\">{}</p></article>",
        if primary { " primary" } else { "" },
        escape_html(label),
        escape_html(value),
        escape_html(detail)
    );
}

fn push_activity_page(out: &mut String, dataset: &ReportDataset, insights: &ReportInsights) {
    out.push_str("<section class=\"report-page\" data-report-page=\"activity\">");
    push_deck_header(out, dataset, "Activity");
    out.push_str("<div class=\"section-title\"><div><p class=\"deck-kicker\">Activity profile</p><h2>Timeline and usage cadence</h2></div><span class=\"page-number\">02</span></div>");
    out.push_str("<div class=\"activity-layout\"><div>");
    out.push_str(
        "<section class=\"report-panel\"><span class=\"panel-label\">Calendar heatmap</span>",
    );
    push_heatmap_calendar_html(out, dataset);
    out.push_str("</section><section class=\"report-panel trend-panel\"><span class=\"panel-label\">Daily call trend</span>");
    out.push_str(&activity_trend_svg(dataset, 760.0, 178.0));
    out.push_str("</section></div><aside class=\"activity-facts\">");
    push_fact_card(
        out,
        "Busiest day",
        &insights.busiest_day_value,
        &insights.busiest_day_detail,
    );
    push_fact_card(
        out,
        "Active days",
        &format_int(insights.active_days),
        &insights.activity_range,
    );
    push_fact_card(
        out,
        "Usage density",
        &insights.calls_per_active_day,
        "calls per active day",
    );
    out.push_str("</aside></div></section>");
}

fn push_fact_card(out: &mut String, label: &str, value: &str, detail: &str) {
    let _ = write!(
        out,
        "<div class=\"fact-card\"><span class=\"panel-label\">{}</span><strong>{}</strong><span>{}</span></div>",
        escape_html(label),
        escape_html(value),
        escape_html(detail)
    );
}

fn push_heatmap_calendar_html(out: &mut String, dataset: &ReportDataset) {
    let days = visible_heatmap_days(dataset);
    if days.len() <= 31 {
        push_heatmap_strip_html(out, &days);
        return;
    }
    out.push_str("<div class=\"heatmap-viewport\"><div class=\"heatmap-grid\">");
    if let Some(first) = days.first() {
        for _ in 0..first.date.weekday().num_days_from_monday() {
            out.push_str("<span class=\"heat-cell\" aria-hidden=\"true\"></span>");
        }
    }
    for day in &days {
        let _ = write!(
            out,
            "<span class=\"heat-cell i{}\" title=\"{} - {} calls - {} tokens\" aria-label=\"{} - {} calls - {} tokens\"></span>",
            heat_class(day.intensity),
            escape_html(&day.date.format("%Y-%m-%d").to_string()),
            format_int(day.calls),
            format_int(day.tokens),
            escape_html(&day.date.format("%Y-%m-%d").to_string()),
            format_int(day.calls),
            format_int(day.tokens)
        );
    }
    if days.is_empty() {
        out.push_str("<span>No activity in this report.</span>");
    }
    out.push_str("</div></div><div class=\"heat-legend\"><span>Less</span>");
    for class in 0..=5 {
        let _ = write!(out, "<span class=\"heat-cell i{class}\"></span>");
    }
    out.push_str("<span>More</span></div>");
}

fn push_heatmap_strip_html(out: &mut String, days: &[HeatmapDay]) {
    if days.is_empty() {
        out.push_str("<span>No activity in this report.</span>");
        return;
    }
    out.push_str("<div class=\"heatmap-strip\">");
    for day in days {
        let _ = write!(
            out,
            "<div class=\"heat-tile i{}\"><strong>{}</strong><span>{} calls</span><span>{} tokens</span></div>",
            heat_class(day.intensity),
            escape_html(&day.date.format("%b %d").to_string()),
            format_int(day.calls),
            format_int(day.tokens)
        );
    }
    out.push_str("</div><div class=\"heat-legend\"><span>Less</span>");
    for class in 0..=5 {
        let _ = write!(out, "<span class=\"heat-cell i{class}\"></span>");
    }
    out.push_str("<span>More</span></div>");
}

fn push_breakdown_page(out: &mut String, dataset: &ReportDataset) {
    out.push_str("<section class=\"report-page\" data-report-page=\"breakdown\">");
    push_deck_header(out, dataset, "Breakdown");
    out.push_str("<div class=\"section-title\"><div><p class=\"deck-kicker\">Breakdown</p><h2>Where usage concentrated</h2></div><span class=\"page-number\">03</span></div>");
    out.push_str("<div class=\"breakdown-grid\">");
    push_rank_card_html(out, "Top Projects", &project_rank_rows(dataset), 5);
    push_rank_card_html(out, "Top Models", &model_rank_rows(dataset), 5);
    push_rank_card_html(out, "Top Sessions", &session_rank_rows(dataset), 5);
    out.push_str("<div class=\"signal-strip\">");
    push_signal_card_html(out, "Tools", &count_rank_rows(&dataset.tools), 3);
    push_signal_card_html(out, "Commands", &count_rank_rows(&dataset.commands), 3);
    push_signal_card_html(
        out,
        "MCP Servers",
        &count_rank_rows(&dataset.mcp_servers),
        3,
    );
    out.push_str("</div>");
    out.push_str("</div><p class=\"raw-note\"><strong>Raw data:</strong> JSON, Excel, and CSV include full calls, prompts, paths, limits, and metadata. Visual reports stay summary-only.</p></section>");
}

fn push_rank_card_html(
    out: &mut String,
    title: &str,
    rows: &[(String, String, u64)],
    limit: usize,
) {
    let _ = write!(
        out,
        "<div class=\"rank-card\"><h2>{}</h2>",
        escape_html(title)
    );
    if rows.is_empty() {
        out.push_str("<p class=\"section-count\">No data</p>");
    }
    let name_limit = if title == "Top Sessions" { 30 } else { 38 };
    for (name, value, intensity) in rows.iter().take(limit) {
        let width = (*intensity).clamp(3, 100);
        let _ = write!(
            out,
            "<div class=\"rank-row\"><span class=\"rank-name\" title=\"{}\">{}</span><span class=\"rank-value\">{}</span><div class=\"rank-track\"><span class=\"rank-fill fill-{}\" style=\"width:{}%\"></span></div></div>",
            escape_html(name),
            escape_html(&truncate_middle(name, name_limit)),
            escape_html(&truncate_middle(value, 16)),
            heat_class(*intensity),
            width
        );
    }
    out.push_str("</div>");
}

fn push_signal_card_html(
    out: &mut String,
    title: &str,
    rows: &[(String, String, u64)],
    limit: usize,
) {
    let _ = write!(
        out,
        "<div class=\"signal-card\"><h3>{}</h3>",
        escape_html(title)
    );
    if rows.is_empty() {
        out.push_str("<p class=\"section-count\">No data</p>");
    }
    for (name, value, _) in rows.iter().take(limit) {
        let _ = write!(
            out,
            "<div class=\"signal-row\"><span title=\"{}\">{}</span><strong>{}</strong></div>",
            escape_html(name),
            escape_html(&truncate_middle(name, 32)),
            escape_html(&truncate_middle(value, 16))
        );
    }
    out.push_str("</div>");
}

#[derive(Debug, Clone)]
struct HeatmapDay {
    date: NaiveDate,
    calls: u64,
    tokens: u64,
    intensity: u64,
}

fn heatmap_days(dataset: &ReportDataset) -> Vec<HeatmapDay> {
    let mut by_date: BTreeMap<NaiveDate, &ReportActivity> = BTreeMap::new();
    for day in &dataset.activity {
        if let Ok(date) = NaiveDate::parse_from_str(&day.date, "%Y-%m-%d") {
            by_date.insert(date, day);
        }
    }
    let Some(start) = by_date.keys().next().copied() else {
        return Vec::new();
    };
    let Some(end) = by_date.keys().next_back().copied() else {
        return Vec::new();
    };

    let mut days = Vec::new();
    let mut date = start;
    while date <= end {
        if let Some(row) = by_date.get(&date) {
            days.push(HeatmapDay {
                date,
                calls: row.calls,
                tokens: row.tokens,
                intensity: row.intensity,
            });
        } else {
            days.push(HeatmapDay {
                date,
                calls: 0,
                tokens: 0,
                intensity: 0,
            });
        }
        let Some(next) = date.succ_opt() else {
            break;
        };
        date = next;
    }
    days
}

fn visible_heatmap_days(dataset: &ReportDataset) -> Vec<HeatmapDay> {
    let days = heatmap_days(dataset);
    let start = days.len().saturating_sub(364);
    days[start..].to_vec()
}

#[derive(Debug, Clone)]
struct ReportInsights {
    primary_value: String,
    primary_detail: String,
    top_model_share: String,
    top_model_detail: String,
    busiest_day_value: String,
    busiest_day_detail: String,
    avg_cost_per_session: String,
    calls_per_active_day: String,
    active_days: u64,
    activity_range: String,
}

fn report_insights(dataset: &ReportDataset) -> ReportInsights {
    let top_project = dataset.projects.iter().max_by(compare_project_cost);
    let top_model = dataset.models.iter().max_by(compare_model_cost);
    let busiest_day = dataset.activity.iter().max_by(compare_activity_volume);
    let active_days = dataset
        .activity
        .iter()
        .filter(|row| row.calls > 0 || row.tokens > 0)
        .count() as u64;

    let (primary_value, primary_detail) = top_project
        .map(|row| {
            (
                format!(
                    "{} of spend",
                    format_share(row.cost_usd, dataset.summary.cost_usd)
                ),
                format!(
                    "{} leads project spend with {} calls and {} tokens.",
                    row.project,
                    format_int(row.calls),
                    format_compact_u64(row.tokens)
                ),
            )
        })
        .unwrap_or_else(|| {
            (
                "-".into(),
                "No project usage was recorded for this report scope.".into(),
            )
        });

    let (top_model_share, top_model_detail) = top_model
        .map(|row| {
            (
                format_share(row.cost_usd, dataset.summary.cost_usd),
                format!(
                    "{} {} accounts for {} calls.",
                    row.tool,
                    row.model,
                    format_int(row.calls)
                ),
            )
        })
        .unwrap_or_else(|| ("-".into(), "No model usage was recorded.".into()));

    let (busiest_day_value, busiest_day_detail) = busiest_day
        .filter(|row| row.calls > 0 || row.tokens > 0)
        .map(|row| {
            (
                format_int(row.calls),
                format!("{} with {} tokens", row.date, format_int(row.tokens)),
            )
        })
        .unwrap_or_else(|| ("-".into(), "No activity was recorded.".into()));

    ReportInsights {
        primary_value,
        primary_detail,
        top_model_share,
        top_model_detail,
        busiest_day_value,
        busiest_day_detail,
        avg_cost_per_session: format_average_display_money(
            &dataset.summary.cost,
            dataset.summary.sessions,
        ),
        calls_per_active_day: format_ratio_one_decimal(dataset.summary.calls, active_days),
        active_days,
        activity_range: activity_range_label(dataset),
    }
}

fn compare_project_cost(a: &&ReportProject, b: &&ReportProject) -> std::cmp::Ordering {
    a.cost_usd
        .partial_cmp(&b.cost_usd)
        .unwrap_or(std::cmp::Ordering::Equal)
        .then_with(|| a.tokens.cmp(&b.tokens))
        .then_with(|| a.calls.cmp(&b.calls))
}

fn compare_model_cost(a: &&ReportModel, b: &&ReportModel) -> std::cmp::Ordering {
    a.cost_usd
        .partial_cmp(&b.cost_usd)
        .unwrap_or(std::cmp::Ordering::Equal)
        .then_with(|| a.tokens.cmp(&b.tokens))
        .then_with(|| a.calls.cmp(&b.calls))
}

fn compare_activity_volume(a: &&ReportActivity, b: &&ReportActivity) -> std::cmp::Ordering {
    a.calls
        .cmp(&b.calls)
        .then_with(|| a.tokens.cmp(&b.tokens))
        .then_with(|| a.date.cmp(&b.date))
}

fn activity_range_label(dataset: &ReportDataset) -> String {
    let days = visible_heatmap_days(dataset);
    match (days.first(), days.last()) {
        (Some(first), Some(last)) => {
            format!(
                "{} to {}",
                first.date.format("%Y-%m-%d"),
                last.date.format("%Y-%m-%d")
            )
        }
        _ => "No activity range".into(),
    }
}

fn format_share(value: f64, total: f64) -> String {
    if value <= 0.0 || total <= 0.0 || !value.is_finite() || !total.is_finite() {
        "-".into()
    } else {
        format!("{:.1}%", (value / total) * 100.0)
    }
}

fn format_ratio_one_decimal(value: u64, denominator: u64) -> String {
    if denominator == 0 {
        "-".into()
    } else {
        format!("{:.1}", value as f64 / denominator as f64)
    }
}

fn format_average_display_money(total: &str, count: u64) -> String {
    if count == 0 {
        return "-".into();
    }
    let Some(first_digit) = total
        .char_indices()
        .find(|(_, ch)| ch.is_ascii_digit() || *ch == '-')
        .map(|(idx, _)| idx)
    else {
        return "-".into();
    };
    let Some(last_digit) = total
        .char_indices()
        .rev()
        .find(|(_, ch)| ch.is_ascii_digit())
        .map(|(idx, ch)| idx + ch.len_utf8())
    else {
        return "-".into();
    };
    let prefix = &total[..first_digit];
    let suffix = &total[last_digit..];
    let number = total[first_digit..last_digit]
        .chars()
        .filter(|ch| ch.is_ascii_digit() || matches!(ch, '.' | '-'))
        .collect::<String>()
        .parse::<f64>()
        .ok();
    number
        .map(|value| format!("{}{:.2}{}", prefix, value / count as f64, suffix))
        .unwrap_or_else(|| "-".into())
}

fn activity_trend_svg(dataset: &ReportDataset, width: f64, height: f64) -> String {
    let rows: Vec<&ReportActivity> = dataset
        .activity
        .iter()
        .filter(|row| NaiveDate::parse_from_str(&row.date, "%Y-%m-%d").is_ok())
        .collect();
    let visible_start = rows.len().saturating_sub(42);
    let visible = &rows[visible_start..];
    let max_calls = visible.iter().map(|row| row.calls).max().unwrap_or(0);
    let mut out = String::new();
    let _ = write!(
        out,
        "<svg class=\"trend-svg\" viewBox=\"0 0 {:.0} {:.0}\" role=\"img\" aria-label=\"Daily call trend\">",
        width, height
    );
    out.push_str("<rect x=\"0\" y=\"0\" width=\"100%\" height=\"100%\" fill=\"#ffffff\"/>");
    let left = 24.0;
    let right = width - 18.0;
    let top = 16.0;
    let bottom = height - 30.0;
    let chart_width = (right - left).max(1.0);
    let chart_height = (bottom - top).max(1.0);
    for idx in 0..=3 {
        let y = top + chart_height * idx as f64 / 3.0;
        let _ = write!(
            out,
            "<line x1=\"{left:.1}\" x2=\"{right:.1}\" y1=\"{y:.1}\" y2=\"{y:.1}\" stroke=\"#edf0f5\" stroke-width=\"1\"/>"
        );
    }
    if visible.is_empty() || max_calls == 0 {
        let _ = write!(
            out,
            "<text x=\"{left:.1}\" y=\"{:.1}\" fill=\"#667085\" font-size=\"16\" font-family=\"Aptos, Segoe UI, sans-serif\">No activity in this period</text>",
            height / 2.0
        );
    } else {
        let gap = 4.0;
        let bar_width = ((chart_width - gap * visible.len().saturating_sub(1) as f64)
            / visible.len() as f64)
            .max(4.0);
        for (idx, row) in visible.iter().enumerate() {
            let x = left + idx as f64 * (bar_width + gap);
            let bar_height = (row.calls as f64 / max_calls as f64) * chart_height;
            let y = bottom - bar_height;
            let fill = if row.intensity >= 85 {
                "#d95f68"
            } else if row.intensity >= 65 {
                "#df6f3f"
            } else if row.intensity >= 40 {
                "#c9971f"
            } else {
                "#3478c7"
            };
            let _ = write!(
                out,
                "<rect x=\"{x:.1}\" y=\"{y:.1}\" width=\"{bar_width:.1}\" height=\"{bar_height:.1}\" rx=\"2\" fill=\"{fill}\"/>"
            );
        }
        if let (Some(first), Some(last)) = (visible.first(), visible.last()) {
            let _ = write!(
                out,
                "<text x=\"{left:.1}\" y=\"{:.1}\" fill=\"#667085\" font-size=\"12\" font-family=\"Aptos, Segoe UI, sans-serif\">{}</text>",
                height - 8.0,
                escape_html(&first.date)
            );
            let _ = write!(
                out,
                "<text x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"end\" fill=\"#667085\" font-size=\"12\" font-family=\"Aptos, Segoe UI, sans-serif\">{}</text>",
                right,
                height - 8.0,
                escape_html(&last.date)
            );
        }
    }
    out.push_str("</svg>");
    out
}

const CANVAS_W: u32 = 1600;
const CANVAS_H: u32 = 900;

fn write_summary_svg(path: &Path, dataset: &ReportDataset) -> Result<()> {
    let backend = SVGBackend::new(path, (CANVAS_W, CANVAS_H));
    render_summary_chart(backend, dataset)
        .map_err(|e| color_eyre::eyre::eyre!("svg render failed: {e}"))
}

fn write_summary_png(path: &Path, dataset: &ReportDataset) -> Result<()> {
    let backend = BitMapBackend::new(path, (CANVAS_W, CANVAS_H));
    render_summary_chart(backend, dataset)
        .map_err(|e| color_eyre::eyre::eyre!("png render failed: {e}"))
}

type ChartResult = std::result::Result<(), Box<dyn std::error::Error + Send + Sync>>;

#[derive(Clone, Copy)]
struct ChartRect {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

fn render_summary_chart<DB>(backend: DB, dataset: &ReportDataset) -> ChartResult
where
    DB: DrawingBackend,
    DB::ErrorType: 'static,
{
    let root = backend.into_drawing_area();
    root.fill(&RGBColor(245, 242, 236))?;
    root.draw(&Rectangle::new(
        [(32, 28), (1568, 872)],
        ShapeStyle::from(&WHITE).filled(),
    ))?;
    root.draw(&Rectangle::new(
        [(32, 28), (1568, 872)],
        ShapeStyle::from(&RGBColor(217, 222, 232)).stroke_width(1),
    ))?;

    draw_report_mark(&root, 70, 58)?;
    let eyebrow_style = ("sans-serif", 18)
        .into_font()
        .style(FontStyle::Bold)
        .color(&RGBColor(102, 112, 133));
    let title_style = ("sans-serif", 42)
        .into_font()
        .style(FontStyle::Bold)
        .color(&RGBColor(37, 43, 55));
    let body_style = ("sans-serif", 20)
        .into_font()
        .color(&RGBColor(102, 112, 133));
    root.draw(&Text::new("Executive Summary", (118, 72), eyebrow_style))?;
    root.draw(&Text::new(report_title(dataset), (70, 122), title_style))?;
    root.draw(&Text::new(
        format!("{} | {}", dataset.metadata.period, dataset.metadata.project),
        (70, 154),
        body_style.clone(),
    ))?;
    root.draw(&Text::new(
        format!("Generated {}", dataset.metadata.generated_at),
        (1110, 78),
        body_style.clone(),
    ))?;

    let kpis = [
        ("Cost", dataset.summary.cost.clone()),
        ("Calls", format_int(dataset.summary.calls)),
        ("Sessions", format_int(dataset.summary.sessions)),
        ("Tokens", format_compact_u64(dataset.summary.total_tokens)),
    ];
    for (idx, (label, value)) in kpis.iter().enumerate() {
        draw_kpi_card(&root, 70 + idx as i32 * 368, 190, 340, label, value)?;
    }

    draw_activity_panel(&root, dataset, 70, 330, 900, 300)?;
    draw_rank_panel(
        &root,
        "Top Projects",
        &project_rank_rows(dataset),
        ChartRect {
            x: 1010,
            y: 330,
            width: 520,
            height: 145,
        },
        3,
    )?;
    draw_rank_panel(
        &root,
        "Top Models",
        &model_rank_rows(dataset),
        ChartRect {
            x: 1010,
            y: 485,
            width: 520,
            height: 145,
        },
        3,
    )?;
    draw_rank_panel(
        &root,
        "Top Sessions",
        &session_rank_rows(dataset),
        ChartRect {
            x: 70,
            y: 660,
            width: 1460,
            height: 170,
        },
        4,
    )?;
    let footer_style = ("sans-serif", 16)
        .into_font()
        .color(&RGBColor(102, 112, 133));
    root.draw(&Text::new(
        "Raw calls, prompts, limits, and metadata are available in JSON, Excel, and CSV reports.",
        (70, 850),
        footer_style,
    ))?;
    root.present()?;
    Ok(())
}

fn draw_report_mark<DB>(
    root: &DrawingArea<DB, plotters::coord::Shift>,
    x: i32,
    y: i32,
) -> ChartResult
where
    DB: DrawingBackend,
    DB::ErrorType: 'static,
{
    let colors = [
        RGBColor(242, 177, 93),
        RGBColor(237, 138, 71),
        RGBColor(223, 111, 63),
        RGBColor(201, 91, 68),
    ];
    let heights = [24, 38, 54, 42];
    let offsets = [30, 16, 0, 12];
    for idx in 0..4 {
        let x0 = x + idx as i32 * 11;
        root.draw(&Rectangle::new(
            [
                (x0, y + offsets[idx]),
                (x0 + 7, y + offsets[idx] + heights[idx]),
            ],
            ShapeStyle::from(&colors[idx]).filled(),
        ))?;
    }
    Ok(())
}

fn draw_kpi_card<DB>(
    root: &DrawingArea<DB, plotters::coord::Shift>,
    x: i32,
    y: i32,
    width: i32,
    label: &str,
    value: &str,
) -> ChartResult
where
    DB: DrawingBackend,
    DB::ErrorType: 'static,
{
    root.draw(&Rectangle::new(
        [(x, y), (x + width, y + 106)],
        ShapeStyle::from(&RGBColor(247, 248, 251)).filled(),
    ))?;
    root.draw(&Rectangle::new(
        [(x, y), (x + width, y + 106)],
        ShapeStyle::from(&RGBColor(217, 222, 232)).stroke_width(1),
    ))?;
    root.draw(&Rectangle::new(
        [(x, y), (x + width, y + 5)],
        ShapeStyle::from(&RGBColor(223, 111, 63)).filled(),
    ))?;
    let label_style = ("sans-serif", 16)
        .into_font()
        .style(FontStyle::Bold)
        .color(&RGBColor(102, 112, 133));
    let value_style = ("monospace", 30)
        .into_font()
        .style(FontStyle::Bold)
        .color(&RGBColor(37, 43, 55));
    root.draw(&Text::new(label.to_string(), (x + 22, y + 34), label_style))?;
    root.draw(&Text::new(
        truncate_middle(value, 18),
        (x + 22, y + 78),
        value_style,
    ))?;
    Ok(())
}

fn draw_panel_box<DB>(
    root: &DrawingArea<DB, plotters::coord::Shift>,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) -> ChartResult
where
    DB: DrawingBackend,
    DB::ErrorType: 'static,
{
    root.draw(&Rectangle::new(
        [(x, y), (x + width, y + height)],
        ShapeStyle::from(&WHITE).filled(),
    ))?;
    root.draw(&Rectangle::new(
        [(x, y), (x + width, y + height)],
        ShapeStyle::from(&RGBColor(217, 222, 232)).stroke_width(1),
    ))?;
    Ok(())
}

fn draw_activity_panel<DB>(
    root: &DrawingArea<DB, plotters::coord::Shift>,
    dataset: &ReportDataset,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) -> ChartResult
where
    DB: DrawingBackend,
    DB::ErrorType: 'static,
{
    draw_panel_box(root, x, y, width, height)?;
    let title_style = ("sans-serif", 22)
        .into_font()
        .style(FontStyle::Bold)
        .color(&RGBColor(37, 43, 55));
    let label_style = ("sans-serif", 15)
        .into_font()
        .color(&RGBColor(102, 112, 133));
    root.draw(&Text::new(
        "Activity Heatmap",
        (x + 26, y + 36),
        title_style.clone(),
    ))?;
    root.draw(&Text::new(
        activity_range_label(dataset),
        (x + 26, y + 62),
        label_style.clone(),
    ))?;

    let days = visible_heatmap_days(dataset);
    if days.is_empty() {
        root.draw(&Text::new(
            "No activity in this report",
            (x + 26, y + 110),
            label_style,
        ))?;
        return Ok(());
    }

    let cell = 11;
    let heat_x = x + 26;
    let heat_y = y + 86;
    if days.len() <= 31 {
        draw_heatmap_strip(root, &days, heat_x, heat_y, width - 52, 86)?;
    } else {
        let leading = days
            .first()
            .map(|day| day.date.weekday().num_days_from_monday() as usize)
            .unwrap_or(0);
        let gap = 4;
        for idx in 0..leading {
            let row = idx as i32 % 7;
            let y0 = heat_y + row * (cell + gap);
            root.draw(&Rectangle::new(
                [(heat_x, y0), (heat_x + cell, y0 + cell)],
                ShapeStyle::from(&RGBColor(237, 240, 245)).filled(),
            ))?;
        }
        for (idx, day) in days.iter().enumerate() {
            let cell_idx = idx + leading;
            let col = cell_idx as i32 / 7;
            let row = cell_idx as i32 % 7;
            let x0 = heat_x + col * (cell + gap);
            let y0 = heat_y + row * (cell + gap);
            root.draw(&Rectangle::new(
                [(x0, y0), (x0 + cell, y0 + cell)],
                ShapeStyle::from(&heat_color(day.intensity)).filled(),
            ))?;
        }
    }

    root.draw(&Text::new("Less", (x + 26, y + 215), label_style.clone()))?;
    for class in 0..=5 {
        let x0 = x + 78 + class * 20;
        root.draw(&Rectangle::new(
            [(x0, y + 202), (x0 + cell, y + 213)],
            ShapeStyle::from(&heat_color(class as u64 * 20)).filled(),
        ))?;
    }
    root.draw(&Text::new("More", (x + 214, y + 215), label_style.clone()))?;
    root.draw(&Text::new(
        "Daily Call Trend",
        (x + 26, y + 256),
        title_style,
    ))?;
    draw_trend_bars(root, dataset, x + 250, y + 232, width - 290, 48)?;
    Ok(())
}

fn draw_heatmap_strip<DB>(
    root: &DrawingArea<DB, plotters::coord::Shift>,
    days: &[HeatmapDay],
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) -> ChartResult
where
    DB: DrawingBackend,
    DB::ErrorType: 'static,
{
    let gap = 8;
    let tile_w =
        ((width - gap * days.len().saturating_sub(1) as i32) / days.len() as i32).clamp(58, 116);
    let date_style = ("sans-serif", 13)
        .into_font()
        .style(FontStyle::Bold)
        .color(&RGBColor(37, 43, 55));
    let body_style = ("sans-serif", 12)
        .into_font()
        .color(&RGBColor(102, 112, 133));
    for (idx, day) in days.iter().enumerate() {
        let x0 = x + idx as i32 * (tile_w + gap);
        let color = heat_color(day.intensity);
        root.draw(&Rectangle::new(
            [(x0, y), (x0 + tile_w, y + height)],
            ShapeStyle::from(&color).filled(),
        ))?;
        root.draw(&Rectangle::new(
            [(x0, y), (x0 + tile_w, y + height)],
            ShapeStyle::from(&RGBColor(217, 222, 232)).stroke_width(1),
        ))?;
        root.draw(&Text::new(
            day.date.format("%b %d").to_string(),
            (x0 + 8, y + 22),
            date_style.clone(),
        ))?;
        root.draw(&Text::new(
            format!("{} calls", format_int(day.calls)),
            (x0 + 8, y + 48),
            body_style.clone(),
        ))?;
        root.draw(&Text::new(
            format!("{} tokens", format_int(day.tokens)),
            (x0 + 8, y + 68),
            body_style.clone(),
        ))?;
    }
    Ok(())
}

fn draw_trend_bars<DB>(
    root: &DrawingArea<DB, plotters::coord::Shift>,
    dataset: &ReportDataset,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) -> ChartResult
where
    DB: DrawingBackend,
    DB::ErrorType: 'static,
{
    let rows: Vec<&ReportActivity> = dataset.activity.iter().rev().take(42).collect();
    let max_calls = rows.iter().map(|row| row.calls).max().unwrap_or(0);
    if rows.is_empty() || max_calls == 0 {
        return Ok(());
    }
    let gap = 3;
    let bar_width =
        ((width - gap * rows.len().saturating_sub(1) as i32) / rows.len() as i32).max(3);
    for (idx, row) in rows.iter().rev().enumerate() {
        let bar_height = ((row.calls as f64 / max_calls as f64) * height as f64).round() as i32;
        let x0 = x + idx as i32 * (bar_width + gap);
        let y0 = y + height - bar_height;
        root.draw(&Rectangle::new(
            [(x0, y0), (x0 + bar_width, y + height)],
            ShapeStyle::from(&heat_color(row.intensity)).filled(),
        ))?;
    }
    Ok(())
}

fn draw_rank_panel<DB>(
    root: &DrawingArea<DB, plotters::coord::Shift>,
    title: &str,
    rows: &[(String, String, u64)],
    rect: ChartRect,
    limit: usize,
) -> ChartResult
where
    DB: DrawingBackend,
    DB::ErrorType: 'static,
{
    let ChartRect {
        x,
        y,
        width,
        height,
    } = rect;
    draw_panel_box(root, x, y, width, height)?;
    let compact = height <= 150;
    let title_style = ("sans-serif", if compact { 19 } else { 22 })
        .into_font()
        .style(FontStyle::Bold)
        .color(&RGBColor(37, 43, 55));
    let body_style = ("sans-serif", if compact { 14 } else { 16 })
        .into_font()
        .color(&RGBColor(37, 43, 55));
    let value_style = ("monospace", if compact { 13 } else { 15 })
        .into_font()
        .style(FontStyle::Bold)
        .color(&RGBColor(102, 112, 133));
    root.draw(&Text::new(title.to_string(), (x + 24, y + 34), title_style))?;
    if rows.is_empty() {
        root.draw(&Text::new(
            "No data",
            (x + 24, y + 74),
            ("sans-serif", 16)
                .into_font()
                .color(&RGBColor(102, 112, 133)),
        ))?;
        return Ok(());
    }
    let row_gap = ((height - 72) / limit.max(1) as i32).clamp(25, 36);
    for (idx, (name, value, intensity)) in rows.iter().take(limit).enumerate() {
        let row_y = y + 64 + idx as i32 * row_gap;
        let track_y = row_y + 14;
        let track_w = width - 48;
        let fill_w = ((*intensity as i32 * track_w) / 100).max(6);
        root.draw(&Text::new(
            truncate_middle(name, if width > 700 { 70 } else { 34 }),
            (x + 24, row_y),
            body_style.clone(),
        ))?;
        root.draw(&Text::new(
            truncate_middle(value, 18),
            (x + width - 160, row_y),
            value_style.clone(),
        ))?;
        root.draw(&Rectangle::new(
            [(x + 24, track_y), (x + 24 + track_w, track_y + 5)],
            ShapeStyle::from(&RGBColor(237, 240, 245)).filled(),
        ))?;
        root.draw(&Rectangle::new(
            [(x + 24, track_y), (x + 24 + fill_w, track_y + 5)],
            ShapeStyle::from(&heat_color(*intensity)).filled(),
        ))?;
    }
    Ok(())
}

fn project_rank_rows(dataset: &ReportDataset) -> Vec<(String, String, u64)> {
    let max = dataset
        .projects
        .iter()
        .map(|row| row.tokens)
        .max()
        .unwrap_or(0);
    dataset
        .projects
        .iter()
        .map(|row| {
            (
                row.project.clone(),
                row.cost.clone(),
                scale_u64(row.tokens, max),
            )
        })
        .collect()
}

fn model_rank_rows(dataset: &ReportDataset) -> Vec<(String, String, u64)> {
    let max = dataset
        .models
        .iter()
        .map(|row| row.tokens)
        .max()
        .unwrap_or(0);
    dataset
        .models
        .iter()
        .map(|row| {
            (
                format!("{} {}", row.tool, row.model),
                row.cost.clone(),
                scale_u64(row.tokens, max),
            )
        })
        .collect()
}

fn count_rank_rows(rows: &[ReportCount]) -> Vec<(String, String, u64)> {
    let max = rows.iter().map(|row| row.tokens).max().unwrap_or(0);
    rows.iter()
        .map(|row| {
            (
                row.name.clone(),
                format!("{} calls", format_int(row.calls)),
                scale_u64(row.tokens, max),
            )
        })
        .collect()
}

fn session_rank_rows(dataset: &ReportDataset) -> Vec<(String, String, u64)> {
    let max = dataset
        .sessions
        .iter()
        .map(|row| row.tokens)
        .max()
        .unwrap_or(0);
    let mut rows: Vec<_> = dataset
        .sessions
        .iter()
        .map(|row| {
            let date = format_short_report_date(&row.started_at);
            let label = if date.is_empty() {
                format!("{} {}", row.tool, row.project)
            } else {
                format!("{} {} {}", row.tool, row.project, date)
            };
            (
                label,
                format!("{} | {}", row.cost, format_compact_u64(row.tokens)),
                scale_u64(row.tokens, max),
            )
        })
        .collect();
    rows.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.0.cmp(&b.0)));
    rows
}

fn in_report_period(ts: Option<DateTime<Utc>>, period: Period, now: DateTime<Local>) -> bool {
    let Some(ts) = ts else {
        return matches!(period, Period::AllTime);
    };
    let local = ts.with_timezone(&Local);
    let today = now.date_naive();
    let date = local.date_naive();
    match period {
        Period::Today => local >= now - Duration::hours(24) && local <= now,
        Period::Week => date >= today - Duration::days(6),
        Period::ThirtyDays => date >= today - Duration::days(29),
        Period::Month => date.year() == today.year() && date.month() == today.month(),
        Period::AllTime => true,
    }
}

fn period_label(period: Period) -> &'static str {
    match period {
        Period::Today => copy().periods.today.as_str(),
        Period::Week => copy().periods.week.as_str(),
        Period::ThirtyDays => copy().periods.thirty_days.as_str(),
        Period::Month => copy().periods.month.as_str(),
        Period::AllTime => copy().periods.all_time.as_str(),
    }
}

fn period_slug(label: &str) -> String {
    slugify(label)
}

fn report_slug(dataset: &ReportDataset) -> String {
    format!(
        "{}-{}",
        period_slug(&dataset.metadata.period),
        slugify(&dataset.metadata.project)
    )
}

fn report_title(dataset: &ReportDataset) -> String {
    template(
        &copy().reports.report_title,
        &[
            ("period", dataset.metadata.period.clone()),
            ("project", dataset.metadata.project.clone()),
        ],
    )
}

fn session_key(call: &ParsedCall) -> Option<String> {
    if call.session_id.is_empty() {
        None
    } else {
        Some(format!("{}:{}", call.tool, call.session_id))
    }
}

fn call_tokens(call: &ParsedCall) -> u64 {
    call.input_tokens
        .saturating_add(call.output_tokens)
        .saturating_add(call.cache_read_input_tokens)
        .saturating_add(call.cache_creation_input_tokens)
        .saturating_add(call.reasoning_tokens)
}

fn first_word(command: &str) -> &str {
    command.split_whitespace().next().unwrap_or("(unknown)")
}

fn format_ts(ts: Option<DateTime<Utc>>) -> String {
    ts.map(|ts| {
        ts.with_timezone(&Local)
            .format("%Y-%m-%d %H:%M:%S")
            .to_string()
    })
    .unwrap_or_default()
}

fn scale_u64(value: u64, max: u64) -> u64 {
    if value == 0 || max == 0 {
        0
    } else {
        ((value as f64 / max as f64) * 100.0)
            .round()
            .clamp(1.0, 100.0) as u64
    }
}

fn heat_class(value: u64) -> u64 {
    match value {
        0 => 0,
        1..=20 => 1,
        21..=40 => 2,
        41..=65 => 3,
        66..=85 => 4,
        _ => 5,
    }
}

fn heat_color(value: u64) -> RGBColor {
    match heat_class(value) {
        1 => RGBColor(126, 188, 255),
        2 => RGBColor(76, 242, 160),
        3 => RGBColor(255, 214, 10),
        4 => RGBColor(255, 156, 72),
        5 => RGBColor(255, 95, 109),
        _ => RGBColor(226, 232, 240),
    }
}

fn opt_f64(value: Option<f64>) -> String {
    value.map(|value| value.to_string()).unwrap_or_default()
}

fn format_int(value: u64) -> String {
    let raw = value.to_string();
    let mut out = String::with_capacity(raw.len() + raw.len() / 3);
    for (idx, ch) in raw.chars().rev().enumerate() {
        if idx > 0 && idx % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out.chars().rev().collect()
}

fn format_compact_u64(value: u64) -> String {
    if value >= 1_000_000_000 {
        format!("{:.2}B", value as f64 / 1_000_000_000.0)
    } else if value >= 1_000_000 {
        format!("{:.1}M", value as f64 / 1_000_000.0)
    } else if value >= 1_000 {
        format!("{:.1}K", value as f64 / 1_000.0)
    } else {
        format_int(value)
    }
}

fn format_short_report_date(value: &str) -> String {
    value
        .get(..10)
        .and_then(|date| NaiveDate::parse_from_str(date, "%Y-%m-%d").ok())
        .map(|date| date.format("%b %d").to_string())
        .unwrap_or_default()
}

fn parse_display_u64(value: &str) -> u64 {
    value
        .chars()
        .filter(|c| c.is_ascii_digit())
        .collect::<String>()
        .parse()
        .unwrap_or(0)
}

fn parse_display_money_value(value: &str) -> f64 {
    value
        .chars()
        .filter(|c| c.is_ascii_digit() || matches!(c, '.' | '-'))
        .collect::<String>()
        .parse()
        .unwrap_or(0.0)
}

fn parse_compact(value: &str) -> u64 {
    let value = value.trim();
    let multiplier = if value.ends_with('M') {
        1_000_000.0
    } else if value.ends_with('K') || value.ends_with('k') {
        1_000.0
    } else {
        1.0
    };
    let numeric = value
        .trim_end_matches(['M', 'K', 'k'])
        .replace(',', "")
        .parse::<f64>()
        .unwrap_or(0.0);
    (numeric * multiplier).round() as u64
}

fn sample_report_date(period: Period, idx: usize, len: usize) -> String {
    let today = Local::now().date_naive();
    let offset = len.saturating_sub(idx + 1) as i64;
    let start_offset = match period {
        Period::Today => offset.min(1),
        Period::Week => offset.min(6),
        Period::ThirtyDays => offset.min(29),
        Period::Month => offset.min(today.day0() as i64),
        Period::AllTime => offset,
    };
    (today - Duration::days(start_offset))
        .format("%Y-%m-%d")
        .to_string()
}

fn truncate_middle(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        return value.to_string();
    }
    let keep = max.saturating_sub(3) / 2;
    let start: String = value.chars().take(keep).collect();
    let end: String = value
        .chars()
        .rev()
        .take(keep)
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    format!("{start}...{end}")
}

fn clean_text(text: &str) -> String {
    let cleaned: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    cleaned.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn snippet(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_string()
    } else {
        let mut out: String = text.chars().take(max.saturating_sub(3)).collect();
        out.push_str("...");
        out
    }
}

fn escape_html(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

fn slugify(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut dash = false;
    for c in value.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            dash = false;
        } else if !dash && !out.is_empty() {
            out.push('-');
            dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        "all-projects".into()
    } else {
        out
    }
}

struct Redactor {
    enabled: bool,
    projects: HashMap<String, String>,
    sessions: HashMap<String, String>,
    dedup: HashMap<String, String>,
}

impl Redactor {
    fn new(enabled: bool) -> Self {
        Self {
            enabled,
            projects: HashMap::new(),
            sessions: HashMap::new(),
            dedup: HashMap::new(),
        }
    }

    fn project_label(&mut self, value: String) -> String {
        self.project_path(&value)
    }

    fn project_path(&mut self, value: &str) -> String {
        if !self.enabled || value.is_empty() {
            return value.to_string();
        }
        let next = self.projects.len() + 1;
        self.projects
            .entry(value.to_string())
            .or_insert_with(|| format!("Project {next}"))
            .clone()
    }

    fn session_id(&mut self, value: &str) -> String {
        if !self.enabled || value.is_empty() {
            return value.to_string();
        }
        let next = self.sessions.len() + 1;
        self.sessions
            .entry(value.to_string())
            .or_insert_with(|| format!("session-{next}"))
            .clone()
    }

    fn dedup_key(&mut self, value: &str) -> String {
        if !self.enabled || value.is_empty() {
            return value.to_string();
        }
        let next = self.dedup.len() + 1;
        self.dedup
            .entry(value.to_string())
            .or_insert_with(|| format!("call-{next}"))
            .clone()
    }

    fn prompt(&self, value: &str) -> String {
        if self.enabled {
            "[redacted]".into()
        } else {
            value.to_string()
        }
    }

    fn prompt_preview(&self, value: &str) -> String {
        if self.enabled {
            "[redacted]".into()
        } else {
            snippet(value, 140)
        }
    }

    fn bash_commands(&self, value: &[String]) -> String {
        if self.enabled && !value.is_empty() {
            "[redacted]".into()
        } else {
            value.join("\n")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Read};

    fn call(project: &str, session: &str, key: &str, days_ago: i64) -> ParsedCall {
        ParsedCall {
            tool: crate::tools::codex::config::TOOL_ID,
            model: "gpt-5".into(),
            input_tokens: 100,
            output_tokens: 50,
            cache_creation_input_tokens: 10,
            cache_read_input_tokens: 20,
            cached_input_tokens: 20,
            reasoning_tokens: 5,
            web_search_requests: 1,
            cost_usd: 0.25,
            tools: vec!["Bash".into(), "mcp__server__tool".into()],
            bash_commands: vec!["cargo test".into()],
            timestamp: Some(Utc::now() - Duration::days(days_ago)),
            speed: crate::tools::Speed::Standard,
            dedup_key: key.into(),
            user_message: "full prompt text".into(),
            session_id: session.into(),
            project: project.into(),
        }
    }

    fn request(format: ReportFormat, redacted: bool) -> ReportRequest {
        ReportRequest {
            format,
            period: Period::ThirtyDays,
            scope: ReportScope::AllProjects,
            redacted,
        }
    }

    fn xlsx_text(path: &Path) -> String {
        let bytes = fs::read(path).unwrap();
        let cursor = Cursor::new(bytes);
        let mut archive = zip::ZipArchive::new(cursor).unwrap();
        let mut out = String::new();
        for idx in 0..archive.len() {
            let mut file = archive.by_index(idx).unwrap();
            if file.name().ends_with(".xml") {
                let mut text = String::new();
                file.read_to_string(&mut text).unwrap();
                out.push_str(&text);
            }
        }
        out
    }

    #[test]
    fn report_dataset_filters_by_period_and_project() {
        let ingested = Ingested {
            calls: vec![
                call("/tmp/a", "s1", "k1", 1),
                call("/tmp/b", "s2", "k2", 1),
                call("/tmp/a", "old", "k3", 45),
            ],
            limits: Vec::new(),
        };
        let project = project_identity("/tmp/a");
        let req = ReportRequest {
            format: ReportFormat::Json,
            period: Period::ThirtyDays,
            scope: ReportScope::Project {
                identity: project,
                label: "a".into(),
            },
            redacted: false,
        };
        let dataset =
            build_ingested_dataset(&req, &ingested, &CurrencyFormatter::usd(), "live", "id");
        assert_eq!(dataset.summary.calls, 1);
        assert_eq!(dataset.projects.len(), 1);
        assert_eq!(dataset.calls[0].session_id, "s1");
    }

    #[test]
    fn redacted_report_hides_sensitive_fields() {
        let ingested = Ingested {
            calls: vec![call(
                "/Users/me/secret",
                "session-secret",
                "dedup-secret",
                0,
            )],
            limits: Vec::new(),
        };
        let dataset = build_ingested_dataset(
            &request(ReportFormat::Json, true),
            &ingested,
            &CurrencyFormatter::usd(),
            "live",
            "id",
        );
        let raw = serde_json::to_string(&dataset).unwrap();
        assert!(!raw.contains("/Users/me/secret"));
        assert!(!raw.contains("session-secret"));
        assert!(!raw.contains("dedup-secret"));
        assert!(!raw.contains("full prompt text"));
        assert!(raw.contains("[redacted]"));
    }

    #[test]
    fn redaction_applies_to_report_outputs() {
        let dir = std::env::temp_dir().join(format!(
            "tokenuse-report-redaction-{}",
            Utc::now().timestamp_nanos_opt().unwrap()
        ));
        let ingested = Ingested {
            calls: vec![call(
                "/Users/me/secret",
                "session-secret",
                "dedup-secret",
                0,
            )],
            limits: Vec::new(),
        };
        let redacted = build_ingested_dataset(
            &request(ReportFormat::Json, true),
            &ingested,
            &CurrencyFormatter::usd(),
            "live",
            "id",
        );
        let unredacted = build_ingested_dataset(
            &request(ReportFormat::Json, false),
            &ingested,
            &CurrencyFormatter::usd(),
            "live",
            "id",
        );

        let json = serde_json::to_string(&unredacted).unwrap();
        assert!(json.contains("/Users/me/secret"));
        assert!(json.contains("session-secret"));
        assert!(json.contains("dedup-secret"));
        assert!(json.contains("full prompt text"));

        let redacted_json = serde_json::to_string(&redacted).unwrap();
        assert!(!redacted_json.contains("/Users/me/secret"));
        assert!(!redacted_json.contains("session-secret"));
        assert!(!redacted_json.contains("dedup-secret"));
        assert!(!redacted_json.contains("full prompt text"));

        let csv_dir = dir.join("csv");
        fs::create_dir_all(&csv_dir).unwrap();
        write_csv_report_dir(&csv_dir, &redacted).unwrap();
        let csv = fs::read_to_string(csv_dir.join("calls.csv")).unwrap();
        assert!(!csv.contains("/Users/me/secret"));
        assert!(!csv.contains("session-secret"));
        assert!(!csv.contains("dedup-secret"));
        assert!(!csv.contains("full prompt text"));

        let html = build_html_report(&redacted);
        assert!(!html.contains("/Users/me/secret"));
        assert!(!html.contains("session-secret"));
        assert!(!html.contains("dedup-secret"));
        assert!(!html.contains("full prompt text"));

        let xlsx = dir.join("redacted.xlsx");
        write_xlsx_report(&xlsx, &redacted).unwrap();
        let workbook = xlsx_text(&xlsx);
        assert!(workbook.contains("Summary"));
        assert!(workbook.contains("Activity"));
        assert!(workbook.contains("Project Tools"));
        assert!(workbook.contains("Limits Raw"));
        assert!(!workbook.contains("/Users/me/secret"));
        assert!(!workbook.contains("session-secret"));
        assert!(!workbook.contains("dedup-secret"));
        assert!(!workbook.contains("full prompt text"));

        let pdf_source_html = build_html_report(&redacted);
        assert!(!pdf_source_html.contains("<script"));
        assert!(!pdf_source_html.contains("/Users/me/secret"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn csv_report_contains_all_area_files() {
        let dir = std::env::temp_dir().join(format!(
            "tokenuse-report-csv-{}",
            Utc::now().timestamp_nanos_opt().unwrap()
        ));
        let ingested = Ingested {
            calls: vec![call("/tmp/a", "s1", "k1", 0)],
            limits: Vec::new(),
        };
        let response = write_ingested_to_dir(
            &dir,
            &request(ReportFormat::Csv, false),
            &ingested,
            &CurrencyFormatter::usd(),
            "live",
        )
        .unwrap();
        for name in [
            "summary.csv",
            "activity.csv",
            "projects.csv",
            "project_tools.csv",
            "sessions.csv",
            "calls.csv",
            "models.csv",
            "tools.csv",
            "commands.csv",
            "mcp_servers.csv",
            "limits_latest.csv",
            "limits_raw.csv",
            "metadata.csv",
        ] {
            assert!(response.path.join(name).exists(), "missing {name}");
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn batch_report_writes_multiple_formats_with_one_report_id() {
        let dir = std::env::temp_dir().join(format!(
            "tokenuse-report-batch-{}",
            Utc::now().timestamp_nanos_opt().unwrap()
        ));
        let ingested = Ingested {
            calls: vec![call("/tmp/a", "s1", "k1", 0)],
            limits: Vec::new(),
        };
        let request = ReportBatchRequest {
            formats: vec![ReportFormat::Json, ReportFormat::Html],
            period: Period::ThirtyDays,
            scope: ReportScope::AllProjects,
            redacted: false,
        };

        let responses = write_ingested_batch_to_dir(
            &dir,
            &request,
            &ingested,
            &CurrencyFormatter::usd(),
            "live",
        )
        .unwrap();

        assert_eq!(responses.len(), 2);
        assert_eq!(responses[0].format, ReportFormat::Json);
        assert_eq!(responses[1].format, ReportFormat::Html);
        let json = fs::read_to_string(&responses[0].path).unwrap();
        let html = fs::read_to_string(&responses[1].path).unwrap();
        assert!(json.contains("\"period\": \"30 Days\""));
        assert!(html.contains("30 Days"));
        assert_eq!(
            responses[0]
                .path
                .file_stem()
                .and_then(|value| value.to_str()),
            responses[1]
                .path
                .file_stem()
                .and_then(|value| value.to_str())
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn html_report_is_executive_deck_and_no_script() {
        let ingested = Ingested {
            calls: vec![call("/tmp/a", "s1", "k1", 0)],
            limits: Vec::new(),
        };
        let dataset = build_ingested_dataset(
            &request(ReportFormat::Html, false),
            &ingested,
            &CurrencyFormatter::usd(),
            "live",
            "id",
        );
        let html = build_html_report(&dataset);
        assert!(html.contains("data-report-page=\"overview\""));
        assert!(html.contains("data-report-page=\"activity\""));
        assert!(html.contains("data-report-page=\"breakdown\""));
        assert!(html.contains("@page { size: A4 landscape"));
        assert!(html.contains("class=\"kpi-ribbon overview-kpis\""));
        assert!(html.contains(".overview-kpis .kpi-label"));
        assert!(html.contains("color: #fff"));
        for label in [
            "Cost",
            "Calls",
            "Sessions",
            "Total tokens",
            "Cache hit",
            "Web search",
        ] {
            assert!(html.contains(label));
        }
        assert!(html.contains("Executive snapshot"));
        assert!(html.contains("Calendar heatmap"));
        assert!(html.contains("Daily call trend"));
        assert!(html.contains("Top Projects"));
        assert!(html.contains("Raw data:"));
        assert!(!html.contains("Executive usage report"));
        assert!(!html.contains("A client-ready summary of AI tool usage"));
        assert!(!html.contains("<h1>"));
        assert!(!html.contains("Prompt Appendix"));
        assert!(!html.contains("Raw Calls"));
        assert!(!html.contains("Limits Raw"));
        assert!(!html.contains("full prompt text"));
        assert!(!html.contains("<script"));
    }

    #[test]
    fn svg_report_is_executive_summary_not_dashboard_snapshot() {
        let dir = std::env::temp_dir().join(format!(
            "tokenuse-report-svg-{}",
            Utc::now().timestamp_nanos_opt().unwrap()
        ));
        let ingested = Ingested {
            calls: vec![call("/tmp/a", "s1", "k1", 0)],
            limits: Vec::new(),
        };
        let response = write_ingested_to_dir(
            &dir,
            &request(ReportFormat::Svg, false),
            &ingested,
            &CurrencyFormatter::usd(),
            "live",
        )
        .unwrap();
        let body = fs::read_to_string(response.path).unwrap();
        assert!(body.contains("Executive Summary"));
        assert!(body.contains("Activity Heatmap"));
        assert!(body.contains("Daily Call Trend"));
        assert!(body.contains("Top Projects"));
        assert!(body.contains("Top Models"));
        assert!(body.contains("Top Sessions"));
        assert!(!body.contains("Project Spend by Tool"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn visual_report_formats_write_files() {
        let dir = std::env::temp_dir().join(format!(
            "tokenuse-report-visual-{}",
            Utc::now().timestamp_nanos_opt().unwrap()
        ));
        let ingested = Ingested {
            calls: vec![call("/tmp/a", "s1", "k1", 0), call("/tmp/b", "s2", "k2", 1)],
            limits: Vec::new(),
        };
        let html = write_ingested_to_dir(
            &dir,
            &request(ReportFormat::Html, false),
            &ingested,
            &CurrencyFormatter::usd(),
            "live",
        )
        .unwrap();
        let pdf = write_ingested_to_dir(
            &dir,
            &request(ReportFormat::Pdf, false),
            &ingested,
            &CurrencyFormatter::usd(),
            "live",
        )
        .unwrap();
        let svg = write_ingested_to_dir(
            &dir,
            &request(ReportFormat::Svg, false),
            &ingested,
            &CurrencyFormatter::usd(),
            "live",
        )
        .unwrap();
        let png = write_ingested_to_dir(
            &dir,
            &request(ReportFormat::Png, false),
            &ingested,
            &CurrencyFormatter::usd(),
            "live",
        )
        .unwrap();

        assert!(fs::read_to_string(html.path)
            .unwrap()
            .contains("data-report-page=\"overview\""));
        assert!(fs::read(pdf.path).unwrap().starts_with(b"%PDF-"));
        assert!(fs::read_to_string(svg.path)
            .unwrap()
            .contains("Executive Summary"));
        assert!(fs::read(png.path)
            .unwrap()
            .starts_with(b"\x89PNG\r\n\x1a\n"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn report_insights_are_deterministic() {
        let ingested = Ingested {
            calls: vec![
                call("/tmp/a", "s1", "k1", 0),
                call("/tmp/a", "s1", "k2", 0),
                call("/tmp/b", "s2", "k3", 1),
            ],
            limits: Vec::new(),
        };
        let dataset = build_ingested_dataset(
            &request(ReportFormat::Html, false),
            &ingested,
            &CurrencyFormatter::usd(),
            "live",
            "id",
        );
        let insights = report_insights(&dataset);
        assert!(insights.primary_value.contains("66.7%"));
        assert_eq!(insights.avg_cost_per_session, "$0.38");
        assert_eq!(insights.calls_per_active_day, "1.5");
    }

    #[test]
    fn xlsx_report_writes_workbook() {
        let dir = std::env::temp_dir().join(format!(
            "tokenuse-report-xlsx-{}",
            Utc::now().timestamp_nanos_opt().unwrap()
        ));
        let ingested = Ingested {
            calls: vec![call("/tmp/a", "s1", "k1", 0)],
            limits: Vec::new(),
        };
        let response = write_ingested_to_dir(
            &dir,
            &request(ReportFormat::Xlsx, false),
            &ingested,
            &CurrencyFormatter::usd(),
            "live",
        )
        .unwrap();
        let bytes = fs::read(response.path).unwrap();
        assert!(bytes.starts_with(b"PK"));
        let _ = std::fs::remove_dir_all(dir);
    }
}
