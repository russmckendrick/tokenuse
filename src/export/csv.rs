use std::{fs, path::Path};

use color_eyre::{eyre::WrapErr, Result};

use crate::{
    copy::copy,
    data::{
        CountMetric, DailyMetric, DashboardData, ModelMetric, ProjectMetric, ProjectToolMetric,
        SessionMetric, Summary,
    },
};

pub(super) fn write_csv_dir(dir: &Path, data: &DashboardData) -> Result<()> {
    write_summary_csv(dir, &data.summary)?;
    write_daily_csv(dir, &data.daily)?;
    write_projects_csv(dir, &data.projects)?;
    write_project_tools_csv(dir, &data.project_tools)?;
    write_sessions_csv(dir, &data.sessions)?;
    write_models_csv(dir, &data.models)?;
    write_counts_csv(dir, &copy().export.csv_files.tools_file, &data.tools)?;
    write_counts_csv(dir, &copy().export.csv_files.commands_file, &data.commands)?;
    write_counts_csv(
        dir,
        &copy().export.csv_files.mcp_servers_file,
        &data.mcp_servers,
    )?;
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

pub(super) fn csv_escape(value: &str) -> String {
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
        &copy().export.csv_files.summary_file,
        &[
            copy().tables.cost.as_str(),
            copy().tables.calls.as_str(),
            copy().tables.sessions.as_str(),
            copy().tables.cache_hit.as_str(),
            copy().metrics.input.as_str(),
            copy().metrics.output.as_str(),
            copy().metrics.cached.as_str(),
            copy().metrics.written.as_str(),
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
    write_csv(
        dir,
        &copy().export.csv_files.daily_file,
        &[
            copy().tables.day.as_str(),
            copy().tables.cost.as_str(),
            copy().tables.calls.as_str(),
        ],
        &data,
    )
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
        &copy().export.csv_files.projects_file,
        &[
            copy().tables.name.as_str(),
            copy().tables.cost.as_str(),
            copy().tables.avg_per_session.as_str(),
            copy().tables.sessions.as_str(),
            copy().tables.tool_mix.as_str(),
        ],
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
        &copy().export.csv_files.project_tools_file,
        &[
            copy().tables.project.as_str(),
            copy().tables.tool.as_str(),
            copy().tables.cost.as_str(),
            copy().tables.calls.as_str(),
            copy().tables.sessions.as_str(),
            copy().tables.avg_per_session.as_str(),
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
        &copy().export.csv_files.sessions_file,
        &[
            copy().tables.date.as_str(),
            copy().tables.project.as_str(),
            copy().tables.cost.as_str(),
            copy().tables.calls.as_str(),
        ],
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
        &copy().export.csv_files.models_file,
        &[
            copy().tables.name.as_str(),
            copy().tables.cost.as_str(),
            copy().tables.cache.as_str(),
            copy().tables.calls.as_str(),
        ],
        &data,
    )
}

fn write_counts_csv(dir: &Path, name: &str, rows: &[CountMetric]) -> Result<()> {
    let data: Vec<Vec<String>> = rows
        .iter()
        .map(|r| vec![r.name.to_string(), r.calls.to_string()])
        .collect();
    write_csv(
        dir,
        name,
        &[copy().tables.name.as_str(), copy().tables.calls.as_str()],
        &data,
    )
}
