use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    fs,
    io::Read,
    path::{Path, PathBuf},
};

use chrono::{Duration, Utc};
use color_eyre::{eyre::Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use walkdir::WalkDir;

use crate::{
    config::ConfigPaths,
    ingest::{
        projects::{project_identity, project_label, project_label_lookup, tool_short_label},
        Ingested,
    },
    tools::{paths, ParsedCall},
};

pub const SCHEMA_VERSION: &str = "agent-setup-audit/v3";
const SCANNER_VERSION: &str = "3";
const KNOWLEDGE_CONTENT_LIMIT: usize = 24_000;
const MIN_KNOWLEDGE_CONTENT_LIMIT: usize = 2_000;
const MAX_STORED_SNAPSHOT_BYTES: usize = 200_000;
const MAX_KNOWLEDGE_FILES: usize = 8;
const MAX_ARRAY_ENTRIES: usize = 100;
const MAX_LOG_BYTES: u64 = 1_000_000;
const MAX_PROJECT_ROOTS_CHECKED: usize = 100;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AuditSnapshot {
    pub schema_version: String,
    pub scanner_version: String,
    pub captured_at: String,
    pub root: Option<String>,
    pub primary_tool_guess: Option<String>,
    pub redaction: AuditRedaction,
    pub tools: Vec<AuditToolSummary>,
    pub usage_summary: AuditUsageSummary,
    pub recent_usage: AuditUsageSummary,
    pub project_coverage: AuditProjectCoverage,
    pub activity_signals: AuditActivitySignals,
    pub behavior: AuditBehavior,
    pub summary: AuditSummary,
    pub findings: Vec<AuditFinding>,
    pub knowledge_files: Vec<AuditKnowledgeFile>,
}

impl AuditSnapshot {
    pub fn not_run(paths: &ConfigPaths) -> Self {
        Self {
            schema_version: SCHEMA_VERSION.into(),
            scanner_version: SCANNER_VERSION.into(),
            captured_at: String::new(),
            root: None,
            primary_tool_guess: None,
            redaction: AuditRedaction::default(),
            tools: Vec::new(),
            usage_summary: AuditUsageSummary::unavailable("all time"),
            recent_usage: AuditUsageSummary::unavailable("last 7 days"),
            project_coverage: AuditProjectCoverage::default(),
            activity_signals: AuditActivitySignals::default(),
            behavior: AuditBehavior::default(),
            summary: AuditSummary::default(),
            findings: vec![AuditFinding {
                id: "audit_not_run".into(),
                section: AuditSection::Readiness,
                severity: AuditSeverity::Info,
                title: "No setup audit captured yet".into(),
                body: "Run a local audit refresh to inspect agent home folders, tool knowledge files, and recent session hygiene.".into(),
                evidence: vec![display_path(&paths.agent_audit_file)],
                source_paths: Vec::new(),
            }],
            knowledge_files: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuditRedaction {
    pub enabled: bool,
    pub secrets_redacted: bool,
    pub home_paths_folded: bool,
}

impl Default for AuditRedaction {
    fn default() -> Self {
        Self {
            enabled: true,
            secrets_redacted: true,
            home_paths_folded: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct AuditSummary {
    pub total_findings: usize,
    pub security_findings: usize,
    pub efficiency_findings: usize,
    pub context_findings: usize,
    pub readiness_findings: usize,
    pub risk_findings: usize,
    pub warning_findings: usize,
    pub info_findings: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuditToolSummary {
    pub id: String,
    pub label: String,
    pub present: bool,
    pub config_paths: Vec<String>,
    pub mcp_servers: Vec<String>,
    pub hooks_count: usize,
    pub knowledge_files: usize,
    pub scoped_assets: usize,
    pub dangerous_alias_detected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AuditUsageSummary {
    pub available: bool,
    pub window_label: String,
    pub calls: u64,
    pub sessions: u64,
    pub cost_usd: f64,
    pub cost_label: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_creation_tokens: u64,
    pub cache_read_tokens: u64,
    pub total_tokens: u64,
    pub cache_read_ratio: Option<f64>,
    pub cache_hit_ratio: Option<f64>,
    pub top_tools: Vec<AuditRankedItem>,
    pub top_models: Vec<AuditRankedItem>,
    pub top_projects: Vec<AuditRankedItem>,
}

impl AuditUsageSummary {
    fn unavailable(window_label: &str) -> Self {
        Self {
            available: false,
            window_label: window_label.into(),
            calls: 0,
            sessions: 0,
            cost_usd: 0.0,
            cost_label: "-".into(),
            input_tokens: 0,
            output_tokens: 0,
            cache_creation_tokens: 0,
            cache_read_tokens: 0,
            total_tokens: 0,
            cache_read_ratio: None,
            cache_hit_ratio: None,
            top_tools: Vec::new(),
            top_models: Vec::new(),
            top_projects: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AuditRankedItem {
    pub name: String,
    pub calls: u64,
    pub sessions: u64,
    pub cost_usd: f64,
    pub cost_label: String,
    pub tokens: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct AuditProjectCoverage {
    pub available: bool,
    pub known_project_roots: usize,
    pub checked_project_roots: usize,
    pub roots_with_agent_instructions: usize,
    pub roots_with_ci: usize,
    pub roots_with_manifests: usize,
    pub skipped_project_roots: usize,
    pub omitted_project_roots: usize,
    pub entries: Vec<AuditProjectCoverageEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct AuditProjectCoverageEntry {
    pub label: String,
    pub path: String,
    pub calls: u64,
    pub sessions: u64,
    pub agent_files: Vec<String>,
    pub has_ci: bool,
    pub has_manifest: bool,
    pub checked: bool,
    pub skipped_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct AuditActivitySignals {
    pub available: bool,
    pub tool_call_uses: u64,
    pub shell_command_uses: u64,
    pub mcp_tool_uses: u64,
    pub distinct_tools_used: usize,
    pub distinct_models_used: usize,
    pub high_cost_projects: Vec<String>,
    pub high_cost_sessions: Vec<String>,
    pub repeated_model_patterns: Vec<String>,
    pub repeated_tool_patterns: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct AuditBehavior {
    pub recent_sessions_inspected: usize,
    pub clear_uses: Option<u64>,
    pub compact_uses: Option<u64>,
    pub subagent_calls: Option<u64>,
    pub plan_mode_uses: Option<u64>,
    pub skill_invocations: Option<u64>,
    pub longest_session_turns_without_reset: Option<u64>,
    pub avg_user_turn_chars: Option<f64>,
    pub correction_turn_ratio: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuditKnowledgeFile {
    pub path: String,
    pub exists: bool,
    pub size_bytes: u64,
    pub line_count: usize,
    pub content_preview: String,
    pub content_truncated: bool,
    pub feature_flags: AuditKnowledgeFlags,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct AuditKnowledgeFlags {
    pub mentions_testing: bool,
    pub mentions_security: bool,
    pub mentions_secrets: bool,
    pub has_wrong_right_patterns: bool,
    pub has_command_table: bool,
    pub has_dont_section: bool,
    pub has_external_links: bool,
    pub style_keywords: Vec<String>,
    pub imports_other_files: bool,
    pub imported_paths: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuditSection {
    Security,
    Efficiency,
    Context,
    Readiness,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuditSeverity {
    Risk,
    Warning,
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuditFinding {
    pub id: String,
    pub section: AuditSection,
    pub severity: AuditSeverity,
    pub title: String,
    pub body: String,
    pub evidence: Vec<String>,
    pub source_paths: Vec<String>,
}

struct ScanContext {
    root: PathBuf,
    home: Option<PathBuf>,
}

#[derive(Default)]
struct BehaviorAccumulator {
    logs_inspected: usize,
    clear_uses: u64,
    compact_uses: u64,
    subagent_calls: u64,
    plan_mode_uses: u64,
    skill_invocations: u64,
    longest_without_reset: u64,
    user_turn_chars: u64,
    user_turns: u64,
    correction_turns: u64,
}

struct FindingInputs<'a> {
    usage_summary: &'a AuditUsageSummary,
    recent_usage: &'a AuditUsageSummary,
    project_coverage: &'a AuditProjectCoverage,
    activity_signals: &'a AuditActivitySignals,
    behavior: &'a AuditBehavior,
    knowledge_files: &'a [AuditKnowledgeFile],
}

pub fn load_latest(paths: &ConfigPaths) -> Option<AuditSnapshot> {
    let text = fs::read_to_string(&paths.agent_audit_file).ok()?;
    let snapshot = serde_json::from_str::<AuditSnapshot>(&text).ok()?;
    (snapshot.schema_version == SCHEMA_VERSION).then_some(snapshot)
}

pub fn refresh(paths: &ConfigPaths, ingested: Option<&Ingested>) -> Result<AuditSnapshot> {
    let snapshot = scan(ingested)?;
    let snapshot = cap_snapshot_payload(sanitize_snapshot(snapshot)?)?;
    paths.ensure_dir()?;
    let json = serde_json::to_string_pretty(&snapshot)?;
    fs::write(&paths.agent_audit_file, json)
        .wrap_err_with(|| format!("write {}", paths.agent_audit_file.display()))?;
    Ok(snapshot)
}

pub fn scan(ingested: Option<&Ingested>) -> Result<AuditSnapshot> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let root = nearest_git_root(&cwd).unwrap_or(cwd);
    let ctx = ScanContext {
        root,
        home: paths::home(),
    };

    let mut knowledge_files = collect_knowledge_files(&ctx);
    knowledge_files.truncate(MAX_KNOWLEDGE_FILES);

    let mut tools = vec![
        collect_claude(&ctx, &knowledge_files),
        collect_cursor(&ctx, &knowledge_files),
        collect_codex(&ctx, &knowledge_files),
        collect_copilot(&ctx, &knowledge_files),
        collect_gemini(&ctx, &knowledge_files),
    ];
    tools.iter_mut().for_each(limit_tool_summary);

    let usage_summary = collect_usage_summary(ingested, None, "all time");
    let recent_usage = collect_usage_summary(ingested, Some(Duration::days(7)), "last 7 days");
    let project_coverage = collect_project_coverage(ingested);
    let activity_signals = collect_activity_signals(ingested);
    let behavior = collect_behavior(&ctx);
    let primary_tool_guess = infer_primary_tool(&tools);
    let mut findings = build_findings(
        &ctx,
        &tools,
        FindingInputs {
            usage_summary: &usage_summary,
            recent_usage: &recent_usage,
            project_coverage: &project_coverage,
            activity_signals: &activity_signals,
            behavior: &behavior,
            knowledge_files: &knowledge_files,
        },
    );
    findings.truncate(100);
    let summary = summarize_findings(&findings);

    Ok(AuditSnapshot {
        schema_version: SCHEMA_VERSION.into(),
        scanner_version: SCANNER_VERSION.into(),
        captured_at: Utc::now().to_rfc3339(),
        root: Some(display_path(&ctx.root)),
        primary_tool_guess,
        redaction: AuditRedaction::default(),
        tools,
        usage_summary,
        recent_usage,
        project_coverage,
        activity_signals,
        behavior,
        summary,
        findings,
        knowledge_files,
    })
}

fn sanitize_snapshot(snapshot: AuditSnapshot) -> Result<AuditSnapshot> {
    let mut value = serde_json::to_value(snapshot)?;
    redact_value(&mut value);
    Ok(serde_json::from_value(value)?)
}

fn cap_snapshot_payload(mut snapshot: AuditSnapshot) -> Result<AuditSnapshot> {
    let mut per_file_limit = KNOWLEDGE_CONTENT_LIMIT;
    while serde_json::to_vec_pretty(&snapshot)?.len() > MAX_STORED_SNAPSHOT_BYTES
        && per_file_limit > MIN_KNOWLEDGE_CONTENT_LIMIT
    {
        per_file_limit = (per_file_limit / 2).max(MIN_KNOWLEDGE_CONTENT_LIMIT);
        for file in &mut snapshot.knowledge_files {
            if file.content_preview.chars().count() > per_file_limit {
                file.content_preview = file.content_preview.chars().take(per_file_limit).collect();
                file.content_truncated = true;
            }
        }
    }
    Ok(snapshot)
}

fn collect_claude(ctx: &ScanContext, knowledge_files: &[AuditKnowledgeFile]) -> AuditToolSummary {
    let Some(home) = &ctx.home else {
        return missing_tool("claude-code", "Claude Code");
    };
    let tool_home = home.join(".claude");
    let config_files = existing_pathbufs(&[
        tool_home.join("settings.json"),
        tool_home.join("settings.local.json"),
        home.join(".claude.json"),
    ]);

    let mut mcp_servers = Vec::new();
    for path in &config_files {
        mcp_servers.extend(json_mcp_servers(path));
    }
    sort_dedup_truncate(&mut mcp_servers);

    let hooks_count = config_files
        .iter()
        .map(|path| json_object_count(path, "hooks"))
        .sum();

    let scoped_assets = count_dir_entries(&tool_home.join("agents"))
        + count_dir_entries(&tool_home.join("commands"))
        + count_dir_entries(&tool_home.join("skills"))
        + count_dir_entries(&tool_home.join("rules"));
    let knowledge_count = knowledge_files
        .iter()
        .filter(|file| file.path.contains(".claude/"))
        .count();

    let dangerous_alias_detected =
        shell_or_project_files_contain(ctx, "--dangerously-skip-permissions");
    let present = tool_home.exists() || !config_files.is_empty() || knowledge_count > 0;

    AuditToolSummary {
        id: "claude-code".into(),
        label: "Claude Code".into(),
        present,
        config_paths: display_paths(&config_files),
        mcp_servers,
        hooks_count,
        knowledge_files: knowledge_count,
        scoped_assets,
        dangerous_alias_detected,
    }
}

fn collect_cursor(ctx: &ScanContext, knowledge_files: &[AuditKnowledgeFile]) -> AuditToolSummary {
    let Some(home) = &ctx.home else {
        return missing_tool("cursor", "Cursor");
    };
    let tool_home = home.join(".cursor");
    let config_files = existing_pathbufs(&[
        tool_home.join("mcp.json"),
        tool_home.join("settings.json"),
        tool_home.join("permissions.json"),
    ]);
    let mut mcp_servers = Vec::new();
    for path in &config_files {
        mcp_servers.extend(json_mcp_servers(path));
    }
    sort_dedup_truncate(&mut mcp_servers);

    let scoped_assets =
        count_dir_entries(&tool_home.join("rules")) + count_dir_entries(&tool_home.join("skills"));
    let knowledge_count = knowledge_files
        .iter()
        .filter(|file| file.path.contains(".cursor/"))
        .count();
    let present = tool_home.exists() || !config_files.is_empty() || knowledge_count > 0;

    AuditToolSummary {
        id: "cursor".into(),
        label: "Cursor".into(),
        present,
        config_paths: display_paths(&config_files),
        mcp_servers,
        hooks_count: 0,
        knowledge_files: knowledge_count,
        scoped_assets,
        dangerous_alias_detected: false,
    }
}

fn collect_codex(ctx: &ScanContext, knowledge_files: &[AuditKnowledgeFile]) -> AuditToolSummary {
    let Some(home) = &ctx.home else {
        return missing_tool("codex", "Codex");
    };
    let tool_home = std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".codex"));
    let config_files = existing_pathbufs(&[tool_home.join("config.toml")]);

    let mut mcp_servers = Vec::new();
    for path in &config_files {
        mcp_servers.extend(toml_mcp_servers(path));
    }
    sort_dedup_truncate(&mut mcp_servers);

    let knowledge_count = knowledge_files
        .iter()
        .filter(|file| file.path.contains(".codex/"))
        .count();
    let dangerous_alias_detected = shell_or_project_files_contain(ctx, "--yolo")
        || shell_or_project_files_contain(ctx, "--full-auto")
        || shell_or_project_files_contain(ctx, "--dangerously-bypass-approvals-and-sandbox");

    AuditToolSummary {
        id: "codex".into(),
        label: "Codex".into(),
        present: tool_home.exists() || !config_files.is_empty() || knowledge_count > 0,
        config_paths: display_paths(&config_files),
        mcp_servers,
        hooks_count: config_files.iter().map(|path| toml_hooks_count(path)).sum(),
        knowledge_files: knowledge_count,
        scoped_assets: count_dir_entries(&tool_home.join("skills"))
            + count_dir_entries(&tool_home.join("agents"))
            + count_dir_entries(&tool_home.join("rules")),
        dangerous_alias_detected,
    }
}

fn collect_copilot(ctx: &ScanContext, knowledge_files: &[AuditKnowledgeFile]) -> AuditToolSummary {
    let Some(home) = &ctx.home else {
        return missing_tool("copilot", "Copilot");
    };
    let tool_home = home.join(".copilot");
    let config_files = existing_pathbufs(&[
        tool_home.join("config.json"),
        tool_home.join("settings.json"),
        tool_home.join("mcp.json"),
    ]);
    let mut mcp_servers = Vec::new();
    for path in &config_files {
        mcp_servers.extend(json_mcp_servers(path));
    }
    sort_dedup_truncate(&mut mcp_servers);

    let knowledge_count = knowledge_files
        .iter()
        .filter(|file| file.path.contains(".copilot/"))
        .count();

    AuditToolSummary {
        id: "copilot".into(),
        label: "Copilot".into(),
        present: tool_home.exists() || !config_files.is_empty() || knowledge_count > 0,
        config_paths: display_paths(&config_files),
        mcp_servers,
        hooks_count: 0,
        knowledge_files: knowledge_count,
        scoped_assets: count_dir_entries(&tool_home.join("instructions"))
            + count_dir_entries(&tool_home.join("skills"))
            + count_dir_entries(&tool_home.join("rules")),
        dangerous_alias_detected: false,
    }
}

fn collect_gemini(ctx: &ScanContext, knowledge_files: &[AuditKnowledgeFile]) -> AuditToolSummary {
    let Some(home) = &ctx.home else {
        return missing_tool("gemini", "Gemini");
    };
    let tool_home = home.join(".gemini");
    let config_files = existing_pathbufs(&[
        tool_home.join("settings.json"),
        tool_home.join("config.json"),
        tool_home.join("config.toml"),
    ]);
    let mut mcp_servers = Vec::new();
    for path in &config_files {
        mcp_servers.extend(json_mcp_servers(path));
        mcp_servers.extend(toml_mcp_servers(path));
    }
    sort_dedup_truncate(&mut mcp_servers);

    let knowledge_count = knowledge_files
        .iter()
        .filter(|file| file.path.contains(".gemini/"))
        .count();

    AuditToolSummary {
        id: "gemini".into(),
        label: "Gemini".into(),
        present: tool_home.exists() || !config_files.is_empty() || knowledge_count > 0,
        config_paths: display_paths(&config_files),
        mcp_servers,
        hooks_count: 0,
        knowledge_files: knowledge_count,
        scoped_assets: count_dir_entries(&tool_home.join("extensions"))
            + count_dir_entries(&tool_home.join("skills"))
            + count_dir_entries(&tool_home.join("rules")),
        dangerous_alias_detected: false,
    }
}

fn missing_tool(id: &str, label: &str) -> AuditToolSummary {
    AuditToolSummary {
        id: id.into(),
        label: label.into(),
        present: false,
        config_paths: Vec::new(),
        mcp_servers: Vec::new(),
        hooks_count: 0,
        knowledge_files: 0,
        scoped_assets: 0,
        dangerous_alias_detected: false,
    }
}

fn collect_knowledge_files(ctx: &ScanContext) -> Vec<AuditKnowledgeFile> {
    let mut paths = Vec::new();
    if let Some(home) = &ctx.home {
        collect_claude_knowledge_paths(&home.join(".claude"), &mut paths);
        collect_codex_knowledge_paths(
            &std::env::var_os("CODEX_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".codex")),
            &mut paths,
        );
        collect_cursor_knowledge_paths(&home.join(".cursor"), &mut paths);
        collect_simple_tool_knowledge_paths(&home.join(".copilot"), &mut paths);
        collect_gemini_knowledge_paths(&home.join(".gemini"), &mut paths);
    }

    let mut seen = BTreeSet::new();
    paths
        .into_iter()
        .filter(|path| path.exists())
        .filter(|path| seen.insert(path.clone()))
        .filter_map(|path| knowledge_file(&path))
        .collect()
}

fn collect_claude_knowledge_paths(tool_home: &Path, paths: &mut Vec<PathBuf>) {
    paths.push(tool_home.join("CLAUDE.md"));
    paths.extend(markdown_files(&tool_home.join("agents"), 2));
    paths.extend(markdown_files(&tool_home.join("commands"), 3));
    paths.extend(skill_files(&tool_home.join("skills")));
    paths.extend(markdown_files(&tool_home.join("rules"), 2));
}

fn collect_codex_knowledge_paths(tool_home: &Path, paths: &mut Vec<PathBuf>) {
    paths.push(tool_home.join("AGENTS.md"));
    paths.extend(skill_files(&tool_home.join("skills")));
    paths.extend(markdown_files(&tool_home.join("agents"), 2));
    paths.extend(markdown_files(&tool_home.join("rules"), 2));
}

fn collect_cursor_knowledge_paths(tool_home: &Path, paths: &mut Vec<PathBuf>) {
    paths.push(tool_home.join("AGENTS.md"));
    paths.push(tool_home.join(".cursorrules"));
    paths.extend(markdown_files(&tool_home.join("rules"), 3));
    paths.extend(skill_files(&tool_home.join("skills")));
}

fn collect_gemini_knowledge_paths(tool_home: &Path, paths: &mut Vec<PathBuf>) {
    paths.push(tool_home.join("GEMINI.md"));
    paths.extend(markdown_files(&tool_home.join("extensions"), 3));
    paths.extend(markdown_files(&tool_home.join("rules"), 2));
    paths.extend(skill_files(&tool_home.join("skills")));
}

fn collect_simple_tool_knowledge_paths(tool_home: &Path, paths: &mut Vec<PathBuf>) {
    paths.push(tool_home.join("AGENTS.md"));
    paths.push(tool_home.join("instructions.md"));
    paths.push(tool_home.join("copilot-instructions.md"));
    paths.extend(markdown_files(&tool_home.join("instructions"), 2));
    paths.extend(markdown_files(&tool_home.join("rules"), 2));
    paths.extend(skill_files(&tool_home.join("skills")));
}

fn markdown_files(dir: &Path, max_depth: usize) -> Vec<PathBuf> {
    if !dir.exists() {
        return Vec::new();
    }
    let mut files = WalkDir::new(dir)
        .max_depth(max_depth)
        .into_iter()
        .flatten()
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| entry.path().to_path_buf())
        .filter(|path| {
            matches!(
                path.extension().and_then(|ext| ext.to_str()),
                Some("md" | "mdc" | "txt")
            )
        })
        .collect::<Vec<_>>();
    files.sort();
    files.truncate(MAX_ARRAY_ENTRIES);
    files
}

fn skill_files(dir: &Path) -> Vec<PathBuf> {
    if !dir.exists() {
        return Vec::new();
    }
    let mut files = WalkDir::new(dir)
        .max_depth(3)
        .into_iter()
        .flatten()
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| entry.path().to_path_buf())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .map(|name| name == "SKILL.md")
                .unwrap_or(false)
        })
        .collect::<Vec<_>>();
    files.sort();
    files.truncate(MAX_ARRAY_ENTRIES);
    files
}

fn knowledge_file(path: &Path) -> Option<AuditKnowledgeFile> {
    let metadata = fs::metadata(path).ok()?;
    let raw = fs::read_to_string(path).ok()?;
    let line_count = raw.lines().count();
    let redacted = redact_text(&raw);
    let content_truncated = redacted.chars().count() > KNOWLEDGE_CONTENT_LIMIT;
    let content_preview = if content_truncated {
        redacted.chars().take(KNOWLEDGE_CONTENT_LIMIT).collect()
    } else {
        redacted
    };
    let feature_flags = knowledge_flags(&raw);
    Some(AuditKnowledgeFile {
        path: display_path(path),
        exists: true,
        size_bytes: metadata.len(),
        line_count,
        content_preview,
        content_truncated,
        feature_flags,
    })
}

fn knowledge_flags(raw: &str) -> AuditKnowledgeFlags {
    let lower = raw.to_lowercase();
    let style_keywords = ["terse", "minimal", "explicit", "concise", "no emojis"]
        .into_iter()
        .filter(|keyword| lower.contains(keyword))
        .map(str::to_string)
        .take(5)
        .collect::<Vec<_>>();
    let imported_paths = raw
        .lines()
        .filter_map(import_path)
        .take(20)
        .collect::<Vec<_>>();

    AuditKnowledgeFlags {
        mentions_testing: lower.contains("test")
            || lower.contains("cargo check")
            || lower.contains("pnpm run check")
            || lower.contains("typecheck"),
        mentions_security: lower.contains("security")
            || lower.contains("threat")
            || lower.contains(".env")
            || lower.contains("secret"),
        mentions_secrets: lower.contains("secret")
            || lower.contains("keychain")
            || lower.contains("rotation")
            || lower.contains(".env"),
        has_wrong_right_patterns: lower.contains("wrong") && lower.contains("right"),
        has_command_table: lower.contains("| command")
            || lower.contains("| script")
            || lower.contains("## commands")
            || lower.contains("# commands"),
        has_dont_section: lower.contains("don't")
            || lower.contains("do not")
            || lower.contains("avoid"),
        has_external_links: lower.contains("https://") || lower.contains("http://"),
        style_keywords,
        imports_other_files: !imported_paths.is_empty(),
        imported_paths,
    }
}

fn import_path(line: &str) -> Option<String> {
    let at = line.find('@')?;
    let candidate = line[at + 1..]
        .split_whitespace()
        .next()
        .unwrap_or("")
        .trim_matches(|c: char| c == '`' || c == '"' || c == '\'' || c == ')' || c == ']');
    if candidate.ends_with(".md") || candidate.ends_with(".mdc") || candidate.ends_with(".txt") {
        Some(candidate.to_string())
    } else {
        None
    }
}

fn collect_usage_summary(
    ingested: Option<&Ingested>,
    window: Option<Duration>,
    window_label: &str,
) -> AuditUsageSummary {
    let Some(ingested) = ingested else {
        return AuditUsageSummary::unavailable(window_label);
    };

    let now = Utc::now();
    let calls = ingested
        .calls
        .iter()
        .filter(|call| {
            window
                .map(|window| {
                    call.timestamp
                        .map(|ts| ts >= now - window && ts <= now)
                        .unwrap_or(false)
                })
                .unwrap_or(true)
        })
        .collect::<Vec<_>>();

    summarize_calls(&calls, window_label)
}

fn summarize_calls(calls: &[&ParsedCall], window_label: &str) -> AuditUsageSummary {
    let sessions = calls
        .iter()
        .filter_map(|call| audit_session_key(call))
        .collect::<HashSet<_>>();
    let input_tokens = calls.iter().map(|call| call.input_tokens).sum::<u64>();
    let output_tokens = calls.iter().map(|call| call.output_tokens).sum::<u64>();
    let cache_creation_tokens = calls
        .iter()
        .map(|call| call.cache_creation_input_tokens)
        .sum::<u64>();
    let cache_read_tokens = calls
        .iter()
        .map(|call| call.cache_read_input_tokens)
        .sum::<u64>();
    let cached_input_tokens = calls
        .iter()
        .map(|call| call.cached_input_tokens)
        .sum::<u64>();
    let cost_usd = calls.iter().map(|call| call.cost_usd).sum::<f64>();
    let prompt_tokens =
        input_tokens + cache_creation_tokens + cache_read_tokens + cached_input_tokens;
    let total_tokens = prompt_tokens + output_tokens;

    AuditUsageSummary {
        available: true,
        window_label: window_label.into(),
        calls: calls.len() as u64,
        sessions: sessions.len() as u64,
        cost_usd,
        cost_label: format_usd(cost_usd),
        input_tokens,
        output_tokens,
        cache_creation_tokens,
        cache_read_tokens,
        total_tokens,
        cache_read_ratio: ratio(cache_read_tokens, prompt_tokens),
        cache_hit_ratio: ratio(cache_read_tokens + cached_input_tokens, prompt_tokens),
        top_tools: ranked_items(calls, RankBy::Tool, 5),
        top_models: ranked_items(calls, RankBy::Model, 5),
        top_projects: ranked_items(calls, RankBy::Project, 5),
    }
}

enum RankBy {
    Tool,
    Model,
    Project,
}

fn ranked_items(calls: &[&ParsedCall], rank_by: RankBy, limit: usize) -> Vec<AuditRankedItem> {
    #[derive(Default)]
    struct Acc {
        calls: u64,
        sessions: HashSet<String>,
        cost: f64,
        tokens: u64,
    }

    let project_labels = project_label_lookup(calls.iter().map(|call| call.project.as_str()));
    let mut rows: HashMap<String, Acc> = HashMap::new();
    for call in calls {
        let name = match rank_by {
            RankBy::Tool => tool_short_label(call.tool).to_string(),
            RankBy::Model => {
                if call.model.trim().is_empty() {
                    "(unknown model)".into()
                } else {
                    call.model.clone()
                }
            }
            RankBy::Project => {
                let identity = project_identity(&call.project);
                project_label(&project_labels, &identity)
            }
        };
        let entry = rows.entry(name).or_default();
        entry.calls += 1;
        entry.cost += call.cost_usd;
        entry.tokens += call_total_tokens(call);
        if let Some(session) = audit_session_key(call) {
            entry.sessions.insert(session);
        }
    }

    let mut rows = rows.into_iter().collect::<Vec<_>>();
    rows.sort_by(|a, b| {
        b.1.cost
            .partial_cmp(&a.1.cost)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.1.calls.cmp(&a.1.calls))
            .then_with(|| a.0.cmp(&b.0))
    });

    rows.into_iter()
        .take(limit)
        .map(|(name, acc)| AuditRankedItem {
            name,
            calls: acc.calls,
            sessions: acc.sessions.len() as u64,
            cost_usd: acc.cost,
            cost_label: format_usd(acc.cost),
            tokens: acc.tokens,
        })
        .collect()
}

fn collect_project_coverage(ingested: Option<&Ingested>) -> AuditProjectCoverage {
    let Some(ingested) = ingested else {
        return AuditProjectCoverage::default();
    };

    #[derive(Default)]
    struct Acc {
        calls: u64,
        sessions: HashSet<String>,
    }

    let labels = project_label_lookup(ingested.calls.iter().map(|call| call.project.as_str()));
    let mut projects: HashMap<String, Acc> = HashMap::new();
    for call in &ingested.calls {
        let identity = project_identity(&call.project);
        let entry = projects.entry(identity).or_default();
        entry.calls += 1;
        if let Some(session) = audit_session_key(call) {
            entry.sessions.insert(session);
        }
    }

    let known_project_roots = projects.len();
    let mut rows = projects.into_iter().collect::<Vec<_>>();
    rows.sort_by(|a, b| {
        b.1.calls
            .cmp(&a.1.calls)
            .then_with(|| project_label(&labels, &a.0).cmp(&project_label(&labels, &b.0)))
    });

    let omitted_project_roots = known_project_roots.saturating_sub(MAX_PROJECT_ROOTS_CHECKED);
    let mut coverage = AuditProjectCoverage {
        available: true,
        known_project_roots,
        omitted_project_roots,
        ..AuditProjectCoverage::default()
    };

    for (identity, acc) in rows.into_iter().take(MAX_PROJECT_ROOTS_CHECKED) {
        let path = PathBuf::from(&identity);
        let label = project_label(&labels, &identity);
        if !path.is_absolute() || !path.exists() || !path.is_dir() {
            coverage.skipped_project_roots += 1;
            coverage.entries.push(AuditProjectCoverageEntry {
                label,
                path: redact_text(&identity),
                calls: acc.calls,
                sessions: acc.sessions.len() as u64,
                agent_files: Vec::new(),
                has_ci: false,
                has_manifest: false,
                checked: false,
                skipped_reason: Some("not a readable local directory".into()),
            });
            continue;
        }

        let agent_files = project_agent_files(&path);
        let has_ci = project_has_ci(&path);
        let has_manifest = project_has_manifest(&path);
        coverage.checked_project_roots += 1;
        if !agent_files.is_empty() {
            coverage.roots_with_agent_instructions += 1;
        }
        if has_ci {
            coverage.roots_with_ci += 1;
        }
        if has_manifest {
            coverage.roots_with_manifests += 1;
        }
        coverage.entries.push(AuditProjectCoverageEntry {
            label,
            path: display_path(&path),
            calls: acc.calls,
            sessions: acc.sessions.len() as u64,
            agent_files,
            has_ci,
            has_manifest,
            checked: true,
            skipped_reason: None,
        });
    }

    coverage
}

fn project_agent_files(root: &Path) -> Vec<String> {
    let mut files = Vec::new();
    for rel in [
        "AGENTS.md",
        "CLAUDE.md",
        "GEMINI.md",
        ".github/copilot-instructions.md",
        ".mcp.json",
    ] {
        if root.join(rel).exists() {
            files.push(rel.to_string());
        }
    }
    for rel in [".claude", ".codex", ".cursor", ".gemini"] {
        if root.join(rel).is_dir() {
            files.push(format!("{rel}/"));
        }
    }
    if !markdown_files(&root.join(".cursor/rules"), 2).is_empty() {
        files.push(".cursor/rules/*.mdc".into());
    }
    files.sort();
    files.truncate(20);
    files
}

fn project_has_ci(root: &Path) -> bool {
    let workflows = root.join(".github/workflows");
    workflows.is_dir()
        && fs::read_dir(workflows)
            .map(|entries| {
                entries.flatten().any(|entry| {
                    matches!(
                        entry.path().extension().and_then(|ext| ext.to_str()),
                        Some("yml" | "yaml")
                    )
                })
            })
            .unwrap_or(false)
}

fn project_has_manifest(root: &Path) -> bool {
    [
        "Cargo.toml",
        "package.json",
        "pyproject.toml",
        "go.mod",
        "pom.xml",
        "build.gradle",
        "Gemfile",
        "composer.json",
    ]
    .iter()
    .any(|file| root.join(file).exists())
}

fn collect_activity_signals(ingested: Option<&Ingested>) -> AuditActivitySignals {
    let Some(ingested) = ingested else {
        return AuditActivitySignals::default();
    };

    let calls = ingested.calls.iter().collect::<Vec<_>>();
    let distinct_tools_used = calls
        .iter()
        .map(|call| call.tool)
        .collect::<HashSet<_>>()
        .len();
    let distinct_models_used = calls
        .iter()
        .filter(|call| !call.model.trim().is_empty())
        .map(|call| call.model.as_str())
        .collect::<HashSet<_>>()
        .len();
    let tool_call_uses = calls
        .iter()
        .map(|call| {
            call.tools
                .iter()
                .filter(|tool| !tool.starts_with("mcp__"))
                .count() as u64
        })
        .sum();
    let shell_command_uses = calls
        .iter()
        .map(|call| call.bash_commands.len() as u64)
        .sum();
    let mcp_tool_uses = calls
        .iter()
        .map(|call| {
            call.tools
                .iter()
                .filter(|tool| tool.starts_with("mcp__"))
                .count() as u64
        })
        .sum();

    AuditActivitySignals {
        available: true,
        tool_call_uses,
        shell_command_uses,
        mcp_tool_uses,
        distinct_tools_used,
        distinct_models_used,
        high_cost_projects: ranked_items(&calls, RankBy::Project, 3)
            .into_iter()
            .filter(|item| item.cost_usd > 0.0)
            .map(|item| format!("{} {}", item.name, item.cost_label))
            .collect(),
        high_cost_sessions: high_cost_sessions(&calls, 3),
        repeated_model_patterns: repeated_patterns(&calls, RankBy::Model, 3),
        repeated_tool_patterns: repeated_patterns(&calls, RankBy::Tool, 3),
    }
}

fn high_cost_sessions(calls: &[&ParsedCall], limit: usize) -> Vec<String> {
    #[derive(Default)]
    struct Acc {
        tool: &'static str,
        cost: f64,
        calls: u64,
    }

    let mut rows: HashMap<String, Acc> = HashMap::new();
    for call in calls {
        let Some(session) = audit_session_key(call) else {
            continue;
        };
        let entry = rows.entry(session).or_insert_with(|| Acc {
            tool: call.tool,
            ..Acc::default()
        });
        entry.cost += call.cost_usd;
        entry.calls += 1;
    }
    let mut rows = rows.into_iter().collect::<Vec<_>>();
    rows.sort_by(|a, b| {
        b.1.cost
            .partial_cmp(&a.1.cost)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.1.calls.cmp(&a.1.calls))
    });
    rows.into_iter()
        .take(limit)
        .map(|(session, acc)| {
            let short = session
                .split(':')
                .next_back()
                .unwrap_or(session.as_str())
                .chars()
                .take(12)
                .collect::<String>();
            format!(
                "{}:{} {}",
                tool_short_label(acc.tool),
                short,
                format_usd(acc.cost)
            )
        })
        .collect()
}

fn repeated_patterns(calls: &[&ParsedCall], rank_by: RankBy, limit: usize) -> Vec<String> {
    ranked_items(calls, rank_by, limit)
        .into_iter()
        .filter(|item| item.calls >= 3)
        .map(|item| format!("{} · {} calls", item.name, item.calls))
        .collect()
}

fn audit_session_key(call: &ParsedCall) -> Option<String> {
    let session = call.session_id.trim();
    if session.is_empty() {
        None
    } else {
        Some(format!("{}:{session}", call.tool))
    }
}

fn call_total_tokens(call: &ParsedCall) -> u64 {
    call.input_tokens
        + call.output_tokens
        + call.cache_creation_input_tokens
        + call.cache_read_input_tokens
        + call.cached_input_tokens
}

fn ratio(numerator: u64, denominator: u64) -> Option<f64> {
    (denominator > 0).then_some((numerator as f64 / denominator as f64 * 100.0).round() / 100.0)
}

fn format_usd(value: f64) -> String {
    if value.abs() >= 1_000.0 {
        format!("${value:.0}")
    } else if value.abs() >= 1.0 {
        format!("${value:.2}")
    } else {
        format!("${value:.4}")
    }
}

fn collect_behavior(ctx: &ScanContext) -> AuditBehavior {
    let mut files = Vec::new();
    files.extend(recent_claude_logs(ctx));
    files.extend(recent_codex_logs(ctx));
    files.truncate(20);

    let mut acc = BehaviorAccumulator::default();
    for path in files {
        if let Some(text) = read_limited(&path, MAX_LOG_BYTES) {
            acc.logs_inspected += 1;
            acc.clear_uses +=
                count_patterns(&text, &["/clear", "<command-name>/clear</command-name>"]);
            acc.compact_uses += count_patterns(
                &text,
                &["/compact", "<command-name>/compact</command-name>"],
            );
            acc.subagent_calls += count_patterns(
                &text,
                &["\"Task\"", "\"Agent\"", "spawn_agent", "@subagent"],
            );
            acc.plan_mode_uses += count_patterns(&text, &["ExitPlanMode", "/plan", "plan mode"]);
            acc.skill_invocations +=
                count_patterns(&text, &["\"Skill\"", "skill_invocations", "/skill"]);
            analyze_user_turns(&text, &mut acc);
        }
    }

    if acc.logs_inspected == 0 {
        return AuditBehavior::default();
    }

    AuditBehavior {
        recent_sessions_inspected: acc.logs_inspected,
        clear_uses: Some(acc.clear_uses),
        compact_uses: Some(acc.compact_uses),
        subagent_calls: Some(acc.subagent_calls),
        plan_mode_uses: Some(acc.plan_mode_uses),
        skill_invocations: Some(acc.skill_invocations),
        longest_session_turns_without_reset: Some(acc.longest_without_reset),
        avg_user_turn_chars: (acc.user_turns > 0)
            .then_some((acc.user_turn_chars as f64 / acc.user_turns as f64 * 10.0).round() / 10.0),
        correction_turn_ratio: (acc.user_turns > 0).then_some(
            (acc.correction_turns as f64 / acc.user_turns as f64 * 100.0).round() / 100.0,
        ),
    }
}

fn analyze_user_turns(text: &str, acc: &mut BehaviorAccumulator) {
    let mut since_reset = 0_u64;
    for line in text.lines() {
        let reset = line.contains("/clear") || line.contains("/compact");
        if reset {
            acc.longest_without_reset = acc.longest_without_reset.max(since_reset);
            since_reset = 0;
        }
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if !json_is_user_turn(&value) {
            continue;
        }
        let content = user_turn_text(&value);
        if content.is_empty() {
            continue;
        }
        since_reset += 1;
        acc.user_turns += 1;
        acc.user_turn_chars += content.chars().count() as u64;
        if starts_with_correction(&content) {
            acc.correction_turns += 1;
        }
    }
    acc.longest_without_reset = acc.longest_without_reset.max(since_reset);
}

fn json_is_user_turn(value: &Value) -> bool {
    value
        .get("type")
        .and_then(Value::as_str)
        .map(|v| v == "user")
        .unwrap_or(false)
        || value
            .get("role")
            .and_then(Value::as_str)
            .map(|v| v == "user")
            .unwrap_or(false)
        || value
            .pointer("/message/role")
            .and_then(Value::as_str)
            .map(|v| v == "user")
            .unwrap_or(false)
}

fn user_turn_text(value: &Value) -> String {
    let mut out = String::new();
    collect_strings_from_keys(value, &mut out, &["content", "text", "prompt", "input"]);
    out
}

fn collect_strings_from_keys(value: &Value, out: &mut String, keys: &[&str]) {
    match value {
        Value::Object(map) => {
            for (key, value) in map {
                if keys.contains(&key.as_str()) {
                    if let Some(text) = value.as_str() {
                        out.push_str(text);
                        out.push('\n');
                    }
                }
                collect_strings_from_keys(value, out, keys);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_strings_from_keys(item, out, keys);
            }
        }
        _ => {}
    }
}

fn starts_with_correction(text: &str) -> bool {
    let lower = text.trim_start().to_lowercase();
    ["no", "actually", "stop", "wait", "wrong", "don't", "do not"]
        .into_iter()
        .any(|prefix| lower == prefix || lower.starts_with(&format!("{prefix} ")))
}

fn recent_claude_logs(ctx: &ScanContext) -> Vec<PathBuf> {
    let Some(home) = &ctx.home else {
        return Vec::new();
    };
    let projects = home.join(".claude/projects");
    recent_files_under(&projects, "jsonl", 10)
}

fn recent_codex_logs(ctx: &ScanContext) -> Vec<PathBuf> {
    let Some(home) = &ctx.home else {
        return Vec::new();
    };
    let sessions = std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".codex"))
        .join("sessions");
    recent_files_under(&sessions, "jsonl", 10)
}

fn recent_files_under(root: &Path, ext: &str, limit: usize) -> Vec<PathBuf> {
    let mut files = WalkDir::new(root)
        .max_depth(6)
        .into_iter()
        .flatten()
        .filter(|entry| entry.file_type().is_file())
        .filter(|entry| entry.path().extension().and_then(|e| e.to_str()) == Some(ext))
        .filter_map(|entry| {
            let modified = entry.metadata().ok()?.modified().ok()?;
            Some((modified, entry.path().to_path_buf()))
        })
        .collect::<Vec<_>>();
    files.sort_by_key(|(modified, _)| std::cmp::Reverse(*modified));
    files
        .into_iter()
        .map(|(_, path)| path)
        .take(limit)
        .collect()
}

fn build_findings(
    ctx: &ScanContext,
    tools: &[AuditToolSummary],
    inputs: FindingInputs<'_>,
) -> Vec<AuditFinding> {
    let mut findings = Vec::new();
    let usage_summary = inputs.usage_summary;
    let recent_usage = inputs.recent_usage;
    let project_coverage = inputs.project_coverage;
    let activity_signals = inputs.activity_signals;
    let behavior = inputs.behavior;
    let knowledge_files = inputs.knowledge_files;

    let secret_hits = suspected_hardcoded_secrets(ctx);
    if !secret_hits.is_empty() {
        findings.push(finding(
            "suspected_hardcoded_secrets",
            AuditSection::Security,
            AuditSeverity::Risk,
            "Suspected secrets in agent config",
            "Agent configuration appears to contain token-like values. Rotate affected credentials and keep only helper commands or environment key names in config.",
            secret_hits.clone(),
            paths_from_reasons(&secret_hits),
        ));
    }

    let dangerous_tools = tools
        .iter()
        .filter(|tool| tool.dangerous_alias_detected)
        .map(|tool| tool.label.clone())
        .collect::<Vec<_>>();
    if !dangerous_tools.is_empty() {
        findings.push(finding(
            "dangerous_agent_alias",
            AuditSection::Security,
            AuditSeverity::Warning,
            "Dangerous bypass alias detected",
            "Shell or project scripts reference an agent mode that bypasses permissions or sandboxing. Keep that flow explicit and rare.",
            dangerous_tools,
            Vec::new(),
        ));
    }

    let bloated_files = knowledge_files
        .iter()
        .filter(|file| file.line_count > 400 || file.size_bytes > 32_000 || file.content_truncated)
        .map(|file| format!("{} · {} lines", file.path, file.line_count))
        .collect::<Vec<_>>();
    if !bloated_files.is_empty() {
        findings.push(finding(
            "knowledge_file_bloat",
            AuditSection::Efficiency,
            AuditSeverity::Warning,
            "Large always-loaded knowledge files",
            "Move narrow instructions into scoped rules, skills, or per-directory files so every agent turn carries less context.",
            bloated_files,
            knowledge_files.iter().map(|file| file.path.clone()).collect(),
        ));
    }

    let duplicate_mcps = duplicate_mcp_servers(tools);
    if !duplicate_mcps.is_empty() {
        findings.push(finding(
            "duplicate_mcp_servers",
            AuditSection::Efficiency,
            AuditSeverity::Info,
            "MCP servers repeated across tools",
            "Repeated MCP configuration can be intentional, but duplicated servers often mean duplicated maintenance and tool-list noise.",
            duplicate_mcps,
            Vec::new(),
        ));
    }

    let verification_mentioned = knowledge_files
        .iter()
        .any(|file| file.feature_flags.mentions_testing);
    if project_coverage.available && !verification_mentioned && project_coverage.roots_with_ci == 0
    {
        findings.push(finding(
            "verification_commands_missing",
            AuditSection::Efficiency,
            AuditSeverity::Warning,
            "Verification commands are not obvious",
            "Add explicit local check commands to agent instructions or CI so agents can validate changes without stopping for routine confirmation.",
            Vec::new(),
            Vec::new(),
        ));
    }

    if project_coverage.available {
        let missing_active = project_coverage
            .entries
            .iter()
            .filter(|entry| entry.checked && entry.agent_files.is_empty() && entry.calls >= 25)
            .take(5)
            .map(|entry| format!("{} · {} calls", entry.label, entry.calls))
            .collect::<Vec<_>>();
        if !missing_active.is_empty() {
            findings.push(finding(
                "active_projects_missing_agent_files",
                AuditSection::Readiness,
                AuditSeverity::Warning,
                "Active projects missing project-level instructions",
                "High-activity project roots do not appear to have local agent instruction files. Add scoped AGENTS.md, CLAUDE.md, Cursor rules, or equivalent where project behavior differs.",
                missing_active,
                project_coverage
                    .entries
                    .iter()
                    .filter(|entry| entry.checked && entry.agent_files.is_empty() && entry.calls >= 25)
                    .map(|entry| entry.path.clone())
                    .collect(),
            ));
        }
    }

    if activity_signals.available
        && activity_signals.distinct_tools_used >= 4
        && activity_signals.mcp_tool_uses >= 50
    {
        findings.push(finding(
            "high_tool_mcp_churn",
            AuditSection::Efficiency,
            AuditSeverity::Info,
            "High tool and MCP activity in archive",
            "The archive shows broad tool usage and frequent MCP calls. Review whether the same servers are configured in multiple agents or whether noisy tools can be scoped.",
            vec![format!(
                "{} tools · {} MCP calls",
                activity_signals.distinct_tools_used, activity_signals.mcp_tool_uses
            )],
            Vec::new(),
        ));
    }

    if recent_usage.available && recent_usage.cost_usd > 0.0 {
        if let Some(top_project) = recent_usage.top_projects.first() {
            if top_project.cost_usd / recent_usage.cost_usd >= 0.7 && recent_usage.calls >= 10 {
                findings.push(finding(
                    "recent_spend_concentration",
                    AuditSection::Efficiency,
                    AuditSeverity::Info,
                    "Recent usage is concentrated in one project",
                    "The fixed 7-day archive view is dominated by one project. Check that project first when tightening instructions, MCP scope, or model choices.",
                    vec![format!(
                        "{} · {} of {}",
                        top_project.name, top_project.cost_label, recent_usage.cost_label
                    )],
                    Vec::new(),
                ));
            }
        }
    }

    if recent_usage.available {
        let recent_tools = recent_usage
            .top_tools
            .iter()
            .map(|item| item.name.to_lowercase())
            .collect::<HashSet<_>>();
        let unused = tools
            .iter()
            .filter(|tool| tool.present)
            .filter(|tool| {
                let label = tool_short_label(&tool.id).to_lowercase();
                !recent_tools.contains(&label)
            })
            .map(|tool| tool.label.clone())
            .collect::<Vec<_>>();
        if !unused.is_empty() && recent_usage.calls > 0 {
            findings.push(finding(
                "configured_tools_unused_recently",
                AuditSection::Efficiency,
                AuditSeverity::Info,
                "Configured tools unused in recent archive",
                "Some agent homes exist but have no calls in the fixed 7-day usage slice. Consider whether those setups still need MCP servers, skills, or local credentials.",
                unused,
                Vec::new(),
            ));
        }
    } else if usage_summary.available && usage_summary.calls == 0 {
        findings.push(finding(
            "archive_has_no_calls",
            AuditSection::Readiness,
            AuditSeverity::Info,
            "Archive has no usage calls",
            "The audit can inspect tool homes, but usage-derived activity and project coverage need local archive rows.",
            Vec::new(),
            Vec::new(),
        ));
    }

    if behavior.recent_sessions_inspected > 0
        && behavior.clear_uses == Some(0)
        && behavior.compact_uses == Some(0)
    {
        findings.push(finding(
            "context_reset_rhythm_missing",
            AuditSection::Context,
            AuditSeverity::Info,
            "No clear or compact rhythm observed",
            "Recent readable sessions did not show clear or compact commands. Periodic resets keep unrelated tasks from sharing stale context.",
            vec![format!("{} session logs inspected", behavior.recent_sessions_inspected)],
            Vec::new(),
        ));
    }

    if behavior.recent_sessions_inspected > 0 && behavior.subagent_calls == Some(0) {
        findings.push(finding(
            "subagent_use_missing",
            AuditSection::Context,
            AuditSeverity::Info,
            "No subagent delegation observed",
            "For expensive investigations, delegating side reads can keep the main agent context slimmer.",
            vec![format!("{} session logs inspected", behavior.recent_sessions_inspected)],
            Vec::new(),
        ));
    }

    let has_scoped_assets = tools.iter().any(|tool| tool.scoped_assets > 0);
    if !has_scoped_assets && knowledge_files.len() > 1 {
        findings.push(finding(
            "scoped_agent_assets_missing",
            AuditSection::Context,
            AuditSeverity::Info,
            "Few scoped agent assets detected",
            "Consider moving tool-specific or path-specific guidance into scoped skills, rules, or subagents instead of broad global files.",
            Vec::new(),
            Vec::new(),
        ));
    }

    findings
}

fn finding(
    id: &str,
    section: AuditSection,
    severity: AuditSeverity,
    title: &str,
    body: &str,
    evidence: Vec<String>,
    source_paths: Vec<String>,
) -> AuditFinding {
    AuditFinding {
        id: id.into(),
        section,
        severity,
        title: title.into(),
        body: body.into(),
        evidence: evidence
            .into_iter()
            .map(|s| redact_text(&s))
            .take(8)
            .collect(),
        source_paths: source_paths
            .into_iter()
            .map(|s| redact_text(&s))
            .take(8)
            .collect(),
    }
}

fn summarize_findings(findings: &[AuditFinding]) -> AuditSummary {
    let mut summary = AuditSummary {
        total_findings: findings.len(),
        ..AuditSummary::default()
    };
    for finding in findings {
        match finding.section {
            AuditSection::Security => summary.security_findings += 1,
            AuditSection::Efficiency => summary.efficiency_findings += 1,
            AuditSection::Context => summary.context_findings += 1,
            AuditSection::Readiness => summary.readiness_findings += 1,
        }
        match finding.severity {
            AuditSeverity::Risk => summary.risk_findings += 1,
            AuditSeverity::Warning => summary.warning_findings += 1,
            AuditSeverity::Info => summary.info_findings += 1,
        }
    }
    summary
}

fn duplicate_mcp_servers(tools: &[AuditToolSummary]) -> Vec<String> {
    let mut seen: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for tool in tools {
        for server in &tool.mcp_servers {
            seen.entry(server).or_default().push(tool.label.as_str());
        }
    }
    seen.into_iter()
        .filter(|(_, tools)| tools.len() > 1)
        .map(|(server, tools)| format!("{server} · {}", tools.join(", ")))
        .collect()
}

fn suspected_hardcoded_secrets(ctx: &ScanContext) -> Vec<String> {
    let mut files = Vec::new();
    if let Some(home) = &ctx.home {
        let codex_home = std::env::var_os("CODEX_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".codex"));
        files.extend([
            home.join(".claude/settings.json"),
            home.join(".claude/settings.local.json"),
            home.join(".claude.json"),
            home.join(".cursor/mcp.json"),
            home.join(".cursor/settings.json"),
            home.join(".cursor/permissions.json"),
            codex_home.join("config.toml"),
            home.join(".copilot/config.json"),
            home.join(".copilot/settings.json"),
            home.join(".copilot/mcp.json"),
            home.join(".gemini/settings.json"),
            home.join(".gemini/config.json"),
            home.join(".gemini/config.toml"),
        ]);
    }

    let mut hits = Vec::new();
    for file in files {
        let Ok(text) = fs::read_to_string(&file) else {
            continue;
        };
        for (idx, line) in text.lines().enumerate() {
            if contains_secret_shape(line) {
                hits.push(format!("{} line {}", display_path(&file), idx + 1));
            }
        }
    }
    hits.sort();
    hits.dedup();
    hits.truncate(20);
    hits
}

fn paths_from_reasons(reasons: &[String]) -> Vec<String> {
    reasons
        .iter()
        .filter_map(|reason| reason.split(" line ").next().map(str::to_string))
        .collect()
}

fn contains_secret_shape(line: &str) -> bool {
    let markers = [
        "sk-",
        "sk-ant-",
        "sk-or-",
        "AKIA",
        "ghp_",
        "ghs_",
        "xoxb-",
        "xoxp-",
        "xoxc-",
        "xoxd-",
        "\"private_key\"",
        "-----BEGIN PRIVATE KEY-----",
    ];
    markers.iter().any(|marker| line.contains(marker))
}

fn infer_primary_tool(tools: &[AuditToolSummary]) -> Option<String> {
    let mut scored = tools
        .iter()
        .filter(|tool| tool.present)
        .map(|tool| {
            let score = tool.config_paths.len()
                + tool.mcp_servers.len().saturating_mul(2)
                + tool.hooks_count.saturating_mul(2)
                + tool.knowledge_files
                + tool.scoped_assets.saturating_mul(2);
            (score, tool.id.clone())
        })
        .collect::<Vec<_>>();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    let (score, id) = scored.first()?.clone();
    if score == 0 {
        return None;
    }
    if scored
        .get(1)
        .map(|(next, _)| *next == score)
        .unwrap_or(false)
    {
        None
    } else {
        Some(id)
    }
}

fn json_mcp_servers(path: &Path) -> Vec<String> {
    let Ok(text) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let Ok(value) = serde_json::from_str::<Value>(&text) else {
        return Vec::new();
    };
    ["mcpServers", "mcp_servers", "servers"]
        .into_iter()
        .filter_map(|key| value.get(key).and_then(Value::as_object))
        .flat_map(|servers| servers.keys().cloned())
        .collect()
}

fn json_object_count(path: &Path, key: &str) -> usize {
    let Ok(text) = fs::read_to_string(path) else {
        return 0;
    };
    let Ok(value) = serde_json::from_str::<Value>(&text) else {
        return 0;
    };
    value
        .get(key)
        .and_then(Value::as_object)
        .map(|object| object.len())
        .unwrap_or(0)
}

fn toml_mcp_servers(path: &Path) -> Vec<String> {
    let Ok(text) = fs::read_to_string(path) else {
        return Vec::new();
    };
    text.lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            let inner = trimmed.strip_prefix("[mcp_servers.")?.strip_suffix(']')?;
            Some(inner.split('.').next()?.trim_matches('"').to_string())
        })
        .collect()
}

fn toml_hooks_count(path: &Path) -> usize {
    fs::read_to_string(path)
        .map(|text| {
            text.lines()
                .filter(|line| line.trim().starts_with("[hooks"))
                .count()
        })
        .unwrap_or(0)
}

fn shell_or_project_files_contain(ctx: &ScanContext, needle: &str) -> bool {
    let mut files = vec![ctx.root.join("Makefile"), ctx.root.join("package.json")];
    if let Some(home) = &ctx.home {
        files.extend([
            home.join(".zshrc"),
            home.join(".bashrc"),
            home.join(".zprofile"),
            home.join(".config/fish/config.fish"),
        ]);
    }
    files.into_iter().any(|path| {
        fs::read_to_string(path)
            .map(|text| {
                text.lines()
                    .map(str::trim_start)
                    .filter(|line| !line.starts_with('#'))
                    .any(|line| line.contains(needle))
            })
            .unwrap_or(false)
    })
}

fn existing_pathbufs(paths: &[PathBuf]) -> Vec<PathBuf> {
    paths.iter().filter(|path| path.exists()).cloned().collect()
}

fn display_paths(paths: &[PathBuf]) -> Vec<String> {
    paths.iter().map(|path| display_path(path)).collect()
}

fn count_dir_entries(path: &Path) -> usize {
    fs::read_dir(path)
        .map(|entries| entries.flatten().count())
        .unwrap_or(0)
}

fn sort_dedup_truncate(values: &mut Vec<String>) {
    values.sort();
    values.dedup();
    values.truncate(MAX_ARRAY_ENTRIES);
}

fn limit_tool_summary(tool: &mut AuditToolSummary) {
    tool.config_paths.truncate(MAX_ARRAY_ENTRIES);
    tool.mcp_servers.truncate(MAX_ARRAY_ENTRIES);
}

fn nearest_git_root(start: &Path) -> Option<PathBuf> {
    let mut current = Some(start);
    while let Some(dir) = current {
        if dir.join(".git").exists() {
            return Some(dir.to_path_buf());
        }
        current = dir.parent();
    }
    None
}

fn read_limited(path: &Path, limit: u64) -> Option<String> {
    let mut file = fs::File::open(path).ok()?;
    let mut text = String::new();
    file.by_ref().take(limit).read_to_string(&mut text).ok()?;
    Some(text)
}

fn count_patterns(text: &str, patterns: &[&str]) -> u64 {
    patterns
        .iter()
        .map(|pattern| text.matches(pattern).count() as u64)
        .sum()
}

fn display_path(path: &Path) -> String {
    redact_text(&path.display().to_string())
}

fn redact_value(value: &mut Value) {
    match value {
        Value::String(text) => {
            *text = redact_text(text);
        }
        Value::Array(items) => {
            for item in items {
                redact_value(item);
            }
        }
        Value::Object(map) => {
            for value in map.values_mut() {
                redact_value(value);
            }
        }
        _ => {}
    }
}

fn redact_text(input: &str) -> String {
    let mut out = input.to_string();
    if let Some(home) = paths::home() {
        let home = home.display().to_string();
        out = out.replace(&home, "~");
    }
    for prefix in [
        "sk-ant-", "sk-or-", "sk-", "AKIA", "ghp_", "ghs_", "xoxb-", "xoxp-", "xoxc-", "xoxd-",
    ] {
        out = redact_tokens_with_prefix(&out, prefix);
    }
    out = redact_emails(&out);
    out
}

fn redact_tokens_with_prefix(input: &str, prefix: &str) -> String {
    let mut out = String::new();
    let mut rest = input;
    while let Some(idx) = rest.find(prefix) {
        out.push_str(&rest[..idx]);
        out.push_str("<REDACTED>");
        let token = &rest[idx..];
        let end = token
            .char_indices()
            .find(|(_, c)| !(c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.')))
            .map(|(idx, _)| idx)
            .unwrap_or(token.len());
        rest = &token[end..];
    }
    out.push_str(rest);
    out
}

fn redact_emails(input: &str) -> String {
    let mut out = String::new();
    let mut token = String::new();
    for ch in input.chars() {
        if ch.is_whitespace() {
            out.push_str(&redact_email_token(&token));
            token.clear();
            out.push(ch);
        } else {
            token.push(ch);
        }
    }
    out.push_str(&redact_email_token(&token));
    out
}

fn redact_email_token(token: &str) -> String {
    let trimmed = token.trim_matches(|c: char| {
        matches!(
            c,
            ',' | ';' | ':' | '(' | ')' | '[' | ']' | '<' | '>' | '"' | '\''
        )
    });
    if looks_like_email(trimmed) {
        token.replace(trimmed, "<REDACTED_EMAIL>")
    } else {
        token.to_string()
    }
}

fn looks_like_email(value: &str) -> bool {
    let Some((local, domain)) = value.split_once('@') else {
        return false;
    };
    !local.is_empty() && domain.contains('.') && !domain.ends_with('.')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(
        tool: &'static str,
        project: &str,
        session: &str,
        cost: f64,
        days_ago: i64,
    ) -> ParsedCall {
        ParsedCall {
            tool,
            model: if tool == "codex" {
                "gpt-5.3-codex".into()
            } else {
                "claude-sonnet-4-6".into()
            },
            input_tokens: 100,
            output_tokens: 50,
            cache_creation_input_tokens: 10,
            cache_read_input_tokens: 40,
            cached_input_tokens: 0,
            cost_usd: cost,
            tools: vec!["Read".into(), "mcp__codebase__search".into()],
            bash_commands: vec!["cargo check".into()],
            timestamp: Some(Utc::now() - Duration::days(days_ago)),
            session_id: session.into(),
            project: project.into(),
            ..ParsedCall::default()
        }
    }

    #[test]
    fn redacts_secret_shapes_and_email_addresses() {
        let redacted = redact_text("token sk-ant-abc123 and user@example.com and ghp_deadbeef");
        assert!(!redacted.contains("sk-ant-abc123"));
        assert!(!redacted.contains("user@example.com"));
        assert!(redacted.contains("<REDACTED>"));
        assert!(redacted.contains("<REDACTED_EMAIL>"));
    }

    #[test]
    fn detects_knowledge_file_flags() {
        let flags = knowledge_flags(
            "## Commands\n| script | action |\nDo not commit .env\nhttps://example.com\n@docs/style.md",
        );
        assert!(flags.has_command_table);
        assert!(flags.has_dont_section);
        assert!(flags.mentions_secrets);
        assert!(flags.has_external_links);
        assert!(flags.imports_other_files);
        assert_eq!(flags.imported_paths, vec!["docs/style.md"]);
    }

    #[test]
    fn measured_zero_behavior_is_distinct_from_unmeasured() {
        let mut acc = BehaviorAccumulator::default();
        analyze_user_turns(r#"{"type":"user","message":{"content":"hello"}}"#, &mut acc);
        assert_eq!(acc.user_turns, 1);
        assert_eq!(acc.longest_without_reset, 1);
        assert_eq!(acc.clear_uses, 0);
    }

    #[test]
    fn caps_stored_snapshot_payload_by_trimming_previews() {
        let mut snapshot = empty_test_snapshot();
        snapshot.knowledge_files = (0..MAX_KNOWLEDGE_FILES)
            .map(|idx| AuditKnowledgeFile {
                path: format!("AGENTS-{idx}.md"),
                exists: true,
                size_bytes: 60_000,
                line_count: 1_000,
                content_preview: "x".repeat(60_000),
                content_truncated: false,
                feature_flags: AuditKnowledgeFlags::default(),
            })
            .collect();

        let capped = cap_snapshot_payload(snapshot).expect("caps payload");
        let bytes = serde_json::to_vec_pretty(&capped).expect("serializes snapshot");

        assert!(bytes.len() <= MAX_STORED_SNAPSHOT_BYTES);
        assert!(capped
            .knowledge_files
            .iter()
            .all(|file| file.content_truncated));
    }

    #[test]
    fn knowledge_preview_roundtrips_through_json_string_encoding() {
        let mut snapshot = empty_test_snapshot();
        snapshot.knowledge_files.push(AuditKnowledgeFile {
            path: "AGENTS.md".into(),
            exists: true,
            size_bytes: 20,
            line_count: 2,
            content_preview: "first line\n\"quoted\"\tvalue".into(),
            content_truncated: false,
            feature_flags: AuditKnowledgeFlags::default(),
        });

        let json = serde_json::to_string(&snapshot).expect("serializes snapshot");
        let decoded: AuditSnapshot = serde_json::from_str(&json).expect("decodes snapshot");

        assert_eq!(
            decoded.knowledge_files[0].content_preview,
            "first line\n\"quoted\"\tvalue"
        );
    }

    #[test]
    fn archive_usage_summary_splits_all_time_and_recent_windows() {
        let ingested = Ingested {
            calls: vec![
                call("claude-code", "/tmp/project-a", "s1", 1.25, 1),
                call("codex", "/tmp/project-a", "s2", 2.00, 3),
                call("codex", "/tmp/project-b", "s3", 4.00, 10),
            ],
            limits: Vec::new(),
        };

        let all_time = collect_usage_summary(Some(&ingested), None, "all time");
        let recent = collect_usage_summary(Some(&ingested), Some(Duration::days(7)), "last 7 days");

        assert!(all_time.available);
        assert_eq!(all_time.calls, 3);
        assert_eq!(all_time.sessions, 3);
        assert_eq!(all_time.total_tokens, 600);
        assert_eq!(recent.calls, 2);
        assert_eq!(recent.cost_label, "$3.25");
        assert_eq!(recent.top_tools[0].name, "Codex");
    }

    #[test]
    fn project_coverage_checks_archive_known_roots() {
        let root = temp_project("audit-coverage");
        fs::write(root.join("AGENTS.md"), "instructions").unwrap();
        fs::create_dir_all(root.join(".github/workflows")).unwrap();
        fs::write(root.join(".github/workflows/ci.yml"), "name: ci").unwrap();
        fs::write(root.join("Cargo.toml"), "[package]\nname = \"demo\"").unwrap();
        let missing = root.join("missing");
        let ingested = Ingested {
            calls: vec![
                call("claude-code", &root.to_string_lossy(), "s1", 1.0, 1),
                call("codex", &missing.to_string_lossy(), "s2", 1.0, 1),
            ],
            limits: Vec::new(),
        };

        let coverage = collect_project_coverage(Some(&ingested));

        assert!(coverage.available);
        assert_eq!(coverage.known_project_roots, 2);
        assert_eq!(coverage.checked_project_roots, 1);
        assert_eq!(coverage.skipped_project_roots, 1);
        assert_eq!(coverage.roots_with_agent_instructions, 1);
        assert_eq!(coverage.roots_with_ci, 1);
        assert_eq!(coverage.roots_with_manifests, 1);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn findings_use_archive_data_without_warning_when_unavailable() {
        let ctx = ScanContext {
            root: std::env::temp_dir(),
            home: None,
        };
        let findings = build_findings(
            &ctx,
            &[],
            FindingInputs {
                usage_summary: &AuditUsageSummary::unavailable("all time"),
                recent_usage: &AuditUsageSummary::unavailable("last 7 days"),
                project_coverage: &AuditProjectCoverage::default(),
                activity_signals: &AuditActivitySignals::default(),
                behavior: &AuditBehavior::default(),
                knowledge_files: &[],
            },
        );

        assert!(!findings
            .iter()
            .any(|finding| finding.id == "verification_commands_missing"));
    }

    #[test]
    fn findings_flag_active_project_without_agent_instructions() {
        let root = temp_project("audit-missing-agent-files");
        let calls = (0..25)
            .map(|idx| {
                call(
                    "claude-code",
                    &root.to_string_lossy(),
                    &format!("s{idx}"),
                    0.05,
                    1,
                )
            })
            .collect::<Vec<_>>();
        let ingested = Ingested {
            calls,
            limits: Vec::new(),
        };
        let usage = collect_usage_summary(Some(&ingested), None, "all time");
        let recent = collect_usage_summary(Some(&ingested), Some(Duration::days(7)), "last 7 days");
        let coverage = collect_project_coverage(Some(&ingested));
        let signals = collect_activity_signals(Some(&ingested));
        let ctx = ScanContext {
            root: root.clone(),
            home: None,
        };

        let findings = build_findings(
            &ctx,
            &[],
            FindingInputs {
                usage_summary: &usage,
                recent_usage: &recent,
                project_coverage: &coverage,
                activity_signals: &signals,
                behavior: &AuditBehavior::default(),
                knowledge_files: &[],
            },
        );

        assert!(findings
            .iter()
            .any(|finding| finding.id == "active_projects_missing_agent_files"));

        let _ = fs::remove_dir_all(root);
    }

    fn empty_test_snapshot() -> AuditSnapshot {
        AuditSnapshot {
            schema_version: SCHEMA_VERSION.into(),
            scanner_version: SCANNER_VERSION.into(),
            captured_at: "2026-05-14T00:00:00Z".into(),
            root: None,
            primary_tool_guess: None,
            redaction: AuditRedaction::default(),
            tools: Vec::new(),
            usage_summary: AuditUsageSummary::unavailable("all time"),
            recent_usage: AuditUsageSummary::unavailable("last 7 days"),
            project_coverage: AuditProjectCoverage::default(),
            activity_signals: AuditActivitySignals::default(),
            behavior: AuditBehavior::default(),
            summary: AuditSummary::default(),
            findings: Vec::new(),
            knowledge_files: Vec::new(),
        }
    }

    fn temp_project(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "tokenuse-{name}-{}",
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }
}
