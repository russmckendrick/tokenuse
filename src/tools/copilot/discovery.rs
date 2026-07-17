use std::fs;
use std::path::{Path, PathBuf};

use color_eyre::Result;
use serde_json::Value;

use crate::tools::SessionSource;

use super::config;

pub fn discover() -> Result<Vec<SessionSource>> {
    let mut sources = Vec::new();

    if let Some(legacy) = config::legacy_root() {
        if let Ok(entries) = fs::read_dir(&legacy) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    sources.push(SessionSource::session(
                        path,
                        entry.file_name().to_string_lossy().to_string(),
                        config::TOOL_ID,
                    ));
                }
            }
        }
    }

    for storage in config::vscode_storage_roots() {
        sources.extend(discover_vscode_variant(&storage));
    }

    if let Some(cli_root) = config::cli_root() {
        for store in [config::CLI_SESSION_STORE_FILE, config::CLI_DATA_STORE_FILE] {
            let path = cli_root.join(store);
            if path.is_file() {
                sources.push(SessionSource::session(
                    path,
                    config::CLI_STORE_PROJECT_LABEL.to_string(),
                    config::TOOL_ID,
                ));
            }
        }
    }

    // The legacy single-account sidecar is copilot.json; multi-account syncs
    // write one copilot-<host>-<login>.json per account alongside it.
    if let Some(sidecar) = config::limit_sidecar() {
        if let Some(limits_dir) = sidecar.parent() {
            if let Ok(entries) = fs::read_dir(limits_dir) {
                let mut files: Vec<PathBuf> = entries
                    .flatten()
                    .map(|entry| entry.path())
                    .filter(|path| {
                        path.is_file()
                            && path
                                .file_name()
                                .and_then(|name| name.to_str())
                                .is_some_and(config::is_limit_sidecar_name)
                    })
                    .collect();
                files.sort();
                for file in files {
                    sources.push(SessionSource::limit(
                        file,
                        "copilot-limits",
                        config::TOOL_ID,
                    ));
                }
            }
        }
    }

    Ok(sources)
}

/// Per-variant source selection, most-authoritative first. When the Copilot
/// Chat extension's OTel span store exists it carries real token and cache
/// counts for every recorded turn, so the variant's journals and transcripts
/// (estimates or metadata for the same turns) are skipped to avoid double
/// counting. Without it, a workspace's chatSessions journals (real prompt and
/// output counts) win over its transcripts (chars/4 estimates).
fn discover_vscode_variant(storage: &config::VsCodeStorage) -> Vec<SessionSource> {
    let mut sources = Vec::new();

    let otel_db = config::otel_trace_db(storage);
    if otel_db.is_file() {
        sources.push(SessionSource::session(
            otel_db,
            config::OTEL_PROJECT_LABEL.to_string(),
            config::TOOL_ID,
        ));
        return sources;
    }

    if let Ok(entries) = fs::read_dir(&storage.workspace_storage) {
        for entry in entries.flatten() {
            let hash_dir = entry.path();
            if !hash_dir.is_dir() {
                continue;
            }
            let project = read_workspace_project(&hash_dir)
                .or_else(|| workspace_hash_label(&hash_dir))
                .unwrap_or_else(|| "vscode-workspace".into());
            let chat_dir = hash_dir.join(config::CHAT_SESSIONS_DIR);
            if dir_has_jsonl(&chat_dir) {
                sources.push(SessionSource::session(chat_dir, project, config::TOOL_ID));
                continue;
            }
            let transcripts = hash_dir.join(config::VSCODE_EXTENSION_DIR);
            if transcripts.is_dir() {
                sources.push(SessionSource::session(
                    transcripts,
                    project,
                    config::TOOL_ID,
                ));
            }
        }
    }

    let empty_window = storage
        .global_storage
        .join(config::EMPTY_WINDOW_CHAT_SESSIONS_DIR);
    if dir_has_jsonl(&empty_window) {
        sources.push(SessionSource::session(
            empty_window,
            config::OTEL_PROJECT_LABEL.to_string(),
            config::TOOL_ID,
        ));
    }

    sources
}

fn dir_has_jsonl(dir: &Path) -> bool {
    let Ok(entries) = fs::read_dir(dir) else {
        return false;
    };
    entries.flatten().any(|entry| {
        let path = entry.path();
        path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("jsonl")
    })
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

    fn temp_variant(name: &str) -> config::VsCodeStorage {
        let base = std::env::temp_dir().join(format!(
            "tokenuse-copilot-discovery-{}-{}-{}",
            name,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        config::VsCodeStorage {
            workspace_storage: base.join("workspaceStorage"),
            global_storage: base.join("globalStorage"),
        }
    }

    fn cleanup(storage: &config::VsCodeStorage) {
        if let Some(base) = storage.workspace_storage.parent() {
            let _ = fs::remove_dir_all(base);
        }
    }

    #[test]
    fn otel_store_gates_a_variants_journals_and_transcripts() {
        let storage = temp_variant("otel");
        let otel = config::otel_trace_db(&storage);
        fs::create_dir_all(otel.parent().unwrap()).unwrap();
        fs::write(&otel, b"sqlite placeholder").unwrap();
        // Journals and transcripts that would double count the OTel spans.
        let hash = storage.workspace_storage.join("hash1");
        fs::create_dir_all(hash.join(config::CHAT_SESSIONS_DIR)).unwrap();
        fs::write(hash.join(config::CHAT_SESSIONS_DIR).join("s.jsonl"), b"{}").unwrap();
        fs::create_dir_all(hash.join(config::VSCODE_EXTENSION_DIR)).unwrap();

        let sources = discover_vscode_variant(&storage);

        assert_eq!(sources.len(), 1, "only the OTel store is ingested");
        assert_eq!(sources[0].path, otel);
        cleanup(&storage);
    }

    #[test]
    fn chat_sessions_win_over_transcripts_per_workspace() {
        let storage = temp_variant("journals");
        // hash1 has journals (and transcripts, which lose); hash2 only
        // transcripts; the global empty-window folder has journals.
        let hash1 = storage.workspace_storage.join("hash1");
        fs::create_dir_all(hash1.join(config::CHAT_SESSIONS_DIR)).unwrap();
        fs::write(hash1.join(config::CHAT_SESSIONS_DIR).join("s.jsonl"), b"{}").unwrap();
        fs::create_dir_all(hash1.join(config::VSCODE_EXTENSION_DIR)).unwrap();
        let hash2 = storage.workspace_storage.join("hash2");
        fs::create_dir_all(hash2.join(config::VSCODE_EXTENSION_DIR)).unwrap();
        let empty = storage
            .global_storage
            .join(config::EMPTY_WINDOW_CHAT_SESSIONS_DIR);
        fs::create_dir_all(&empty).unwrap();
        fs::write(empty.join("e.jsonl"), b"{}").unwrap();

        let mut sources = discover_vscode_variant(&storage);
        sources.sort_by(|a, b| a.path.cmp(&b.path));

        let paths: Vec<_> = sources.iter().map(|s| s.path.clone()).collect();
        assert!(
            paths.contains(&hash1.join(config::CHAT_SESSIONS_DIR)),
            "journal dir wins for hash1"
        );
        assert!(
            !paths.contains(&hash1.join(config::VSCODE_EXTENSION_DIR)),
            "hash1 transcripts are skipped"
        );
        assert!(
            paths.contains(&hash2.join(config::VSCODE_EXTENSION_DIR)),
            "hash2 keeps its transcripts"
        );
        assert!(paths.contains(&empty), "empty-window journals are ingested");
        cleanup(&storage);
    }
}
