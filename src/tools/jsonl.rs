use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

pub fn read_lines(path: &Path) -> std::io::Result<impl Iterator<Item = String>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    Ok(reader.lines().map_while(Result::ok))
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
}
