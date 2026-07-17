//! Advisory setup findings: configuration waste visible from local Claude
//! Code config files plus the archived call history. Deliberately unscored —
//! these describe the setup as it is now, not practice over time, so they
//! sit beside the practice groups instead of inside them and never move the
//! grade. All estimates are labelled heuristics; the per-unit token costs
//! mirror upstream-observed figures (schema ~400 tokens per MCP tool, ~5
//! tools per server, ~600 tokens per file read, ~13 tokens per CLAUDE.md
//! line).

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::copy::{copy, template};
use crate::data::CoachSetupFinding;
use crate::tools::ParsedCall;

const TOKENS_PER_MCP_TOOL: u64 = 400;
const TOOLS_PER_MCP_SERVER: u64 = 5;
const TOKENS_PER_READ: u64 = 600;
const TOKENS_PER_CLAUDE_MD_LINE: u64 = 13;
const CLAUDE_MD_LINE_THRESHOLD: u64 = 200;
const IMPORT_DEPTH_LIMIT: usize = 5;
const MIN_DUPLICATE_READS: u64 = 5;
const MIN_JUNK_READS: u64 = 3;
const MAX_LISTED_ITEMS: usize = 6;

const JUNK_DIR_COMPONENTS: &[&str] = &[
    "node_modules",
    ".git",
    "dist",
    "build",
    "target",
    "__pycache__",
    ".next",
    ".nuxt",
    ".output",
    "coverage",
    ".cache",
    ".venv",
    "venv",
];

#[derive(Debug, Clone, Default)]
pub struct SetupScan {
    pub mcp_servers: Vec<McpServer>,
    pub claude_md_files: Vec<ClaudeMdFile>,
}

#[derive(Debug, Clone)]
pub struct McpServer {
    pub name: String,
    pub source: String,
}

#[derive(Debug, Clone)]
pub struct ClaudeMdFile {
    pub path: String,
    /// Line count with `@import`ed files inlined (depth-limited).
    pub expanded_lines: u64,
}

/// Live-only filesystem scan over the user's Claude Code configuration and
/// the CLAUDE.md files of recently active project roots. Sample mode passes
/// an empty scan.
pub fn scan(project_roots: &[PathBuf]) -> SetupScan {
    let mut scan = SetupScan::default();
    let home = crate::tools::paths::home();

    if let Some(home) = &home {
        collect_mcp_servers(
            &home.join(".claude.json"),
            &["mcpServers"],
            true,
            &mut scan.mcp_servers,
        );
        collect_mcp_servers(
            &home.join(".claude").join("settings.json"),
            &["mcpServers"],
            false,
            &mut scan.mcp_servers,
        );
        collect_claude_md(&home.join(".claude").join("CLAUDE.md"), &mut scan);
    }
    for root in project_roots {
        collect_mcp_servers(
            &root.join(".mcp.json"),
            &["mcpServers"],
            false,
            &mut scan.mcp_servers,
        );
        collect_claude_md(&root.join("CLAUDE.md"), &mut scan);
        collect_claude_md(&root.join(".claude").join("CLAUDE.md"), &mut scan);
    }

    // One row per server name; keep the first source that declared it.
    let mut seen = BTreeSet::new();
    scan.mcp_servers
        .retain(|server| seen.insert(normalize_server(&server.name)));
    scan
}

fn collect_mcp_servers(
    path: &Path,
    keys: &[&str],
    include_project_entries: bool,
    out: &mut Vec<McpServer>,
) {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return;
    };
    let Ok(value) = serde_json::from_str::<Value>(&raw) else {
        return;
    };
    let source = path.display().to_string();
    for key in keys {
        if let Some(servers) = value.get(key).and_then(|v| v.as_object()) {
            for name in servers.keys() {
                out.push(McpServer {
                    name: name.clone(),
                    source: source.clone(),
                });
            }
        }
    }
    if include_project_entries {
        if let Some(projects) = value.get("projects").and_then(|v| v.as_object()) {
            for project in projects.values() {
                if let Some(servers) = project.get("mcpServers").and_then(|v| v.as_object()) {
                    for name in servers.keys() {
                        out.push(McpServer {
                            name: name.clone(),
                            source: source.clone(),
                        });
                    }
                }
            }
        }
    }
}

fn collect_claude_md(path: &Path, scan: &mut SetupScan) {
    if !path.is_file() {
        return;
    }
    let mut visited = HashSet::new();
    let lines = expanded_line_count(path, 0, &mut visited);
    if lines > 0 {
        scan.claude_md_files.push(ClaudeMdFile {
            path: path.display().to_string(),
            expanded_lines: lines,
        });
    }
}

/// Claude Code inlines `@path` lines into the context; count what actually
/// rides along, depth-limited and cycle-guarded.
fn expanded_line_count(path: &Path, depth: usize, visited: &mut HashSet<PathBuf>) -> u64 {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    if !visited.insert(canonical) {
        return 0;
    }
    let Ok(raw) = std::fs::read_to_string(path) else {
        return 0;
    };
    let base = path.parent().unwrap_or(Path::new("."));
    let mut lines = 0u64;
    for line in raw.lines() {
        lines += 1;
        if depth >= IMPORT_DEPTH_LIMIT {
            continue;
        }
        let trimmed = line.trim();
        let Some(target) = trimmed.strip_prefix('@') else {
            continue;
        };
        let target = target.split_whitespace().next().unwrap_or("");
        if target.is_empty() {
            continue;
        }
        let resolved = if let Some(rest) = target.strip_prefix("~/") {
            match crate::tools::paths::home() {
                Some(home) => home.join(rest),
                None => continue,
            }
        } else {
            base.join(target)
        };
        if resolved.is_file() {
            lines += expanded_line_count(&resolved, depth + 1, visited);
        }
    }
    lines
}

fn normalize_server(name: &str) -> String {
    name.to_ascii_lowercase().replace('-', "_")
}

pub fn findings(calls: &[&ParsedCall], scan: &SetupScan) -> Vec<CoachSetupFinding> {
    let mut findings = Vec::new();
    findings.extend(unused_mcp_servers(calls, scan));
    findings.extend(bloated_claude_md(scan));
    findings.extend(duplicate_reads(calls));
    findings.extend(junk_reads(calls));
    findings.sort_by_key(|finding| std::cmp::Reverse(finding.savings_tokens));
    findings
}

fn unused_mcp_servers(calls: &[&ParsedCall], scan: &SetupScan) -> Option<CoachSetupFinding> {
    if scan.mcp_servers.is_empty() {
        return None;
    }
    let invoked: BTreeSet<String> = calls
        .iter()
        .flat_map(|call| call.tools.iter())
        .filter_map(|tool| tool.strip_prefix("mcp__"))
        .filter_map(|rest| rest.split("__").next())
        .map(normalize_server)
        .collect();
    let unused: Vec<&McpServer> = scan
        .mcp_servers
        .iter()
        .filter(|server| !invoked.contains(&normalize_server(&server.name)))
        .collect();
    if unused.is_empty() {
        return None;
    }
    let sessions = calls
        .iter()
        .filter_map(|call| crate::ingest::session_key(call))
        .collect::<HashSet<_>>()
        .len() as u64;
    let savings =
        unused.len() as u64 * TOOLS_PER_MCP_SERVER * TOKENS_PER_MCP_TOOL * sessions.max(1);
    let setup_copy = &copy().coach.setup;
    Some(build_finding(
        "unused-mcp-servers",
        template(
            &setup_copy.unused_mcp_title,
            &[("count", unused.len().to_string())],
        ),
        template(
            &setup_copy.unused_mcp_detail,
            &[("names", listed(unused.iter().map(|s| s.name.clone())))],
        ),
        savings,
    ))
}

fn bloated_claude_md(scan: &SetupScan) -> Option<CoachSetupFinding> {
    let bloated: Vec<&ClaudeMdFile> = scan
        .claude_md_files
        .iter()
        .filter(|file| file.expanded_lines > CLAUDE_MD_LINE_THRESHOLD)
        .collect();
    if bloated.is_empty() {
        return None;
    }
    let excess: u64 = bloated
        .iter()
        .map(|file| file.expanded_lines - CLAUDE_MD_LINE_THRESHOLD)
        .sum();
    let setup_copy = &copy().coach.setup;
    Some(build_finding(
        "claude-md-bloat",
        template(
            &setup_copy.claude_md_title,
            &[("threshold", CLAUDE_MD_LINE_THRESHOLD.to_string())],
        ),
        template(
            &setup_copy.claude_md_detail,
            &[(
                "files",
                listed(
                    bloated
                        .iter()
                        .map(|f| format!("{} ({})", short_path(&f.path), f.expanded_lines)),
                ),
            )],
        ),
        excess * TOKENS_PER_CLAUDE_MD_LINE,
    ))
}

fn duplicate_reads(calls: &[&ParsedCall]) -> Option<CoachSetupFinding> {
    let mut per_session: BTreeMap<String, BTreeMap<&str, u64>> = BTreeMap::new();
    for call in calls {
        let Some(session) = crate::ingest::session_key(call) else {
            continue;
        };
        let entry = per_session.entry(session).or_default();
        for file in &call.referenced_files {
            if is_junk_path(file) {
                continue;
            }
            *entry.entry(file.as_str()).or_default() += 1;
        }
    }
    let extras: u64 = per_session
        .values()
        .flat_map(|files| files.values())
        .map(|count| count.saturating_sub(1))
        .sum();
    if extras < MIN_DUPLICATE_READS {
        return None;
    }
    let setup_copy = &copy().coach.setup;
    Some(build_finding(
        "redundant-rereads",
        template(&setup_copy.rereads_title, &[("count", extras.to_string())]),
        setup_copy.rereads_detail.clone(),
        extras * TOKENS_PER_READ,
    ))
}

fn junk_reads(calls: &[&ParsedCall]) -> Option<CoachSetupFinding> {
    let mut count = 0u64;
    let mut dirs: BTreeSet<&str> = BTreeSet::new();
    for call in calls {
        for file in &call.referenced_files {
            if let Some(component) = junk_component(file) {
                count += 1;
                dirs.insert(component);
            }
        }
    }
    if count < MIN_JUNK_READS {
        return None;
    }
    let setup_copy = &copy().coach.setup;
    Some(build_finding(
        "junk-directory-reads",
        template(&setup_copy.junk_title, &[("count", count.to_string())]),
        template(
            &setup_copy.junk_detail,
            &[("dirs", listed(dirs.iter().map(|d| d.to_string())))],
        ),
        count * TOKENS_PER_READ,
    ))
}

fn is_junk_path(path: &str) -> bool {
    junk_component(path).is_some()
}

fn junk_component(path: &str) -> Option<&'static str> {
    Path::new(path).components().find_map(|component| {
        let name = component.as_os_str().to_str()?;
        JUNK_DIR_COMPONENTS
            .iter()
            .find(|junk| **junk == name)
            .copied()
    })
}

fn listed(items: impl Iterator<Item = String>) -> String {
    let all: Vec<String> = items.collect();
    if all.len() > MAX_LISTED_ITEMS {
        format!("{}, …", all[..MAX_LISTED_ITEMS].join(", "))
    } else {
        all.join(", ")
    }
}

fn short_path(path: &str) -> String {
    match crate::tools::paths::home() {
        Some(home) => path
            .strip_prefix(&format!("{}/", home.display()))
            .map(|rest| format!("~/{rest}"))
            .unwrap_or_else(|| path.to_string()),
        None => path.to_string(),
    }
}

fn build_finding(
    id: &'static str,
    title: String,
    detail: String,
    savings_tokens: u64,
) -> CoachSetupFinding {
    let savings_label = super::leak(template(
        &copy().coach.setup.savings,
        &[("tokens", compact_tokens(savings_tokens))],
    ));
    CoachSetupFinding {
        id,
        title: super::leak(title),
        detail: super::leak(detail),
        savings_tokens,
        savings_label,
    }
}

fn compact_tokens(n: u64) -> String {
    if n >= 1_000_000_000 {
        format!("{:.1}B", n as f64 / 1_000_000_000.0)
    } else if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(session: &str, tools: &[&str], referenced: &[&str]) -> ParsedCall {
        ParsedCall {
            tool: "claude-code",
            session_id: session.into(),
            tools: tools.iter().map(|t| t.to_string()).collect(),
            referenced_files: referenced.iter().map(|f| f.to_string()).collect(),
            ..ParsedCall::default()
        }
    }

    #[test]
    fn unused_servers_are_flagged_with_session_scaled_savings() {
        let scan = SetupScan {
            mcp_servers: vec![
                McpServer {
                    name: "codebase-memory-mcp".into(),
                    source: "~/.claude.json".into(),
                },
                McpServer {
                    name: "ghost-server".into(),
                    source: "~/.claude.json".into(),
                },
            ],
            claude_md_files: Vec::new(),
        };
        // Invoked with underscores (Codex-style) while configured with
        // dashes: normalization must join them.
        let calls = [
            call("s1", &["mcp__codebase_memory_mcp__search_graph"], &[]),
            call("s2", &[], &[]),
        ];
        let refs: Vec<&ParsedCall> = calls.iter().collect();

        let findings = findings(&refs, &scan);
        let unused = findings
            .iter()
            .find(|f| f.id == "unused-mcp-servers")
            .expect("ghost server flagged");
        assert!(unused.detail.contains("ghost-server"));
        assert!(!unused.detail.contains("codebase-memory-mcp"));
        assert_eq!(unused.savings_tokens, 5 * 400 * 2);
    }

    #[test]
    fn duplicate_and_junk_reads_come_from_referenced_files() {
        let calls = [
            call(
                "s1",
                &[],
                &["/repo/src/lib.rs", "/repo/node_modules/x/index.js"],
            ),
            call("s1", &[], &["/repo/src/lib.rs"]),
            call("s1", &[], &["/repo/src/lib.rs"]),
            call("s1", &[], &["/repo/node_modules/y/index.js"]),
            call("s2", &[], &["/repo/src/lib.rs", "/repo/.git/config"]),
            call("s2", &[], &["/repo/src/lib.rs", "/repo/src/lib.rs"]),
        ];
        let refs: Vec<&ParsedCall> = calls.iter().collect();
        let all = findings(&refs, &SetupScan::default());

        // s1: lib.rs x3 => 2 extras; s2: lib.rs x3 (dup within one call
        // counts too) => 2 extras. Junk: 3 reads (2x node_modules, 1x .git).
        let rereads = all.iter().find(|f| f.id == "redundant-rereads");
        assert!(rereads.is_none(), "4 extras stay under the threshold of 5");
        let junk = all
            .iter()
            .find(|f| f.id == "junk-directory-reads")
            .expect("junk reads flagged");
        assert_eq!(junk.savings_tokens, 3 * 600);
        assert!(junk.detail.contains("node_modules"));
    }

    #[test]
    fn claude_md_expansion_follows_imports_with_cycle_guard() {
        let dir = std::env::temp_dir().join(format!(
            "tokenuse-setup-scan-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let main = dir.join("CLAUDE.md");
        let import = dir.join("RULES.md");
        // Cycle: RULES.md imports CLAUDE.md back.
        std::fs::write(&main, "line\n@RULES.md\nline\n").unwrap();
        std::fs::write(&import, "a\nb\n@CLAUDE.md\n").unwrap();

        let mut visited = HashSet::new();
        let lines = expanded_line_count(&main, 0, &mut visited);
        assert_eq!(lines, 6, "3 own lines + 3 imported, cycle counted once");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
