use std::collections::HashSet;
use std::fs::{self, Metadata};
use std::path::Path;
use std::time::UNIX_EPOCH;

use color_eyre::{eyre::Context, Result};
use walkdir::WalkDir;

pub mod claude_code;
pub mod claude_subscription;
pub mod codex;
pub mod codex_subscription;
pub mod copilot;
pub mod cursor;
pub mod gemini;
pub mod jsonl;
pub mod paths;
pub mod types;

pub use types::{
    AdapterParse, CodeBlock, InteractionMode, LimitCredits, LimitSnapshot, LimitWindow, ParsedCall,
    SessionSource, SessionSourceKind, Speed, TimestampQuality, TokenQuality,
};

/// A candidate data location `tokenuse doctor` reports on. The label is a
/// short lowercase identifier for the kind of location (matching how the
/// adapter's config names it), not display prose.
#[derive(Debug, Clone)]
pub struct ProbeRoot {
    pub label: &'static str,
    pub path: std::path::PathBuf,
}

impl ProbeRoot {
    pub fn new(label: &'static str, path: std::path::PathBuf) -> Self {
        Self { label, path }
    }
}

pub trait ToolAdapter: Send + Sync {
    fn id(&self) -> &'static str;
    fn display_name(&self) -> &'static str;

    fn discover(&self) -> Result<Vec<SessionSource>>;

    fn parse(&self, source: &SessionSource, seen: &mut HashSet<String>) -> Result<Vec<ParsedCall>>;

    /// Incremental parse: `cursor` is the JSON this adapter returned for the
    /// same source on a previous sync (persisted in `source_state`), letting
    /// it skip already-parsed bytes. The default ignores cursors and parses
    /// fully; adapters whose sources are append-only may override.
    fn parse_with_cursor(
        &self,
        source: &SessionSource,
        seen: &mut HashSet<String>,
        _cursor: Option<&str>,
    ) -> Result<AdapterParse> {
        Ok(AdapterParse::full(self.parse(source, seen)?))
    }

    fn parse_limits(&self, _source: &SessionSource) -> Result<Vec<LimitSnapshot>> {
        Ok(Vec::new())
    }

    fn source_fingerprint(&self, source: &SessionSource) -> Result<String> {
        fingerprint_source(source)
    }

    fn tool_display(&self, tool: &str) -> String {
        tool.to_string()
    }

    /// Candidate data locations for diagnostics (`tokenuse doctor`).
    fn probe_roots(&self) -> Vec<ProbeRoot> {
        Vec::new()
    }

    /// Environment variables that change where this adapter looks for data.
    fn env_overrides(&self) -> &'static [&'static str] {
        &[]
    }
}

pub fn registry() -> Vec<Box<dyn ToolAdapter>> {
    vec![
        Box::new(claude_code::ClaudeCode),
        Box::new(claude_subscription::ClaudeSubscription),
        Box::new(cursor::Cursor),
        Box::new(codex::Codex),
        Box::new(codex_subscription::CodexSubscription),
        Box::new(copilot::Copilot),
        Box::new(gemini::Gemini),
    ]
}

pub fn fingerprint_source(source: &SessionSource) -> Result<String> {
    let path = &source.path;
    if path.is_file() {
        return fingerprint_file(path);
    }
    if path.is_dir() {
        return fingerprint_dir(path);
    }
    Ok(format!("missing:{}", path.display()))
}

fn fingerprint_file(path: &Path) -> Result<String> {
    let metadata = fs::metadata(path).wrap_err_with(|| format!("stat {}", path.display()))?;
    Ok(format!(
        "file:{}:{}:{}",
        path.display(),
        metadata.len(),
        modified_nanos(&metadata)
    ))
}

fn fingerprint_dir(path: &Path) -> Result<String> {
    let mut entries = Vec::new();
    for entry in WalkDir::new(path).follow_links(false).into_iter().flatten() {
        if !entry.file_type().is_file() {
            continue;
        }
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        let rel = entry
            .path()
            .strip_prefix(path)
            .unwrap_or_else(|_| entry.path())
            .to_string_lossy();
        entries.push(format!(
            "{}:{}:{}",
            rel,
            metadata.len(),
            modified_nanos(&metadata)
        ));
    }
    entries.sort();
    Ok(format!("dir:{}:{}", path.display(), entries.join("|")))
}

fn modified_nanos(metadata: &Metadata) -> u128 {
    metadata
        .modified()
        .ok()
        .and_then(|mtime| mtime.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos())
        .unwrap_or(0)
}
