use std::{
    collections::{BTreeMap, HashMap},
    ffi::OsString,
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use chrono::{DateTime, Utc};
use color_eyre::{
    eyre::{eyre, Context},
    Result,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{
    config::ConfigPaths,
    ingest::Ingested,
    pricing::{configured_book_status, PricingBookSource},
    tools::ParsedCall,
};

const SYSTEM_PROMPT_FILE: &str = "system.md";
const USER_REDACTED_PROMPT_FILE: &str = "user-redacted.md";
const USER_SNIPPETS_PROMPT_FILE: &str = "user-snippets.md";
const TOKEN_USE_APP_PROJECT: &str = "Token Use App";
const MAX_PROMPT_SNIPPETS: usize = 12;
const MAX_PROMPT_SNIPPET_CHARS: usize = 220;

const DEFAULT_PROMPT_FILES: [&str; 3] = [
    SYSTEM_PROMPT_FILE,
    USER_REDACTED_PROMPT_FILE,
    USER_SNIPPETS_PROMPT_FILE,
];

const OUTPUT_SCHEMA: &str = r#"{
  "type": "object",
  "required": ["items"],
  "properties": {
    "summary": { "type": "string" },
    "items": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["title", "body", "category", "severity", "confidence", "impact", "evidence", "next_step"],
        "properties": {
          "title": { "type": "string" },
          "body": { "type": "string" },
          "category": { "type": "string" },
          "severity": { "type": "string", "enum": ["risk", "warn", "info"] },
          "confidence": { "type": "number", "minimum": 0, "maximum": 1 },
          "impact": { "type": "string" },
          "estimated_savings_usd": { "type": ["number", "null"] },
          "evidence": { "type": "array", "items": { "type": "string" } },
          "next_step": { "type": "string" }
        }
      }
    }
  }
}"#;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AdviceTool {
    Codex,
    ClaudeCode,
    Gemini,
}

impl AdviceTool {
    pub const ALL: [Self; 3] = [Self::Codex, Self::ClaudeCode, Self::Gemini];

    pub fn id(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::ClaudeCode => "claude-code",
            Self::Gemini => "gemini",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Codex => "Codex",
            Self::ClaudeCode => "Claude Code",
            Self::Gemini => "Gemini",
        }
    }

    pub fn binary(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::ClaudeCode => "claude",
            Self::Gemini => "gemini",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "codex" => Some(Self::Codex),
            "claude-code" => Some(Self::ClaudeCode),
            "gemini" => Some(Self::Gemini),
            _ => None,
        }
    }

    pub fn from_config(id: &str) -> Self {
        Self::from_id(id).unwrap_or(Self::Codex)
    }

    fn command_spec(self, prompts: RenderedPrompts, app_dir: &Path) -> CommandSpec {
        match self {
            Self::Codex => CommandSpec {
                program: self.binary().into(),
                args: vec![
                    "exec".into(),
                    "--cd".into(),
                    app_dir.as_os_str().to_os_string(),
                    "--sandbox".into(),
                    "read-only".into(),
                    "--ask-for-approval".into(),
                    "never".into(),
                    "-".into(),
                ],
                cwd: app_dir.to_path_buf(),
                stdin: Some(prompts.combined()),
            },
            Self::ClaudeCode => CommandSpec {
                program: self.binary().into(),
                args: vec![
                    "--print".into(),
                    "--permission-mode".into(),
                    "plan".into(),
                    "--output-format".into(),
                    "text".into(),
                    "--system-prompt".into(),
                    prompts.system.into(),
                    prompts.user.into(),
                ],
                cwd: app_dir.to_path_buf(),
                stdin: None,
            },
            Self::Gemini => CommandSpec {
                program: self.binary().into(),
                args: vec![
                    "--prompt".into(),
                    prompts.combined().into(),
                    "--approval-mode".into(),
                    "plan".into(),
                    "--output-format".into(),
                    "text".into(),
                    "--skip-trust".into(),
                ],
                cwd: app_dir.to_path_buf(),
                stdin: None,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdviceDataScope {
    Redacted,
    PromptSnippets,
}

impl AdviceDataScope {
    pub fn id(self) -> &'static str {
        match self {
            Self::Redacted => "redacted",
            Self::PromptSnippets => "prompt_snippets",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "redacted" => Some(Self::Redacted),
            "prompt_snippets" => Some(Self::PromptSnippets),
            _ => None,
        }
    }

    fn prompt_file(self) -> &'static str {
        match self {
            Self::Redacted => USER_REDACTED_PROMPT_FILE,
            Self::PromptSnippets => USER_SNIPPETS_PROMPT_FILE,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdviceRunStatus {
    Succeeded,
    Failed,
}

impl AdviceRunStatus {
    pub fn id(self) -> &'static str {
        match self {
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdviceItemStatus {
    Open,
    Done,
    Dismissed,
}

impl AdviceItemStatus {
    pub fn id(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Done => "done",
            Self::Dismissed => "dismissed",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "open" => Some(Self::Open),
            "done" => Some(Self::Done),
            "dismissed" => Some(Self::Dismissed),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct AdviceHistory {
    pub runs: Vec<AdviceRunView>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AdviceRunView {
    pub id: i64,
    pub created_at: DateTime<Utc>,
    pub tool: String,
    pub tool_label: String,
    pub data_scope: String,
    pub status: String,
    pub summary: Option<String>,
    pub raw_output: String,
    pub error: Option<String>,
    pub items: Vec<AdviceItemView>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AdviceItemView {
    pub id: i64,
    pub run_id: i64,
    pub title: String,
    pub body: String,
    pub category: String,
    pub severity: String,
    pub confidence: f64,
    pub impact: String,
    pub estimated_savings_usd: Option<f64>,
    pub evidence: Vec<String>,
    pub next_step: String,
    pub status: String,
    pub notes: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AdviceRunInsert {
    pub created_at: DateTime<Utc>,
    pub tool: AdviceTool,
    pub data_scope: AdviceDataScope,
    pub status: AdviceRunStatus,
    pub prompt_digest: String,
    pub summary: Option<String>,
    pub raw_output: String,
    pub error: Option<String>,
    pub items: Vec<AdviceItemInsert>,
}

#[derive(Debug, Clone)]
pub struct AdviceItemInsert {
    pub title: String,
    pub body: String,
    pub category: String,
    pub severity: String,
    pub confidence: f64,
    pub impact: String,
    pub estimated_savings_usd: Option<f64>,
    pub evidence: Vec<String>,
    pub next_step: String,
}

#[derive(Debug, Clone)]
pub struct PromptFileStatus {
    pub ready: bool,
    pub missing: Vec<&'static str>,
    pub dir: PathBuf,
}

impl PromptFileStatus {
    pub fn label(&self) -> String {
        if self.ready {
            format!("Prompt files ready · {}", self.dir.display())
        } else {
            format!(
                "Prompt files missing ({}) · {}",
                self.missing.join(", "),
                self.dir.display()
            )
        }
    }
}

#[derive(Debug, Clone)]
pub struct CommandSpec {
    pub program: OsString,
    pub args: Vec<OsString>,
    pub cwd: PathBuf,
    pub stdin: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CommandOutput {
    pub success: bool,
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

pub trait AdviceCommandExecutor {
    fn output(&self, spec: CommandSpec) -> Result<CommandOutput>;
}

pub struct StdAdviceCommandExecutor;

impl AdviceCommandExecutor for StdAdviceCommandExecutor {
    fn output(&self, spec: CommandSpec) -> Result<CommandOutput> {
        let mut child = Command::new(&spec.program)
            .args(&spec.args)
            .current_dir(&spec.cwd)
            .stdin(if spec.stdin.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .wrap_err_with(|| format!("start {}", spec.program.to_string_lossy()))?;

        if let Some(stdin) = spec.stdin {
            let Some(mut child_stdin) = child.stdin.take() else {
                return Err(eyre!("stdin unavailable for advice command"));
            };
            child_stdin
                .write_all(stdin.as_bytes())
                .wrap_err("write advice prompt to stdin")?;
        }

        let output = child.wait_with_output().wrap_err("wait for advice tool")?;
        Ok(CommandOutput {
            success: output.status.success(),
            code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }
}

pub fn prompt_file_status(paths: &ConfigPaths) -> PromptFileStatus {
    let missing: Vec<&'static str> = DEFAULT_PROMPT_FILES
        .iter()
        .filter_map(|name| (!paths.advice_prompts_dir.join(name).exists()).then_some(*name))
        .collect();
    PromptFileStatus {
        ready: missing.is_empty(),
        missing,
        dir: paths.advice_prompts_dir.clone(),
    }
}

pub fn ensure_prompt_files(paths: &ConfigPaths) -> Result<()> {
    fs::create_dir_all(&paths.advice_prompts_dir)
        .wrap_err_with(|| format!("create {}", paths.advice_prompts_dir.display()))?;
    let source_dir = default_prompt_source_dir()?;
    for name in DEFAULT_PROMPT_FILES {
        let path = paths.advice_prompts_dir.join(name);
        if !path.exists() {
            fs::copy(source_dir.join(name), &path).wrap_err_with(|| {
                format!("copy default advice prompt {} to {}", name, path.display())
            })?;
        }
    }
    Ok(())
}

fn default_prompt_source_dir() -> Result<PathBuf> {
    default_prompt_source_candidates()
        .into_iter()
        .find(|dir| {
            DEFAULT_PROMPT_FILES
                .iter()
                .all(|name| dir.join(*name).is_file())
        })
        .ok_or_else(|| {
            eyre!(
                "default advice prompt templates not found; expected {}",
                DEFAULT_PROMPT_FILES.join(", ")
            )
        })
}

fn default_prompt_source_candidates() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(dir) = std::env::var_os("TOKENUSE_ADVICE_PROMPTS_DIR") {
        dirs.push(PathBuf::from(dir));
    }
    if let Ok(current_dir) = std::env::current_dir() {
        dirs.push(current_dir.join("config").join("advice-prompts"));
    }
    dirs.push(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("config")
            .join("advice-prompts"),
    );
    if let Ok(exe) = std::env::current_exe() {
        if let Some(bin_dir) = exe.parent() {
            dirs.push(bin_dir.join("advice-prompts"));
            dirs.push(bin_dir.join("config").join("advice-prompts"));
            if let Some(contents_dir) = bin_dir.parent() {
                dirs.push(contents_dir.join("Resources").join("advice-prompts"));
            }
        }
    }
    dirs
}

pub fn tool_available(tool: AdviceTool) -> bool {
    find_executable(tool.binary()).is_some()
}

pub fn generate_advice_run(
    ingested: &Ingested,
    paths: &ConfigPaths,
    tool: AdviceTool,
    data_scope: AdviceDataScope,
) -> AdviceRunInsert {
    generate_advice_run_with_executor(ingested, paths, tool, data_scope, &StdAdviceCommandExecutor)
}

pub fn generate_advice_run_with_executor(
    ingested: &Ingested,
    paths: &ConfigPaths,
    tool: AdviceTool,
    data_scope: AdviceDataScope,
    executor: &dyn AdviceCommandExecutor,
) -> AdviceRunInsert {
    let created_at = Utc::now();
    match run_advice(ingested, paths, tool, data_scope, executor) {
        Ok(run) => run,
        Err(error) => AdviceRunInsert {
            created_at,
            tool,
            data_scope,
            status: AdviceRunStatus::Failed,
            prompt_digest: String::new(),
            summary: None,
            raw_output: String::new(),
            error: Some(error.to_string()),
            items: Vec::new(),
        },
    }
}

fn run_advice(
    ingested: &Ingested,
    paths: &ConfigPaths,
    tool: AdviceTool,
    data_scope: AdviceDataScope,
    executor: &dyn AdviceCommandExecutor,
) -> Result<AdviceRunInsert> {
    ensure_prompt_files(paths)?;
    fs::create_dir_all(&paths.token_use_app_dir)
        .wrap_err_with(|| format!("create {}", paths.token_use_app_dir.display()))?;

    let prompts = load_rendered_prompts(ingested, paths, data_scope)?;
    let digest = prompt_digest(&prompts.combined());
    let spec = tool.command_spec(prompts, &paths.token_use_app_dir);
    let output = executor.output(spec)?;
    let raw_output = if output.stdout.trim().is_empty() {
        output.stderr.clone()
    } else {
        output.stdout.clone()
    };

    if !output.success {
        return Ok(AdviceRunInsert {
            created_at: Utc::now(),
            tool,
            data_scope,
            status: AdviceRunStatus::Failed,
            prompt_digest: digest,
            summary: None,
            raw_output,
            error: Some(command_error(&output)),
            items: Vec::new(),
        });
    }

    match parse_advice_response(&raw_output) {
        Ok(parsed) => Ok(AdviceRunInsert {
            created_at: Utc::now(),
            tool,
            data_scope,
            status: AdviceRunStatus::Succeeded,
            prompt_digest: digest,
            summary: parsed.summary,
            raw_output,
            error: None,
            items: parsed.items,
        }),
        Err(error) => Ok(AdviceRunInsert {
            created_at: Utc::now(),
            tool,
            data_scope,
            status: AdviceRunStatus::Failed,
            prompt_digest: digest,
            summary: None,
            raw_output,
            error: Some(format!("parse advice JSON: {error}")),
            items: Vec::new(),
        }),
    }
}

fn command_error(output: &CommandOutput) -> String {
    let code = output
        .code
        .map(|code| code.to_string())
        .unwrap_or_else(|| "signal".into());
    let stderr = output.stderr.trim();
    if stderr.is_empty() {
        format!("advice tool exited with status {code}")
    } else {
        format!("advice tool exited with status {code}: {stderr}")
    }
}

#[derive(Debug, Clone)]
struct RenderedPrompts {
    system: String,
    user: String,
}

impl RenderedPrompts {
    fn combined(&self) -> String {
        format!(
            "System instructions:\n{}\n\nUser request:\n{}",
            self.system, self.user
        )
    }
}

fn load_rendered_prompts(
    ingested: &Ingested,
    paths: &ConfigPaths,
    data_scope: AdviceDataScope,
) -> Result<RenderedPrompts> {
    let system = read_prompt_file(paths, SYSTEM_PROMPT_FILE)?;
    let user_template = read_prompt_file(paths, data_scope.prompt_file())?;
    let variables = prompt_variables(ingested, paths, data_scope)?;
    let user = render_template(&user_template, &variables);
    Ok(RenderedPrompts { system, user })
}

fn read_prompt_file(paths: &ConfigPaths, name: &str) -> Result<String> {
    let path = paths.advice_prompts_dir.join(name);
    fs::read_to_string(&path).wrap_err_with(|| format!("read {}", path.display()))
}

fn prompt_variables(
    ingested: &Ingested,
    paths: &ConfigPaths,
    data_scope: AdviceDataScope,
) -> Result<HashMap<&'static str, String>> {
    let mut vars = HashMap::new();
    vars.insert("data_scope", data_scope.id().to_string());
    vars.insert("signals_json", signals_json(ingested)?);
    vars.insert("pricing_context", pricing_context_json(paths)?);
    vars.insert("output_schema", OUTPUT_SCHEMA.to_string());
    vars.insert(
        "prompt_snippets_json",
        prompt_snippets_json(ingested, data_scope)?,
    );
    Ok(vars)
}

fn render_template(template: &str, variables: &HashMap<&'static str, String>) -> String {
    variables
        .iter()
        .fold(template.to_string(), |out, (key, value)| {
            out.replace(&format!("{{{key}}}"), value)
        })
}

fn signals_json(ingested: &Ingested) -> Result<String> {
    let insights = ingested.insights();
    let mut tool_counts: BTreeMap<&'static str, u64> = BTreeMap::new();
    let mut model_counts: BTreeMap<String, u64> = BTreeMap::new();
    let mut total_cost_usd = 0.0;
    for call in &ingested.calls {
        *tool_counts.entry(call.tool).or_default() += 1;
        *model_counts.entry(call.model.clone()).or_default() += 1;
        total_cost_usd += call.cost_usd;
    }

    serde_json::to_string_pretty(&json!({
        "generated_at": insights.generated_at,
        "baseline_window_days": insights.baseline_window_days,
        "summary": insights.summary,
        "signals": insights.recommendations,
        "dataset": {
            "calls": ingested.calls.len(),
            "limits": ingested.limits.len(),
            "total_cost_usd": total_cost_usd,
            "tool_counts": tool_counts,
            "model_counts": model_counts,
            "data_quality": {
                "estimated_token_tools": ["cursor", "copilot"],
                "cache_reporting_tools": ["claude-code", "codex", "gemini"]
            }
        }
    }))
    .map_err(Into::into)
}

fn pricing_context_json(paths: &ConfigPaths) -> Result<String> {
    let status = configured_book_status(paths);
    let source = match status.source {
        PricingBookSource::LocalBooks => "local_books",
        PricingBookSource::LegacySnapshot => "legacy_snapshot",
        PricingBookSource::EmbeddedBooks => "embedded_books",
    };
    serde_json::to_string_pretty(&json!({
        "source": source,
        "date": status.date,
        "note": "Costs are local Token Use estimates from configured pricing books."
    }))
    .map_err(Into::into)
}

fn prompt_snippets_json(ingested: &Ingested, data_scope: AdviceDataScope) -> Result<String> {
    if data_scope != AdviceDataScope::PromptSnippets {
        return Ok("[]".into());
    }
    let mut calls: Vec<&ParsedCall> = ingested
        .calls
        .iter()
        .filter(|call| !call.user_message.trim().is_empty())
        .collect();
    calls.sort_by(|a, b| {
        b.cost_usd
            .partial_cmp(&a.cost_usd)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let snippets: Vec<_> = calls
        .into_iter()
        .take(MAX_PROMPT_SNIPPETS)
        .map(|call| {
            json!({
                "tool": call.tool,
                "project": call.project,
                "model": call.model,
                "cost_usd": call.cost_usd,
                "prompt_snippet": truncate_clean(&call.user_message, MAX_PROMPT_SNIPPET_CHARS)
            })
        })
        .collect();
    serde_json::to_string_pretty(&snippets).map_err(Into::into)
}

#[derive(Debug, Deserialize)]
struct AdviceResponse {
    #[serde(default)]
    summary: Option<String>,
    #[serde(default)]
    items: Vec<AdviceResponseItem>,
}

#[derive(Debug, Deserialize)]
struct AdviceResponseItem {
    title: String,
    body: String,
    category: String,
    severity: String,
    confidence: f64,
    impact: String,
    #[serde(default)]
    estimated_savings_usd: Option<f64>,
    #[serde(default)]
    evidence: Vec<String>,
    next_step: String,
}

struct ParsedAdviceResponse {
    summary: Option<String>,
    items: Vec<AdviceItemInsert>,
}

fn parse_advice_response(raw: &str) -> Result<ParsedAdviceResponse> {
    let json = extract_json_object(raw).ok_or_else(|| eyre!("no JSON object found"))?;
    let response: AdviceResponse = serde_json::from_str(json)?;
    let items = response
        .items
        .into_iter()
        .filter(|item| !item.title.trim().is_empty() && !item.body.trim().is_empty())
        .map(|item| AdviceItemInsert {
            title: truncate_clean(&item.title, 180),
            body: truncate_clean(&item.body, 1_000),
            category: truncate_clean(&item.category, 80),
            severity: normalize_severity(&item.severity).to_string(),
            confidence: item.confidence.clamp(0.0, 1.0),
            impact: truncate_clean(&item.impact, 500),
            estimated_savings_usd: item
                .estimated_savings_usd
                .filter(|amount| amount.is_finite() && *amount >= 0.0),
            evidence: item
                .evidence
                .into_iter()
                .map(|e| truncate_clean(&e, 220))
                .filter(|e| !e.is_empty())
                .collect(),
            next_step: truncate_clean(&item.next_step, 500),
        })
        .collect();
    Ok(ParsedAdviceResponse {
        summary: response.summary.map(|s| truncate_clean(&s, 1_000)),
        items,
    })
}

fn extract_json_object(raw: &str) -> Option<&str> {
    let start = raw.find('{')?;
    let end = raw.rfind('}')?;
    (start <= end).then_some(&raw[start..=end])
}

fn normalize_severity(raw: &str) -> &'static str {
    match raw {
        "risk" => "risk",
        "warn" | "warning" => "warn",
        _ => "info",
    }
}

fn truncate_clean(raw: &str, max_chars: usize) -> String {
    let clean = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    if clean.chars().count() <= max_chars {
        return clean;
    }
    let mut out: String = clean.chars().take(max_chars.saturating_sub(1)).collect();
    out.push_str("...");
    out
}

fn prompt_digest(prompt: &str) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in prompt.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

fn find_executable(program: &str) -> Option<PathBuf> {
    let candidate = Path::new(program);
    if candidate.components().count() > 1 && candidate.is_file() {
        return Some(candidate.to_path_buf());
    }

    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(program);
        if candidate.is_file() {
            return Some(candidate);
        }
        #[cfg(windows)]
        {
            let candidate = dir.join(format!("{program}.exe"));
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

pub fn token_use_app_project() -> &'static str {
    TOKEN_USE_APP_PROJECT
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::insights::fixtures::{now, CallBuilder};

    struct MockExecutor {
        output: CommandOutput,
    }

    impl AdviceCommandExecutor for MockExecutor {
        fn output(&self, _spec: CommandSpec) -> Result<CommandOutput> {
            Ok(self.output.clone())
        }
    }

    fn temp_paths(name: &str) -> ConfigPaths {
        let dir = std::env::temp_dir().join(format!(
            "tokenuse-advice-{name}-{}",
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        ConfigPaths::new(dir)
    }

    #[test]
    fn ensure_prompt_files_copies_defaults_once() {
        let paths = temp_paths("prompts");
        assert!(!prompt_file_status(&paths).ready);

        ensure_prompt_files(&paths).unwrap();
        assert!(prompt_file_status(&paths).ready);

        let system_path = paths.advice_prompts_dir.join(SYSTEM_PROMPT_FILE);
        fs::write(&system_path, "custom").unwrap();
        ensure_prompt_files(&paths).unwrap();
        assert_eq!(fs::read_to_string(system_path).unwrap(), "custom");

        let _ = fs::remove_dir_all(paths.dir);
    }

    #[test]
    fn template_render_replaces_known_variables() {
        let mut vars = HashMap::new();
        vars.insert("signals_json", "{}".to_string());
        vars.insert("data_scope", "redacted".to_string());

        let got = render_template("{data_scope} {signals_json}", &vars);
        assert_eq!(got, "redacted {}");
    }

    #[test]
    fn redacted_scope_omits_prompt_snippets() {
        let paths = temp_paths("redacted");
        let ingested = Ingested {
            calls: vec![CallBuilder::new("codex", "gpt-5", "alpha")
                .at(1)
                .input(100)
                .output(40)
                .cost(0.1)
                .build()],
            limits: Vec::new(),
        };
        let snippets = prompt_snippets_json(&ingested, AdviceDataScope::Redacted).unwrap();
        assert_eq!(snippets, "[]");
        let _ = fs::remove_dir_all(paths.dir);
    }

    #[test]
    fn snippets_scope_includes_limited_user_messages() {
        let mut call = CallBuilder::new("codex", "gpt-5", "alpha")
            .at(1)
            .input(100)
            .output(40)
            .cost(0.1)
            .build();
        call.user_message = "please inspect the cache regression in this project".into();
        let ingested = Ingested {
            calls: vec![call],
            limits: Vec::new(),
        };

        let snippets = prompt_snippets_json(&ingested, AdviceDataScope::PromptSnippets).unwrap();
        assert!(snippets.contains("cache regression"));
    }

    #[test]
    fn parses_json_even_when_wrapped_in_text() {
        let raw = r#"Here:
        {"summary":"ok","items":[{"title":"Review cache","body":"Cache fell.","category":"cache","severity":"warn","confidence":0.8,"impact":"medium","estimated_savings_usd":1.2,"evidence":["cache_hit_trend_drop:project=a"],"next_step":"Inspect prompts."}]}
        done"#;

        let parsed = parse_advice_response(raw).unwrap();
        assert_eq!(parsed.items.len(), 1);
        assert_eq!(parsed.items[0].severity, "warn");
        assert_eq!(parsed.items[0].confidence, 0.8);
    }

    #[test]
    fn missing_prompt_file_reports_clear_path() {
        let paths = temp_paths("missing-prompt");
        ensure_prompt_files(&paths).unwrap();
        fs::remove_file(paths.advice_prompts_dir.join(USER_REDACTED_PROMPT_FILE)).unwrap();
        let ingested = Ingested {
            calls: Vec::new(),
            limits: Vec::new(),
        };

        let error = load_rendered_prompts(&ingested, &paths, AdviceDataScope::Redacted)
            .unwrap_err()
            .to_string();
        assert!(error.contains(USER_REDACTED_PROMPT_FILE));
        let _ = fs::remove_dir_all(paths.dir);
    }

    #[test]
    fn runner_specs_use_noninteractive_planning_modes() {
        let prompts = RenderedPrompts {
            system: "system".into(),
            user: "user".into(),
        };
        let app_dir = PathBuf::from("/tmp/Token Use App");

        let codex = AdviceTool::Codex.command_spec(prompts.clone(), &app_dir);
        let codex_args = args(&codex);
        let combined = prompts.combined();
        assert_eq!(codex.program.to_string_lossy(), "codex");
        assert!(codex_args.contains(&"exec".to_string()));
        assert!(codex_args.contains(&"--sandbox".to_string()));
        assert!(codex_args.contains(&"read-only".to_string()));
        assert_eq!(codex.stdin.as_deref(), Some(combined.as_str()));

        let claude = AdviceTool::ClaudeCode.command_spec(prompts.clone(), &app_dir);
        let claude_args = args(&claude);
        assert_eq!(claude.program.to_string_lossy(), "claude");
        assert!(claude_args.contains(&"--print".to_string()));
        assert!(claude_args.contains(&"--permission-mode".to_string()));
        assert!(claude_args.contains(&"plan".to_string()));
        assert!(claude.stdin.is_none());

        let gemini = AdviceTool::Gemini.command_spec(prompts, &app_dir);
        let gemini_args = args(&gemini);
        assert_eq!(gemini.program.to_string_lossy(), "gemini");
        assert!(gemini_args.contains(&"--prompt".to_string()));
        assert!(gemini_args.contains(&"--approval-mode".to_string()));
        assert!(gemini_args.contains(&"plan".to_string()));
        assert!(gemini.stdin.is_none());
    }

    fn args(spec: &CommandSpec) -> Vec<String> {
        spec.args
            .iter()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect()
    }

    #[test]
    fn failed_command_is_stored_as_failed_run() {
        let paths = temp_paths("failed-run");
        let ingested = Ingested {
            calls: Vec::new(),
            limits: Vec::new(),
        };
        let executor = MockExecutor {
            output: CommandOutput {
                success: false,
                code: Some(2),
                stdout: String::new(),
                stderr: "auth missing".into(),
            },
        };

        let run = generate_advice_run_with_executor(
            &ingested,
            &paths,
            AdviceTool::Codex,
            AdviceDataScope::Redacted,
            &executor,
        );
        assert_eq!(run.status, AdviceRunStatus::Failed);
        assert!(run.error.unwrap().contains("auth missing"));
        let _ = fs::remove_dir_all(paths.dir);
    }

    #[test]
    fn successful_command_parses_items() {
        let paths = temp_paths("ok-run");
        let ingested = Ingested {
            calls: Vec::new(),
            limits: Vec::new(),
        };
        let executor = MockExecutor {
            output: CommandOutput {
                success: true,
                code: Some(0),
                stdout: r#"{"items":[{"title":"Right-size","body":"Try a smaller model.","category":"model_rightsizing","severity":"info","confidence":0.6,"impact":"low","evidence":["short_output_sonnet:project=a"],"next_step":"Sample 5 calls."}]}"#.into(),
                stderr: String::new(),
            },
        };

        let run = generate_advice_run_with_executor(
            &ingested,
            &paths,
            AdviceTool::Codex,
            AdviceDataScope::Redacted,
            &executor,
        );
        assert_eq!(run.status, AdviceRunStatus::Succeeded);
        assert_eq!(run.items.len(), 1);
        assert_eq!(run.items[0].title, "Right-size");
        let _ = fs::remove_dir_all(paths.dir);
    }

    #[test]
    fn invalid_json_run_keeps_raw_output_for_review() {
        let paths = temp_paths("invalid-json");
        let ingested = Ingested {
            calls: Vec::new(),
            limits: Vec::new(),
        };
        let executor = MockExecutor {
            output: CommandOutput {
                success: true,
                code: Some(0),
                stdout: String::new(),
                stderr: "not json".into(),
            },
        };

        let run = generate_advice_run_with_executor(
            &ingested,
            &paths,
            AdviceTool::Codex,
            AdviceDataScope::Redacted,
            &executor,
        );
        assert_eq!(run.status, AdviceRunStatus::Failed);
        assert_eq!(run.raw_output, "not json");
        assert!(run.error.unwrap().contains("parse advice JSON"));
        let _ = fs::remove_dir_all(paths.dir);
    }

    #[test]
    fn mocked_runners_cover_supported_tools() {
        let paths = temp_paths("tools");
        let ingested = Ingested {
            calls: Vec::new(),
            limits: Vec::new(),
        };
        for tool in AdviceTool::ALL {
            let executor = MockExecutor {
                output: CommandOutput {
                    success: true,
                    code: Some(0),
                    stdout: r#"{"items":[{"title":"Right-size","body":"Try a smaller model.","category":"model_rightsizing","severity":"info","confidence":0.6,"impact":"low","evidence":["short_output_sonnet sample=50 confidence=0.6"],"next_step":"Sample 5 calls."}]}"#.into(),
                    stderr: String::new(),
                },
            };
            let run = generate_advice_run_with_executor(
                &ingested,
                &paths,
                tool,
                AdviceDataScope::Redacted,
                &executor,
            );
            assert_eq!(run.status, AdviceRunStatus::Succeeded);
            assert_eq!(run.tool, tool);
            assert_eq!(run.items.len(), 1);
        }
        let _ = fs::remove_dir_all(paths.dir);
    }

    #[test]
    fn prompt_digest_is_deterministic() {
        assert_eq!(prompt_digest("abc"), prompt_digest("abc"));
        assert_ne!(prompt_digest("abc"), prompt_digest("abcd"));
        let _ = now();
    }
}
