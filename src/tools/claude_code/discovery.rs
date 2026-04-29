use std::fs;
use std::path::{Path, PathBuf};

use color_eyre::Result;
use walkdir::WalkDir;

use crate::tools::SessionSource;

use super::config;

pub fn discover() -> Result<Vec<SessionSource>> {
    let mut sources = Vec::new();

    if let Some(projects) = config::projects_dir() {
        sources.extend(list_project_dirs(&projects));
    }

    if let Some(desktop) = config::desktop_sessions_dir() {
        sources.extend(walk_desktop(&desktop));
    }

    Ok(sources)
}

fn list_project_dirs(root: &Path) -> Vec<SessionSource> {
    let mut out = Vec::new();
    let entries = match fs::read_dir(root) {
        Ok(e) => e,
        Err(_) => return out,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        out.push(SessionSource {
            project: config::unsanitize_project(&name),
            path,
            tool: config::TOOL_ID,
        });
    }
    out
}

fn walk_desktop(root: &Path) -> Vec<SessionSource> {
    let mut out = Vec::new();
    if !root.exists() {
        return out;
    }
    for entry in WalkDir::new(root)
        .max_depth(config::DESKTOP_WALK_DEPTH)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            name != "node_modules" && name != ".git"
        })
        .flatten()
    {
        if !entry.file_type().is_dir() || entry.file_name() != "projects" {
            continue;
        }
        let projects_dir: PathBuf = entry.path().to_path_buf();
        out.extend(list_project_dirs(&projects_dir));
    }
    out
}
