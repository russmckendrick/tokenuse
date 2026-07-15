//! Text heuristics used by the coach rules.
//!
//! Portions ported from Microsoft's AI-Engineering-Coach
//! (https://github.com/microsoft/AI-Engineering-Coach, MIT License,
//! Copyright (c) Microsoft Corporation). Regex-based patterns from the
//! reference implementation are reimplemented as word-boundary phrase
//! matching so no regex dependency is needed; semantics are documented
//! per function.

/// Ratio of uppercase ASCII letters to all ASCII letters, over the first
/// 2000 chars. Mirrors the reference `capsLetterRatio`.
pub fn caps_letter_ratio(text: &str) -> f64 {
    let mut letters = 0u32;
    let mut upper = 0u32;
    for c in text.chars().take(2000) {
        if c.is_ascii_uppercase() {
            letters += 1;
            upper += 1;
        } else if c.is_ascii_lowercase() {
            letters += 1;
        }
    }
    if letters == 0 {
        0.0
    } else {
        f64::from(upper) / f64::from(letters)
    }
}

/// Ratio of ALL-CAPS words (length >= `min_len`) to all words of that
/// length, over the first 2000 chars. Mirrors the reference `capsWordRatio`.
pub fn caps_word_ratio(text: &str, min_len: usize) -> f64 {
    let mut words = 0u32;
    let mut caps_words = 0u32;
    let clipped: String = text.chars().take(2000).collect();
    for word in clipped.split_whitespace() {
        if word.chars().count() < min_len {
            continue;
        }
        let mut has_letter = false;
        let mut all_upper = true;
        for c in word.chars() {
            if c.is_ascii_uppercase() {
                has_letter = true;
            } else if c.is_ascii_lowercase() {
                all_upper = false;
            }
        }
        words += 1;
        if has_letter && all_upper {
            caps_words += 1;
        }
    }
    if words == 0 {
        0.0
    } else {
        f64::from(caps_words) / f64::from(words)
    }
}

/// Case-insensitive word-boundary search for `phrase` (which may contain
/// spaces). A boundary is any non-alphanumeric char or the string edge.
pub fn contains_phrase(text: &str, phrase: &str) -> bool {
    let haystack = text.to_lowercase();
    let needle = phrase.to_lowercase();
    if needle.is_empty() {
        return false;
    }
    let bytes = haystack.as_bytes();
    let mut from = 0;
    while let Some(pos) = haystack[from..].find(&needle) {
        let start = from + pos;
        let end = start + needle.len();
        let boundary_before = start == 0 || !is_word_byte(bytes[start - 1]);
        let boundary_after = end >= bytes.len() || !is_word_byte(bytes[end]);
        if boundary_before && boundary_after {
            return true;
        }
        from = start + 1;
    }
    false
}

pub fn contains_any_phrase(text: &str, phrases: &[&str]) -> bool {
    phrases.iter().any(|p| contains_phrase(text, p))
}

/// Count phrase-list hits (each phrase at most once). Used by the verbose
/// prompt rule, which requires two or more distinct filler words.
pub fn count_phrase_hits(text: &str, phrases: &[&str]) -> usize {
    phrases.iter().filter(|p| contains_phrase(text, p)).count()
}

fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'\'' || b == b'_'
}

/// True when the text contains a run of `ch` at least `n` long.
pub fn has_punct_run(text: &str, ch: char, n: usize) -> bool {
    let mut run = 0;
    for c in text.chars() {
        if c == ch {
            run += 1;
            if run >= n {
                return true;
            }
        } else {
            run = 0;
        }
    }
    false
}

/// Constraint keywords from the reference `hasConstraint`, checked over the
/// first 500 chars.
pub fn has_constraint(text: &str) -> bool {
    let sample: String = text.chars().take(500).collect();
    contains_any_phrase(
        &sample,
        &[
            "do not",
            "don't",
            "dont",
            "must not",
            "mustn't",
            "never",
            "without",
            "avoid",
            "only",
            "strictly",
            "limit to",
            "at most",
            "at least",
            "no more than",
            "require",
            "restrict",
            "exclude",
            "ensure",
            "must",
            "shall",
            "should not",
            "shouldn't",
        ],
    )
}

/// Frustration signals from the reference rule: `!!!`, `???`, or hostile
/// phrases ("wtf", "come on", "why won't", "this is broken", "doesn't work").
pub fn has_frustration_signal(text: &str) -> bool {
    has_punct_run(text, '!', 3)
        || has_punct_run(text, '?', 3)
        || contains_any_phrase(
            text,
            &[
                "wtf",
                "come on",
                "why won't",
                "why wont",
                "this is broken",
                "doesn't work",
                "doesnt work",
            ],
        )
}

/// Lookup-style question openers from the reference
/// `premium-for-lookup-questions` rule (anchored at the start of the prompt).
pub fn is_lookup_question(text: &str) -> bool {
    let trimmed = text.trim_start().to_lowercase();
    [
        "what's ",
        "what is ",
        "what are ",
        "where's ",
        "where is ",
        "where are ",
        "how do i ",
        "how do you ",
        "explain ",
        "why does ",
        "why is ",
        "why are ",
        "when should ",
        "when do ",
        "which ",
        "tell me about ",
        "define ",
    ]
    .iter()
    .any(|prefix| trimmed.starts_with(prefix))
}

/// Refinement keywords from the reference `copy-paste-blindness` rule.
pub fn has_refinement_language(text: &str) -> bool {
    contains_any_phrase(
        text,
        &[
            "change",
            "fix",
            "modify",
            "update",
            "refactor",
            "wrong",
            "instead",
            "actually",
            "revert",
            "redo",
            "try again",
        ],
    )
}

/// Structured-prompt shapes from the reference `vibe-coding` rule: bullet or
/// numbered lists, markdown headings, spec keywords, or 4+ lines.
pub fn looks_like_spec(text: &str) -> bool {
    let mut lines = 0;
    for line in text.lines() {
        lines += 1;
        let t = line.trim_start();
        if t.starts_with("- ") || t.starts_with("* ") || t.starts_with('#') {
            return true;
        }
        if starts_with_numbered_item(t) {
            return true;
        }
    }
    if lines >= 4 {
        return true;
    }
    contains_any_phrase(
        text,
        &[
            "requirement",
            "requirements",
            "spec",
            "acceptance criteria",
            "user story",
            "user stories",
            "given",
            "when",
            "then",
            "should",
            "must",
        ],
    )
}

fn starts_with_numbered_item(line: &str) -> bool {
    let mut chars = line.chars();
    let mut digits = 0;
    for c in chars.by_ref() {
        if c.is_ascii_digit() {
            digits += 1;
            continue;
        }
        return digits > 0 && (c == '.' || c == ')');
    }
    false
}

/// Work-type classification from the reference `classifyWorkText`: first
/// matching category wins, over the first 300 chars; default "feature".
pub fn classify_work(text: &str) -> &'static str {
    let sample: String = text.chars().take(300).collect();
    const PATTERNS: &[(&[&str], &str)] = &[
        (
            &[
                "bug", "fix", "error", "issue", "crash", "broken", "wrong", "fail", "debug",
            ],
            "bug fix",
        ),
        (
            &[
                "refactor",
                "clean up",
                "cleanup",
                "rename",
                "restructure",
                "reorganize",
                "simplify",
            ],
            "refactor",
        ),
        (
            &[
                "test", "spec", "coverage", "assert", "expect", "mock", "stub",
            ],
            "test",
        ),
        (
            &["doc", "readme", "comment", "jsdoc", "typedoc", "explain"],
            "documentation",
        ),
        (
            &[
                "deploy",
                "ci",
                "cd",
                "pipeline",
                "docker",
                "kubernetes",
                "helm",
                "terraform",
                "infra",
            ],
            "devops",
        ),
        (
            &[
                "style", "css", "layout", "design", "ui", "ux", "theme", "color", "font",
            ],
            "styling",
        ),
        (
            &[
                "config",
                "setup",
                "install",
                "init",
                "bootstrap",
                "scaffold",
            ],
            "configuration",
        ),
        (
            &[
                "perf",
                "optim",
                "optimize",
                "optimise",
                "speed",
                "cache",
                "memory",
                "benchmark",
            ],
            "performance",
        ),
        (
            &[
                "security",
                "auth",
                "permission",
                "encrypt",
                "token",
                "oauth",
                "cors",
            ],
            "security",
        ),
        (
            &[
                "migration",
                "migrate",
                "upgrade",
                "update",
                "version",
                "deprecate",
                "deprecated",
            ],
            "migration",
        ),
    ];
    for (phrases, label) in PATTERNS {
        if contains_any_phrase(&sample, phrases) {
            return label;
        }
    }
    "feature"
}

/// Filler words from the reference `verbose-prompt-no-compression` rule.
pub const FILLER_WORDS: &[&str] = &[
    "please",
    "kindly",
    "thanks",
    "thank you",
    "basically",
    "essentially",
    "definitely",
    "absolutely",
    "simply",
    "very",
    "quite",
    "somewhat",
    "certainly",
    "actually",
    "literally",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caps_ratios_match_reference_semantics() {
        assert!(caps_letter_ratio("WHY WONT THIS WORK") > 0.9);
        assert!(caps_letter_ratio("Please refactor the auth module") < 0.2);
        assert_eq!(caps_letter_ratio("1234 !!"), 0.0);
        assert!(caps_word_ratio("THIS IS SO BROKEN fix it now", 3) >= 0.4);
        assert_eq!(caps_word_ratio("all lower case words here", 3), 0.0);
    }

    #[test]
    fn phrase_matching_respects_word_boundaries() {
        assert!(contains_phrase("Do NOT use classes", "do not"));
        assert!(!contains_phrase("makemustard sauce", "must"));
        assert!(contains_phrase("ensure it compiles", "ensure"));
        assert!(has_constraint("Only use async/await, avoid callbacks"));
        assert!(!has_constraint("write a hello world program"));
    }

    #[test]
    fn frustration_and_lookup_detection() {
        assert!(has_frustration_signal("WHY WONT THIS WORK???"));
        assert!(has_frustration_signal("this is broken again"));
        assert!(!has_frustration_signal("Add error handling to the API"));
        assert!(is_lookup_question("What is a lifetime in Rust?"));
        assert!(!is_lookup_question("Refactor the parser to use serde"));
    }

    #[test]
    fn spec_and_work_type_classification() {
        assert!(looks_like_spec("- step one\n- step two"));
        assert!(looks_like_spec("Requirements: must parse JSONL"));
        assert!(!looks_like_spec("make it work"));
        assert_eq!(classify_work("fix the crash in parser"), "bug fix");
        assert_eq!(classify_work("write a test for the archive"), "test");
        assert_eq!(classify_work("add a new page"), "feature");
    }
}
