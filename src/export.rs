use std::fs;
use std::path::{Path, PathBuf};

use chrono::Local;
use color_eyre::{eyre::WrapErr, Result};
use plotters::prelude::*;

use crate::app::{Period, ProjectFilter, Tool};
use crate::config::ConfigPaths;
use crate::data::{
    CountMetric, DailyMetric, DashboardData, ModelMetric, ProjectMetric, ProjectToolMetric,
    SessionMetric, Summary,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Json,
    Csv,
    Svg,
    Png,
}

impl ExportFormat {
    pub fn label(self) -> &'static str {
        match self {
            Self::Json => "JSON",
            Self::Csv => "CSV (one file per panel)",
            Self::Svg => "SVG (full dashboard)",
            Self::Png => "PNG (full dashboard)",
        }
    }

    pub const ALL: [Self; 4] = [Self::Json, Self::Csv, Self::Svg, Self::Png];
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
const TITLE_SIZE: u32 = 16;
const HEAD_SIZE: u32 = 12;
const BODY_SIZE: u32 = 13;
const NUM_SIZE: u32 = 22;
const ROW_HEIGHT: i32 = 18;
const CHAR_W: i32 = 8;

pub fn write(
    paths: &ConfigPaths,
    format: ExportFormat,
    data: &DashboardData,
    period: Period,
    tool: Tool,
    project_filter: &ProjectFilter,
) -> Result<PathBuf> {
    let exports_root = paths.dir.join("exports");
    fs::create_dir_all(&exports_root)
        .wrap_err_with(|| format!("create {}", exports_root.display()))?;

    let slug = filter_slug(period, tool, project_filter);
    let stamp = Local::now().format("%Y%m%dT%H%M%S").to_string();

    match format {
        ExportFormat::Json => {
            let file = exports_root.join(format!("tokenuse-{stamp}-{slug}.json"));
            let text = serde_json::to_string_pretty(data).wrap_err("serialize json")?;
            fs::write(&file, text).wrap_err_with(|| format!("write {}", file.display()))?;
            Ok(file)
        }
        ExportFormat::Csv => {
            let dir = exports_root.join(format!("tokenuse-{stamp}-{slug}-csv"));
            fs::create_dir_all(&dir).wrap_err_with(|| format!("create {}", dir.display()))?;
            write_csv_dir(&dir, data)?;
            Ok(dir)
        }
        ExportFormat::Svg => {
            let file = exports_root.join(format!("tokenuse-{stamp}-{slug}.svg"));
            write_chart_svg(&file, data, period, tool, project_filter)?;
            Ok(file)
        }
        ExportFormat::Png => {
            let file = exports_root.join(format!("tokenuse-{stamp}-{slug}.png"));
            write_chart_png(&file, data, period, tool, project_filter)?;
            Ok(file)
        }
    }
}

fn filter_slug(period: Period, tool: Tool, project_filter: &ProjectFilter) -> String {
    let period = match period {
        Period::Today => "today",
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

const CANVAS_W: u32 = 1800;
const CANVAS_H: u32 = 1500;

fn write_chart_svg(
    path: &Path,
    data: &DashboardData,
    period: Period,
    tool: Tool,
    project_filter: &ProjectFilter,
) -> Result<()> {
    let backend = SVGBackend::new(path, (CANVAS_W, CANVAS_H));
    render_dashboard_chart(backend, data, period, tool, project_filter)
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
    let backend = BitMapBackend::new(path, (CANVAS_W, CANVAS_H));
    render_dashboard_chart(backend, data, period, tool, project_filter)
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
) -> ChartResult
where
    DB: DrawingBackend,
    DB::ErrorType: 'static,
{
    let root = backend.into_drawing_area();
    root.fill(&SURFACE)?;

    let outer_x = 24i32;
    let mut y = 24i32;
    let panel_w = CANVAS_W as i32 - outer_x * 2;
    let half_gap = 12i32;
    let half_w = (panel_w - half_gap) / 2;

    // Header band (period chips + summary)
    draw_period_strip(&root, outer_x, y, panel_w, period, tool, project_filter)?;
    y += 32;
    draw_summary_panel(&root, outer_x, y, panel_w, 110, &data.summary, period, tool)?;
    y += 110 + 12;

    // Row: Daily | Projects
    let row_h = 256;
    draw_panel(
        &root,
        outer_x,
        y,
        half_w,
        row_h,
        "Daily Activity",
        &BLUE,
        |x, y, w, h| draw_daily(&root, x, y, w, h, &data.daily),
    )?;
    draw_panel(
        &root,
        outer_x + half_w + half_gap,
        y,
        half_w,
        row_h,
        "By Project",
        &GREEN,
        |x, y, w, h| draw_projects(&root, x, y, w, h, &data.projects),
    )?;
    y += row_h + 12;

    // Row: Top Sessions full width
    let sessions_h = 200;
    draw_panel(
        &root,
        outer_x,
        y,
        panel_w,
        sessions_h,
        "Top Sessions",
        &RED,
        |x, y, w, h| draw_sessions(&root, x, y, w, h, &data.sessions),
    )?;
    y += sessions_h + 12;

    // Row: Project Tools | Models
    let row_h = 320;
    draw_panel(
        &root,
        outer_x,
        y,
        half_w,
        row_h,
        "Project Spend by Tool",
        &YELLOW,
        |x, y, w, h| draw_project_tools(&root, x, y, w, h, &data.project_tools),
    )?;
    draw_panel(
        &root,
        outer_x + half_w + half_gap,
        y,
        half_w,
        row_h,
        "By Model",
        &MAGENTA,
        |x, y, w, h| draw_models(&root, x, y, w, h, &data.models),
    )?;
    y += row_h + 12;

    // Row: Core Tools | Shell Commands
    let row_h = 240;
    draw_panel(
        &root,
        outer_x,
        y,
        half_w,
        row_h,
        "Core Tools",
        &CYAN,
        |x, y, w, h| draw_counts(&root, x, y, w, h, &data.tools),
    )?;
    draw_panel(
        &root,
        outer_x + half_w + half_gap,
        y,
        half_w,
        row_h,
        "Shell Commands",
        &PRIMARY,
        |x, y, w, h| draw_counts(&root, x, y, w, h, &data.commands),
    )?;
    y += row_h + 12;

    // Row: MCP Servers full width
    let mcp_h = (CANVAS_H as i32 - y - 24).max(140);
    draw_panel(
        &root,
        outer_x,
        y,
        panel_w,
        mcp_h,
        "MCP Servers",
        &MAGENTA,
        |x, y, w, h| draw_counts(&root, x, y, w, h, &data.mcp_servers),
    )?;

    root.present()?;
    Ok(())
}

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

    body(x + 16, y + 14, w - 32, h - 24)
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
        (Period::Today, "Today"),
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
    };
    let project_label = project_filter.label();
    let right_text = format!("[t] {tool_label}    [p] {project_label}");
    let right_w = right_text.chars().count() as i32 * CHAR_W;
    root.draw_text(&right_text, &body_style(&MUTED), (x + w - right_w, y))?;
    Ok(())
}

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
        return empty_note(root, x, y + 24);
    }

    for (i, row) in rows.iter().take(8).enumerate() {
        let ry = y + 24 + i as i32 * ROW_HEIGHT;
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
        return empty_note(root, x, y + 24);
    }

    for (i, row) in rows.iter().take(8).enumerate() {
        let ry = y + 24 + i as i32 * ROW_HEIGHT;
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
        return empty_note(root, x, y + 24);
    }

    for (i, row) in rows.iter().take(8).enumerate() {
        let ry = y + 24 + i as i32 * ROW_HEIGHT;
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
        return empty_note(root, x, y + 24);
    }

    for (i, row) in rows.iter().take(13).enumerate() {
        let ry = y + 24 + i as i32 * ROW_HEIGHT;
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
        return empty_note(root, x, y + 24);
    }

    for (i, row) in rows.iter().take(13).enumerate() {
        let ry = y + 24 + i as i32 * ROW_HEIGHT;
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
        return empty_note(root, x, y + 24);
    }

    let max_rows = ((h - 24) / ROW_HEIGHT).max(1) as usize;
    for (i, row) in rows.iter().take(max_rows).enumerate() {
        let ry = y + 24 + i as i32 * ROW_HEIGHT;
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
        Period::Today => "Today",
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ConfigPaths;
    use crate::currency::CurrencyFormatter;
    use crate::data::dashboard_data;
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

    fn fixture() -> (ConfigPaths, DashboardData) {
        let dir = tempdir("paths");
        let paths = ConfigPaths::new(dir);
        let data = dashboard_data(
            Period::AllTime,
            Tool::All,
            &ProjectFilter::All,
            &CurrencyFormatter::usd(),
        );
        (paths, data)
    }

    #[test]
    fn json_export_writes_pretty_file_with_summary() {
        let (paths, data) = fixture();
        let path = write(
            &paths,
            ExportFormat::Json,
            &data,
            Period::AllTime,
            Tool::All,
            &ProjectFilter::All,
        )
        .unwrap();
        assert!(path.exists());
        let body = fs::read_to_string(&path).unwrap();
        assert!(body.contains("\"summary\""));
        assert!(body.contains("\"daily\""));
        let _ = fs::remove_dir_all(&paths.dir);
    }

    #[test]
    fn csv_export_writes_one_file_per_panel() {
        let (paths, data) = fixture();
        let dir = write(
            &paths,
            ExportFormat::Csv,
            &data,
            Period::AllTime,
            Tool::All,
            &ProjectFilter::All,
        )
        .unwrap();
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
        let path = write(
            &paths,
            ExportFormat::Svg,
            &data,
            Period::AllTime,
            Tool::All,
            &ProjectFilter::All,
        )
        .unwrap();
        let body = fs::read_to_string(&path).unwrap();
        assert!(body.contains("<svg"));
        let _ = fs::remove_dir_all(&paths.dir);
    }

    #[test]
    fn png_export_writes_png_signature() {
        let _lock = CHART_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let (paths, data) = fixture();
        let path = write(
            &paths,
            ExportFormat::Png,
            &data,
            Period::AllTime,
            Tool::All,
            &ProjectFilter::All,
        )
        .unwrap();
        let bytes = fs::read(&path).unwrap();
        assert!(bytes.len() > 8);
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
        let _ = fs::remove_dir_all(&paths.dir);
    }

    #[test]
    fn csv_escape_handles_commas_and_quotes() {
        assert_eq!(csv_escape("simple"), "simple");
        assert_eq!(csv_escape("a,b"), "\"a,b\"");
        assert_eq!(csv_escape("she said \"hi\""), "\"she said \"\"hi\"\"\"");
    }
}
