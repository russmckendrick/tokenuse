use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::{Datelike, Duration, Local, NaiveDate};
use color_eyre::{eyre::WrapErr, Result};
use fulgur::{Engine, Margin, PageSize};
use plotters::prelude::*;

use crate::app::{Period, ProjectFilter, SortMode, Tool};
use crate::config::ConfigPaths;
use crate::data::{
    CountMetric, DailyMetric, DashboardData, ModelMetric, ProjectMetric, ProjectToolMetric,
    SessionDetailView, SessionMetric, Summary,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Json,
    Csv,
    Svg,
    Png,
    Html,
    Pdf,
}

impl ExportFormat {
    pub fn label(self) -> &'static str {
        match self {
            Self::Json => "JSON",
            Self::Csv => "CSV (one file per panel)",
            Self::Svg => "SVG (full dashboard)",
            Self::Png => "PNG (full dashboard)",
            Self::Html => "HTML (full workbook report)",
            Self::Pdf => "PDF (full workbook report)",
        }
    }

    pub const ALL: [Self; 6] = [
        Self::Json,
        Self::Csv,
        Self::Svg,
        Self::Png,
        Self::Html,
        Self::Pdf,
    ];
}

pub struct ExportContext<'a> {
    pub dashboard: &'a DashboardData,
    pub session: Option<&'a SessionDetailView>,
    pub period: Period,
    pub tool: Tool,
    pub project_filter: &'a ProjectFilter,
    pub sort: SortMode,
    pub currency_code: &'a str,
    pub source_label: &'a str,
}

// Palette mirrors src/theme.rs and DESIGN.md.
const SURFACE: RGBColor = RGBColor(32, 36, 56);
const BAR_EMPTY: RGBColor = RGBColor(41, 45, 66);
const TEXT: RGBColor = RGBColor(203, 212, 242);
const MUTED: RGBColor = RGBColor(161, 167, 195);
const DIM: RGBColor = RGBColor(110, 116, 146);
const PRIMARY: RGBColor = RGBColor(255, 143, 64);
const BLUE: RGBColor = RGBColor(98, 166, 255);
const BLUE_SOFT: RGBColor = RGBColor(126, 188, 255);
const GREEN: RGBColor = RGBColor(76, 242, 160);
const YELLOW: RGBColor = RGBColor(255, 214, 10);
const YELLOW_SOFT: RGBColor = RGBColor(245, 207, 108);
const ORANGE: RGBColor = RGBColor(255, 156, 72);
const RED: RGBColor = RGBColor(255, 95, 109);
const CYAN: RGBColor = RGBColor(77, 243, 232);
const MAGENTA: RGBColor = RGBColor(240, 90, 242);

const FONT_FAMILY: &str = "monospace";
const TITLE_SIZE: u32 = 22;
const HEAD_SIZE: u32 = 14;
const BODY_SIZE: u32 = 17;
const NUM_SIZE: u32 = 30;
const ROW_HEIGHT: i32 = 24;
const CHAR_W: i32 = 10;

// Row caps per panel — used both by the layout calculator and by each
// draw_* function so the panel size and the data we render line up.
const DAILY_CAP: usize = 8;
const PROJECTS_CAP: usize = 8;
const SESSIONS_CAP: usize = 10;
const PROJECT_TOOLS_CAP: usize = 13;
const MODELS_CAP: usize = 13;
const COUNTS_CAP: usize = 10;

// Panel chrome — these must agree with the offsets used inside draw_panel
// and the per-table draw_* helpers:
//   draw_panel   passes body(x+16, y + PANEL_BODY_TOP, w-32, h - PANEL_BODY_TOP - PANEL_BODY_BOTTOM)
//   draw_*       puts the column header at body_y and the first data row at
//                body_y + PANEL_HEADER_GAP, then ROW_HEIGHT per row after that.
const PANEL_BODY_TOP: i32 = 14;
const PANEL_BODY_BOTTOM: i32 = 12;
const PANEL_HEADER_GAP: i32 = 28;

pub fn write(
    paths: &ConfigPaths,
    format: ExportFormat,
    context: &ExportContext<'_>,
) -> Result<PathBuf> {
    let exports_root = default_export_dir(paths);
    write_to_dir(&exports_root, format, context)
}

pub fn default_export_dir(paths: &ConfigPaths) -> PathBuf {
    default_export_dir_from(paths, dirs::download_dir(), dirs::home_dir())
}

fn default_export_dir_from(
    paths: &ConfigPaths,
    download_dir: Option<PathBuf>,
    home_dir: Option<PathBuf>,
) -> PathBuf {
    download_dir
        .or_else(|| home_dir.map(|home| home.join("Downloads")))
        .unwrap_or_else(|| paths.dir.join("exports"))
}

pub fn write_to_dir(
    exports_root: &Path,
    format: ExportFormat,
    context: &ExportContext<'_>,
) -> Result<PathBuf> {
    fs::create_dir_all(exports_root)
        .wrap_err_with(|| format!("create {}", exports_root.display()))?;

    let slug = filter_slug(context.period, context.tool, context.project_filter);
    let stamp = Local::now().format("%Y%m%dT%H%M%S").to_string();

    match format {
        ExportFormat::Json => {
            let file = exports_root.join(format!("tokenuse-{stamp}-{slug}.json"));
            let text =
                serde_json::to_string_pretty(context.dashboard).wrap_err("serialize json")?;
            fs::write(&file, text).wrap_err_with(|| format!("write {}", file.display()))?;
            Ok(file)
        }
        ExportFormat::Csv => {
            let dir = exports_root.join(format!("tokenuse-{stamp}-{slug}-csv"));
            fs::create_dir_all(&dir).wrap_err_with(|| format!("create {}", dir.display()))?;
            write_csv_dir(&dir, context.dashboard)?;
            Ok(dir)
        }
        ExportFormat::Svg => {
            let file = exports_root.join(format!("tokenuse-{stamp}-{slug}.svg"));
            write_chart_svg(
                &file,
                context.dashboard,
                context.period,
                context.tool,
                context.project_filter,
            )?;
            Ok(file)
        }
        ExportFormat::Png => {
            let file = exports_root.join(format!("tokenuse-{stamp}-{slug}.png"));
            write_chart_png(
                &file,
                context.dashboard,
                context.period,
                context.tool,
                context.project_filter,
            )?;
            Ok(file)
        }
        ExportFormat::Html => {
            let file = exports_root.join(format!("tokenuse-{stamp}-{slug}.html"));
            write_html_report(&file, context, &stamp)?;
            Ok(file)
        }
        ExportFormat::Pdf => {
            let file = exports_root.join(format!("tokenuse-{stamp}-{slug}.pdf"));
            write_pdf_report(&file, context, &stamp)?;
            Ok(file)
        }
    }
}

fn filter_slug(period: Period, tool: Tool, project_filter: &ProjectFilter) -> String {
    let period = match period {
        Period::Today => "24h",
        Period::Week => "week",
        Period::ThirtyDays => "30d",
        Period::Month => "month",
        Period::AllTime => "all",
    };
    let tool = match tool {
        Tool::All => "alltools",
        Tool::ClaudeCode => "claude",
        Tool::Cursor => "cursor",
        Tool::Codex => "codex",
        Tool::Copilot => "copilot",
        Tool::Gemini => "gemini",
    };
    let project = match project_filter {
        ProjectFilter::All => "allprojects".to_string(),
        ProjectFilter::Selected { label, .. } => slugify(label),
    };
    format!("{period}-{tool}-{project}")
}

fn slugify(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut last_dash = false;
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash && !out.is_empty() {
            out.push('-');
            last_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        "untitled".into()
    } else {
        out
    }
}

fn write_csv_dir(dir: &Path, data: &DashboardData) -> Result<()> {
    write_summary_csv(dir, &data.summary)?;
    write_daily_csv(dir, &data.daily)?;
    write_projects_csv(dir, &data.projects)?;
    write_project_tools_csv(dir, &data.project_tools)?;
    write_sessions_csv(dir, &data.sessions)?;
    write_models_csv(dir, &data.models)?;
    write_counts_csv(dir, "tools.csv", &data.tools)?;
    write_counts_csv(dir, "commands.csv", &data.commands)?;
    write_counts_csv(dir, "mcp_servers.csv", &data.mcp_servers)?;
    Ok(())
}

fn write_csv(dir: &Path, name: &str, header: &[&str], rows: &[Vec<String>]) -> Result<()> {
    let path = dir.join(name);
    let mut out = String::with_capacity(rows.len() * 64);
    for (i, h) in header.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&csv_escape(h));
    }
    out.push('\n');
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            out.push_str(&csv_escape(cell));
        }
        out.push('\n');
    }
    fs::write(&path, out).wrap_err_with(|| format!("write {}", path.display()))
}

fn csv_escape(value: &str) -> String {
    if value.contains([',', '"', '\n', '\r']) {
        let escaped = value.replace('"', "\"\"");
        format!("\"{escaped}\"")
    } else {
        value.to_string()
    }
}

fn write_summary_csv(dir: &Path, summary: &Summary) -> Result<()> {
    write_csv(
        dir,
        "summary.csv",
        &[
            "cost",
            "calls",
            "sessions",
            "cache_hit",
            "input",
            "output",
            "cached",
            "written",
        ],
        &[vec![
            summary.cost.to_string(),
            summary.calls.to_string(),
            summary.sessions.to_string(),
            summary.cache_hit.to_string(),
            summary.input.to_string(),
            summary.output.to_string(),
            summary.cached.to_string(),
            summary.written.to_string(),
        ]],
    )
}

fn write_daily_csv(dir: &Path, rows: &[DailyMetric]) -> Result<()> {
    let data: Vec<Vec<String>> = rows
        .iter()
        .map(|r| vec![r.day.to_string(), r.cost.to_string(), r.calls.to_string()])
        .collect();
    write_csv(dir, "daily.csv", &["day", "cost", "calls"], &data)
}

fn write_projects_csv(dir: &Path, rows: &[ProjectMetric]) -> Result<()> {
    let data: Vec<Vec<String>> = rows
        .iter()
        .map(|r| {
            vec![
                r.name.to_string(),
                r.cost.to_string(),
                r.avg_per_session.to_string(),
                r.sessions.to_string(),
                r.tool_mix.to_string(),
            ]
        })
        .collect();
    write_csv(
        dir,
        "projects.csv",
        &["name", "cost", "avg_per_session", "sessions", "tool_mix"],
        &data,
    )
}

fn write_project_tools_csv(dir: &Path, rows: &[ProjectToolMetric]) -> Result<()> {
    let data: Vec<Vec<String>> = rows
        .iter()
        .map(|r| {
            vec![
                r.project.to_string(),
                r.tool.to_string(),
                r.cost.to_string(),
                r.calls.to_string(),
                r.sessions.to_string(),
                r.avg_per_session.to_string(),
            ]
        })
        .collect();
    write_csv(
        dir,
        "project_tools.csv",
        &[
            "project",
            "tool",
            "cost",
            "calls",
            "sessions",
            "avg_per_session",
        ],
        &data,
    )
}

fn write_sessions_csv(dir: &Path, rows: &[SessionMetric]) -> Result<()> {
    let data: Vec<Vec<String>> = rows
        .iter()
        .map(|r| {
            vec![
                r.date.to_string(),
                r.project.to_string(),
                r.cost.to_string(),
                r.calls.to_string(),
            ]
        })
        .collect();
    write_csv(
        dir,
        "sessions.csv",
        &["date", "project", "cost", "calls"],
        &data,
    )
}

fn write_models_csv(dir: &Path, rows: &[ModelMetric]) -> Result<()> {
    let data: Vec<Vec<String>> = rows
        .iter()
        .map(|r| {
            vec![
                r.name.to_string(),
                r.cost.to_string(),
                r.cache.to_string(),
                r.calls.to_string(),
            ]
        })
        .collect();
    write_csv(
        dir,
        "models.csv",
        &["name", "cost", "cache", "calls"],
        &data,
    )
}

fn write_counts_csv(dir: &Path, name: &str, rows: &[CountMetric]) -> Result<()> {
    let data: Vec<Vec<String>> = rows
        .iter()
        .map(|r| vec![r.name.to_string(), r.calls.to_string()])
        .collect();
    write_csv(dir, name, &["name", "calls"], &data)
}

fn write_html_report(path: &Path, context: &ExportContext<'_>, stamp: &str) -> Result<()> {
    let out = build_html_report(context, stamp);
    fs::write(path, out).wrap_err_with(|| format!("write {}", path.display()))
}

fn build_html_report(context: &ExportContext<'_>, stamp: &str) -> String {
    let generated_at = Local::now().format("%Y-%m-%d %H:%M:%S %Z").to_string();
    let title = report_title(context);
    let mut out = String::with_capacity(96 * 1024);

    push_report_document_open(&mut out, &title, HTML_REPORT_CSS, "report");

    push_report_header(&mut out, context, &generated_at, stamp);
    push_summary_cards(&mut out, &context.dashboard.summary, context.currency_code);
    push_dashboard_workbook(&mut out, context.dashboard);
    if let Some(session) = context.session {
        push_session_workbook(&mut out, session);
    }

    out.push_str("</main>\n</body>\n</html>\n");
    out
}

fn build_pdf_html_report(context: &ExportContext<'_>, stamp: &str) -> String {
    let generated_at = Local::now().format("%Y-%m-%d %H:%M:%S %Z").to_string();
    let title = report_title(context);
    let mut out = String::with_capacity(96 * 1024);
    let css = format!("{HTML_REPORT_CSS}\n{PDF_REPORT_CSS}");

    push_report_document_open(&mut out, &title, &css, "report pdf-report");

    push_report_header(&mut out, context, &generated_at, stamp);
    push_summary_cards_pdf(&mut out, &context.dashboard.summary, context.currency_code);
    push_dashboard_workbook_pdf(&mut out, context.dashboard);
    if let Some(session) = context.session {
        push_session_workbook_pdf(&mut out, session);
    }

    out.push_str("</main>\n</body>\n</html>\n");
    out
}

fn push_report_document_open(out: &mut String, title: &str, css: &str, main_class: &str) {
    out.push_str("<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n");
    out.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
    out.push_str("<title>");
    out.push_str(&escape_html(title));
    out.push_str("</title>\n<style>\n");
    out.push_str(css);
    out.push_str("\n</style>\n</head>\n<body>\n<main class=\"");
    out.push_str(main_class);
    out.push_str("\">\n");
}

fn report_title(context: &ExportContext<'_>) -> String {
    format!(
        "Token Use report - {} - {}",
        period_label(context.period),
        tool_label(context.tool)
    )
}

const HTML_REPORT_CSS: &str = r##"
:root {
  color-scheme: light;
  --paper: #f7f8fb;
  --panel: #ffffff;
  --ink: #1b2030;
  --muted: #5f667a;
  --faint: #e4e8f2;
  --line: #cfd5e6;
  --primary: #ff8f40;
  --blue: #2d72d9;
  --green: #14875a;
  --yellow: #9b7400;
  --red: #c53f4d;
  --cyan: #008b8a;
  --magenta: #b347b8;
  --heat-empty: #edf0f7;
  --heat-1: #62a6ff;
  --heat-2: #4cf2a0;
  --heat-3: #ffd60a;
  --heat-4: #ff9c48;
  --heat-5: #ff5f6d;
  font-family: "JetBrains Mono", SFMono-Regular, Menlo, Consolas, monospace;
}

* {
  box-sizing: border-box;
}

body {
  margin: 0;
  color: var(--ink);
  background: var(--paper);
  font: 13px/1.45 "JetBrains Mono", SFMono-Regular, Menlo, Consolas, monospace;
}

.report {
  width: min(1180px, calc(100% - 32px));
  margin: 0 auto;
  padding: 22px 0 36px;
}

.report-head,
.panel,
.kpi {
  background: var(--panel);
  border: 1px solid var(--line);
  border-radius: 3px;
}

.report-head {
  padding: 18px;
  border-color: #d6a172;
}

.brand-row,
.panel-head,
.meta-grid,
.kpis,
.call-facts {
  display: grid;
  gap: 8px;
}

.brand-row {
  grid-template-columns: auto 1fr;
  align-items: center;
  gap: 12px;
}

.brand-mark {
  width: 32px;
  height: 42px;
  display: block;
}

h1,
h2,
h3,
h4,
p {
  margin: 0;
}

h1 {
  color: var(--ink);
  font-size: 24px;
  line-height: 1.15;
}

.eyebrow {
  color: var(--primary);
  font-size: 12px;
  font-weight: 800;
  text-transform: uppercase;
}

.meta-grid {
  grid-template-columns: repeat(auto-fit, minmax(160px, 1fr));
  margin-top: 16px;
}

.meta-grid div,
.call-facts div {
  border-left: 2px solid var(--faint);
  padding-left: 8px;
  min-width: 0;
}

.meta-grid span,
.call-facts span,
.kpi span {
  display: block;
  color: var(--muted);
  font-size: 11px;
  font-weight: 700;
  text-transform: uppercase;
}

.meta-grid strong,
.call-facts strong,
.kpi strong {
  display: block;
  overflow-wrap: anywhere;
}

.kpis {
  grid-template-columns: repeat(auto-fit, minmax(170px, 1fr));
  margin: 14px 0;
}

.kpi {
  display: grid;
  grid-template-columns: auto 1fr;
  align-items: center;
  gap: 8px;
  padding: 11px;
}

.kpi svg,
.panel-head svg {
  width: 18px;
  height: 18px;
}

.kpi strong {
  font-size: 18px;
}

.kpi small {
  color: var(--muted);
}

.workbook-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 14px;
}

.panel {
  min-width: 0;
  overflow: hidden;
}

.panel.wide {
  grid-column: 1 / -1;
}

.panel-head {
  grid-template-columns: auto 1fr;
  align-items: center;
  padding: 10px 12px;
  border-bottom: 1px solid var(--line);
}

.panel h2 {
  font-size: 15px;
}

.tone-blue { --tone: var(--blue); }
.tone-green { --tone: var(--green); }
.tone-yellow { --tone: var(--yellow); }
.tone-red { --tone: var(--red); }
.tone-cyan { --tone: var(--cyan); }
.tone-magenta { --tone: var(--magenta); }
.tone-orange { --tone: var(--primary); }

.panel-head,
.money {
  color: var(--tone, var(--primary));
}

.table-wrap {
  overflow-x: auto;
}

table {
  width: 100%;
  border-collapse: collapse;
}

th,
td {
  padding: 7px 8px;
  border-bottom: 1px solid var(--faint);
  text-align: left;
  vertical-align: top;
}

th {
  color: var(--muted);
  font-size: 11px;
  font-weight: 800;
  text-transform: uppercase;
}

td.num,
th.num {
  text-align: right;
  white-space: nowrap;
}

.muted {
  color: var(--muted);
}

.empty {
  color: var(--muted);
  text-align: center;
}

.calendar-months {
  display: grid;
  gap: 12px;
  padding: 12px;
}

.calendar-title {
  color: var(--blue);
  font-size: 13px;
  font-weight: 800;
}

.calendar-grid {
  display: grid;
  grid-template-columns: repeat(7, minmax(0, 1fr));
  gap: 4px;
}

.calendar-weekday {
  color: var(--muted);
  font-size: 10px;
  font-weight: 800;
  text-align: center;
  text-transform: uppercase;
}

.calendar-blank {
  min-height: 72px;
}

.calendar-cell {
  min-height: 72px;
  display: grid;
  align-content: space-between;
  gap: 5px;
  padding: 7px;
  background: #fbfcff;
  border: 1px solid var(--faint);
  border-radius: 2px;
}

.calendar-cell.i1 { background: #eef6ff; border-color: #c7ddff; }
.calendar-cell.i2 { background: #ebfff6; border-color: #bff0db; }
.calendar-cell.i3 { background: #fff9d8; border-color: #f0df91; }
.calendar-cell.i4 { background: #fff0df; border-color: #efc08b; }
.calendar-cell.i5 { background: #fff0f2; border-color: #efa7af; }

.calendar-day-head {
  display: flex;
  justify-content: space-between;
  gap: 6px;
  color: var(--ink);
}

.calendar-day-head span,
.calendar-calls {
  color: var(--muted);
  font-size: 10px;
}

.calendar-cost {
  color: var(--yellow);
  font-weight: 800;
}

.heat {
  display: inline-flex;
  align-items: end;
  gap: 2px;
  min-width: 86px;
}

.heat span {
  width: 5px;
  height: 16px;
  background: var(--heat-empty);
}

.heat .filled.l0,
.heat .filled.l1 { background: var(--heat-1); }
.heat .filled.l2,
.heat .filled.l3 { background: var(--heat-2); }
.heat .filled.l4,
.heat .filled.l5,
.heat .filled.l6 { background: var(--heat-3); }
.heat .filled.l7,
.heat .filled.l8 { background: var(--heat-4); }
.heat .filled.l9,
.heat .filled.l10,
.heat .filled.l11 { background: var(--heat-5); }

.session-kpis {
  padding: 12px;
}

details.call-detail {
  border-top: 1px solid var(--faint);
  padding: 9px 12px;
}

details.call-detail summary {
  cursor: pointer;
  color: var(--red);
  font-weight: 800;
}

pre {
  margin: 8px 0 0;
  padding: 10px;
  overflow: auto;
  color: var(--ink);
  background: #f0f3f9;
  border: 1px solid var(--line);
  border-radius: 2px;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}

.report-footnote {
  color: var(--muted);
  margin-top: 8px;
}

@page {
  size: A4;
  margin: 8mm;
}

@media (max-width: 820px) {
  .report {
    width: calc(100% - 20px);
    padding-top: 10px;
  }

  .workbook-grid {
    grid-template-columns: 1fr;
  }

}

@media print {
  body {
    background: #ffffff;
  }

  .report {
    width: 100%;
    padding: 0;
  }

  .panel,
  .report-head,
  .kpi {
    break-inside: avoid;
  }

  details.call-detail {
    break-inside: avoid;
  }
}
"##;

const PDF_REPORT_CSS: &str = r##"
.pdf-report {
  --paper: #ffffff;
  --panel: #ffffff;
  --heat-empty: #f1f4fa;
  width: 100%;
  margin: 0;
  padding: 0;
  background: var(--paper);
}

html,
body {
  background: var(--paper);
}

.pdf-report .report-head {
  padding: 14px;
}

.pdf-report .brand-row {
  gap: 10px;
}

.pdf-report .brand-mark {
  width: 28px;
  height: 36px;
}

.pdf-report h1 {
  font-size: 22px;
}

.pdf-report .meta-grid {
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 8px 10px;
}

.pdf-report .report-head,
.pdf-report .pdf-kpis td {
  break-inside: avoid;
}

.pdf-kpis {
  width: 100%;
  margin: 0;
  padding: 10px 0;
  background: var(--paper);
  border-collapse: separate;
  border-spacing: 4px;
  table-layout: fixed;
}

.pdf-kpis td {
  height: 56px;
  padding: 8px 10px;
  background: var(--panel);
  border: 1px solid var(--line);
  vertical-align: top;
}

.pdf-kpis span,
.pdf-facts span {
  display: block;
  color: var(--muted);
  font-size: 10px;
  font-weight: 800;
  text-transform: uppercase;
}

.pdf-kpis strong {
  display: block;
  margin-top: 4px;
  color: var(--ink);
  font-size: 17px;
}

.pdf-kpis small {
  display: none;
}

.pdf-kpi-icon {
  display: inline-block;
  width: 15px;
  margin-right: 5px;
  vertical-align: -2px;
}

.pdf-kpi-icon svg {
  width: 13px;
  height: 13px;
}

.pdf-panel-title {
  display: block;
  margin: 8px 0 0;
  overflow: visible;
  padding: 9px 12px;
  background: var(--panel);
  border: 1px solid var(--line);
  border-bottom: 1px solid currentColor;
  font-size: 15px;
  break-after: avoid;
  page-break-after: avoid;
}

.pdf-workbook {
  padding-top: 8px;
  background: var(--paper);
}

.pdf-workbook .pdf-panel-title:first-child {
  margin-top: 0;
}

.pdf-panel-title.tone-blue { color: var(--blue); }
.pdf-panel-title.tone-green { color: var(--green); }
.pdf-panel-title.tone-yellow { color: var(--yellow); }
.pdf-panel-title.tone-red { color: var(--red); }
.pdf-panel-title.tone-cyan { color: var(--cyan); }
.pdf-panel-title.tone-magenta { color: var(--magenta); }
.pdf-panel-title.tone-orange { color: var(--primary); }

.pdf-panel-body {
  display: block;
  padding: 10px 12px 12px;
  background: var(--paper);
}

.pdf-panel-icon svg {
  display: inline-block;
  width: 15px;
  height: 15px;
  margin-right: 6px;
  vertical-align: -2px;
}

.pdf-table,
.pdf-calendar,
.pdf-facts {
  width: 100%;
  border-collapse: collapse;
  table-layout: fixed;
}

.pdf-table {
  font-size: 11px;
}

.pdf-table th,
.pdf-table td {
  padding: 5px 6px;
  border-bottom: 1px solid var(--faint);
  text-align: left;
  vertical-align: top;
  overflow-wrap: anywhere;
}

.pdf-table th {
  color: var(--muted);
  font-size: 10px;
  font-weight: 800;
  text-transform: uppercase;
}

.pdf-table .num {
  text-align: right;
  white-space: nowrap;
}

.pdf-table .money {
  color: var(--yellow);
  font-weight: 800;
}

.pdf-heat span {
  display: inline-block;
  width: 5px;
  height: 15px;
  margin-right: 2px;
  background: var(--heat-empty);
}

.pdf-heat .filled.l0,
.pdf-heat .filled.l1 { background: var(--heat-1); }
.pdf-heat .filled.l2,
.pdf-heat .filled.l3 { background: var(--heat-2); }
.pdf-heat .filled.l4,
.pdf-heat .filled.l5,
.pdf-heat .filled.l6 { background: var(--heat-3); }
.pdf-heat .filled.l7,
.pdf-heat .filled.l8 { background: var(--heat-4); }
.pdf-heat .filled.l9,
.pdf-heat .filled.l10,
.pdf-heat .filled.l11 { background: var(--heat-5); }

.pdf-month {
  margin: 0;
  padding-bottom: 12px;
  background: var(--paper);
}

.pdf-month-title {
  margin: 0 0 6px;
  color: var(--blue);
  font-size: 13px;
  font-weight: 800;
}

.pdf-calendar th {
  padding: 0 0 4px;
  color: var(--muted);
  font-size: 9px;
  font-weight: 800;
  text-align: center;
  text-transform: uppercase;
}

.pdf-calendar td {
  height: 64px;
  padding: 5px;
  background: var(--panel);
  border: 1px solid var(--faint);
  font-size: 10px;
  line-height: 1.2;
  vertical-align: top;
}

.pdf-calendar td.blank {
  background: var(--paper);
  border: 0;
}

.pdf-calendar td.i1 { background: #eef6ff; border-color: #c7ddff; }
.pdf-calendar td.i2 { background: #ebfff6; border-color: #bff0db; }
.pdf-calendar td.i3 { background: #fff9d8; border-color: #f0df91; }
.pdf-calendar td.i4 { background: #fff0df; border-color: #efc08b; }
.pdf-calendar td.i5 { background: #fff0f2; border-color: #efa7af; }

.pdf-calendar strong {
  color: var(--ink);
  font-size: 11px;
}

.pdf-calendar .cost {
  display: block;
  margin-top: 6px;
  color: var(--yellow);
  font-size: 10px;
  font-weight: 800;
  line-height: 1.2;
}

.pdf-calendar .calls {
  display: block;
  margin-top: 1px;
  color: var(--muted);
  font-size: 7px;
  line-height: 1.2;
}

.pdf-facts td {
  padding: 7px 8px;
  border-left: 2px solid var(--faint);
  vertical-align: top;
}

.pdf-facts strong {
  display: block;
  overflow-wrap: anywhere;
}

.pdf-call-detail {
  margin-top: 12px;
  padding-top: 10px;
  border-top: 1px solid var(--faint);
  break-inside: avoid;
}

.pdf-call-detail h3 {
  margin: 0 0 8px;
  color: var(--red);
  font-size: 13px;
}
"##;

fn push_report_header(
    out: &mut String,
    context: &ExportContext<'_>,
    generated_at: &str,
    stamp: &str,
) {
    out.push_str("<header class=\"report-head\">\n<div class=\"brand-row\">\n");
    out.push_str(BARS_LOGO_SVG);
    out.push_str(
        "<div><p class=\"eyebrow\">Full workbook report</p><h1>Token Use</h1></div>\n</div>\n",
    );
    out.push_str("<div class=\"meta-grid\">\n");
    push_meta(out, "generated", generated_at);
    push_meta(out, "export id", stamp);
    push_meta(out, "source", context.source_label);
    push_meta(out, "currency", context.currency_code);
    push_meta(out, "period", period_label(context.period));
    push_meta(out, "tool", tool_label(context.tool));
    push_meta(out, "project", context.project_filter.label());
    push_meta(out, "sort", context.sort.label());
    out.push_str("</div>\n</header>\n");
}

const BARS_LOGO_SVG: &str = r##"<svg class="brand-mark" viewBox="0 0 440 560" aria-hidden="true" xmlns="http://www.w3.org/2000/svg"><defs><linearGradient id="html-brand-bars" x1="0" y1="0" x2="0" y2="560" gradientUnits="userSpaceOnUse"><stop offset="0%" stop-color="#FFC06A"/><stop offset="45%" stop-color="#FF9A4D"/><stop offset="100%" stop-color="#F26A3D"/></linearGradient></defs><rect x="0" y="280" width="80" height="280" rx="16" fill="url(#html-brand-bars)"/><rect x="120" y="160" width="80" height="400" rx="16" fill="url(#html-brand-bars)"/><rect x="240" y="0" width="80" height="560" rx="16" fill="url(#html-brand-bars)"/><rect x="360" y="120" width="80" height="440" rx="16" fill="url(#html-brand-bars)"/></svg>"##;

fn push_meta(out: &mut String, label: &str, value: &str) {
    out.push_str("<div><span>");
    out.push_str(&escape_html(label));
    out.push_str("</span><strong>");
    out.push_str(&escape_html(value));
    out.push_str("</strong></div>\n");
}

fn push_summary_cards(out: &mut String, summary: &Summary, currency_code: &str) {
    out.push_str("<section class=\"kpis\" aria-label=\"Summary metrics\">\n");
    push_kpi(out, "cost", summary.cost, currency_code, "cost");
    push_kpi(out, "calls", summary.calls, summary.input, "calls");
    push_kpi(out, "sessions", summary.sessions, "active set", "sessions");
    push_kpi(out, "cache hit", summary.cache_hit, summary.cached, "cache");
    let tokens = format!("{} out", summary.output);
    push_kpi(out, "input", summary.input, &tokens, "tokens");
    let written = format!("{} written", summary.written);
    push_kpi(out, "cached", summary.cached, &written, "cache");
    out.push_str("</section>\n");
}

fn push_summary_cards_pdf(out: &mut String, summary: &Summary, currency_code: &str) {
    let tokens = format!("{} out", summary.output);
    let written = format!("{} written", summary.written);
    let cards = [
        ("cost", summary.cost, currency_code, "cost"),
        ("calls", summary.calls, summary.input, "calls"),
        ("sessions", summary.sessions, "active set", "sessions"),
        ("cache hit", summary.cache_hit, summary.cached, "cache"),
        ("input", summary.input, tokens.as_str(), "tokens"),
        ("cached", summary.cached, written.as_str(), "cache"),
    ];

    out.push_str("<table class=\"pdf-kpis\" aria-label=\"Summary metrics\">");
    for row in cards.chunks(3) {
        out.push_str("<tr>");
        for (label, value, detail, icon) in row {
            out.push_str("<td><span>");
            out.push_str(&escape_html(label));
            out.push_str("</span><strong><b class=\"pdf-kpi-icon\">");
            out.push_str(icon_svg(icon));
            out.push_str("</b>");
            out.push_str(&escape_html(value));
            out.push_str("</strong><small>");
            out.push_str(&escape_html(detail));
            out.push_str("</small></td>");
        }
        out.push_str("</tr>");
    }
    out.push_str("</table>\n");
}

fn push_kpi(out: &mut String, label: &str, value: &str, detail: &str, icon: &str) {
    out.push_str("<article class=\"kpi tone-orange\">");
    out.push_str(icon_svg(icon));
    out.push_str("<div><span>");
    out.push_str(&escape_html(label));
    out.push_str("</span><strong>");
    out.push_str(&escape_html(value));
    out.push_str("</strong><small>");
    out.push_str(&escape_html(detail));
    out.push_str("</small></div></article>\n");
}

fn push_dashboard_workbook(out: &mut String, data: &DashboardData) {
    out.push_str("<section class=\"workbook-grid\" aria-label=\"Dashboard workbook\">\n");
    push_daily_html(out, &data.daily);
    push_projects_html(out, &data.projects);
    push_sessions_html(out, &data.sessions);
    push_project_tools_html(out, &data.project_tools);
    push_models_html(out, &data.models);
    push_counts_html(out, "Core Tools", "tone-cyan", "tools", &data.tools);
    push_counts_html(
        out,
        "Shell Commands",
        "tone-orange",
        "terminal",
        &data.commands,
    );
    push_counts_html(
        out,
        "MCP Servers",
        "tone-magenta",
        "network",
        &data.mcp_servers,
    );
    out.push_str("</section>\n");
}

fn push_dashboard_workbook_pdf(out: &mut String, data: &DashboardData) {
    out.push_str("<section class=\"pdf-workbook\" aria-label=\"Dashboard workbook\">\n");
    push_daily_pdf(out, &data.daily);
    push_projects_pdf(out, &data.projects);
    push_sessions_pdf(out, &data.sessions);
    push_project_tools_pdf(out, &data.project_tools);
    push_models_pdf(out, &data.models);
    push_counts_pdf(out, "Core Tools", "tone-cyan", "tools", &data.tools);
    push_counts_pdf(
        out,
        "Shell Commands",
        "tone-orange",
        "terminal",
        &data.commands,
    );
    push_counts_pdf(
        out,
        "MCP Servers",
        "tone-magenta",
        "network",
        &data.mcp_servers,
    );
    out.push_str("</section>\n");
}

fn push_session_workbook(out: &mut String, session: &SessionDetailView) {
    push_section_open(out, "Selected Session", "tone-red wide", "session");
    out.push_str("<div class=\"session-kpis\">\n<div class=\"call-facts\">\n");
    push_meta(out, "project", &session.project);
    push_meta(out, "tool", session.tool);
    push_meta(out, "date range", &session.date_range);
    push_meta(out, "cost", &session.total_cost);
    push_meta(out, "calls", &session.total_calls.to_string());
    push_meta(out, "input", &session.total_input);
    push_meta(out, "output", &session.total_output);
    push_meta(out, "cache read", &session.total_cache_read);
    out.push_str("</div>\n");
    if let Some(note) = &session.note {
        out.push_str("<p class=\"report-footnote\">");
        out.push_str(&escape_html(note));
        out.push_str("</p>\n");
    }
    out.push_str("</div>\n");

    push_table_open(
        out,
        &[
            "time", "model", "cost", "input", "output", "cache", "tools", "prompt",
        ],
    );
    if session.calls.is_empty() {
        push_empty_row(out, 8);
    } else {
        for call in &session.calls {
            out.push_str("<tr>");
            push_text_cell(out, "", &call.timestamp);
            push_text_cell(out, "", &call.model);
            push_text_cell(out, "money num", &call.cost);
            push_text_cell(out, "num", &format_u64(call.input_tokens));
            push_text_cell(out, "num", &format_u64(call.output_tokens));
            push_text_cell(out, "num", &format_u64(call.cache_read + call.cache_write));
            push_text_cell(out, "", &call.tools);
            push_text_cell(out, "muted", &call.prompt);
            out.push_str("</tr>\n");
        }
    }
    push_table_close(out);

    for (idx, call) in session.calls.iter().enumerate() {
        out.push_str("<details class=\"call-detail\"><summary>");
        let _ = write!(
            out,
            "Call {} - {} - {}",
            idx + 1,
            escape_html(&call.model),
            escape_html(&call.cost)
        );
        out.push_str("</summary>\n<div class=\"call-facts\">\n");
        push_meta(out, "time", &call.timestamp);
        push_meta(out, "model", &call.model);
        push_meta(out, "cost", &call.cost);
        push_meta(out, "tools", &call.tools);
        push_meta(out, "input", &format_u64(call.input_tokens));
        push_meta(out, "output", &format_u64(call.output_tokens));
        push_meta(out, "cache read", &format_u64(call.cache_read));
        push_meta(out, "cache write", &format_u64(call.cache_write));
        push_meta(out, "reasoning", &format_u64(call.reasoning_tokens));
        push_meta(out, "web search", &format_u64(call.web_search_requests));
        out.push_str("</div>\n");
        if !call.bash_commands.is_empty() {
            out.push_str("<h4>Shell commands</h4><pre>");
            out.push_str(&escape_html(&call.bash_commands.join("\n")));
            out.push_str("</pre>\n");
        }
        out.push_str("<h4>Prompt</h4><pre>");
        let prompt = if call.prompt_full.is_empty() {
            &call.prompt
        } else {
            &call.prompt_full
        };
        out.push_str(&escape_html(prompt));
        out.push_str("</pre>\n</details>\n");
    }
    push_section_close(out);
}

fn push_session_workbook_pdf(out: &mut String, session: &SessionDetailView) {
    push_pdf_panel_open(out, "Selected Session", "tone-red", "session");
    out.push_str("<table class=\"pdf-facts\"><tbody>");
    let total_calls = session.total_calls.to_string();
    let facts = [
        ("project", session.project.as_str()),
        ("tool", session.tool),
        ("date range", session.date_range.as_str()),
        ("cost", session.total_cost.as_str()),
        ("calls", total_calls.as_str()),
        ("input", session.total_input.as_str()),
        ("output", session.total_output.as_str()),
        ("cache read", session.total_cache_read.as_str()),
    ];
    push_pdf_fact_rows(out, &facts, 4);
    out.push_str("</tbody></table>");
    if let Some(note) = &session.note {
        out.push_str("<p class=\"report-footnote\">");
        out.push_str(&escape_html(note));
        out.push_str("</p>");
    }

    push_pdf_table_open(
        out,
        &[
            "time", "model", "cost", "input", "output", "cache", "tools", "prompt",
        ],
    );
    if session.calls.is_empty() {
        push_pdf_empty_row(out, 8);
    } else {
        for call in &session.calls {
            out.push_str("<tr>");
            push_pdf_text_cell(out, "", &call.timestamp);
            push_pdf_text_cell(out, "", &call.model);
            push_pdf_text_cell(out, "money num", &call.cost);
            push_pdf_text_cell(out, "num", &format_u64(call.input_tokens));
            push_pdf_text_cell(out, "num", &format_u64(call.output_tokens));
            push_pdf_text_cell(out, "num", &format_u64(call.cache_read + call.cache_write));
            push_pdf_text_cell(out, "", &call.tools);
            push_pdf_text_cell(out, "muted", &call.prompt);
            out.push_str("</tr>");
        }
    }
    push_pdf_table_close(out);

    for (idx, call) in session.calls.iter().enumerate() {
        out.push_str("<div class=\"pdf-call-detail\"><h3>");
        let _ = write!(
            out,
            "Call {} - {} - {}",
            idx + 1,
            escape_html(&call.model),
            escape_html(&call.cost)
        );
        out.push_str("</h3><table class=\"pdf-facts\"><tbody>");
        let input = format_u64(call.input_tokens);
        let output = format_u64(call.output_tokens);
        let cache_read = format_u64(call.cache_read);
        let cache_write = format_u64(call.cache_write);
        let reasoning = format_u64(call.reasoning_tokens);
        let web_search = format_u64(call.web_search_requests);
        let call_facts = [
            ("time", call.timestamp.as_str()),
            ("model", call.model.as_str()),
            ("cost", call.cost.as_str()),
            ("tools", call.tools.as_str()),
            ("input", input.as_str()),
            ("output", output.as_str()),
            ("cache read", cache_read.as_str()),
            ("cache write", cache_write.as_str()),
            ("reasoning", reasoning.as_str()),
            ("web search", web_search.as_str()),
        ];
        push_pdf_fact_rows(out, &call_facts, 5);
        out.push_str("</tbody></table>");
        if !call.bash_commands.is_empty() {
            out.push_str("<h4>Shell commands</h4><pre>");
            out.push_str(&escape_html(&call.bash_commands.join("\n")));
            out.push_str("</pre>");
        }
        out.push_str("<h4>Prompt</h4><pre>");
        let prompt = if call.prompt_full.is_empty() {
            &call.prompt
        } else {
            &call.prompt_full
        };
        out.push_str(&escape_html(prompt));
        out.push_str("</pre></div>");
    }

    push_pdf_panel_close(out);
}

fn push_daily_pdf(out: &mut String, rows: &[DailyMetric]) {
    push_pdf_panel_open(out, "Daily Activity", "tone-blue", "calendar");
    if rows.is_empty() {
        out.push_str("<p class=\"empty\">no data</p>");
        push_pdf_panel_close(out);
        return;
    }

    let months = daily_calendar_months(rows);
    if months.is_empty() {
        push_daily_table_pdf(out, rows);
        push_pdf_panel_close(out);
        return;
    }

    for ((year, month), days) in months {
        push_pdf_calendar_month(out, year, month, &days);
    }
    push_pdf_panel_close(out);
}

fn push_daily_table_pdf(out: &mut String, rows: &[DailyMetric]) {
    push_pdf_table_open(out, &["date", "activity", "cost", "calls"]);
    for row in rows {
        out.push_str("<tr>");
        push_pdf_text_cell(out, "", row.day);
        push_pdf_raw_cell(out, "", &heat_html_pdf(row.value));
        push_pdf_text_cell(out, "money num", row.cost);
        push_pdf_text_cell(out, "num", &format_u64(row.calls));
        out.push_str("</tr>");
    }
    push_pdf_table_close(out);
}

fn push_pdf_calendar_month(out: &mut String, year: i32, month: u32, days: &[CalendarDay<'_>]) {
    let days_by_month_day: BTreeMap<u32, &DailyMetric> = days
        .iter()
        .map(|day| (day.date.day(), day.metric))
        .collect();
    let Some(first_day) = NaiveDate::from_ymd_opt(year, month, 1) else {
        return;
    };
    let month_days = days_in_month(year, month);
    let leading = first_day.weekday().num_days_from_monday() as usize;
    let cell_count = leading + month_days as usize;
    let week_count = cell_count.div_ceil(7);

    out.push_str("<section class=\"pdf-month\"><h3 class=\"pdf-month-title\">");
    out.push_str(month_name(month));
    let _ = write!(out, " {year}");
    out.push_str("</h3><table class=\"pdf-calendar\"><thead><tr>");
    for weekday in ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"] {
        out.push_str("<th>");
        out.push_str(weekday);
        out.push_str("</th>");
    }
    out.push_str("</tr></thead><tbody>");

    for week in 0..week_count {
        out.push_str("<tr>");
        for weekday in 0..7 {
            let slot = week * 7 + weekday;
            if slot < leading {
                out.push_str("<td class=\"blank\"></td>");
                continue;
            }
            let day = (slot - leading + 1) as u32;
            if day > month_days {
                out.push_str("<td class=\"blank\"></td>");
            } else if let Some(row) = days_by_month_day.get(&day).copied() {
                let calls = format_u64(row.calls);
                let _ = write!(
                    out,
                    "<td class=\"i{}\"><strong>{}</strong><span class=\"cost\">{}</span><span class=\"calls\">{} calls</span></td>",
                    calendar_intensity(row.value),
                    day,
                    escape_html(row.cost),
                    calls,
                );
            } else {
                let _ = write!(out, "<td><strong>{day}</strong></td>");
            }
        }
        out.push_str("</tr>");
    }
    out.push_str("</tbody></table>");

    out.push_str("</section>");
}

fn push_projects_pdf(out: &mut String, rows: &[ProjectMetric]) {
    push_pdf_panel_open(out, "By Project", "tone-green", "project");
    push_pdf_table_open(out, &["", "project", "cost", "avg/s", "sessions", "tools"]);
    if rows.is_empty() {
        push_pdf_empty_row(out, 6);
    } else {
        for row in rows {
            out.push_str("<tr>");
            push_pdf_raw_cell(out, "", &heat_html_pdf(row.value));
            push_pdf_text_cell(out, "", row.name);
            push_pdf_text_cell(out, "money num", row.cost);
            push_pdf_text_cell(out, "money num", row.avg_per_session);
            push_pdf_text_cell(out, "num", &format_u64(row.sessions));
            let tool_mix = compact_pdf_tool_mix(row.tool_mix);
            push_pdf_text_cell(out, "muted", &tool_mix);
            out.push_str("</tr>");
        }
    }
    push_pdf_table_close(out);
    push_pdf_panel_close(out);
}

fn push_sessions_pdf(out: &mut String, rows: &[SessionMetric]) {
    push_pdf_panel_open(out, "Top Sessions", "tone-red", "session");
    push_pdf_table_open(out, &["", "date", "project", "cost", "calls"]);
    if rows.is_empty() {
        push_pdf_empty_row(out, 5);
    } else {
        for row in rows {
            out.push_str("<tr>");
            push_pdf_raw_cell(out, "", &heat_html_pdf(row.value));
            push_pdf_text_cell(out, "", row.date);
            push_pdf_text_cell(out, "", row.project);
            push_pdf_text_cell(out, "money num", row.cost);
            push_pdf_text_cell(out, "num", &format_u64(row.calls));
            out.push_str("</tr>");
        }
    }
    push_pdf_table_close(out);
    push_pdf_panel_close(out);
}

fn push_project_tools_pdf(out: &mut String, rows: &[ProjectToolMetric]) {
    push_pdf_panel_open(out, "Project Spend by Tool", "tone-yellow", "split");
    push_pdf_table_open(
        out,
        &["", "project", "tool", "cost", "calls", "sessions", "avg/s"],
    );
    if rows.is_empty() {
        push_pdf_empty_row(out, 7);
    } else {
        for row in rows {
            out.push_str("<tr>");
            push_pdf_raw_cell(out, "", &heat_html_pdf(row.value));
            push_pdf_text_cell(out, "", row.project);
            push_pdf_text_cell(out, "", row.tool);
            push_pdf_text_cell(out, "money num", row.cost);
            push_pdf_text_cell(out, "num", &format_u64(row.calls));
            push_pdf_text_cell(out, "num", &format_u64(row.sessions));
            push_pdf_text_cell(out, "money num", row.avg_per_session);
            out.push_str("</tr>");
        }
    }
    push_pdf_table_close(out);
    push_pdf_panel_close(out);
}

fn push_models_pdf(out: &mut String, rows: &[ModelMetric]) {
    push_pdf_panel_open(out, "By Model", "tone-magenta", "model");
    push_pdf_table_open(out, &["", "model", "cost", "cache", "calls"]);
    if rows.is_empty() {
        push_pdf_empty_row(out, 5);
    } else {
        for row in rows {
            out.push_str("<tr>");
            push_pdf_raw_cell(out, "", &heat_html_pdf(row.value));
            push_pdf_text_cell(out, "", row.name);
            push_pdf_text_cell(out, "money num", row.cost);
            push_pdf_text_cell(out, "num", row.cache);
            push_pdf_text_cell(out, "num", &format_u64(row.calls));
            out.push_str("</tr>");
        }
    }
    push_pdf_table_close(out);
    push_pdf_panel_close(out);
}

fn push_counts_pdf(out: &mut String, title: &str, tone: &str, icon: &str, rows: &[CountMetric]) {
    push_pdf_panel_open(out, title, tone, icon);
    push_pdf_table_open(out, &["", "name", "calls"]);
    if rows.is_empty() {
        push_pdf_empty_row(out, 3);
    } else {
        for row in rows {
            out.push_str("<tr>");
            push_pdf_raw_cell(out, "", &heat_html_pdf(row.value));
            push_pdf_text_cell(out, "", row.name);
            push_pdf_text_cell(out, "num", &format_u64(row.calls));
            out.push_str("</tr>");
        }
    }
    push_pdf_table_close(out);
    push_pdf_panel_close(out);
}

fn push_pdf_panel_open(out: &mut String, title: &str, tone: &str, icon: &str) {
    out.push_str("<h2 class=\"pdf-panel-title ");
    out.push_str(tone);
    out.push_str("\"><span class=\"pdf-panel-icon\">");
    out.push_str(icon_svg(icon));
    out.push_str("</span>");
    out.push_str(&escape_html(title));
    out.push_str("</h2><div class=\"pdf-panel-body\">");
}

fn push_pdf_panel_close(out: &mut String) {
    out.push_str("</div>");
}

fn push_pdf_table_open(out: &mut String, headers: &[&str]) {
    out.push_str("<table class=\"pdf-table\"><thead><tr>");
    for header in headers {
        let class = if is_num_header(header) {
            " class=\"num\""
        } else {
            ""
        };
        out.push_str("<th");
        out.push_str(class);
        out.push('>');
        out.push_str(&escape_html(header));
        out.push_str("</th>");
    }
    out.push_str("</tr></thead><tbody>");
}

fn push_pdf_table_close(out: &mut String) {
    out.push_str("</tbody></table>");
}

fn push_pdf_empty_row(out: &mut String, colspan: usize) {
    let _ = write!(
        out,
        "<tr><td class=\"empty\" colspan=\"{}\">no data</td></tr>",
        colspan
    );
}

fn push_pdf_text_cell(out: &mut String, class: &str, value: &str) {
    out.push_str("<td");
    if !class.is_empty() {
        out.push_str(" class=\"");
        out.push_str(class);
        out.push('"');
    }
    out.push('>');
    out.push_str(&escape_html(value));
    out.push_str("</td>");
}

fn push_pdf_raw_cell(out: &mut String, class: &str, value: &str) {
    out.push_str("<td");
    if !class.is_empty() {
        out.push_str(" class=\"");
        out.push_str(class);
        out.push('"');
    }
    out.push('>');
    out.push_str(value);
    out.push_str("</td>");
}

fn push_pdf_fact_rows(out: &mut String, facts: &[(&str, &str)], cols: usize) {
    for row in facts.chunks(cols) {
        out.push_str("<tr>");
        for (label, value) in row {
            out.push_str("<td><span>");
            out.push_str(&escape_html(label));
            out.push_str("</span><strong>");
            out.push_str(&escape_html(value));
            out.push_str("</strong></td>");
        }
        for _ in row.len()..cols {
            out.push_str("<td></td>");
        }
        out.push_str("</tr>");
    }
}

fn heat_html_pdf(value: u64) -> String {
    let cells = 12u64;
    let filled = ((value.min(100) as f64 / 100.0) * cells as f64).ceil() as u64;
    let mut out = String::from("<span class=\"pdf-heat\" aria-hidden=\"true\">");
    for idx in 0..cells {
        if idx < filled {
            let _ = write!(out, "<span class=\"filled l{}\"></span>", idx);
        } else {
            out.push_str("<span></span>");
        }
    }
    out.push_str("</span>");
    out
}

fn compact_pdf_tool_mix(value: &str) -> String {
    let mut labels: Vec<&str> = Vec::new();
    for token in value.split_whitespace() {
        if token
            .chars()
            .any(|ch| ch.is_ascii_digit() || matches!(ch, '$' | '£' | '€' | '¥'))
        {
            continue;
        }
        if !labels.contains(&token) {
            labels.push(token);
        }
    }

    if labels.is_empty() {
        value.to_owned()
    } else {
        labels.join(" ")
    }
}

fn push_daily_html(out: &mut String, rows: &[DailyMetric]) {
    push_section_open(out, "Daily Activity", "tone-blue wide", "calendar");
    if rows.is_empty() {
        out.push_str("<p class=\"empty\">no data</p>\n");
        push_section_close(out);
        return;
    }

    let months = daily_calendar_months(rows);
    if months.is_empty() {
        push_daily_table_html(out, rows);
        push_section_close(out);
        return;
    }

    out.push_str("<div class=\"calendar-months\">\n");
    for ((year, month), days) in months {
        let days_by_month_day: BTreeMap<u32, &DailyMetric> = days
            .iter()
            .map(|day| (day.date.day(), day.metric))
            .collect();
        let Some(first_day) = NaiveDate::from_ymd_opt(year, month, 1) else {
            continue;
        };
        let month_days = days_in_month(year, month);
        out.push_str("<section class=\"calendar-month\"><h3 class=\"calendar-title\">");
        out.push_str(month_name(month));
        let _ = write!(out, " {year}");
        out.push_str("</h3><div class=\"calendar-grid\">\n");
        for weekday in ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"] {
            out.push_str("<div class=\"calendar-weekday\">");
            out.push_str(weekday);
            out.push_str("</div>");
        }
        for _ in 0..first_day.weekday().num_days_from_monday() {
            out.push_str("<div class=\"calendar-blank\" aria-hidden=\"true\"></div>");
        }
        for day in 1..=month_days {
            if let Some(row) = days_by_month_day.get(&day).copied() {
                push_calendar_day(out, day, row);
            } else {
                let _ = write!(
                    out,
                    "<div class=\"calendar-cell calendar-empty\"><div class=\"calendar-day-head\"><strong>{day}</strong></div></div>"
                );
            }
        }
        out.push_str("</div></section>\n");
    }
    out.push_str("</div>\n");
    push_section_close(out);
}

fn push_daily_table_html(out: &mut String, rows: &[DailyMetric]) {
    push_table_open(out, &["date", "activity", "cost", "calls"]);
    for row in rows {
        out.push_str("<tr>");
        push_text_cell(out, "", row.day);
        push_raw_cell(out, "", &heat_html(row.value));
        push_text_cell(out, "money num", row.cost);
        push_text_cell(out, "num", &format_u64(row.calls));
        out.push_str("</tr>\n");
    }
    push_table_close(out);
}

struct CalendarDay<'a> {
    date: NaiveDate,
    metric: &'a DailyMetric,
}

fn daily_calendar_months(rows: &[DailyMetric]) -> BTreeMap<(i32, u32), Vec<CalendarDay<'_>>> {
    let mut months: BTreeMap<(i32, u32), Vec<CalendarDay<'_>>> = BTreeMap::new();
    for row in rows {
        if let Some(date) = parse_report_day(row.day) {
            months
                .entry((date.year(), date.month()))
                .or_default()
                .push(CalendarDay { date, metric: row });
        }
    }
    for days in months.values_mut() {
        days.sort_by_key(|day| day.date);
    }
    months
}

fn parse_report_day(value: &str) -> Option<NaiveDate> {
    let (month, day) = value.split_once('-')?;
    let month = month.parse::<u32>().ok()?;
    let day = day.parse::<u32>().ok()?;
    let today = Local::now().date_naive();
    let mut date = NaiveDate::from_ymd_opt(today.year(), month, day)?;
    if date > today + Duration::days(1) {
        date = NaiveDate::from_ymd_opt(today.year() - 1, month, day)?;
    }
    Some(date)
}

fn days_in_month(year: i32, month: u32) -> u32 {
    let next_month = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1)
    }
    .expect("valid adjacent month");
    (next_month - Duration::days(1)).day()
}

fn month_name(month: u32) -> &'static str {
    match month {
        1 => "January",
        2 => "February",
        3 => "March",
        4 => "April",
        5 => "May",
        6 => "June",
        7 => "July",
        8 => "August",
        9 => "September",
        10 => "October",
        11 => "November",
        12 => "December",
        _ => "",
    }
}

fn push_calendar_day(out: &mut String, day: u32, row: &DailyMetric) {
    let _ = write!(
        out,
        "<div class=\"calendar-cell i{}\"><div class=\"calendar-day-head\"><strong>{}</strong><span>{}</span></div><div class=\"calendar-cost\">{}</div><div class=\"calendar-calls\">{} calls</div></div>",
        calendar_intensity(row.value),
        day,
        escape_html(row.day),
        escape_html(row.cost),
        format_u64(row.calls),
    );
}

fn calendar_intensity(value: u64) -> u64 {
    if value == 0 {
        0
    } else {
        ((value.min(100) - 1) / 20) + 1
    }
}

fn push_projects_html(out: &mut String, rows: &[ProjectMetric]) {
    push_section_open(out, "By Project", "tone-green wide", "project");
    push_table_open(out, &["", "project", "cost", "avg/s", "sessions", "tools"]);
    if rows.is_empty() {
        push_empty_row(out, 6);
    } else {
        for row in rows {
            out.push_str("<tr>");
            push_raw_cell(out, "", &heat_html(row.value));
            push_text_cell(out, "", row.name);
            push_text_cell(out, "money num", row.cost);
            push_text_cell(out, "money num", row.avg_per_session);
            push_text_cell(out, "num", &format_u64(row.sessions));
            push_text_cell(out, "muted", row.tool_mix);
            out.push_str("</tr>\n");
        }
    }
    push_table_close(out);
    push_section_close(out);
}

fn push_sessions_html(out: &mut String, rows: &[SessionMetric]) {
    push_section_open(out, "Top Sessions", "tone-red wide", "session");
    push_table_open(out, &["", "date", "project", "cost", "calls"]);
    if rows.is_empty() {
        push_empty_row(out, 5);
    } else {
        for row in rows {
            out.push_str("<tr>");
            push_raw_cell(out, "", &heat_html(row.value));
            push_text_cell(out, "", row.date);
            push_text_cell(out, "", row.project);
            push_text_cell(out, "money num", row.cost);
            push_text_cell(out, "num", &format_u64(row.calls));
            out.push_str("</tr>\n");
        }
    }
    push_table_close(out);
    push_section_close(out);
}

fn push_project_tools_html(out: &mut String, rows: &[ProjectToolMetric]) {
    push_section_open(out, "Project Spend by Tool", "tone-yellow", "split");
    push_table_open(
        out,
        &["", "project", "tool", "cost", "calls", "sessions", "avg/s"],
    );
    if rows.is_empty() {
        push_empty_row(out, 7);
    } else {
        for row in rows {
            out.push_str("<tr>");
            push_raw_cell(out, "", &heat_html(row.value));
            push_text_cell(out, "", row.project);
            push_text_cell(out, "", row.tool);
            push_text_cell(out, "money num", row.cost);
            push_text_cell(out, "num", &format_u64(row.calls));
            push_text_cell(out, "num", &format_u64(row.sessions));
            push_text_cell(out, "money num", row.avg_per_session);
            out.push_str("</tr>\n");
        }
    }
    push_table_close(out);
    push_section_close(out);
}

fn push_models_html(out: &mut String, rows: &[ModelMetric]) {
    push_section_open(out, "By Model", "tone-magenta", "model");
    push_table_open(out, &["", "model", "cost", "cache", "calls"]);
    if rows.is_empty() {
        push_empty_row(out, 5);
    } else {
        for row in rows {
            out.push_str("<tr>");
            push_raw_cell(out, "", &heat_html(row.value));
            push_text_cell(out, "", row.name);
            push_text_cell(out, "money num", row.cost);
            push_text_cell(out, "num", row.cache);
            push_text_cell(out, "num", &format_u64(row.calls));
            out.push_str("</tr>\n");
        }
    }
    push_table_close(out);
    push_section_close(out);
}

fn push_counts_html(out: &mut String, title: &str, tone: &str, icon: &str, rows: &[CountMetric]) {
    push_section_open(out, title, tone, icon);
    push_table_open(out, &["", "name", "calls"]);
    if rows.is_empty() {
        push_empty_row(out, 3);
    } else {
        for row in rows {
            out.push_str("<tr>");
            push_raw_cell(out, "", &heat_html(row.value));
            push_text_cell(out, "", row.name);
            push_text_cell(out, "num", &format_u64(row.calls));
            out.push_str("</tr>\n");
        }
    }
    push_table_close(out);
    push_section_close(out);
}

fn push_section_open(out: &mut String, title: &str, class: &str, icon: &str) {
    out.push_str("<section class=\"panel ");
    out.push_str(class);
    out.push_str("\">\n<header class=\"panel-head\">");
    out.push_str(icon_svg(icon));
    out.push_str("<h2>");
    out.push_str(&escape_html(title));
    out.push_str("</h2></header>\n");
}

fn push_section_close(out: &mut String) {
    out.push_str("</section>\n");
}

fn push_table_open(out: &mut String, headers: &[&str]) {
    out.push_str("<div class=\"table-wrap\"><table><thead><tr>");
    for header in headers {
        let class = if is_num_header(header) {
            " class=\"num\""
        } else {
            ""
        };
        out.push_str("<th");
        out.push_str(class);
        out.push('>');
        out.push_str(&escape_html(header));
        out.push_str("</th>");
    }
    out.push_str("</tr></thead><tbody>\n");
}

fn is_num_header(header: &str) -> bool {
    matches!(
        header,
        "cost"
            | "calls"
            | "avg/s"
            | "sessions"
            | "used"
            | "left"
            | "tokens"
            | "input"
            | "output"
            | "cache"
    )
}

fn push_table_close(out: &mut String) {
    out.push_str("</tbody></table></div>\n");
}

fn push_empty_row(out: &mut String, colspan: usize) {
    let _ = writeln!(
        out,
        "<tr><td class=\"empty\" colspan=\"{}\">no data</td></tr>",
        colspan
    );
}

fn push_text_cell(out: &mut String, class: &str, value: &str) {
    out.push_str("<td");
    if !class.is_empty() {
        out.push_str(" class=\"");
        out.push_str(class);
        out.push('"');
    }
    out.push('>');
    out.push_str(&escape_html(value));
    out.push_str("</td>");
}

fn push_raw_cell(out: &mut String, class: &str, value: &str) {
    out.push_str("<td");
    if !class.is_empty() {
        out.push_str(" class=\"");
        out.push_str(class);
        out.push('"');
    }
    out.push('>');
    out.push_str(value);
    out.push_str("</td>");
}

fn heat_html(value: u64) -> String {
    let cells = 12u64;
    let filled = ((value.min(100) as f64 / 100.0) * cells as f64).ceil() as u64;
    let mut out = String::from("<span class=\"heat\" aria-hidden=\"true\">");
    for idx in 0..cells {
        if idx < filled {
            let _ = write!(out, "<span class=\"filled l{}\"></span>", idx);
        } else {
            out.push_str("<span></span>");
        }
    }
    out.push_str("</span>");
    out
}

fn icon_svg(kind: &str) -> &'static str {
    match kind {
        "calendar" => {
            r#"<svg viewBox="0 0 24 24" aria-hidden="true"><rect x="4" y="5" width="16" height="15" rx="2" fill="none" stroke="currentColor" stroke-width="2"/><path d="M8 3v4M16 3v4M4 10h16" fill="none" stroke="currentColor" stroke-width="2"/></svg>"#
        }
        "project" => {
            r#"<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 7h6l2 3h8v9H4z" fill="none" stroke="currentColor" stroke-width="2"/></svg>"#
        }
        "session" => {
            r#"<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M5 6h14M5 12h14M5 18h10" fill="none" stroke="currentColor" stroke-width="2"/></svg>"#
        }
        "split" => {
            r#"<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M5 5h5v5H5zM14 5h5v5h-5zM5 14h5v5H5zM14 14h5v5h-5z" fill="none" stroke="currentColor" stroke-width="2"/></svg>"#
        }
        "model" => {
            r#"<svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="7" fill="none" stroke="currentColor" stroke-width="2"/><path d="M12 5v14M5 12h14" fill="none" stroke="currentColor" stroke-width="2"/></svg>"#
        }
        "tools" => {
            r#"<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M14 6l4 4-8 8H6v-4z" fill="none" stroke="currentColor" stroke-width="2"/></svg>"#
        }
        "terminal" => {
            r#"<svg viewBox="0 0 24 24" aria-hidden="true"><rect x="4" y="5" width="16" height="14" rx="2" fill="none" stroke="currentColor" stroke-width="2"/><path d="M7 9l3 3-3 3M12 15h5" fill="none" stroke="currentColor" stroke-width="2"/></svg>"#
        }
        "network" => {
            r#"<svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="6" cy="12" r="2" fill="none" stroke="currentColor" stroke-width="2"/><circle cx="18" cy="6" r="2" fill="none" stroke="currentColor" stroke-width="2"/><circle cx="18" cy="18" r="2" fill="none" stroke="currentColor" stroke-width="2"/><path d="M8 11l8-4M8 13l8 4" fill="none" stroke="currentColor" stroke-width="2"/></svg>"#
        }
        "cost" => {
            r#"<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 3v18M17 7.5c-.8-1-2.4-1.8-4.4-1.8-2.4 0-4.1 1.1-4.1 2.8 0 4.5 9 1.8 9 6.6 0 1.9-1.9 3.2-4.6 3.2-2.2 0-4.1-.8-5.1-2" fill="none" stroke="currentColor" stroke-width="2"/></svg>"#
        }
        "calls" => {
            r#"<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M5 6h14M5 12h14M5 18h14" fill="none" stroke="currentColor" stroke-width="2"/></svg>"#
        }
        "sessions" => {
            r#"<svg viewBox="0 0 24 24" aria-hidden="true"><rect x="5" y="5" width="14" height="14" rx="2" fill="none" stroke="currentColor" stroke-width="2"/><path d="M8 9h8M8 13h8M8 17h5" fill="none" stroke="currentColor" stroke-width="2"/></svg>"#
        }
        "cache" => {
            r#"<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M6 7c0-2 12-2 12 0v10c0 2-12 2-12 0zM6 7c0 2 12 2 12 0M6 12c0 2 12 2 12 0" fill="none" stroke="currentColor" stroke-width="2"/></svg>"#
        }
        _ => {
            r#"<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M5 12h14M12 5v14" fill="none" stroke="currentColor" stroke-width="2"/></svg>"#
        }
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

fn write_pdf_report(path: &Path, context: &ExportContext<'_>, stamp: &str) -> Result<()> {
    let html = build_pdf_html_report(context, stamp);
    let bytes = Engine::builder()
        .page_size(PageSize::A4)
        .margin(Margin::uniform_mm(8.0))
        .title(report_title(context))
        .build()
        .render_html(&html)
        .wrap_err("render branded HTML workbook to PDF")?;

    fs::write(path, bytes).wrap_err_with(|| format!("write {}", path.display()))
}

fn format_u64(value: u64) -> String {
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

const CANVAS_W: u32 = 1800;

// Top-of-canvas chrome above the panel grid.
const OUTER_PAD: i32 = 24;
const PERIOD_STRIP_H: i32 = 32;
const PERIOD_STRIP_GAP: i32 = 12;
const SUMMARY_H: i32 = 130;
const ROW_GAP: i32 = 12;

struct Layout {
    row1_h: i32, // Daily | Projects
    sessions_h: i32,
    row3_h: i32, // Project Spend by Tool | By Model
    row4_h: i32, // Core Tools | Shell Commands
    mcp_h: i32,
    canvas_h: u32,
}

fn compute_layout(data: &DashboardData) -> Layout {
    let row1_h = panel_h(data.daily.len().min(DAILY_CAP))
        .max(panel_h(data.projects.len().min(PROJECTS_CAP)));
    let sessions_h = panel_h(data.sessions.len().min(SESSIONS_CAP));
    let row3_h = panel_h(data.project_tools.len().min(PROJECT_TOOLS_CAP))
        .max(panel_h(data.models.len().min(MODELS_CAP)));
    let row4_h =
        panel_h(data.tools.len().min(COUNTS_CAP)).max(panel_h(data.commands.len().min(COUNTS_CAP)));
    let mcp_h = panel_h(data.mcp_servers.len().min(COUNTS_CAP));

    // Stack: outer pad, period strip, gap, summary, gap, four panel rows
    // separated by ROW_GAP, then the MCP row, then outer pad.
    let total = OUTER_PAD
        + PERIOD_STRIP_H
        + PERIOD_STRIP_GAP
        + SUMMARY_H
        + ROW_GAP
        + row1_h
        + ROW_GAP
        + sessions_h
        + ROW_GAP
        + row3_h
        + ROW_GAP
        + row4_h
        + ROW_GAP
        + mcp_h
        + OUTER_PAD;

    Layout {
        row1_h,
        sessions_h,
        row3_h,
        row4_h,
        mcp_h,
        canvas_h: total as u32,
    }
}

fn write_chart_svg(
    path: &Path,
    data: &DashboardData,
    period: Period,
    tool: Tool,
    project_filter: &ProjectFilter,
) -> Result<()> {
    let layout = compute_layout(data);
    let backend = SVGBackend::new(path, (CANVAS_W, layout.canvas_h));
    render_dashboard_chart(backend, data, period, tool, project_filter, &layout)
        .map_err(|e| color_eyre::eyre::eyre!("svg render failed: {e}"))?;
    Ok(())
}

fn write_chart_png(
    path: &Path,
    data: &DashboardData,
    period: Period,
    tool: Tool,
    project_filter: &ProjectFilter,
) -> Result<()> {
    let layout = compute_layout(data);
    let backend = BitMapBackend::new(path, (CANVAS_W, layout.canvas_h));
    render_dashboard_chart(backend, data, period, tool, project_filter, &layout)
        .map_err(|e| color_eyre::eyre::eyre!("png render failed: {e}"))?;
    Ok(())
}

type ChartResult = std::result::Result<(), Box<dyn std::error::Error + Send + Sync>>;

fn body_style(color: &RGBColor) -> TextStyle<'static> {
    (FONT_FAMILY, BODY_SIZE).into_font().color(color)
}

fn head_style() -> TextStyle<'static> {
    (FONT_FAMILY, HEAD_SIZE).into_font().color(&DIM)
}

fn title_style(color: &RGBColor) -> TextStyle<'static> {
    (FONT_FAMILY, TITLE_SIZE)
        .into_font()
        .style(FontStyle::Bold)
        .color(color)
}

fn num_style(color: &RGBColor) -> TextStyle<'static> {
    (FONT_FAMILY, NUM_SIZE)
        .into_font()
        .style(FontStyle::Bold)
        .color(color)
}

fn render_dashboard_chart<DB>(
    backend: DB,
    data: &DashboardData,
    period: Period,
    tool: Tool,
    project_filter: &ProjectFilter,
    layout: &Layout,
) -> ChartResult
where
    DB: DrawingBackend,
    DB::ErrorType: 'static,
{
    let root = backend.into_drawing_area();
    root.fill(&SURFACE)?;

    let outer_x = OUTER_PAD;
    let mut y = OUTER_PAD;
    let panel_w = CANVAS_W as i32 - outer_x * 2;
    let half_w = (panel_w - ROW_GAP) / 2;

    // Header band (period chips + summary)
    draw_period_strip(&root, outer_x, y, panel_w, period, tool, project_filter)?;
    y += PERIOD_STRIP_H + PERIOD_STRIP_GAP;
    draw_summary_panel(
        &root,
        outer_x,
        y,
        panel_w,
        SUMMARY_H,
        &data.summary,
        period,
        tool,
    )?;
    y += SUMMARY_H + ROW_GAP;

    // Row: Daily | Projects
    draw_panel(
        &root,
        outer_x,
        y,
        half_w,
        layout.row1_h,
        "Daily Activity",
        &BLUE,
        |x, y, w, h| draw_daily(&root, x, y, w, h, &data.daily),
    )?;
    draw_panel(
        &root,
        outer_x + half_w + ROW_GAP,
        y,
        half_w,
        layout.row1_h,
        "By Project",
        &GREEN,
        |x, y, w, h| draw_projects(&root, x, y, w, h, &data.projects),
    )?;
    y += layout.row1_h + ROW_GAP;

    // Row: Top Sessions full width
    draw_panel(
        &root,
        outer_x,
        y,
        panel_w,
        layout.sessions_h,
        "Top Sessions",
        &RED,
        |x, y, w, h| draw_sessions(&root, x, y, w, h, &data.sessions),
    )?;
    y += layout.sessions_h + ROW_GAP;

    // Row: Project Tools | Models
    draw_panel(
        &root,
        outer_x,
        y,
        half_w,
        layout.row3_h,
        "Project Spend by Tool",
        &YELLOW,
        |x, y, w, h| draw_project_tools(&root, x, y, w, h, &data.project_tools),
    )?;
    draw_panel(
        &root,
        outer_x + half_w + ROW_GAP,
        y,
        half_w,
        layout.row3_h,
        "By Model",
        &MAGENTA,
        |x, y, w, h| draw_models(&root, x, y, w, h, &data.models),
    )?;
    y += layout.row3_h + ROW_GAP;

    // Row: Core Tools | Shell Commands
    draw_panel(
        &root,
        outer_x,
        y,
        half_w,
        layout.row4_h,
        "Core Tools",
        &CYAN,
        |x, y, w, h| draw_counts(&root, x, y, w, h, &data.tools),
    )?;
    draw_panel(
        &root,
        outer_x + half_w + ROW_GAP,
        y,
        half_w,
        layout.row4_h,
        "Shell Commands",
        &PRIMARY,
        |x, y, w, h| draw_counts(&root, x, y, w, h, &data.commands),
    )?;
    y += layout.row4_h + ROW_GAP;

    // Row: MCP Servers full width
    draw_panel(
        &root,
        outer_x,
        y,
        panel_w,
        layout.mcp_h,
        "MCP Servers",
        &MAGENTA,
        |x, y, w, h| draw_counts(&root, x, y, w, h, &data.mcp_servers),
    )?;

    root.present()?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn draw_panel<DB, F>(
    root: &DrawingArea<DB, plotters::coord::Shift>,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    title: &str,
    accent: &RGBColor,
    body: F,
) -> ChartResult
where
    DB: DrawingBackend,
    DB::ErrorType: 'static,
    F: FnOnce(i32, i32, i32, i32) -> ChartResult,
{
    // Border
    root.draw(&Rectangle::new(
        [(x, y), (x + w - 1, y + h - 1)],
        accent.stroke_width(1),
    ))?;

    // Punch a small surface-colored rect over the top-left of the border so the
    // title can sit on top of the line, matching the TUI panel_block look.
    let title_text = format!(" {title} ");
    let title_w = title_text.chars().count() as i32 * (CHAR_W + 1);
    root.draw(&Rectangle::new(
        [(x + 14, y - 1), (x + 14 + title_w + 6, y + 1)],
        SURFACE.filled(),
    ))?;
    root.draw_text(&title_text, &title_style(accent), (x + 18, y - 9))?;

    body(
        x + 16,
        y + PANEL_BODY_TOP,
        w - 32,
        h - PANEL_BODY_TOP - PANEL_BODY_BOTTOM,
    )
}

/// Natural height of a panel rendering `rows` data rows (plus a column
/// header). Empty data still gets a single row's worth of room for the
/// "no data" placeholder.
fn panel_h(rows: usize) -> i32 {
    let n = rows.max(1) as i32;
    PANEL_BODY_TOP + PANEL_HEADER_GAP + n * ROW_HEIGHT + PANEL_BODY_BOTTOM
}

fn draw_period_strip<DB>(
    root: &DrawingArea<DB, plotters::coord::Shift>,
    x: i32,
    y: i32,
    w: i32,
    period: Period,
    tool: Tool,
    project_filter: &ProjectFilter,
) -> ChartResult
where
    DB: DrawingBackend,
    DB::ErrorType: 'static,
{
    let chips = [
        (Period::Today, "24 Hours"),
        (Period::Week, "7 Days"),
        (Period::ThirtyDays, "30 Days"),
        (Period::Month, "This Month"),
        (Period::AllTime, "All Time"),
    ];

    let mut cx = x;
    for (p, label) in chips {
        let text = if p == period {
            format!("[ {label} ]")
        } else {
            label.to_string()
        };
        let style = if p == period {
            title_style(&PRIMARY)
        } else {
            body_style(&DIM)
        };
        root.draw_text(&text, &style, (cx, y))?;
        cx += text.chars().count() as i32 * CHAR_W + 24;
    }

    // Right-aligned filter chips
    let tool_label = match tool {
        Tool::All => "All",
        Tool::ClaudeCode => "Claude Code",
        Tool::Cursor => "Cursor",
        Tool::Codex => "Codex",
        Tool::Copilot => "Copilot",
        Tool::Gemini => "Gemini",
    };
    let project_label = project_filter.label();
    let right_text = format!("[t] {tool_label}    [p] {project_label}");
    let right_w = right_text.chars().count() as i32 * CHAR_W;
    root.draw_text(&right_text, &body_style(&MUTED), (x + w - right_w, y))?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn draw_summary_panel<DB>(
    root: &DrawingArea<DB, plotters::coord::Shift>,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    summary: &Summary,
    period: Period,
    tool: Tool,
) -> ChartResult
where
    DB: DrawingBackend,
    DB::ErrorType: 'static,
{
    root.draw(&Rectangle::new(
        [(x, y), (x + w - 1, y + h - 1)],
        PRIMARY.stroke_width(1),
    ))?;

    let pad_x = x + 16;
    let mut row_y = y + 12;

    // Title line
    let title = format!(
        "tokenuse  ·  {}  ·  {}",
        period_label(period),
        tool_label(tool)
    );
    root.draw_text(&title, &title_style(&PRIMARY), (pad_x, row_y))?;
    row_y += 28;

    // Big numbers row
    let mut cx = pad_x;
    draw_metric(root, &mut cx, row_y, summary.cost, "cost", &YELLOW)?;
    draw_metric(root, &mut cx, row_y, summary.calls, "calls", &TEXT)?;
    draw_metric(root, &mut cx, row_y, summary.sessions, "sessions", &TEXT)?;
    draw_metric(root, &mut cx, row_y, summary.cache_hit, "cache hit", &TEXT)?;
    row_y += 32;

    // Tokens row
    let line = format!(
        "{} in   {} out   {} cached   {} written",
        summary.input, summary.output, summary.cached, summary.written,
    );
    root.draw_text(&line, &body_style(&MUTED), (pad_x, row_y))?;
    Ok(())
}

fn draw_metric<DB>(
    root: &DrawingArea<DB, plotters::coord::Shift>,
    cx: &mut i32,
    y: i32,
    value: &str,
    label: &str,
    color: &RGBColor,
) -> ChartResult
where
    DB: DrawingBackend,
    DB::ErrorType: 'static,
{
    root.draw_text(value, &num_style(color), (*cx, y - 4))?;
    let value_w = value.chars().count() as i32 * (CHAR_W + 5);
    let label_x = *cx + value_w + 6;
    root.draw_text(label, &body_style(&MUTED), (label_x, y + 4))?;
    *cx = label_x + label.chars().count() as i32 * CHAR_W + 28;
    Ok(())
}

fn draw_heatbar<DB>(
    root: &DrawingArea<DB, plotters::coord::Shift>,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    value: u64,
) -> ChartResult
where
    DB: DrawingBackend,
    DB::ErrorType: 'static,
{
    let cells = 12i32;
    let cell_w = (width / cells).max(4);
    let actual_w = cell_w * cells;
    let filled = ((value.min(100) as f64 / 100.0) * cells as f64).ceil() as i32;
    let palette = [BLUE, BLUE_SOFT, YELLOW_SOFT, YELLOW, ORANGE, RED];
    for i in 0..cells {
        let cx = x + i * cell_w;
        let color = if i < filled {
            palette[(i as usize * palette.len() / cells as usize).min(palette.len() - 1)]
        } else {
            BAR_EMPTY
        };
        root.draw(&Rectangle::new(
            [(cx, y), (cx + cell_w - 1, y + height - 1)],
            color.filled(),
        ))?;
    }
    let _ = actual_w;
    Ok(())
}

fn truncate(s: &str, max: usize) -> String {
    let count = s.chars().count();
    if count <= max {
        s.to_string()
    } else if max == 0 {
        String::new()
    } else {
        let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

fn draw_text_left<DB>(
    root: &DrawingArea<DB, plotters::coord::Shift>,
    x: i32,
    y: i32,
    text: &str,
    style: &TextStyle<'static>,
) -> ChartResult
where
    DB: DrawingBackend,
    DB::ErrorType: 'static,
{
    root.draw_text(text, style, (x, y))?;
    Ok(())
}

fn draw_text_right<DB>(
    root: &DrawingArea<DB, plotters::coord::Shift>,
    right_x: i32,
    y: i32,
    text: &str,
    style: &TextStyle<'static>,
) -> ChartResult
where
    DB: DrawingBackend,
    DB::ErrorType: 'static,
{
    let w = text.chars().count() as i32 * CHAR_W;
    root.draw_text(text, style, (right_x - w, y))?;
    Ok(())
}

fn empty_note<DB>(root: &DrawingArea<DB, plotters::coord::Shift>, x: i32, y: i32) -> ChartResult
where
    DB: DrawingBackend,
    DB::ErrorType: 'static,
{
    root.draw_text("no data", &body_style(&DIM), (x, y))?;
    Ok(())
}

fn draw_daily<DB>(
    root: &DrawingArea<DB, plotters::coord::Shift>,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    rows: &[DailyMetric],
) -> ChartResult
where
    DB: DrawingBackend,
    DB::ErrorType: 'static,
{
    let bar_w = 130;
    let date_w = 64;
    let calls_w = 80;

    let head_y = y;
    draw_text_left(root, x, head_y, "date", &head_style())?;
    draw_text_right(root, x + w - calls_w, head_y, "cost", &head_style())?;
    draw_text_right(root, x + w, head_y, "calls", &head_style())?;

    if rows.is_empty() {
        return empty_note(root, x, y + PANEL_HEADER_GAP);
    }

    for (i, row) in rows.iter().take(DAILY_CAP).enumerate() {
        let ry = y + PANEL_HEADER_GAP + i as i32 * ROW_HEIGHT;
        draw_text_left(root, x, ry, row.day, &body_style(&MUTED))?;
        draw_heatbar(root, x + date_w, ry + 2, bar_w, ROW_HEIGHT - 6, row.value)?;
        draw_text_right(root, x + w - calls_w, ry, row.cost, &body_style(&YELLOW))?;
        draw_text_right(root, x + w, ry, &row.calls.to_string(), &body_style(&TEXT))?;
    }
    let _ = h;
    Ok(())
}

fn draw_projects<DB>(
    root: &DrawingArea<DB, plotters::coord::Shift>,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    rows: &[ProjectMetric],
) -> ChartResult
where
    DB: DrawingBackend,
    DB::ErrorType: 'static,
{
    let bar_w = 110;
    let cost_w = 100;
    let avg_w = 80;
    let sess_w = 50;
    let tools_w = 220;

    let head_y = y;
    let name_x = x + bar_w + 8;
    draw_text_left(root, name_x, head_y, "project", &head_style())?;
    let cost_x = x + w - tools_w - sess_w - avg_w - cost_w;
    draw_text_right(root, cost_x + cost_w, head_y, "cost", &head_style())?;
    draw_text_right(
        root,
        cost_x + cost_w + avg_w,
        head_y,
        "avg/s",
        &head_style(),
    )?;
    draw_text_right(
        root,
        cost_x + cost_w + avg_w + sess_w,
        head_y,
        "sess",
        &head_style(),
    )?;
    draw_text_left(root, x + w - tools_w + 6, head_y, "tools", &head_style())?;

    if rows.is_empty() {
        return empty_note(root, x, y + PANEL_HEADER_GAP);
    }

    for (i, row) in rows.iter().take(PROJECTS_CAP).enumerate() {
        let ry = y + PANEL_HEADER_GAP + i as i32 * ROW_HEIGHT;
        draw_heatbar(root, x, ry + 2, bar_w, ROW_HEIGHT - 6, row.value)?;
        let name = truncate(row.name, 30);
        draw_text_left(root, name_x, ry, &name, &body_style(&MUTED))?;
        draw_text_right(root, cost_x + cost_w, ry, row.cost, &body_style(&YELLOW))?;
        draw_text_right(
            root,
            cost_x + cost_w + avg_w,
            ry,
            row.avg_per_session,
            &body_style(&YELLOW),
        )?;
        draw_text_right(
            root,
            cost_x + cost_w + avg_w + sess_w,
            ry,
            &row.sessions.to_string(),
            &body_style(&TEXT),
        )?;
        let mix = truncate(row.tool_mix, 28);
        draw_text_left(root, x + w - tools_w + 6, ry, &mix, &body_style(&BLUE_SOFT))?;
    }
    let _ = h;
    Ok(())
}

fn draw_sessions<DB>(
    root: &DrawingArea<DB, plotters::coord::Shift>,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    rows: &[SessionMetric],
) -> ChartResult
where
    DB: DrawingBackend,
    DB::ErrorType: 'static,
{
    let bar_w = 110;
    let date_w = 110;
    let calls_w = 80;
    let cost_w = 110;

    let head_y = y;
    draw_text_left(root, x + bar_w + 8, head_y, "date", &head_style())?;
    draw_text_left(
        root,
        x + bar_w + 8 + date_w,
        head_y,
        "project",
        &head_style(),
    )?;
    draw_text_right(root, x + w - calls_w, head_y, "cost", &head_style())?;
    draw_text_right(root, x + w, head_y, "calls", &head_style())?;

    if rows.is_empty() {
        return empty_note(root, x, y + PANEL_HEADER_GAP);
    }

    for (i, row) in rows.iter().take(SESSIONS_CAP).enumerate() {
        let ry = y + PANEL_HEADER_GAP + i as i32 * ROW_HEIGHT;
        draw_heatbar(root, x, ry + 2, bar_w, ROW_HEIGHT - 6, row.value)?;
        draw_text_left(root, x + bar_w + 8, ry, row.date, &body_style(&MUTED))?;
        draw_text_left(
            root,
            x + bar_w + 8 + date_w,
            ry,
            &truncate(row.project, 56),
            &body_style(&MUTED),
        )?;
        draw_text_right(root, x + w - calls_w, ry, row.cost, &body_style(&YELLOW))?;
        draw_text_right(root, x + w, ry, &row.calls.to_string(), &body_style(&TEXT))?;
    }
    let _ = (h, cost_w);
    Ok(())
}

fn draw_project_tools<DB>(
    root: &DrawingArea<DB, plotters::coord::Shift>,
    root_x: i32,
    y: i32,
    w: i32,
    h: i32,
    rows: &[ProjectToolMetric],
) -> ChartResult
where
    DB: DrawingBackend,
    DB::ErrorType: 'static,
{
    let x = root_x;
    let bar_w = 100;
    let tool_w = 70;
    let cost_w = 80;
    let calls_w = 60;
    let sess_w = 50;
    let avg_w = 80;

    let head_y = y;
    let name_x = x + bar_w + 8;
    draw_text_left(root, name_x, head_y, "project", &head_style())?;
    let tool_x = x + w - avg_w - sess_w - calls_w - cost_w - tool_w;
    draw_text_left(root, tool_x, head_y, "tool", &head_style())?;
    draw_text_right(
        root,
        tool_x + tool_w + cost_w,
        head_y,
        "cost",
        &head_style(),
    )?;
    draw_text_right(
        root,
        tool_x + tool_w + cost_w + calls_w,
        head_y,
        "calls",
        &head_style(),
    )?;
    draw_text_right(
        root,
        tool_x + tool_w + cost_w + calls_w + sess_w,
        head_y,
        "sess",
        &head_style(),
    )?;
    draw_text_right(root, x + w, head_y, "avg/s", &head_style())?;

    if rows.is_empty() {
        return empty_note(root, x, y + PANEL_HEADER_GAP);
    }

    for (i, row) in rows.iter().take(PROJECT_TOOLS_CAP).enumerate() {
        let ry = y + PANEL_HEADER_GAP + i as i32 * ROW_HEIGHT;
        draw_heatbar(root, x, ry + 2, bar_w, ROW_HEIGHT - 6, row.value)?;
        draw_text_left(
            root,
            name_x,
            ry,
            &truncate(row.project, 18),
            &body_style(&MUTED),
        )?;
        draw_text_left(root, tool_x, ry, row.tool, &body_style(&YELLOW_SOFT))?;
        draw_text_right(
            root,
            tool_x + tool_w + cost_w,
            ry,
            row.cost,
            &body_style(&YELLOW),
        )?;
        draw_text_right(
            root,
            tool_x + tool_w + cost_w + calls_w,
            ry,
            &row.calls.to_string(),
            &body_style(&TEXT),
        )?;
        draw_text_right(
            root,
            tool_x + tool_w + cost_w + calls_w + sess_w,
            ry,
            &row.sessions.to_string(),
            &body_style(&TEXT),
        )?;
        draw_text_right(root, x + w, ry, row.avg_per_session, &body_style(&YELLOW))?;
    }
    let _ = h;
    Ok(())
}

fn draw_models<DB>(
    root: &DrawingArea<DB, plotters::coord::Shift>,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    rows: &[ModelMetric],
) -> ChartResult
where
    DB: DrawingBackend,
    DB::ErrorType: 'static,
{
    let bar_w = 100;
    let cost_w = 120;
    let cache_w = 90;
    let calls_w = 70;

    let name_x = x + bar_w + 8;
    let head_y = y;
    draw_text_left(root, name_x, head_y, "model", &head_style())?;
    let cost_x = x + w - calls_w - cache_w - cost_w;
    draw_text_right(root, cost_x + cost_w, head_y, "cost", &head_style())?;
    draw_text_right(
        root,
        cost_x + cost_w + cache_w,
        head_y,
        "cache",
        &head_style(),
    )?;
    draw_text_right(root, x + w, head_y, "calls", &head_style())?;

    if rows.is_empty() {
        return empty_note(root, x, y + PANEL_HEADER_GAP);
    }

    for (i, row) in rows.iter().take(MODELS_CAP).enumerate() {
        let ry = y + PANEL_HEADER_GAP + i as i32 * ROW_HEIGHT;
        draw_heatbar(root, x, ry + 2, bar_w, ROW_HEIGHT - 6, row.value)?;
        draw_text_left(
            root,
            name_x,
            ry,
            &truncate(row.name, 24),
            &body_style(&TEXT),
        )?;
        draw_text_right(root, cost_x + cost_w, ry, row.cost, &body_style(&YELLOW))?;
        draw_text_right(
            root,
            cost_x + cost_w + cache_w,
            ry,
            row.cache,
            &body_style(&TEXT),
        )?;
        draw_text_right(root, x + w, ry, &row.calls.to_string(), &body_style(&TEXT))?;
    }
    let _ = h;
    Ok(())
}

fn draw_counts<DB>(
    root: &DrawingArea<DB, plotters::coord::Shift>,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    rows: &[CountMetric],
) -> ChartResult
where
    DB: DrawingBackend,
    DB::ErrorType: 'static,
{
    let bar_w = 100;
    let name_x = x + bar_w + 8;

    draw_text_left(root, name_x, y, "name", &head_style())?;
    draw_text_right(root, x + w, y, "calls", &head_style())?;

    if rows.is_empty() {
        return empty_note(root, x, y + PANEL_HEADER_GAP);
    }

    let fit_rows = ((h - PANEL_HEADER_GAP) / ROW_HEIGHT).max(1) as usize;
    let max_rows = COUNTS_CAP.min(fit_rows);
    for (i, row) in rows.iter().take(max_rows).enumerate() {
        let ry = y + PANEL_HEADER_GAP + i as i32 * ROW_HEIGHT;
        draw_heatbar(root, x, ry + 2, bar_w, ROW_HEIGHT - 6, row.value)?;
        draw_text_left(
            root,
            name_x,
            ry,
            &truncate(row.name, 32),
            &body_style(&TEXT),
        )?;
        draw_text_right(root, x + w, ry, &row.calls.to_string(), &body_style(&TEXT))?;
    }
    Ok(())
}

fn period_label(period: Period) -> &'static str {
    match period {
        Period::Today => "24 Hours",
        Period::Week => "7 Days",
        Period::ThirtyDays => "30 Days",
        Period::Month => "This Month",
        Period::AllTime => "All Time",
    }
}

fn tool_label(tool: Tool) -> &'static str {
    match tool {
        Tool::All => "All tools",
        Tool::ClaudeCode => "Claude Code",
        Tool::Cursor => "Cursor",
        Tool::Codex => "Codex",
        Tool::Copilot => "Copilot",
        Tool::Gemini => "Gemini",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ConfigPaths;
    use crate::currency::CurrencyFormatter;
    use crate::data::{dashboard_data, SessionDetail, SessionDetailView};
    use std::sync::Mutex;
    use std::time::{SystemTime, UNIX_EPOCH};

    /// plotters' font lookup on macOS is not safe across threads, so chart
    /// tests must serialize their access. JSON/CSV tests do not need this.
    static CHART_LOCK: Mutex<()> = Mutex::new(());

    fn tempdir(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "tokenuse-export-{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            name
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    static ALL_PROJECTS: ProjectFilter = ProjectFilter::All;

    fn fixture() -> (ConfigPaths, DashboardData) {
        let dir = tempdir("paths");
        let paths = ConfigPaths::new(dir);
        let data = dashboard_data(
            Period::AllTime,
            Tool::All,
            &ProjectFilter::All,
            crate::app::SortMode::Spend,
            &CurrencyFormatter::usd(),
        );
        (paths, data)
    }

    #[test]
    fn gemini_filter_slug_and_label_are_stable() {
        assert_eq!(
            filter_slug(Period::Week, Tool::Gemini, &ProjectFilter::All),
            "week-gemini-allprojects"
        );
        assert_eq!(tool_label(Tool::Gemini), "Gemini");
    }

    fn context<'a>(
        data: &'a DashboardData,
        session: Option<&'a SessionDetailView>,
    ) -> ExportContext<'a> {
        ExportContext {
            dashboard: data,
            session,
            period: Period::AllTime,
            tool: Tool::All,
            project_filter: &ALL_PROJECTS,
            sort: crate::app::SortMode::Spend,
            currency_code: "USD",
            source_label: "sample",
        }
    }

    fn selected_session() -> SessionDetailView {
        SessionDetailView {
            key: "session-key".into(),
            session_id: "session-id".into(),
            project: "Project <Danger>".into(),
            tool: "Codex",
            date_range: "2026-05-01 10:00 - 10:20".into(),
            total_cost: "$1.23".into(),
            total_calls: 1,
            total_input: "1,000".into(),
            total_output: "500".into(),
            total_cache_read: "250".into(),
            calls: vec![SessionDetail {
                timestamp: "2026-05-01 10:00".into(),
                model: "model <x>".into(),
                cost: "$1.23".into(),
                input_tokens: 1000,
                output_tokens: 500,
                cache_read: 200,
                cache_write: 50,
                reasoning_tokens: 25,
                web_search_requests: 1,
                tools: "shell & read".into(),
                bash_commands: vec![
                    "echo \"<hi>\" & exit".into(),
                    "printf 'a deliberately long command with flags and quoted values' -- --format json --project tokenuse".into(),
                ],
                prompt: "prompt preview".into(),
                prompt_full: format!(
                    "full <prompt> & \"quote\"\n{}",
                    "long prompt segment with wrapping pressure ".repeat(16)
                ),
            }],
            note: Some("note <with> & detail".into()),
        }
    }

    #[test]
    fn json_export_writes_pretty_file_with_summary() {
        let (paths, data) = fixture();
        let context = context(&data, None);
        let export_root = paths.dir.join("exports");
        let path = write_to_dir(&export_root, ExportFormat::Json, &context).unwrap();
        assert!(path.exists());
        let body = fs::read_to_string(&path).unwrap();
        assert!(body.contains("\"summary\""));
        assert!(body.contains("\"daily\""));
        let _ = fs::remove_dir_all(&paths.dir);
    }

    #[test]
    fn csv_export_writes_one_file_per_panel() {
        let (paths, data) = fixture();
        let context = context(&data, None);
        let export_root = paths.dir.join("exports");
        let dir = write_to_dir(&export_root, ExportFormat::Csv, &context).unwrap();
        for name in [
            "summary.csv",
            "daily.csv",
            "projects.csv",
            "project_tools.csv",
            "sessions.csv",
            "models.csv",
            "tools.csv",
            "commands.csv",
            "mcp_servers.csv",
        ] {
            assert!(dir.join(name).exists(), "missing {name}");
        }
        let _ = fs::remove_dir_all(&paths.dir);
    }

    #[test]
    fn svg_export_writes_xml_chart() {
        let _lock = CHART_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let (paths, data) = fixture();
        let context = context(&data, None);
        let export_root = paths.dir.join("exports");
        let path = write_to_dir(&export_root, ExportFormat::Svg, &context).unwrap();
        let body = fs::read_to_string(&path).unwrap();
        assert!(body.contains("<svg"));
        let _ = fs::remove_dir_all(&paths.dir);
    }

    #[test]
    fn png_export_writes_png_signature() {
        let _lock = CHART_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let (paths, data) = fixture();
        let context = context(&data, None);
        let export_root = paths.dir.join("exports");
        let path = write_to_dir(&export_root, ExportFormat::Png, &context).unwrap();
        let bytes = fs::read(&path).unwrap();
        assert!(bytes.len() > 8);
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
        let _ = fs::remove_dir_all(&paths.dir);
    }

    #[test]
    fn html_export_writes_self_contained_workbook() {
        let (paths, data) = fixture();
        let context = context(&data, None);
        let export_root = paths.dir.join("exports");
        let path = write_to_dir(&export_root, ExportFormat::Html, &context).unwrap();

        assert!(path.extension().is_some_and(|ext| ext == "html"));
        let body = fs::read_to_string(&path).unwrap();
        assert!(body.contains("<style>"));
        assert!(body.contains("brand-mark"));
        assert!(body.contains("Daily Activity"));
        assert!(body.contains("calendar-grid"));
        assert!(body.contains("calendar-cell"));
        assert!(!body.contains("Usage Limits"));
        assert!(body.contains("Full workbook report"));
        assert!(!body.contains("<script"));
        let _ = fs::remove_dir_all(&paths.dir);
    }

    #[test]
    fn html_export_includes_selected_session_full_detail_and_escapes_text() {
        let (paths, mut data) = fixture();
        data.projects.insert(
            0,
            ProjectMetric {
                name: "<project & \"quoted\">",
                cost: "$0.10",
                avg_per_session: "$0.10",
                sessions: 1,
                tool_mix: "Codex & Claude",
                value: 75,
            },
        );
        data.commands.insert(
            0,
            CountMetric {
                name: "cmd <unsafe> & \"quoted\"",
                calls: 2,
                value: 50,
            },
        );
        let session = selected_session();
        let context = context(&data, Some(&session));
        let export_root = paths.dir.join("exports");
        let path = write_to_dir(&export_root, ExportFormat::Html, &context).unwrap();
        let body = fs::read_to_string(&path).unwrap();

        assert!(body.contains("Selected Session"));
        assert!(body.contains("&lt;project &amp; &quot;quoted&quot;&gt;"));
        assert!(body.contains("cmd &lt;unsafe&gt; &amp; &quot;quoted&quot;"));
        assert!(body.contains("Project &lt;Danger&gt;"));
        assert!(body.contains("note &lt;with&gt; &amp; detail"));
        assert!(body.contains("echo &quot;&lt;hi&gt;&quot; &amp; exit"));
        assert!(body.contains("full &lt;prompt&gt; &amp; &quot;quote&quot;"));
        assert!(!body.contains("full <prompt>"));
        let _ = fs::remove_dir_all(&paths.dir);
    }

    #[test]
    fn pdf_export_writes_branded_workbook_file() {
        let (paths, data) = fixture();
        let session = selected_session();
        let context = context(&data, Some(&session));
        let export_root = paths.dir.join("exports");
        let path = write_to_dir(&export_root, ExportFormat::Pdf, &context).unwrap();

        assert!(path.extension().is_some_and(|ext| ext == "pdf"));
        let bytes = fs::read(&path).unwrap();
        assert!(bytes.len() > 1_000);
        assert_eq!(&bytes[..5], b"%PDF-");
        let _ = fs::remove_dir_all(&paths.dir);
    }

    #[test]
    fn export_formats_include_full_workbook_formats() {
        assert!(ExportFormat::ALL.contains(&ExportFormat::Html));
        assert!(ExportFormat::ALL.contains(&ExportFormat::Pdf));
        assert_eq!(ExportFormat::Html.label(), "HTML (full workbook report)");
        assert_eq!(ExportFormat::Pdf.label(), "PDF (full workbook report)");
    }

    #[test]
    fn csv_escape_handles_commas_and_quotes() {
        assert_eq!(csv_escape("simple"), "simple");
        assert_eq!(csv_escape("a,b"), "\"a,b\"");
        assert_eq!(csv_escape("she said \"hi\""), "\"she said \"\"hi\"\"\"");
    }

    #[test]
    fn default_export_dir_prefers_platform_downloads() {
        let paths = ConfigPaths::new(PathBuf::from("/tmp/tokenuse-config"));
        let downloads = PathBuf::from("/tmp/downloads");
        let home = PathBuf::from("/tmp/home");

        assert_eq!(
            default_export_dir_from(&paths, Some(downloads.clone()), Some(home)),
            downloads
        );
    }

    #[test]
    fn default_export_dir_uses_home_downloads_before_config_fallback() {
        let paths = ConfigPaths::new(PathBuf::from("/tmp/tokenuse-config"));
        let home = PathBuf::from("/tmp/home");

        assert_eq!(
            default_export_dir_from(&paths, None, Some(home.clone())),
            home.join("Downloads")
        );
    }

    #[test]
    fn default_export_dir_falls_back_to_config_exports() {
        let paths = ConfigPaths::new(PathBuf::from("/tmp/tokenuse-config"));

        assert_eq!(
            default_export_dir_from(&paths, None, None),
            PathBuf::from("/tmp/tokenuse-config/exports")
        );
    }
}
