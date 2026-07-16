//! Shared model-name registry.
//!
//! Every raw model identifier that reaches a renderer goes through
//! [`resolve`], which maps `(tool_id, raw id)` to one [`ModelIdentity`]:
//! a canonical fold key, a human display name, a provider, and a family.
//! The known mappings live in `registry.json` (ordered, first match wins);
//! unknown identifiers fall back to provider-inferred prettifying so raw
//! ids never render. Model, provider, and family names are data-derived
//! values and deliberately not part of the copy deck.

use std::sync::OnceLock;

use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Provider {
    Anthropic,
    OpenAI,
    Google,
    GitHub,
    Cursor,
    XAI,
    Other,
}

impl Provider {
    /// Stable identifier used for serialization and icon lookup.
    pub fn id(self) -> &'static str {
        match self {
            Self::Anthropic => "anthropic",
            Self::OpenAI => "openai",
            Self::Google => "google",
            Self::GitHub => "github",
            Self::Cursor => "cursor",
            Self::XAI => "xai",
            Self::Other => "other",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Anthropic => "Anthropic",
            Self::OpenAI => "OpenAI",
            Self::Google => "Google",
            Self::GitHub => "GitHub",
            Self::Cursor => "Cursor",
            Self::XAI => "xAI",
            Self::Other => "Other",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "anthropic" => Some(Self::Anthropic),
            "openai" => Some(Self::OpenAI),
            "google" => Some(Self::Google),
            "github" => Some(Self::GitHub),
            "cursor" => Some(Self::Cursor),
            "xai" => Some(Self::XAI),
            "other" => Some(Self::Other),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelIdentity {
    /// Fold key: calls with the same canonical id aggregate into one row.
    pub canonical_id: String,
    pub display: String,
    pub provider: Provider,
    pub family: String,
}

/// Normalize a raw model id into the key the registry matches against:
/// trimmed, lowercased, vendor path prefix and `@suffix` removed, and a
/// trailing `-YYYYMMDD` date stripped. Pricing lookups share this exact
/// normalization via `pricing`.
pub fn canonical_key(model: &str) -> String {
    let mut s = model.trim().to_lowercase();
    if let Some(idx) = s.find('@') {
        s.truncate(idx);
    }
    if let Some(idx) = s.rfind('/') {
        s = s[idx + 1..].to_string();
    }
    if let Some(stripped) = strip_date_suffix(&s) {
        s = stripped;
    }
    s = normalize_reversed_claude_id(&s);
    s
}

fn normalize_reversed_claude_id(model: &str) -> String {
    let Some(rest) = model.strip_prefix("claude-") else {
        return model.to_string();
    };
    let parts = rest.split('-').collect::<Vec<_>>();
    let Some(family_idx) = parts
        .iter()
        .position(|part| matches!(*part, "opus" | "sonnet" | "haiku"))
    else {
        return model.to_string();
    };
    if family_idx == 0 || !parts[0].chars().next().is_some_and(|c| c.is_ascii_digit()) {
        return model.to_string();
    }

    let version = parts[..family_idx].join("-").replace('.', "-");
    let suffix = parts[family_idx + 1..].join("-");
    if suffix.is_empty() {
        format!("claude-{}-{version}", parts[family_idx])
    } else {
        format!("claude-{}-{version}-{suffix}", parts[family_idx])
    }
}

fn strip_date_suffix(model: &str) -> Option<String> {
    let bytes = model.as_bytes();
    if bytes.len() < 9 {
        return None;
    }
    let tail = &bytes[bytes.len() - 9..];
    if tail[0] == b'-' && tail[1..].iter().all(|b| b.is_ascii_digit()) {
        return Some(model[..model.len() - 9].to_string());
    }
    None
}

pub fn resolve(tool_id: &str, raw: &str) -> ModelIdentity {
    let key = canonical_key(raw);
    for rule in rules() {
        if let Some(tool) = &rule.tool {
            if tool != tool_id {
                continue;
            }
        }
        let matched = if rule.exact {
            rule.keys.iter().any(|k| k == &key)
        } else {
            rule.keys.iter().any(|k| key.starts_with(k.as_str()))
        };
        if matched {
            return ModelIdentity {
                canonical_id: rule.canonical.clone(),
                display: rule.display.clone(),
                provider: rule.provider,
                family: rule.family.clone(),
            };
        }
    }
    fallback_identity(&key)
}

/// Provider-inferred naming for ids the registry does not know yet, so a
/// new model ships with a sensible name before the registry learns it.
fn fallback_identity(key: &str) -> ModelIdentity {
    if key.starts_with("gpt-") {
        let display = format_gpt_model(key);
        return ModelIdentity {
            canonical_id: key.to_string(),
            display,
            provider: Provider::OpenAI,
            family: gpt_family(key),
        };
    }
    if let Some(rest) = key.strip_prefix("claude-") {
        let display = prettify(rest);
        let family = rest
            .split('-')
            .next()
            .filter(|s| !s.is_empty())
            .map(title_case_word)
            .unwrap_or_else(|| "Claude".to_string());
        return ModelIdentity {
            canonical_id: key.to_string(),
            display,
            provider: Provider::Anthropic,
            family,
        };
    }
    if key.starts_with("gemini-") {
        return ModelIdentity {
            canonical_id: key.to_string(),
            display: prettify(key),
            provider: Provider::Google,
            family: "Gemini".to_string(),
        };
    }
    let display = if key.is_empty() {
        "Unknown".to_string()
    } else {
        prettify(key)
    };
    let family = key
        .split('-')
        .next()
        .filter(|s| !s.is_empty())
        .map(title_case_word)
        .unwrap_or_else(|| "Other".to_string());
    ModelIdentity {
        canonical_id: if key.is_empty() {
            "unknown".to_string()
        } else {
            key.to_string()
        },
        display,
        provider: Provider::Other,
        family,
    }
}

fn format_gpt_model(model: &str) -> String {
    let mut parts = model.split('-');
    let _ = parts.next();
    let Some(base) = parts.next() else {
        return "GPT".to_string();
    };

    let mut label = format!("GPT-{base}");
    for suffix in parts {
        if suffix.is_empty() {
            continue;
        }
        label.push(' ');
        label.push_str(&title_case_word(suffix));
    }
    label
}

fn gpt_family(key: &str) -> String {
    let mut parts = key.split('-');
    let _ = parts.next();
    let major = parts
        .next()
        .and_then(|base| base.split('.').next())
        .filter(|s| !s.is_empty())
        .unwrap_or("GPT");
    let family = if major == "GPT" {
        "GPT".to_string()
    } else {
        format!("GPT-{major}")
    };
    if key.split('-').any(|part| part == "codex") {
        format!("{family} Codex")
    } else {
        family
    }
}

fn prettify(key: &str) -> String {
    key.split('-')
        .filter(|part| !part.is_empty())
        .map(title_case_word)
        .collect::<Vec<_>>()
        .join(" ")
}

fn title_case_word(word: &str) -> String {
    let mut chars = word.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    let mut out = String::new();
    out.extend(first.to_uppercase());
    out.push_str(chars.as_str());
    out
}

struct Rule {
    tool: Option<String>,
    exact: bool,
    keys: Vec<String>,
    canonical: String,
    display: String,
    provider: Provider,
    family: String,
}

#[derive(Deserialize)]
struct RegistryFile {
    rules: Vec<RuleDef>,
}

#[derive(Deserialize)]
struct RuleDef {
    #[serde(default)]
    tool: Option<String>,
    #[serde(rename = "match")]
    match_kind: String,
    keys: Vec<String>,
    #[serde(default)]
    canonical: Option<String>,
    display: String,
    provider: String,
    family: String,
}

const REGISTRY_JSON: &str = include_str!("registry.json");

fn rules() -> &'static [Rule] {
    static RULES: OnceLock<Vec<Rule>> = OnceLock::new();
    RULES.get_or_init(|| {
        let file: RegistryFile =
            serde_json::from_str(REGISTRY_JSON).expect("embedded model registry must parse");
        file.rules
            .into_iter()
            .map(|def| {
                let exact = match def.match_kind.as_str() {
                    "exact" => true,
                    "prefix" => false,
                    other => panic!("model registry rule has unknown match kind {other:?}"),
                };
                let canonical = def
                    .canonical
                    .clone()
                    .or_else(|| def.keys.first().cloned())
                    .expect("model registry rule needs at least one key");
                let provider = Provider::from_id(&def.provider).unwrap_or_else(|| {
                    panic!(
                        "model registry rule has unknown provider {:?}",
                        def.provider
                    )
                });
                Rule {
                    tool: def.tool,
                    exact,
                    keys: def.keys,
                    canonical,
                    display: def.display,
                    provider,
                    family: def.family,
                }
            })
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn display(tool: &str, raw: &str) -> String {
        resolve(tool, raw).display
    }

    // Ported from the Codex adapter's model_display tests.
    #[test]
    fn gpt_models_keep_their_full_names() {
        assert_eq!(display("codex", "gpt-5.6-sol"), "GPT-5.6 Sol");
        assert_eq!(display("codex", "gpt-5.6-terra"), "GPT-5.6 Terra");
        assert_eq!(display("codex", "gpt-5.6-luna"), "GPT-5.6 Luna");
        assert_eq!(display("codex", "gpt-5.6"), "GPT-5.6");
        assert_eq!(display("codex", "gpt-5.3-codex"), "GPT-5.3 Codex");
        assert_eq!(
            display("codex", "gpt-5.3-codex-spark"),
            "GPT-5.3 Codex Spark"
        );
        assert_eq!(display("codex", "gpt-4o-mini"), "GPT-4o Mini");
        assert_eq!(display("codex", "gpt-5"), "GPT-5");
    }

    // Ported from the Gemini adapter; unknown ids are now prettified
    // instead of passing through raw.
    #[test]
    fn gemini_models_shorten_and_fold_vendor_paths() {
        assert_eq!(display("gemini", "gemini-2.5-pro"), "Gemini 2.5 Pro");
        assert_eq!(
            display("gemini", "google/gemini-2.5-flash@latest"),
            "Gemini 2.5 Flash"
        );
        assert_eq!(display("gemini", "gemini-future"), "Gemini Future");
        assert_eq!(display("gemini", "gemini-auto"), "Gemini (auto)");
    }

    #[test]
    fn claude_models_fold_dated_ids_and_name_unknowns() {
        let dated = resolve("claude-code", "claude-opus-4-5-20250929");
        assert_eq!(dated.display, "Opus 4.5");
        assert_eq!(dated.canonical_id, "claude-opus-4-5");
        assert_eq!(dated.provider, Provider::Anthropic);
        assert_eq!(dated.family, "Opus");

        let fable = resolve("claude-code", "claude-fable-5");
        assert_eq!(fable.display, "Fable 5");
        assert_eq!(fable.family, "Fable");

        // Claude Code's old table stopped at Opus 4.7; the shared registry
        // knows newer models regardless of which tool saw them.
        assert_eq!(display("claude-code", "claude-opus-4-8"), "Opus 4.8");
        assert_eq!(display("copilot", "claude-opus-4-8"), "Opus 4.8");

        // Unknown Claude ids self-name in the short-name style.
        let unknown = resolve("claude-code", "claude-nova-2");
        assert_eq!(unknown.display, "Nova 2");
        assert_eq!(unknown.family, "Nova");
        assert_eq!(unknown.provider, Provider::Anthropic);
    }

    #[test]
    fn auto_router_models_attribute_to_the_underlying_provider() {
        let openai_auto = resolve("copilot", "openai-auto");
        assert_eq!(openai_auto.display, "OpenAI (auto)");
        assert_eq!(openai_auto.provider, Provider::OpenAI);
        assert_eq!(openai_auto.canonical_id, "copilot-openai-auto");

        let anthropic_auto = resolve("copilot", "anthropic-auto");
        assert_eq!(anthropic_auto.display, "Anthropic (auto)");
        assert_eq!(anthropic_auto.provider, Provider::Anthropic);

        let copilot_auto = resolve("copilot", "auto");
        assert_eq!(copilot_auto.display, "Copilot (auto)");
        assert_eq!(copilot_auto.provider, Provider::GitHub);

        let cursor_auto = resolve("cursor", "auto");
        assert_eq!(cursor_auto.display, "Cursor (auto)");
        assert_eq!(cursor_auto.provider, Provider::Cursor);
        assert_eq!(cursor_auto.canonical_id, "cursor-auto");

        assert_eq!(display("cursor", "default"), "Cursor (auto)");

        // The same raw string resolves per tool, so the folded rows stay
        // distinct across tools.
        assert_ne!(
            resolve("copilot", "auto").canonical_id,
            resolve("cursor", "auto").canonical_id
        );
    }

    #[test]
    fn unknown_models_prettify_instead_of_rendering_raw() {
        let unknown = resolve("cursor", "mystery-model-x");
        assert_eq!(unknown.display, "Mystery Model X");
        assert_eq!(unknown.provider, Provider::Other);
        assert_eq!(unknown.family, "Mystery");

        let gpt = resolve("cursor", "gpt-7-quasar");
        assert_eq!(gpt.display, "GPT-7 Quasar");
        assert_eq!(gpt.provider, Provider::OpenAI);
        assert_eq!(gpt.family, "GPT-7");

        let empty = resolve("codex", "");
        assert_eq!(empty.display, "Unknown");
        assert_eq!(empty.canonical_id, "unknown");
    }

    #[test]
    fn canonical_key_folds_paths_suffixes_and_dates() {
        assert_eq!(
            canonical_key("google/gemini-2.5-pro@latest"),
            "gemini-2.5-pro"
        );
        assert_eq!(canonical_key("Claude-Opus-4-5-20250929"), "claude-opus-4-5");
        assert_eq!(canonical_key(" gpt-5 "), "gpt-5");
        assert_eq!(
            canonical_key("claude-4.5-sonnet-thinking-high"),
            "claude-sonnet-4-5-thinking-high"
        );
    }

    #[test]
    fn cursor_preview_and_fast_models_have_stable_identities() {
        let composer = resolve("cursor", "composer-2.5-fast");
        assert_eq!(composer.display, "Composer 2.5 Fast");
        assert_eq!(composer.canonical_id, "cursor-composer-2.5");
        assert_eq!(composer.provider, Provider::Cursor);

        let grok = resolve("cursor", "grok-4.5-fast-high");
        assert_eq!(grok.display, "Grok 4.5 Fast");
        assert_eq!(grok.canonical_id, "cursor-grok-4.5");

        let vega = resolve("cursor", "vega-fast-xhigh");
        assert_eq!(vega.display, "Vega (Preview)");
        assert_eq!(vega.canonical_id, "cursor-vega");
    }

    #[test]
    fn registry_rules_are_reachable_in_order() {
        let rules = rules();
        assert!(!rules.is_empty());
        for (later_idx, later) in rules.iter().enumerate() {
            for earlier in &rules[..later_idx] {
                // A broader earlier prefix rule must not shadow a longer
                // later key in the same tool scope.
                if earlier.exact || earlier.tool != later.tool {
                    continue;
                }
                for later_key in &later.keys {
                    for earlier_key in &earlier.keys {
                        assert!(
                            !(later_key.starts_with(earlier_key.as_str())
                                && later_key != earlier_key),
                            "rule for {later_key:?} is unreachable behind prefix {earlier_key:?}"
                        );
                    }
                }
            }
        }
    }
}
