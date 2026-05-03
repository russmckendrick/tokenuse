use std::{
    collections::{BTreeSet, HashMap},
    path::Path,
};

use crate::currency::CurrencyFormatter;

pub(super) fn project_identity(raw: &str) -> String {
    let normalized = normalized_project_path(raw);
    nearest_git_root(&normalized).unwrap_or(normalized)
}

pub(super) fn raw_project_display(raw: &str) -> String {
    normalized_project_path(raw)
}

fn normalized_project_path(raw: &str) -> String {
    let normalized = raw.trim().replace('\\', "/");
    let trimmed = normalized.trim_end_matches('/');
    if trimmed.is_empty() {
        "(unknown)".into()
    } else {
        trimmed.to_string()
    }
}

fn nearest_git_root(project: &str) -> Option<String> {
    let path = Path::new(project);
    if !path.is_absolute() {
        return None;
    }

    path.ancestors()
        .find(|ancestor| ancestor.join(".git").exists())
        .map(path_to_project_string)
}

pub(super) fn path_to_project_string(path: &Path) -> String {
    let normalized = path.to_string_lossy().replace('\\', "/");
    let trimmed = normalized.trim_end_matches('/');
    if trimmed.is_empty() {
        normalized
    } else {
        trimmed.to_string()
    }
}

pub(super) fn project_label_lookup<I, S>(raw_projects: I) -> HashMap<String, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let identities: BTreeSet<String> = raw_projects
        .into_iter()
        .map(|raw| project_identity(raw.as_ref()))
        .collect();

    identities
        .iter()
        .map(|identity| {
            (
                identity.clone(),
                shortest_unique_project_label(identity, &identities),
            )
        })
        .collect()
}

pub(super) fn project_label(labels: &HashMap<String, String>, identity: &str) -> String {
    labels.get(identity).cloned().unwrap_or_else(|| {
        shortest_unique_project_label(identity, &BTreeSet::from([identity.to_string()]))
    })
}

fn shortest_unique_project_label(identity: &str, identities: &BTreeSet<String>) -> String {
    let parts = project_parts(identity);
    if parts.is_empty() {
        return "(unknown)".into();
    }

    for suffix_len in 1..=parts.len() {
        let candidate = project_suffix(&parts, suffix_len);
        let conflicts = identities
            .iter()
            .filter(|other| other.as_str() != identity)
            .any(|other| {
                let other_parts = project_parts(other);
                other_parts.len() >= suffix_len
                    && project_suffix(&other_parts, suffix_len) == candidate
            });

        if !conflicts {
            return candidate;
        }
    }

    parts.join("/")
}

fn project_parts(identity: &str) -> Vec<&str> {
    if identity == "(unknown)" {
        return vec![identity];
    }
    identity
        .trim_start_matches('/')
        .split('/')
        .filter(|part| !part.is_empty())
        .collect()
}

fn project_suffix(parts: &[&str], suffix_len: usize) -> String {
    parts[parts.len().saturating_sub(suffix_len)..].join("/")
}

pub(super) fn format_tool_mix(
    tools: &HashMap<&'static str, f64>,
    currency: &CurrencyFormatter,
) -> String {
    let mut rows: Vec<(&'static str, f64)> =
        tools.iter().map(|(tool, cost)| (*tool, *cost)).collect();
    rows.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| tool_short_label(a.0).cmp(tool_short_label(b.0)))
    });

    if rows.is_empty() {
        return "-".into();
    }

    rows.into_iter()
        .take(3)
        .map(|(tool, cost)| {
            format!(
                "{} {}",
                tool_short_label(tool),
                currency.format_money_short(cost)
            )
        })
        .collect::<Vec<_>>()
        .join("  ")
}

pub(super) fn tool_short_label(tool: &str) -> &'static str {
    match tool {
        "claude-code" => "Claude",
        "cursor" => "Cursor",
        "codex" => "Codex",
        "copilot" => "Copilot",
        "gemini" => "Gemini",
        _ => "Other",
    }
}
