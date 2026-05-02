use std::fs;
use std::path::{Path, PathBuf};

use color_eyre::Result;
use serde_json::Value;
use walkdir::WalkDir;

use crate::tools::SessionSource;

use super::config;

pub fn discover() -> Result<Vec<SessionSource>> {
    let mut sources = Vec::new();

    if let Some(legacy) = config::legacy_root() {
        if let Ok(entries) = fs::read_dir(&legacy) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    sources.push(SessionSource {
                        project: entry.file_name().to_string_lossy().to_string(),
                        path,
                        tool: config::TOOL_ID,
                    });
                }
            }
        }
    }

    for ws in config::vscode_workspace_storage_dirs() {
        if ws.exists() {
            for entry in WalkDir::new(&ws)
                .max_depth(3)
                .follow_links(false)
                .into_iter()
                .flatten()
            {
                if !entry.file_type().is_dir() {
                    continue;
                }
                if entry.file_name() == "transcripts" {
                    let workspace_dir = workspace_dir_for_transcripts(entry.path());
                    sources.push(SessionSource {
                        project: workspace_dir
                            .as_deref()
                            .and_then(read_workspace_project)
                            .or_else(|| workspace_dir.as_deref().and_then(workspace_hash_label))
                            .unwrap_or_else(|| "vscode-workspace".into()),
                        path: entry.path().to_path_buf(),
                        tool: config::TOOL_ID,
                    });
                }
            }
        }
    }

    Ok(sources)
}

fn workspace_dir_for_transcripts(path: &Path) -> Option<PathBuf> {
    path.parent()?.parent().map(Path::to_path_buf)
}

fn workspace_hash_label(workspace_dir: &Path) -> Option<String> {
    workspace_dir
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .filter(|name| !name.is_empty())
}

fn read_workspace_project(workspace_dir: &Path) -> Option<String> {
    let raw = fs::read_to_string(workspace_dir.join("workspace.json")).ok()?;
    let value: Value = serde_json::from_str(&raw).ok()?;
    let folder = value.get("folder").and_then(|v| v.as_str())?;
    folder_label(folder)
}

fn folder_label(folder: &str) -> Option<String> {
    let path = folder.strip_prefix("file://").unwrap_or(folder);
    let decoded = percent_decode(path);
    Path::new(&decoded)
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .filter(|name| !name.is_empty())
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(hi), Some(lo)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                out.push((hi << 4) | lo);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

fn hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_json_folder_becomes_project_label() {
        assert_eq!(
            folder_label("file:///Users/me/Code/my%20app").as_deref(),
            Some("my app")
        );
    }
}
