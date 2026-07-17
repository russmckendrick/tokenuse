//! Deterministic task categories classified from a call's tool usage and its
//! stored prompt prefix. No LLM and no network: the same call always lands in
//! the same category. Tool patterns decide first; keyword refinement on the
//! user message only narrows a tool-derived category, using
//! first-match-by-position so "add error handling" reads as feature work,
//! not debugging.

use crate::tools::ParsedCall;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TaskCategory {
    Coding,
    Debugging,
    Feature,
    Refactoring,
    Testing,
    Exploration,
    Planning,
    Delegation,
    Git,
    BuildDeploy,
    Brainstorming,
    Conversation,
    General,
}

impl TaskCategory {
    /// Stable id used as the copy key (`categories.<id>`) and in JSON output.
    pub fn id(self) -> &'static str {
        match self {
            Self::Coding => "coding",
            Self::Debugging => "debugging",
            Self::Feature => "feature",
            Self::Refactoring => "refactoring",
            Self::Testing => "testing",
            Self::Exploration => "exploration",
            Self::Planning => "planning",
            Self::Delegation => "delegation",
            Self::Git => "git",
            Self::BuildDeploy => "build_deploy",
            Self::Brainstorming => "brainstorming",
            Self::Conversation => "conversation",
            Self::General => "general",
        }
    }

    pub fn label(self) -> &'static str {
        let copy = crate::copy::copy();
        copy.categories
            .get(self.id())
            .map(String::as_str)
            .unwrap_or_else(|| self.id())
    }
}

const EDIT_TOOLS: &[&str] = &["Edit", "Write", "MultiEdit", "NotebookEdit", "Delete"];
const READ_TOOLS: &[&str] = &["Read", "Grep", "Glob", "LS"];
const BASH_TOOLS: &[&str] = &["Bash", "BashOutput", "Shell"];
const PLAN_TOOLS: &[&str] = &["EnterPlanMode", "ExitPlanMode"];
const AGENT_TOOLS: &[&str] = &["Agent", "Task"];
const TASK_TOOLS: &[&str] = &[
    "TaskCreate",
    "TaskUpdate",
    "TaskList",
    "TaskGet",
    "TaskOutput",
    "TaskStop",
    "TodoWrite",
];
const SEARCH_TOOLS: &[&str] = &["WebSearch", "WebFetch", "ToolSearch"];
const SKILL_TOOLS: &[&str] = &["Skill"];

const DEBUG_KEYWORDS: &[&str] = &[
    "fix",
    "bug",
    "error",
    "broken",
    "failing",
    "fails",
    "crash",
    "debug",
    "issue",
    "wrong",
    "exception",
    "traceback",
    "regression",
    "404",
    "500",
    "401",
    "403",
];
const FEATURE_KEYWORDS: &[&str] = &[
    "add",
    "create",
    "implement",
    "new",
    "build",
    "feature",
    "support",
    "introduce",
];
const REFACTOR_KEYWORDS: &[&str] = &[
    "refactor",
    "rename",
    "simplify",
    "restructure",
    "clean up",
    "cleanup",
    "reorganize",
    "extract",
    "dedupe",
];
const TEST_KEYWORDS: &[&str] = &["test", "tests", "testing", "unit test", "coverage"];
const RESEARCH_KEYWORDS: &[&str] = &[
    "how does",
    "what is",
    "why does",
    "explain",
    "understand",
    "investigate",
    "research",
    "compare",
    "look at",
];
const BRAINSTORM_KEYWORDS: &[&str] = &[
    "brainstorm",
    "what if",
    "design",
    "idea",
    "ideas",
    "approach",
    "trade-off",
    "tradeoff",
    "options",
];

const TEST_RUNNERS: &[&str] = &[
    "pytest", "vitest", "jest", "rspec", "phpunit", "gotest", "tox",
];
const GIT_COMMANDS: &[&str] = &[
    "git commit",
    "git push",
    "git merge",
    "git rebase",
    "git pull",
    "git checkout",
    "git tag",
    "gh pr",
    "gh release",
];
const BUILD_COMMANDS: &[&str] = &[
    "npm run build",
    "npm install",
    "pnpm install",
    "pip install",
    "brew install",
    "cargo build",
    "cargo install",
    "docker",
    "make",
    "deploy",
    "pm2",
];

pub fn classify(call: &ParsedCall) -> TaskCategory {
    if call.tools.is_empty() && call.bash_commands.is_empty() {
        return classify_conversation(&call.user_message.to_lowercase());
    }

    let has = |set: &[&str]| call.tools.iter().any(|tool| set.contains(&tool.as_str()));
    if has(PLAN_TOOLS) {
        return TaskCategory::Planning;
    }
    if has(AGENT_TOOLS) {
        return TaskCategory::Delegation;
    }

    let message = call.user_message.to_lowercase();
    let has_edits = has(EDIT_TOOLS);
    let has_reads = has(READ_TOOLS);
    let has_bash = has(BASH_TOOLS) || !call.bash_commands.is_empty();

    if has_bash && !has_edits {
        if is_test_activity(&call.bash_commands, &message) {
            return TaskCategory::Testing;
        }
        if bash_matches(&call.bash_commands, GIT_COMMANDS) {
            return TaskCategory::Git;
        }
        if bash_matches(&call.bash_commands, BUILD_COMMANDS) {
            return TaskCategory::BuildDeploy;
        }
    }
    if has_edits {
        return refine_coding(&message);
    }
    if has_bash && has_reads {
        return TaskCategory::Exploration;
    }
    if has_bash {
        return TaskCategory::Coding;
    }
    if has(SEARCH_TOOLS) || call.tools.iter().any(|tool| tool.starts_with("mcp__")) {
        return TaskCategory::Exploration;
    }
    if has_reads {
        return exploration_or_debugging(&message);
    }
    if has(TASK_TOOLS) {
        return TaskCategory::Planning;
    }
    if has(SKILL_TOOLS) {
        return TaskCategory::General;
    }
    TaskCategory::General
}

/// A coding turn narrows to refactoring/feature/debugging by whichever
/// keyword set matches EARLIEST in the message (not by check order); ties
/// break refactoring > feature > debugging.
fn refine_coding(message: &str) -> TaskCategory {
    let candidates = [
        (
            first_hit(message, REFACTOR_KEYWORDS),
            TaskCategory::Refactoring,
        ),
        (first_hit(message, FEATURE_KEYWORDS), TaskCategory::Feature),
        (first_hit(message, DEBUG_KEYWORDS), TaskCategory::Debugging),
    ];
    candidates
        .into_iter()
        .filter_map(|(hit, category)| hit.map(|index| (index, category)))
        .min_by_key(|(index, _)| *index)
        .map(|(_, category)| category)
        .unwrap_or(TaskCategory::Coding)
}

fn exploration_or_debugging(message: &str) -> TaskCategory {
    if first_hit(message, RESEARCH_KEYWORDS).is_some() {
        return TaskCategory::Exploration;
    }
    if first_hit(message, DEBUG_KEYWORDS).is_some() {
        return TaskCategory::Debugging;
    }
    TaskCategory::Exploration
}

fn classify_conversation(message: &str) -> TaskCategory {
    if message.trim().is_empty() {
        return TaskCategory::Conversation;
    }
    if first_hit(message, BRAINSTORM_KEYWORDS).is_some() {
        return TaskCategory::Brainstorming;
    }
    if first_hit(message, RESEARCH_KEYWORDS).is_some() {
        return TaskCategory::Exploration;
    }
    let feature = first_hit(message, FEATURE_KEYWORDS);
    let debug = first_hit(message, DEBUG_KEYWORDS);
    match (feature, debug) {
        (Some(f), Some(d)) if f <= d => return TaskCategory::Feature,
        (Some(_), Some(_)) => return TaskCategory::Debugging,
        (Some(_), None) => return TaskCategory::Feature,
        (None, Some(_)) => return TaskCategory::Debugging,
        (None, None) => {}
    }
    if message.contains("http://") || message.contains("https://") {
        return TaskCategory::Exploration;
    }
    TaskCategory::Conversation
}

fn is_test_activity(bash_commands: &[String], message: &str) -> bool {
    let runs_tests = bash_commands.iter().any(|command| {
        let lower = command.to_lowercase();
        TEST_RUNNERS
            .iter()
            .any(|runner| phrase_index(&lower, runner).is_some())
            || phrase_index(&lower, "cargo test").is_some()
            || phrase_index(&lower, "npm test").is_some()
            || phrase_index(&lower, "go test").is_some()
    });
    runs_tests || first_hit(message, TEST_KEYWORDS).is_some()
}

fn bash_matches(bash_commands: &[String], patterns: &[&str]) -> bool {
    bash_commands.iter().any(|command| {
        let lower = command.to_lowercase();
        patterns
            .iter()
            .any(|pattern| phrase_index(&lower, pattern) == Some(0))
    })
}

/// Byte index of the earliest word-boundary match of any phrase in the set.
fn first_hit(message: &str, phrases: &[&str]) -> Option<usize> {
    phrases
        .iter()
        .filter_map(|phrase| phrase_index(message, phrase))
        .min()
}

/// Word-boundary phrase search over an already-lowercased haystack.
fn phrase_index(haystack: &str, phrase: &str) -> Option<usize> {
    let mut from = 0;
    while let Some(offset) = haystack[from..].find(phrase) {
        let start = from + offset;
        let end = start + phrase.len();
        let boundary_before = start == 0
            || !haystack[..start]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_alphanumeric());
        let boundary_after = end == haystack.len()
            || !haystack[end..]
                .chars()
                .next()
                .is_some_and(|c| c.is_alphanumeric());
        if boundary_before && boundary_after {
            return Some(start);
        }
        from = end;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(tools: &[&str], bash: &[&str], message: &str) -> ParsedCall {
        ParsedCall {
            tools: tools.iter().map(|t| t.to_string()).collect(),
            bash_commands: bash.iter().map(|b| b.to_string()).collect(),
            user_message: message.to_string(),
            ..ParsedCall::default()
        }
    }

    #[test]
    fn plan_and_agent_tools_win_over_everything() {
        assert_eq!(
            classify(&call(&["EnterPlanMode", "Edit"], &[], "fix the bug")),
            TaskCategory::Planning
        );
        assert_eq!(
            classify(&call(&["Agent", "Bash"], &[], "")),
            TaskCategory::Delegation
        );
    }

    #[test]
    fn bash_without_edits_detects_test_git_and_build() {
        assert_eq!(
            classify(&call(&["Bash"], &["cargo test"], "run the suite")),
            TaskCategory::Testing
        );
        assert_eq!(
            classify(&call(&["Bash"], &["git commit -m x"], "")),
            TaskCategory::Git
        );
        assert_eq!(
            classify(&call(&["Bash"], &["docker compose up"], "")),
            TaskCategory::BuildDeploy
        );
    }

    #[test]
    fn keyword_refinement_is_first_match_by_position() {
        // "add" (feature) appears before "error" (debugging).
        assert_eq!(
            classify(&call(&["Edit"], &[], "add error handling to the parser")),
            TaskCategory::Feature
        );
        assert_eq!(
            classify(&call(&["Edit"], &[], "fix the bug before we add tests")),
            TaskCategory::Debugging
        );
        assert_eq!(
            classify(&call(&["Edit"], &[], "refactor and add a feature")),
            TaskCategory::Refactoring
        );
        assert_eq!(
            classify(&call(&["Edit"], &[], "tweak the layout")),
            TaskCategory::Coding
        );
    }

    #[test]
    fn reads_and_searches_classify_as_exploration() {
        assert_eq!(
            classify(&call(&["Read", "Grep"], &[], "how does the cache work")),
            TaskCategory::Exploration
        );
        assert_eq!(
            classify(&call(&["Read"], &[], "why is this failing")),
            TaskCategory::Debugging
        );
        assert_eq!(
            classify(&call(&["mcp__server__tool"], &[], "")),
            TaskCategory::Exploration
        );
    }

    #[test]
    fn conversation_branch_covers_brainstorming_and_smalltalk() {
        assert_eq!(
            classify(&call(&[], &[], "let's brainstorm the roadmap")),
            TaskCategory::Brainstorming
        );
        assert_eq!(
            classify(&call(&[], &[], "what is a lifetime in rust")),
            TaskCategory::Exploration
        );
        assert_eq!(
            classify(&call(&[], &[], "thanks!")),
            TaskCategory::Conversation
        );
        assert_eq!(classify(&call(&[], &[], "")), TaskCategory::Conversation);
    }

    #[test]
    fn word_boundaries_prevent_substring_hits() {
        // "addition" must not match "add"; "buggy" must not match "bug".
        assert_eq!(
            classify(&call(&["Edit"], &[], "the addition looks buggy-free")),
            TaskCategory::Coding
        );
    }

    #[test]
    fn task_tools_without_edits_read_as_planning() {
        assert_eq!(
            classify(&call(&["TodoWrite"], &[], "")),
            TaskCategory::Planning
        );
        assert_eq!(classify(&call(&["Skill"], &[], "")), TaskCategory::General);
    }
}
