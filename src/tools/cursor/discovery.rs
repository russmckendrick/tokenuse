use std::fs;
use std::path::{Component, Path, PathBuf};

use color_eyre::Result;

use crate::tools::{paths, SessionSource};

use super::config;

pub fn discover() -> Result<Vec<SessionSource>> {
    let mut sources = Vec::new();

    if let Some(db) = config::state_db_path() {
        if db.exists() {
            sources.push(SessionSource {
                path: db,
                project: "cursor-workspace".into(),
                tool: config::TOOL_ID,
            });
        }
    }

    if let Some(projects) = config::agent_projects_dir() {
        sources.extend(discover_agent_transcripts_in(&projects)?);
    }

    Ok(sources)
}

fn discover_agent_transcripts_in(projects: &Path) -> Result<Vec<SessionSource>> {
    if !projects.exists() {
        return Ok(Vec::new());
    }

    let mut sources = Vec::new();
    for entry in fs::read_dir(projects)? {
        let Ok(entry) = entry else { continue };
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_dir() {
            continue;
        }

        let project_id = entry.file_name().to_string_lossy().to_string();
        let project = project_from_folder_id(&project_id);
        let transcript_dir = entry.path().join(config::AGENT_TRANSCRIPTS_DIR);
        if !transcript_dir.exists() {
            continue;
        }

        collect_transcript_files(&transcript_dir, &project, &mut sources)?;
    }

    sources.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(sources)
}

fn collect_transcript_files(
    dir: &Path,
    project: &str,
    sources: &mut Vec<SessionSource>,
) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let Ok(entry) = entry else { continue };
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };

        if file_type.is_file() {
            if is_transcript_file(&path) {
                sources.push(SessionSource {
                    path,
                    project: project.to_string(),
                    tool: config::TOOL_ID,
                });
            }
            continue;
        }

        if file_type.is_dir() {
            collect_transcript_files(&path, project, sources)?;
        }
    }
    Ok(())
}

fn is_transcript_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some("jsonl" | "txt")
    )
}

pub(crate) fn project_from_folder_id(raw: &str) -> String {
    project_from_folder_id_with_home(raw, paths::home().as_deref())
}

fn project_from_folder_id_with_home(raw: &str, home: Option<&Path>) -> String {
    if raw.is_empty() {
        return "cursor-workspace".into();
    }

    if raw.chars().all(|c| c.is_ascii_digit()) {
        return format!("cursor-agent:{raw}");
    }

    for base in ["Code", "Desktop", "Documents"] {
        if let Some(project) = project_from_home_folder(raw, base, home) {
            return project;
        }
    }

    for anchor in ["-Code-", "-Desktop-", "-Documents-"] {
        if let Some(idx) = raw.find(anchor) {
            let prefix_len = if raw.starts_with("Users-") {
                "Users-".len()
            } else {
                0
            };
            let user = &raw[prefix_len..idx];
            let base = anchor.trim_matches('-');
            let rest = &raw[idx + anchor.len()..];
            if !user.is_empty() && !rest.is_empty() {
                return format!("/Users/{user}/{base}/{rest}");
            }
        }
    }

    if let Some(rest) = raw.strip_prefix("Users-") {
        return format!("/Users/{}", rest.replace('-', "/"));
    }

    raw.to_string()
}

fn project_from_home_folder(raw: &str, base: &str, home: Option<&Path>) -> Option<String> {
    let home = home?;
    let encoded_home = encoded_path_id(home)?;
    let prefix = format!("{encoded_home}-{base}-");
    let rest = raw.strip_prefix(&prefix)?;
    if rest.is_empty() {
        return None;
    }

    let root = home.join(base);
    if let Some(existing) = existing_project_path(&root, rest) {
        return Some(existing.display().to_string());
    }
    Some(root.join(rest).display().to_string())
}

fn encoded_path_id(path: &Path) -> Option<String> {
    let parts = path
        .components()
        .filter_map(|component| match component {
            Component::Normal(part) => Some(encode_path_component(&part.to_string_lossy())),
            _ => None,
        })
        .collect::<Vec<_>>();
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("-"))
    }
}

fn encode_path_component(raw: &str) -> String {
    raw.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

fn existing_project_path(root: &Path, encoded_rest: &str) -> Option<PathBuf> {
    let direct = root.join(encoded_rest);
    if direct.is_dir() {
        return Some(direct);
    }

    let parts = encoded_rest
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if parts.len() < 2 || parts.len() > 14 {
        return None;
    }

    let mut best: Option<PathBuf> = None;
    let mut current = Vec::new();
    collect_existing_project_paths(root, &parts, 0, &mut current, &mut best);
    best
}

fn collect_existing_project_paths(
    root: &Path,
    parts: &[&str],
    idx: usize,
    current: &mut Vec<String>,
    best: &mut Option<PathBuf>,
) {
    if idx == parts.len() {
        let candidate = current
            .iter()
            .fold(root.to_path_buf(), |path, part| path.join(part));
        if candidate.is_dir()
            && best
                .as_ref()
                .map(|existing| candidate.components().count() > existing.components().count())
                .unwrap_or(true)
        {
            *best = Some(candidate);
        }
        return;
    }

    for end in idx + 1..=parts.len() {
        current.push(parts[idx..end].join("-"));
        collect_existing_project_paths(root, parts, end, current, best);
        current.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let suffix = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let counter = TEMP_COUNTER.fetch_add(1, Ordering::SeqCst);
            let path = std::env::temp_dir().join(format!(
                "tokenuse-cursor-discovery-{}-{suffix}-{counter}",
                std::process::id()
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn discovers_agent_transcript_files_and_subagents() {
        let dir = TempDir::new();
        let project = dir
            .path()
            .join("Users-russ-Code-app")
            .join(config::AGENT_TRANSCRIPTS_DIR)
            .join("11111111-1111-1111-1111-111111111111");
        let subagents = project.join(config::AGENT_SUBAGENTS_DIR);
        fs::create_dir_all(&subagents).unwrap();
        fs::write(
            project.join("11111111-1111-1111-1111-111111111111.jsonl"),
            "",
        )
        .unwrap();
        fs::write(
            subagents.join("22222222-2222-2222-2222-222222222222.jsonl"),
            "",
        )
        .unwrap();
        fs::write(project.join("ignored.md"), "").unwrap();

        let sources = discover_agent_transcripts_in(dir.path()).unwrap();
        assert_eq!(sources.len(), 2);
        assert!(sources.iter().all(|s| s.tool == config::TOOL_ID));
        assert!(sources.iter().all(|s| s.project == "/Users/russ/Code/app"));
        assert!(sources.iter().any(|s| s
            .path
            .ends_with("22222222-2222-2222-2222-222222222222.jsonl")));
    }

    #[test]
    fn decodes_sanitized_user_home_from_cursor_project_id() {
        assert_eq!(
            project_from_folder_id_with_home(
                "Users-russ-mckendrick-Code-M365",
                Some(Path::new("/Users/russ.mckendrick"))
            ),
            "/Users/russ.mckendrick/Code/M365"
        );
    }

    #[test]
    fn resolves_ambiguous_project_id_to_existing_nested_path() {
        let dir = TempDir::new();
        let root = dir.path().join("Code");
        let nested = root.join("Octo").join("Bot-Two-Point-Oh");
        fs::create_dir_all(&nested).unwrap();

        assert_eq!(
            existing_project_path(&root, "Octo-Bot-Two-Point-Oh").as_deref(),
            Some(nested.as_path())
        );
    }
}
