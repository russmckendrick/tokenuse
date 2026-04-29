use std::fs;

use color_eyre::Result;
use walkdir::WalkDir;

use crate::providers::SessionSource;

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
                        provider: config::PROVIDER_ID,
                    });
                }
            }
        }
    }

    if let Some(ws) = config::vscode_workspace_storage() {
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
                    sources.push(SessionSource {
                        project: entry
                            .path()
                            .components()
                            .rev()
                            .nth(2)
                            .map(|c| c.as_os_str().to_string_lossy().to_string())
                            .unwrap_or_else(|| "vscode-workspace".into()),
                        path: entry.path().to_path_buf(),
                        provider: config::PROVIDER_ID,
                    });
                }
            }
        }
    }

    Ok(sources)
}
