use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use chrono::{DateTime, Utc};

use super::types::CodeBlock;

/// Bounds on per-call enrichment collections so a pathological session can't
/// balloon the archive's JSON columns.
pub const MAX_CODE_BLOCKS_PER_CALL: usize = 32;
pub const MAX_FILES_PER_CALL: usize = 64;
pub const MAX_PATH_CHARS: usize = 512;

pub fn read_lines(path: &Path) -> std::io::Result<impl Iterator<Item = String>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    Ok(reader.lines().map_while(Result::ok))
}

/// Count fenced code blocks in assistant text with a single line pass. Lines
/// whose trimmed start is ``` toggle fence state; the opening fence's first
/// token is the language tag. An unclosed trailing fence still counts - the
/// code was emitted even if the fence never closed.
pub fn extract_code_fences(text: &str) -> Vec<CodeBlock> {
    let mut blocks = Vec::new();
    let mut open_language: Option<String> = None;
    let mut loc: u64 = 0;

    for line in text.lines() {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("```") {
            match open_language.take() {
                Some(language) => {
                    if loc > 0 {
                        blocks.push(CodeBlock { language, loc });
                    }
                    loc = 0;
                }
                None => {
                    let tag = rest.split_whitespace().next().unwrap_or("");
                    open_language = Some(normalize_language(tag));
                    loc = 0;
                }
            }
            continue;
        }
        if open_language.is_some() {
            loc += 1;
        }
    }
    if let Some(language) = open_language {
        if loc > 0 {
            blocks.push(CodeBlock { language, loc });
        }
    }
    blocks
}

/// Language label for a file path, from its extension.
pub fn extension_language(path: &str) -> String {
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    normalize_language(ext)
}

/// Canonicalize fence tags and file extensions onto one label set so
/// by-language rollups don't split "rs" from "rust".
pub fn normalize_language(tag: &str) -> String {
    let tag = tag.trim().to_lowercase();
    match tag.as_str() {
        "" => "unknown".to_string(),
        "rs" => "rust".to_string(),
        "py" => "python".to_string(),
        "ts" | "tsx" | "mts" | "cts" => "typescript".to_string(),
        "js" | "jsx" | "mjs" | "cjs" => "javascript".to_string(),
        "md" | "mdx" => "markdown".to_string(),
        "yml" => "yaml".to_string(),
        "sh" | "bash" | "zsh" | "shell" => "shell".to_string(),
        "rb" => "ruby".to_string(),
        "kt" | "kts" => "kotlin".to_string(),
        "cc" | "cxx" | "hpp" | "hh" | "c++" => "cpp".to_string(),
        "h" => "c".to_string(),
        "cs" => "csharp".to_string(),
        "golang" => "go".to_string(),
        other => other.to_string(),
    }
}

/// Merge code blocks by language (summing LoC) and enforce the per-call cap.
pub fn merge_code_blocks(blocks: Vec<CodeBlock>) -> Vec<CodeBlock> {
    let mut merged: Vec<CodeBlock> = Vec::new();
    for block in blocks {
        match merged.iter_mut().find(|b| b.language == block.language) {
            Some(existing) => existing.loc += block.loc,
            None => merged.push(block),
        }
    }
    merged.truncate(MAX_CODE_BLOCKS_PER_CALL);
    merged
}

/// Turn latencies above this are stale user-line carryover, not real waits.
const MAX_TURN_ELAPSED_MS: i64 = 2 * 60 * 60 * 1000;

/// Milliseconds from the user message to an assistant event, or `None` when
/// either timestamp is missing, ordering is wrong, or the gap is implausible.
pub fn turn_elapsed_ms(
    assistant_ts: Option<DateTime<Utc>>,
    user_ts: Option<DateTime<Utc>>,
) -> Option<u64> {
    let (assistant_ts, user_ts) = (assistant_ts?, user_ts?);
    let delta = assistant_ts
        .signed_duration_since(user_ts)
        .num_milliseconds();
    (delta > 0 && delta < MAX_TURN_ELAPSED_MS).then_some(delta as u64)
}

/// Truncate to at most `max` chars on a char boundary.
pub fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    s.chars().take(max).collect()
}

/// Dedup file paths preserving order, drop over-long entries, and cap the list.
pub fn dedup_files(paths: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for path in paths {
        if path.is_empty() || path.chars().count() > MAX_PATH_CHARS {
            continue;
        }
        if !out.contains(&path) {
            out.push(path);
        }
        if out.len() == MAX_FILES_PER_CALL {
            break;
        }
    }
    out
}

pub fn split_bash_commands(input: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut chars = input.chars().peekable();
    let mut in_single = false;
    let mut in_double = false;

    while let Some(c) = chars.next() {
        match c {
            '\'' if !in_double => {
                in_single = !in_single;
                current.push(c);
            }
            '"' if !in_single => {
                in_double = !in_double;
                current.push(c);
            }
            '\\' if !in_single => {
                current.push(c);
                if let Some(&next) = chars.peek() {
                    current.push(next);
                    chars.next();
                }
            }
            '|' | ';' | '&' if !in_single && !in_double => {
                let is_double_op = (c == '&' && chars.peek() == Some(&'&'))
                    || (c == '|' && chars.peek() == Some(&'|'));
                if is_double_op {
                    chars.next();
                    push_command(&mut out, &mut current);
                } else if c == ';' || c == '|' {
                    push_command(&mut out, &mut current);
                } else {
                    current.push(c);
                }
            }
            _ => current.push(c),
        }
    }
    push_command(&mut out, &mut current);
    out
}

fn push_command(out: &mut Vec<String>, current: &mut String) {
    let trimmed = current.trim().to_string();
    if !trimmed.is_empty() {
        out.push(trimmed);
    }
    current.clear();
}

pub fn first_word(command: &str) -> String {
    command.split_whitespace().next().unwrap_or("").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_pipe_and_semicolon() {
        let got = split_bash_commands("ls -la | grep foo; cat README.md && echo done");
        assert_eq!(
            got,
            vec!["ls -la", "grep foo", "cat README.md", "echo done"]
        );
    }

    #[test]
    fn preserves_quoted_separators() {
        let got = split_bash_commands(r#"echo "a;b" | wc -l"#);
        assert_eq!(got, vec![r#"echo "a;b""#, "wc -l"]);
    }

    #[test]
    fn extracts_fences_with_tags_and_counts_lines() {
        let text = "Here you go:\n```rust\nfn main() {}\nlet x = 1;\n```\nand config:\n```yml\nkey: value\n```\ndone";
        let blocks = extract_code_fences(text);
        assert_eq!(
            blocks,
            vec![
                CodeBlock {
                    language: "rust".into(),
                    loc: 2
                },
                CodeBlock {
                    language: "yaml".into(),
                    loc: 1
                },
            ]
        );
    }

    #[test]
    fn unclosed_fence_and_untagged_fence_still_count() {
        let text = "```\nplain\n```\ntail:\n```python\nprint('hi')\nprint('bye')";
        let blocks = extract_code_fences(text);
        assert_eq!(
            blocks,
            vec![
                CodeBlock {
                    language: "unknown".into(),
                    loc: 1
                },
                CodeBlock {
                    language: "python".into(),
                    loc: 2
                },
            ]
        );
    }

    #[test]
    fn crlf_and_indented_fences_are_handled() {
        let text = "intro\r\n  ```ts\r\nconst a = 1;\r\n  ```\r\n";
        let blocks = extract_code_fences(text);
        assert_eq!(
            blocks,
            vec![CodeBlock {
                language: "typescript".into(),
                loc: 1
            }]
        );
    }

    #[test]
    fn empty_fences_are_dropped() {
        assert!(extract_code_fences("```rust\n```").is_empty());
        assert!(extract_code_fences("no code here").is_empty());
    }

    #[test]
    fn extension_language_maps_common_extensions() {
        assert_eq!(extension_language("src/app.rs"), "rust");
        assert_eq!(extension_language("notes.MD"), "markdown");
        assert_eq!(extension_language("web/App.svelte"), "svelte");
        assert_eq!(extension_language("Makefile"), "unknown");
    }

    #[test]
    fn merge_code_blocks_sums_by_language_and_caps() {
        let blocks: Vec<CodeBlock> = (0..40)
            .map(|i| CodeBlock {
                language: format!("lang{i}"),
                loc: 1,
            })
            .chain(std::iter::once(CodeBlock {
                language: "lang0".into(),
                loc: 5,
            }))
            .collect();
        let merged = merge_code_blocks(blocks);
        assert_eq!(merged.len(), MAX_CODE_BLOCKS_PER_CALL);
        assert_eq!(merged[0].loc, 6, "same-language blocks merge");
    }

    #[test]
    fn dedup_files_preserves_order_and_caps() {
        let paths = vec![
            "a.rs".to_string(),
            "b.rs".to_string(),
            "a.rs".to_string(),
            "x".repeat(MAX_PATH_CHARS + 1),
        ];
        assert_eq!(dedup_files(paths), vec!["a.rs", "b.rs"]);
    }
}
