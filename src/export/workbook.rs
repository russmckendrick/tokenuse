use std::fs;
use std::path::{Path, PathBuf};

use chrono::Local;
use color_eyre::{eyre::WrapErr, Result};

use crate::app::{Period, ProjectFilter, SortMode, Tool};
use crate::config::ConfigPaths;
use crate::copy::copy;
use crate::data::{DashboardData, SessionDetailView};

use super::{chart, csv, report};

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
        let copy = copy();
        match self {
            Self::Json => copy.export.json.as_str(),
            Self::Csv => copy.export.csv.as_str(),
            Self::Svg => copy.export.svg.as_str(),
            Self::Png => copy.export.png.as_str(),
            Self::Html => copy.export.html.as_str(),
            Self::Pdf => copy.export.pdf.as_str(),
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
            csv::write_csv_dir(&dir, context.dashboard)?;
            Ok(dir)
        }
        ExportFormat::Svg => {
            let file = exports_root.join(format!("tokenuse-{stamp}-{slug}.svg"));
            chart::write_chart_svg(
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
            chart::write_chart_png(
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
            report::write_html_report(&file, context, &stamp)?;
            Ok(file)
        }
        ExportFormat::Pdf => {
            let file = exports_root.join(format!("tokenuse-{stamp}-{slug}.pdf"));
            report::write_pdf_report(&file, context, &stamp)?;
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

#[cfg(test)]
mod tests {
    use super::super::labels;
    use super::*;
    use crate::config::ConfigPaths;
    use crate::currency::CurrencyFormatter;
    use crate::data::{
        dashboard_data, ActivityMetric, CountMetric, ProjectMetric, SessionDetail,
        SessionDetailView,
    };
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
        assert_eq!(labels::tool_label(Tool::Gemini), "Gemini");
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
        assert!(body.contains("data-export-chart=\"activity-timeline\""));
        assert!(body.contains("data-export-rank=\"true\""));
        assert!(body.contains("Daily Activity"));
        assert!(body.contains("calendar-grid"));
        assert!(body.contains("calendar-cell"));
        assert!(!body.contains("Usage Limits"));
        assert!(body.contains("Full workbook report"));
        assert!(!body.contains("<script"));
        let _ = fs::remove_dir_all(&paths.dir);
    }

    #[test]
    fn pdf_report_source_includes_inline_svg_charts() {
        let (_paths, data) = fixture();
        let context = context(&data, None);
        let body = report::build_pdf_html_report(&context, "pdf-source");

        assert!(body.contains("data-export-chart=\"activity-timeline\""));
        assert!(body.contains("data-export-rank=\"true\""));
        assert!(body.contains("xmlns=\"http://www.w3.org/2000/svg\""));
        assert!(!body.contains("<script"));
    }

    #[test]
    fn activity_timeline_svg_handles_empty_single_point_and_escaped_labels() {
        let (_paths, mut data) = fixture();

        data.activity_timeline.clear();
        let empty = report::build_html_report(&context(&data, None), "empty-activity");
        assert!(empty.contains("data-export-chart=\"activity-timeline\""));
        assert!(empty.contains(copy().timeline.no_activity.as_str()));

        data.activity_timeline = vec![ActivityMetric {
            label: "<only & bucket>",
            cost: "$0.00",
            calls: 0,
            value: 0,
        }];
        let single = report::build_html_report(&context(&data, None), "single-activity");
        assert!(single.contains("&lt;only &amp; bucket&gt;"));
        assert!(single.contains(&format!("$0.00 - 0 {}", copy().metrics.calls)));
        assert!(!single.contains("<only & bucket>"));
        assert!(!single.contains("NaN"));
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
        assert_eq!(csv::csv_escape("simple"), "simple");
        assert_eq!(csv::csv_escape("a,b"), "\"a,b\"");
        assert_eq!(
            csv::csv_escape("she said \"hi\""),
            "\"she said \"\"hi\"\"\""
        );
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
