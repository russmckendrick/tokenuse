use color_eyre::Result;
use walkdir::WalkDir;

use crate::tools::SessionSource;

use super::config;

pub fn discover() -> Result<Vec<SessionSource>> {
    let roots: Vec<_> = [config::sessions_root(), config::archived_sessions_root()]
        .into_iter()
        .flatten()
        .collect();
    discover_roots(&roots)
}

fn discover_roots(roots: &[std::path::PathBuf]) -> Result<Vec<SessionSource>> {
    let mut out = Vec::new();
    for root in roots {
        if !root.exists() {
            continue;
        }
        for entry in WalkDir::new(root).follow_links(false).into_iter().flatten() {
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path();
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if !name.starts_with(config::ROLLOUT_PREFIX)
                || path.extension().and_then(|s| s.to_str()) != Some(config::SESSION_GLOB_EXT)
            {
                continue;
            }
            let project = entry
                .path()
                .strip_prefix(root)
                .ok()
                .and_then(|p| p.parent())
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();
            out.push(SessionSource::session(
                path.to_path_buf(),
                project,
                config::TOOL_ID,
            ));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archived_sessions_are_discovered_alongside_dated_sessions() {
        let base = std::env::temp_dir().join(format!(
            "tokenuse-codex-discovery-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let dated = base.join("sessions/2026/03/29");
        let archived = base.join("archived_sessions");
        std::fs::create_dir_all(&dated).unwrap();
        std::fs::create_dir_all(&archived).unwrap();
        std::fs::write(dated.join("rollout-a.jsonl"), "{}").unwrap();
        std::fs::write(archived.join("rollout-b.jsonl"), "{}").unwrap();
        std::fs::write(archived.join("notes.txt"), "skip me").unwrap();

        let sources =
            discover_roots(&[base.join("sessions"), base.join("archived_sessions")]).unwrap();

        let mut names: Vec<_> = sources
            .iter()
            .filter_map(|s| s.path.file_name().and_then(|n| n.to_str()))
            .collect();
        names.sort_unstable();
        assert_eq!(names, vec!["rollout-a.jsonl", "rollout-b.jsonl"]);
        let dated_source = sources
            .iter()
            .find(|s| s.path.file_name().and_then(|n| n.to_str()) == Some("rollout-a.jsonl"))
            .unwrap();
        assert_eq!(dated_source.project, "2026/03/29");

        let _ = std::fs::remove_dir_all(&base);
    }
}
