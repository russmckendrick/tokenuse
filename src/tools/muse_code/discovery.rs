use std::path::Path;

use color_eyre::Result;
use walkdir::WalkDir;

use crate::tools::SessionSource;

use super::config;

pub fn discover() -> Result<Vec<SessionSource>> {
    let Some(root) = config::sessions_root() else {
        return Ok(Vec::new());
    };
    discover_in(&root)
}

fn discover_in(root: &Path) -> Result<Vec<SessionSource>> {
    let mut sources = Vec::new();
    if !root.exists() {
        return Ok(sources);
    }

    for entry in WalkDir::new(root)
        .follow_links(false)
        .max_depth(config::WALK_MAX_DEPTH)
        .into_iter()
        .flatten()
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if path.file_name().and_then(|n| n.to_str()) != Some(config::SESSION_FILE) {
            continue;
        }
        // The parser reads the in-file `metadata` record for the real
        // project; the relative parent is only a fallback label.
        let project = path
            .strip_prefix(root)
            .ok()
            .and_then(|p| p.parent())
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        sources.push(SessionSource::session(
            path.to_path_buf(),
            project,
            config::TOOL_ID,
        ));
    }
    sources.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(sources)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_file(path: &Path, contents: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        let mut file = std::fs::File::create(path).unwrap();
        file.write_all(contents.as_bytes()).unwrap();
    }

    #[test]
    fn discovers_session_and_subagent_logs() {
        let base = std::env::temp_dir().join(format!(
            "tokenuse-muse-discovery-{}",
            crate::tools::paths::test_run_id()
        ));
        let main = base.join("2026/11/12/abc123/session.jsonl");
        let sub = base.join("2026/11/12/abc123/subagent/def456/session.jsonl");
        write_file(&main, "{}\n");
        write_file(&sub, "{}\n");
        write_file(&base.join("2026/11/12/abc123/notes.txt"), "skip me\n");

        let sources = discover_in(&base).unwrap();
        let names: Vec<_> = sources.iter().map(|s| s.project.clone()).collect();
        assert_eq!(sources.len(), 2);
        assert!(names.iter().any(|p| p.ends_with("abc123")));
        assert!(names.iter().any(|p| p.ends_with("def456")));

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn missing_root_discovers_nothing() {
        let base = std::env::temp_dir().join(format!(
            "tokenuse-muse-discovery-missing-{}",
            crate::tools::paths::test_run_id()
        ));
        let sources = discover_in(&base.join("nope")).unwrap();
        assert!(sources.is_empty());
    }
}
