use std::fs;
use std::path::Path;

use color_eyre::Result;

use crate::tools::SessionSource;

use super::config;

pub fn discover() -> Result<Vec<SessionSource>> {
    let Some(root) = config::gemini_tmp_root() else {
        return Ok(Vec::new());
    };
    discover_in(&root)
}

fn discover_in(root: &Path) -> Result<Vec<SessionSource>> {
    if !root.exists() {
        return Ok(Vec::new());
    }

    let mut sources = Vec::new();
    for project_entry in fs::read_dir(root)?.flatten() {
        let project_path = project_entry.path();
        if !project_path.is_dir() {
            continue;
        }
        let project = project_entry.file_name().to_string_lossy().to_string();
        let chats_dir = project_path.join(config::CHATS_DIR);
        let Ok(chat_entries) = fs::read_dir(chats_dir) else {
            continue;
        };

        for chat_entry in chat_entries.flatten() {
            let path = chat_entry.path();
            if !path.is_file() || !is_session_file(&path) {
                continue;
            }
            sources.push(SessionSource {
                path,
                project: project.clone(),
                tool: config::TOOL_ID,
            });
        }
    }
    sources.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(sources)
}

fn is_session_file(path: &Path) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    name.starts_with(config::SESSION_PREFIX) && matches!(ext, config::JSON_EXT | config::JSONL_EXT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::path::{Path, PathBuf};

    #[test]
    fn discovers_gemini_chat_sessions() {
        let dir = TempDir::new();
        write_file(
            &dir.path().join("project-hash/chats/session-a.jsonl"),
            "{}\n",
        );
        write_file(
            &dir.path().join("project-hash/chats/session-b.json"),
            "{}\n",
        );
        write_file(&dir.path().join("project-hash/chats/notes.jsonl"), "{}\n");
        write_file(
            &dir.path().join("project-hash/other/session-c.jsonl"),
            "{}\n",
        );

        let sources = discover_in(dir.path()).unwrap();

        assert_eq!(sources.len(), 2);
        assert_eq!(sources[0].project, "project-hash");
        assert!(sources[0].path.ends_with("session-a.jsonl"));
        assert!(sources[1].path.ends_with("session-b.json"));
        assert!(sources.iter().all(|source| source.tool == config::TOOL_ID));
    }

    fn write_file(path: &Path, raw: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        let mut file = std::fs::File::create(path).unwrap();
        file.write_all(raw.as_bytes()).unwrap();
    }

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "tokenuse-gemini-discovery-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}
