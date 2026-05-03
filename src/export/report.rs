use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use chrono::{Datelike, Duration, Local, NaiveDate};
use color_eyre::{eyre::WrapErr, Result};
use fulgur::{Engine, Margin, PageSize};

use crate::copy::{copy, template};
use crate::data::{
    ActivityMetric, CountMetric, DailyMetric, DashboardData, ModelMetric, ProjectMetric,
    ProjectToolMetric, SessionDetailView, SessionMetric, Summary,
};

use super::labels::{period_label, tool_label};
use super::ExportContext;

pub(super) fn write_html_report(
    path: &Path,
    context: &ExportContext<'_>,
    stamp: &str,
) -> Result<()> {
    let out = build_html_report(context, stamp);
    fs::write(path, out).wrap_err_with(|| format!("write {}", path.display()))
}

pub(super) fn build_html_report(context: &ExportContext<'_>, stamp: &str) -> String {
    let generated_at = Local::now().format("%Y-%m-%d %H:%M:%S %Z").to_string();
    let title = report_title(context);
    let mut out = String::with_capacity(96 * 1024);

    push_report_document_open(&mut out, &title, HTML_REPORT_CSS, "report");

    push_report_header(&mut out, context, &generated_at, stamp);
    push_summary_cards(&mut out, &context.dashboard.summary, context.currency_code);
    push_activity_timeline_html(&mut out, &context.dashboard.activity_timeline);
    push_dashboard_workbook(&mut out, context.dashboard);
    if let Some(session) = context.session {
        push_session_workbook(&mut out, session);
    }

    out.push_str("</main>\n</body>\n</html>\n");
    out
}

pub(super) fn build_pdf_html_report(context: &ExportContext<'_>, stamp: &str) -> String {
    let generated_at = Local::now().format("%Y-%m-%d %H:%M:%S %Z").to_string();
    let title = report_title(context);
    let mut out = String::with_capacity(96 * 1024);
    let css = format!("{HTML_REPORT_CSS}\n{PDF_REPORT_CSS}");

    push_report_document_open(&mut out, &title, &css, "report pdf-report");

    push_report_header(&mut out, context, &generated_at, stamp);
    push_summary_cards_pdf(&mut out, &context.dashboard.summary, context.currency_code);
    push_activity_timeline_pdf(&mut out, &context.dashboard.activity_timeline);
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
    template(
        &copy().export.report_title,
        &[
            ("period", period_label(context.period).to_string()),
            ("tool", tool_label(context.tool).to_string()),
        ],
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

.activity-export {
  margin: 14px 0;
}

.activity-body {
  padding: 12px;
}

.activity-svg {
  display: block;
  width: 100%;
  height: auto;
}

.export-rank-bar {
  display: block;
  width: 86px;
  height: 16px;
}

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

.pdf-report .activity-svg {
  width: 100%;
  height: auto;
}

.pdf-report .export-rank-bar {
  width: 72px;
  height: 12px;
}

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
    out.push_str("<div><p class=\"eyebrow\">");
    out.push_str(&escape_html(&copy().export.full_workbook_report));
    out.push_str("</p><h1>");
    out.push_str(&escape_html(&copy().brand.name));
    out.push_str("</h1></div>\n</div>\n");
    out.push_str("<div class=\"meta-grid\">\n");
    push_meta(out, &copy().export.generated, generated_at);
    push_meta(out, &copy().export.export_id, stamp);
    push_meta(out, &copy().export.source, context.source_label);
    push_meta(out, &copy().export.currency, context.currency_code);
    push_meta(out, &copy().export.period, period_label(context.period));
    push_meta(out, &copy().export.tool, tool_label(context.tool));
    push_meta(out, &copy().export.project, context.project_filter.label());
    push_meta(out, &copy().export.sort, context.sort.label());
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
    out.push_str("<section class=\"kpis\" aria-label=\"");
    out.push_str(&escape_html(&copy().export.summary_metrics_aria));
    out.push_str("\">\n");
    push_kpi(
        out,
        &copy().metrics.cost,
        summary.cost,
        currency_code,
        "cost",
    );
    push_kpi(
        out,
        &copy().metrics.calls,
        summary.calls,
        summary.input,
        "calls",
    );
    push_kpi(
        out,
        &copy().metrics.sessions,
        summary.sessions,
        &copy().metrics.active_set,
        "sessions",
    );
    push_kpi(
        out,
        &copy().metrics.cache_hit,
        summary.cache_hit,
        summary.cached,
        "cache",
    );
    let tokens = format!("{} {}", summary.output, copy().metrics.out);
    push_kpi(out, &copy().metrics.input, summary.input, &tokens, "tokens");
    let written = format!("{} {}", summary.written, copy().metrics.written);
    push_kpi(
        out,
        &copy().metrics.cached,
        summary.cached,
        &written,
        "cache",
    );
    out.push_str("</section>\n");
}

fn push_summary_cards_pdf(out: &mut String, summary: &Summary, currency_code: &str) {
    let tokens = format!("{} {}", summary.output, copy().metrics.out);
    let written = format!("{} {}", summary.written, copy().metrics.written);
    let cards = [
        (
            copy().metrics.cost.as_str(),
            summary.cost,
            currency_code,
            "cost",
        ),
        (
            copy().metrics.calls.as_str(),
            summary.calls,
            summary.input,
            "calls",
        ),
        (
            copy().metrics.sessions.as_str(),
            summary.sessions,
            copy().metrics.active_set.as_str(),
            "sessions",
        ),
        (
            copy().metrics.cache_hit.as_str(),
            summary.cache_hit,
            summary.cached,
            "cache",
        ),
        (
            copy().metrics.input.as_str(),
            summary.input,
            tokens.as_str(),
            "tokens",
        ),
        (
            copy().metrics.cached.as_str(),
            summary.cached,
            written.as_str(),
            "cache",
        ),
    ];

    out.push_str("<table class=\"pdf-kpis\" aria-label=\"");
    out.push_str(&escape_html(&copy().export.summary_metrics_aria));
    out.push_str("\">");
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

fn push_activity_timeline_html(out: &mut String, rows: &[ActivityMetric]) {
    push_section_open(
        out,
        &copy().panels.activity_timeline,
        "tone-cyan activity-export",
        "pulse",
    );
    out.push_str("<div class=\"activity-body\">");
    out.push_str(&activity_timeline_svg(rows, ActivitySvgSize::Html));
    out.push_str("</div>");
    push_section_close(out);
}

fn push_activity_timeline_pdf(out: &mut String, rows: &[ActivityMetric]) {
    push_pdf_panel_open(out, &copy().panels.activity_timeline, "tone-cyan", "pulse");
    out.push_str(&activity_timeline_svg(rows, ActivitySvgSize::Pdf));
    push_pdf_panel_close(out);
}

#[derive(Clone, Copy)]
enum ActivitySvgSize {
    Html,
    Pdf,
}

impl ActivitySvgSize {
    fn dimensions(self) -> (f64, f64) {
        match self {
            Self::Html => (1100.0, 240.0),
            Self::Pdf => (520.0, 158.0),
        }
    }

    fn font_size(self) -> f64 {
        match self {
            Self::Html => 13.0,
            Self::Pdf => 9.0,
        }
    }

    fn stroke_width(self) -> f64 {
        match self {
            Self::Html => 2.4,
            Self::Pdf => 1.6,
        }
    }
}

fn activity_timeline_svg(rows: &[ActivityMetric], size: ActivitySvgSize) -> String {
    let (width, height) = size.dimensions();
    let font_size = size.font_size();
    let label_x = 4.0;
    let chart_left = match size {
        ActivitySvgSize::Html => 56.0,
        ActivitySvgSize::Pdf => 38.0,
    };
    let chart_right = width
        - match size {
            ActivitySvgSize::Html => 24.0,
            ActivitySvgSize::Pdf => 12.0,
        };
    let spend_top = match size {
        ActivitySvgSize::Html => 24.0,
        ActivitySvgSize::Pdf => 18.0,
    };
    let spend_bottom = match size {
        ActivitySvgSize::Html => 96.0,
        ActivitySvgSize::Pdf => 62.0,
    };
    let calls_top = match size {
        ActivitySvgSize::Html => 126.0,
        ActivitySvgSize::Pdf => 82.0,
    };
    let calls_bottom = height
        - match size {
            ActivitySvgSize::Html => 44.0,
            ActivitySvgSize::Pdf => 28.0,
        };
    let meta_y = height
        - match size {
            ActivitySvgSize::Html => 16.0,
            ActivitySvgSize::Pdf => 10.0,
        };

    let mut out = String::with_capacity(rows.len().saturating_mul(96).saturating_add(2048));
    let _ = write!(
        out,
        r##"<svg class="activity-svg" data-export-chart="activity-timeline" xmlns="http://www.w3.org/2000/svg" width="{}" height="{}" viewBox="0 0 {} {}" role="img" aria-label="{}">"##,
        format_svg_number(width),
        format_svg_number(height),
        format_svg_number(width),
        format_svg_number(height),
        escape_html(&copy().timeline.activity_export_aria)
    );
    out.push_str("<title>");
    out.push_str(&escape_html(&copy().panels.activity_timeline));
    out.push_str("</title>");
    let _ = write!(
        out,
        r##"<rect x="0.5" y="0.5" width="{}" height="{}" fill="#fbfcff" stroke="#e4e8f2"/>"##,
        format_svg_number(width - 1.0),
        format_svg_number(height - 1.0)
    );

    if rows.is_empty() {
        let _ = write!(
            out,
            r##"<text x="{}" y="{}" fill="#5f667a" font-family="JetBrains Mono, SFMono-Regular, Menlo, Consolas, monospace" font-size="{}" font-weight="700">{}</text></svg>"##,
            format_svg_number(chart_left),
            format_svg_number(height / 2.0),
            format_svg_number(font_size),
            escape_html(&copy().timeline.no_activity)
        );
        return out;
    }

    let max_calls = rows.iter().map(|row| row.calls).max().unwrap_or(0).max(1);
    let total_calls = rows.iter().map(|row| row.calls).sum::<u64>();
    let high_idx = rows
        .iter()
        .enumerate()
        .max_by_key(|(_, row)| row.value)
        .map(|(idx, _)| idx)
        .unwrap_or(0);
    let high = &rows[high_idx];
    let latest = rows.last().expect("rows is not empty");
    let first = rows.first().expect("rows is not empty");
    let dense = rows.len() > 192;
    let bucket_span = (chart_right - chart_left) / rows.len().max(1) as f64;
    let bar_width = (bucket_span * if dense { 0.82 } else { 0.62 }).clamp(
        if dense { 1.0 } else { 2.0 },
        if dense { 6.0 } else { 16.0 },
    );

    let _ = write!(
        out,
        r##"<rect x="{}" y="{}" width="{}" height="{}" fill="#f0f7fb"/>"##,
        format_svg_number(chart_left),
        format_svg_number(calls_top - 2.0),
        format_svg_number(chart_right - chart_left),
        format_svg_number(calls_bottom - calls_top + 2.0)
    );

    for y in [spend_top, spend_bottom, calls_bottom] {
        let _ = write!(
            out,
            r##"<line x1="{}" x2="{}" y1="{}" y2="{}" stroke="#cfd5e6" stroke-width="1"/>"##,
            format_svg_number(chart_left),
            format_svg_number(chart_right),
            format_svg_number(y),
            format_svg_number(y)
        );
    }
    let _ = write!(
        out,
        r##"<line x1="{}" x2="{}" y1="{}" y2="{}" stroke="#cfd5e6" stroke-width="1" stroke-dasharray="3 6"/>"##,
        format_svg_number(chart_left),
        format_svg_number(chart_right),
        format_svg_number((calls_top + calls_bottom) / 2.0),
        format_svg_number((calls_top + calls_bottom) / 2.0)
    );
    let _ = write!(
        out,
        r##"<text x="{}" y="{}" fill="#ff8f40" font-family="JetBrains Mono, SFMono-Regular, Menlo, Consolas, monospace" font-size="{}" font-weight="800">{}</text>"##,
        format_svg_number(label_x),
        format_svg_number(spend_bottom - 6.0),
        format_svg_number(font_size),
        escape_html(&copy().timeline.spend)
    );
    let _ = write!(
        out,
        r##"<text x="{}" y="{}" fill="#2d72d9" font-family="JetBrains Mono, SFMono-Regular, Menlo, Consolas, monospace" font-size="{}" font-weight="800">{}</text>"##,
        format_svg_number(label_x),
        format_svg_number(calls_bottom - 6.0),
        format_svg_number(font_size),
        escape_html(&copy().timeline.calls)
    );

    let mut line_points = String::new();
    let mut area_points = String::new();
    for (idx, row) in rows.iter().enumerate() {
        let x = timeline_x(idx, rows.len(), chart_left, chart_right);
        let call_y =
            calls_bottom - ((row.calls as f64 / max_calls as f64) * (calls_bottom - calls_top));
        let _ = write!(
            line_points,
            "{}{},{}",
            if line_points.is_empty() { "" } else { " " },
            format_svg_number(x),
            format_svg_number(call_y)
        );
        let _ = write!(
            area_points,
            "{}{},{}",
            if area_points.is_empty() { "" } else { " " },
            format_svg_number(x),
            format_svg_number(call_y)
        );
    }

    let first_x = timeline_x(0, rows.len(), chart_left, chart_right);
    let last_x = timeline_x(rows.len() - 1, rows.len(), chart_left, chart_right);
    let area_polygon = format!(
        "{},{} {} {},{}",
        format_svg_number(first_x),
        format_svg_number(calls_bottom),
        area_points,
        format_svg_number(last_x),
        format_svg_number(calls_bottom)
    );
    let _ = write!(
        out,
        r##"<polygon points="{}" fill="#4df3e8" opacity="0.12"/>"##,
        area_polygon
    );

    for (idx, row) in rows.iter().enumerate() {
        let x = timeline_x(idx, rows.len(), chart_left, chart_right);
        let clamped = row.value.min(100) as f64;
        let y = if row.value == 0 {
            spend_bottom - 1.0
        } else {
            spend_bottom - (clamped / 100.0) * (spend_bottom - spend_top)
        };
        let h = if row.value == 0 {
            1.0
        } else {
            (spend_bottom - y).max(2.0)
        };
        let fill = if idx == high_idx && row.value > 0 {
            "#ff5f6d"
        } else {
            "#ff8f40"
        };
        let opacity = if row.value == 0 { "0.35" } else { "0.78" };
        let title = format!(
            "{} - {} - {} {}",
            row.label,
            row.cost,
            format_u64(row.calls),
            copy().metrics.calls
        );
        let _ = write!(
            out,
            r##"<rect x="{}" y="{}" width="{}" height="{}" fill="{}" opacity="{}"><title>{}</title></rect>"##,
            format_svg_number((x - bar_width / 2.0).clamp(chart_left, chart_right - bar_width)),
            format_svg_number(y),
            format_svg_number(bar_width),
            format_svg_number(h),
            fill,
            opacity,
            escape_html(&title)
        );
    }

    let _ = write!(
        out,
        r##"<polyline points="{}" fill="none" stroke="#4df3e8" stroke-width="{}" stroke-linejoin="round" stroke-linecap="round"/>"##,
        line_points,
        format_svg_number(size.stroke_width())
    );
    let meta = format!(
        "{} {} {} {}   {} {} {}   {} {}   {} {}",
        copy().timeline.range,
        first.label,
        copy().timeline.to,
        latest.label,
        copy().timeline.high,
        high.label,
        high.cost,
        copy().timeline.latest,
        latest.cost,
        copy().timeline.calls,
        format_u64(total_calls)
    );
    let _ = write!(
        out,
        r##"<text x="{}" y="{}" fill="#5f667a" font-family="JetBrains Mono, SFMono-Regular, Menlo, Consolas, monospace" font-size="{}">{}</text>"##,
        format_svg_number(chart_left),
        format_svg_number(meta_y),
        format_svg_number(font_size),
        escape_html(&meta)
    );
    out.push_str("</svg>");
    out
}

fn timeline_x(index: usize, len: usize, left: f64, right: f64) -> f64 {
    if len <= 1 {
        (left + right) / 2.0
    } else {
        left + (index as f64 / (len - 1) as f64) * (right - left)
    }
}

#[derive(Clone, Copy)]
enum RankBarSize {
    Html,
    Pdf,
}

impl RankBarSize {
    fn dimensions(self) -> (f64, f64) {
        match self {
            Self::Html => (86.0, 16.0),
            Self::Pdf => (72.0, 12.0),
        }
    }

    fn empty_fill(self) -> &'static str {
        match self {
            Self::Html => "#edf0f7",
            Self::Pdf => "#f1f4fa",
        }
    }
}

fn rank_bar_svg(value: u64, size: RankBarSize) -> String {
    const CELLS: usize = 12;
    const COLORS: [&str; 6] = [
        "#62a6ff", "#7ebcff", "#f5cf6c", "#ffd60a", "#ff9c48", "#ff5f6d",
    ];

    let (width, height) = size.dimensions();
    let clamped = value.min(100);
    let filled = if clamped == 0 {
        0
    } else {
        ((clamped as f64 / 100.0) * CELLS as f64).ceil() as usize
    };
    let gap = 1.0;
    let cell_w = (width - gap * (CELLS.saturating_sub(1)) as f64) / CELLS as f64;
    let marker_x = (clamped as f64 / 100.0) * width;
    let mut out = String::with_capacity(900);
    let _ = write!(
        out,
        r##"<svg class="export-rank-bar" data-export-rank="true" xmlns="http://www.w3.org/2000/svg" width="{}" height="{}" viewBox="0 0 {} {}" role="img" aria-label="{}">"##,
        format_svg_number(width),
        format_svg_number(height),
        format_svg_number(width),
        format_svg_number(height),
        escape_html(&template(
            &copy().timeline.relative_rank,
            &[("value", clamped.to_string())]
        ))
    );
    let _ = write!(
        out,
        "<title>{}</title>",
        escape_html(&template(
            &copy().timeline.relative_rank,
            &[("value", clamped.to_string())]
        ))
    );
    for idx in 0..CELLS {
        let active = idx < filled;
        let x = idx as f64 * (cell_w + gap);
        let color = if active {
            COLORS[idx * COLORS.len() / CELLS]
        } else {
            size.empty_fill()
        };
        let stroke = if active { color } else { "#cfd5e6" };
        let opacity = if active {
            0.78 + (idx as f64 / (CELLS - 1) as f64) * 0.22
        } else {
            0.72
        };
        let _ = write!(
            out,
            r##"<rect x="{}" y="0.5" width="{}" height="{}" fill="{}" stroke="{}" opacity="{}"/>"##,
            format_svg_number(x),
            format_svg_number(cell_w),
            format_svg_number(height - 1.0),
            color,
            stroke,
            format_svg_number(opacity)
        );
    }
    if clamped > 0 && clamped < 100 {
        let _ = write!(
            out,
            r##"<line x1="{}" x2="{}" y1="0" y2="{}" stroke="#1b2030" stroke-width="1" opacity="0.8"/>"##,
            format_svg_number(marker_x.clamp(1.0, width - 1.0)),
            format_svg_number(marker_x.clamp(1.0, width - 1.0)),
            format_svg_number(height)
        );
    }
    out.push_str("</svg>");
    out
}

fn format_svg_number(value: f64) -> String {
    let rounded = value.round();
    if (value - rounded).abs() < 0.005 {
        format!("{rounded:.0}")
    } else {
        format!("{value:.2}")
    }
}

fn push_dashboard_workbook(out: &mut String, data: &DashboardData) {
    out.push_str("<section class=\"workbook-grid\" aria-label=\"");
    out.push_str(&escape_html(&copy().export.dashboard_workbook_aria));
    out.push_str("\">\n");
    push_daily_html(out, &data.daily);
    push_projects_html(out, &data.projects);
    push_sessions_html(out, &data.sessions);
    push_project_tools_html(out, &data.project_tools);
    push_models_html(out, &data.models);
    push_counts_html(
        out,
        &copy().panels.core_tools,
        "tone-cyan",
        "tools",
        &data.tools,
    );
    push_counts_html(
        out,
        &copy().panels.shell_commands,
        "tone-orange",
        "terminal",
        &data.commands,
    );
    push_counts_html(
        out,
        &copy().panels.mcp_servers,
        "tone-magenta",
        "network",
        &data.mcp_servers,
    );
    out.push_str("</section>\n");
}

fn push_dashboard_workbook_pdf(out: &mut String, data: &DashboardData) {
    out.push_str("<section class=\"pdf-workbook\" aria-label=\"");
    out.push_str(&escape_html(&copy().export.dashboard_workbook_aria));
    out.push_str("\">\n");
    push_daily_pdf(out, &data.daily);
    push_projects_pdf(out, &data.projects);
    push_sessions_pdf(out, &data.sessions);
    push_project_tools_pdf(out, &data.project_tools);
    push_models_pdf(out, &data.models);
    push_counts_pdf(
        out,
        &copy().panels.core_tools,
        "tone-cyan",
        "tools",
        &data.tools,
    );
    push_counts_pdf(
        out,
        &copy().panels.shell_commands,
        "tone-orange",
        "terminal",
        &data.commands,
    );
    push_counts_pdf(
        out,
        &copy().panels.mcp_servers,
        "tone-magenta",
        "network",
        &data.mcp_servers,
    );
    out.push_str("</section>\n");
}

fn push_session_workbook(out: &mut String, session: &SessionDetailView) {
    push_section_open(
        out,
        &copy().panels.selected_session,
        "tone-red wide",
        "session",
    );
    out.push_str("<div class=\"session-kpis\">\n<div class=\"call-facts\">\n");
    push_meta(out, &copy().tables.project, &session.project);
    push_meta(out, &copy().tables.tool, session.tool);
    push_meta(out, &copy().export.date_range, &session.date_range);
    push_meta(out, &copy().tables.cost, &session.total_cost);
    push_meta(out, &copy().tables.calls, &session.total_calls.to_string());
    push_meta(out, &copy().metrics.input, &session.total_input);
    push_meta(out, &copy().metrics.output, &session.total_output);
    push_meta(out, &copy().metrics.cache_read, &session.total_cache_read);
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
            copy().tables.time.as_str(),
            copy().tables.model.as_str(),
            copy().tables.cost.as_str(),
            copy().metrics.input.as_str(),
            copy().metrics.output.as_str(),
            copy().tables.cache.as_str(),
            copy().tables.tools.as_str(),
            copy().tables.prompt.as_str(),
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
            "{} {} - {} - {}",
            copy().panels.calls,
            idx + 1,
            escape_html(&call.model),
            escape_html(&call.cost)
        );
        out.push_str("</summary>\n<div class=\"call-facts\">\n");
        push_meta(out, &copy().tables.time, &call.timestamp);
        push_meta(out, &copy().tables.model, &call.model);
        push_meta(out, &copy().tables.cost, &call.cost);
        push_meta(out, &copy().tables.tools, &call.tools);
        push_meta(out, &copy().metrics.input, &format_u64(call.input_tokens));
        push_meta(out, &copy().metrics.output, &format_u64(call.output_tokens));
        push_meta(
            out,
            &copy().metrics.cache_read,
            &format_u64(call.cache_read),
        );
        push_meta(
            out,
            &copy().metrics.cache_write,
            &format_u64(call.cache_write),
        );
        push_meta(
            out,
            &copy().session.reasoning,
            &format_u64(call.reasoning_tokens),
        );
        push_meta(
            out,
            &copy().session.web_search,
            &format_u64(call.web_search_requests),
        );
        out.push_str("</div>\n");
        if !call.bash_commands.is_empty() {
            out.push_str("<h4>");
            out.push_str(&escape_html(&copy().panels.shell_commands));
            out.push_str("</h4><pre>");
            out.push_str(&escape_html(&call.bash_commands.join("\n")));
            out.push_str("</pre>\n");
        }
        out.push_str("<h4>");
        out.push_str(&escape_html(&copy().tables.prompt));
        out.push_str("</h4><pre>");
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
    push_pdf_panel_open(out, &copy().panels.selected_session, "tone-red", "session");
    out.push_str("<table class=\"pdf-facts\"><tbody>");
    let total_calls = session.total_calls.to_string();
    let facts = [
        (copy().tables.project.as_str(), session.project.as_str()),
        (copy().tables.tool.as_str(), session.tool),
        (
            copy().export.date_range.as_str(),
            session.date_range.as_str(),
        ),
        (copy().tables.cost.as_str(), session.total_cost.as_str()),
        (copy().tables.calls.as_str(), total_calls.as_str()),
        (copy().metrics.input.as_str(), session.total_input.as_str()),
        (
            copy().metrics.output.as_str(),
            session.total_output.as_str(),
        ),
        (
            copy().metrics.cache_read.as_str(),
            session.total_cache_read.as_str(),
        ),
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
            copy().tables.time.as_str(),
            copy().tables.model.as_str(),
            copy().tables.cost.as_str(),
            copy().metrics.input.as_str(),
            copy().metrics.output.as_str(),
            copy().tables.cache.as_str(),
            copy().tables.tools.as_str(),
            copy().tables.prompt.as_str(),
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
            "{} {} - {} - {}",
            copy().panels.calls,
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
            (copy().tables.time.as_str(), call.timestamp.as_str()),
            (copy().tables.model.as_str(), call.model.as_str()),
            (copy().tables.cost.as_str(), call.cost.as_str()),
            (copy().tables.tools.as_str(), call.tools.as_str()),
            (copy().metrics.input.as_str(), input.as_str()),
            (copy().metrics.output.as_str(), output.as_str()),
            (copy().metrics.cache_read.as_str(), cache_read.as_str()),
            (copy().metrics.cache_write.as_str(), cache_write.as_str()),
            (copy().session.reasoning.as_str(), reasoning.as_str()),
            (copy().session.web_search.as_str(), web_search.as_str()),
        ];
        push_pdf_fact_rows(out, &call_facts, 5);
        out.push_str("</tbody></table>");
        if !call.bash_commands.is_empty() {
            out.push_str("<h4>");
            out.push_str(&escape_html(&copy().panels.shell_commands));
            out.push_str("</h4><pre>");
            out.push_str(&escape_html(&call.bash_commands.join("\n")));
            out.push_str("</pre>");
        }
        out.push_str("<h4>");
        out.push_str(&escape_html(&copy().tables.prompt));
        out.push_str("</h4><pre>");
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
    push_pdf_panel_open(out, &copy().panels.daily_activity, "tone-blue", "calendar");
    if rows.is_empty() {
        out.push_str("<p class=\"empty\">");
        out.push_str(&escape_html(&copy().empty.no_data));
        out.push_str("</p>");
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
    push_pdf_table_open(
        out,
        &[
            copy().tables.date.as_str(),
            copy().tables.activity.as_str(),
            copy().tables.cost.as_str(),
            copy().tables.calls.as_str(),
        ],
    );
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
    for weekday in &copy().export.calendar_weekdays {
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
                    "<td class=\"i{}\"><strong>{}</strong><span class=\"cost\">{}</span><span class=\"calls\">{} {}</span></td>",
                    calendar_intensity(row.value),
                    day,
                    escape_html(row.cost),
                    calls,
                    escape_html(&copy().metrics.calls),
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
    push_pdf_panel_open(out, &copy().panels.by_project, "tone-green", "project");
    push_pdf_table_open(
        out,
        &[
            copy().tables.blank.as_str(),
            copy().tables.project.as_str(),
            copy().tables.cost.as_str(),
            copy().tables.avg_per_session.as_str(),
            copy().tables.sessions.as_str(),
            copy().tables.tools.as_str(),
        ],
    );
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
    push_pdf_panel_open(out, &copy().panels.top_sessions, "tone-red", "session");
    push_pdf_table_open(
        out,
        &[
            copy().tables.blank.as_str(),
            copy().tables.date.as_str(),
            copy().tables.project.as_str(),
            copy().tables.cost.as_str(),
            copy().tables.calls.as_str(),
        ],
    );
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
    push_pdf_panel_open(
        out,
        &copy().panels.project_spend_by_tool,
        "tone-yellow",
        "split",
    );
    push_pdf_table_open(
        out,
        &[
            copy().tables.blank.as_str(),
            copy().tables.project.as_str(),
            copy().tables.tool.as_str(),
            copy().tables.cost.as_str(),
            copy().tables.calls.as_str(),
            copy().tables.sessions.as_str(),
            copy().tables.avg_per_session.as_str(),
        ],
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
    push_pdf_panel_open(out, &copy().panels.by_model, "tone-magenta", "model");
    push_pdf_table_open(
        out,
        &[
            copy().tables.blank.as_str(),
            copy().tables.model.as_str(),
            copy().tables.cost.as_str(),
            copy().tables.cache.as_str(),
            copy().tables.calls.as_str(),
        ],
    );
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
    push_pdf_table_open(
        out,
        &[
            copy().tables.blank.as_str(),
            copy().tables.name.as_str(),
            copy().tables.calls.as_str(),
        ],
    );
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
        "<tr><td class=\"empty\" colspan=\"{}\">{}</td></tr>",
        colspan,
        escape_html(&copy().empty.no_data)
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
    rank_bar_svg(value, RankBarSize::Pdf)
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
    push_section_open(
        out,
        &copy().panels.daily_activity,
        "tone-blue wide",
        "calendar",
    );
    if rows.is_empty() {
        out.push_str("<p class=\"empty\">");
        out.push_str(&escape_html(&copy().empty.no_data));
        out.push_str("</p>\n");
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
        for weekday in &copy().export.calendar_weekdays {
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
    push_table_open(
        out,
        &[
            copy().tables.date.as_str(),
            copy().tables.activity.as_str(),
            copy().tables.cost.as_str(),
            copy().tables.calls.as_str(),
        ],
    );
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
        "<div class=\"calendar-cell i{}\"><div class=\"calendar-day-head\"><strong>{}</strong><span>{}</span></div><div class=\"calendar-cost\">{}</div><div class=\"calendar-calls\">{} {}</div></div>",
        calendar_intensity(row.value),
        day,
        escape_html(row.day),
        escape_html(row.cost),
        format_u64(row.calls),
        escape_html(&copy().metrics.calls),
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
    push_section_open(out, &copy().panels.by_project, "tone-green wide", "project");
    push_table_open(
        out,
        &[
            copy().tables.blank.as_str(),
            copy().tables.project.as_str(),
            copy().tables.cost.as_str(),
            copy().tables.avg_per_session.as_str(),
            copy().tables.sessions.as_str(),
            copy().tables.tools.as_str(),
        ],
    );
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
    push_section_open(out, &copy().panels.top_sessions, "tone-red wide", "session");
    push_table_open(
        out,
        &[
            copy().tables.blank.as_str(),
            copy().tables.date.as_str(),
            copy().tables.project.as_str(),
            copy().tables.cost.as_str(),
            copy().tables.calls.as_str(),
        ],
    );
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
    push_section_open(
        out,
        &copy().panels.project_spend_by_tool,
        "tone-yellow",
        "split",
    );
    push_table_open(
        out,
        &[
            copy().tables.blank.as_str(),
            copy().tables.project.as_str(),
            copy().tables.tool.as_str(),
            copy().tables.cost.as_str(),
            copy().tables.calls.as_str(),
            copy().tables.sessions.as_str(),
            copy().tables.avg_per_session.as_str(),
        ],
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
    push_section_open(out, &copy().panels.by_model, "tone-magenta", "model");
    push_table_open(
        out,
        &[
            copy().tables.blank.as_str(),
            copy().tables.model.as_str(),
            copy().tables.cost.as_str(),
            copy().tables.cache.as_str(),
            copy().tables.calls.as_str(),
        ],
    );
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
    push_table_open(
        out,
        &[
            copy().tables.blank.as_str(),
            copy().tables.name.as_str(),
            copy().tables.calls.as_str(),
        ],
    );
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
    let copy = copy();
    [
        copy.tables.cost.as_str(),
        copy.tables.calls.as_str(),
        copy.tables.avg_per_session.as_str(),
        copy.tables.sessions.as_str(),
        copy.tables.used.as_str(),
        copy.metrics.tokens.as_str(),
        copy.metrics.input.as_str(),
        copy.metrics.output.as_str(),
        copy.tables.cache.as_str(),
    ]
    .contains(&header)
}

fn push_table_close(out: &mut String) {
    out.push_str("</tbody></table></div>\n");
}

fn push_empty_row(out: &mut String, colspan: usize) {
    let _ = writeln!(
        out,
        "<tr><td class=\"empty\" colspan=\"{}\">{}</td></tr>",
        colspan,
        escape_html(&copy().empty.no_data)
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
    rank_bar_svg(value, RankBarSize::Html)
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
        "pulse" => {
            r#"<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 13h4l2-6 4 12 2-6h4" fill="none" stroke="currentColor" stroke-width="2"/></svg>"#
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

pub(super) fn write_pdf_report(
    path: &Path,
    context: &ExportContext<'_>,
    stamp: &str,
) -> Result<()> {
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
