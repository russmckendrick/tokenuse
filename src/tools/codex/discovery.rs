use color_eyre::Result;
use walkdir::WalkDir;

use crate::tools::SessionSource;

use super::config;

pub fn discover() -> Result<Vec<SessionSource>> {
    let Some(root) = config::sessions_root() else {
        return Ok(Vec::new());
    };
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for entry in WalkDir::new(&root)
        .follow_links(false)
        .into_iter()
        .flatten()
    {
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
            .strip_prefix(&root)
            .ok()
            .and_then(|p| p.parent())
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        out.push(SessionSource {
            path: path.to_path_buf(),
            project,
            tool: config::TOOL_ID,
        });
    }
    Ok(out)
}
