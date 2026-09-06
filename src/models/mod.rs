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
/// trailing `-YYYYMMDD` or `-YYYY-MM-DD` date stripped.
pub fn canonical_key(model: &str) -> String {
    normalized_key(model, true)
}

/// Normalize a model id for pricing without discarding a dated snapshot.
///
/// Model identity intentionally folds provider snapshot dates so usage groups
/// under one friendly model name. Pricing cannot do that: dated variants can
/// carry different rates, so the price table must try the exact snapshot first
/// and only then fall back to the undated family prefix.
pub(crate) fn pricing_key(model: &str) -> String {
    normalized_key(model, false)
}

fn normalized_key(model: &str, strip_date: bool) -> String {
    let mut s = model.trim().to_lowercase();
    if let Some(idx) = s.find('@') {
        s.truncate(idx);
    }
    if let Some(idx) = s.rfind('/') {
        s = s[idx + 1..].to_string();
    }
    if strip_date {
        if let Some(stripped) = strip_date_suffix(&s) {
            s = stripped;
        }
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
        .position(|part| matches!(*part, "opus" | "sonnet" | "haiku" | "fable" | "mythos"))
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
    if bytes.len() >= 11 {
        let tail = &bytes[bytes.len() - 11..];
        if tail[0] == b'-'
            && tail[5] == b'-'
            && tail[8] == b'-'
            && tail[1..5].iter().all(|b| b.is_ascii_digit())
            && tail[6..8].iter().all(|b| b.is_ascii_digit())
            && tail[9..11].iter().all(|b| b.is_ascii_digit())
        {
            return Some(model[..model.len() - 11].to_string());
        }
    }
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
    let mut key = canonical_key(raw);
    if tool_id == "cursor" {
        key = cursor_identity_key(key);
    }
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

fn cursor_identity_key(mut key: String) -> String {
    let Some(parameters_start) = key.find('[') else {
        return key;
    };
    let fast = cursor_bracket_fast_parameter(&key[parameters_start + 1..]);
    key.truncate(parameters_start);

    match fast {
        Some(true) if !key.split('-').any(|segment| segment == "fast") => {
            key.push_str("-fast");
            key
        }
        Some(false) => key
            .split('-')
            .filter(|segment| *segment != "fast")
            .collect::<Vec<_>>()
            .join("-"),
        _ => key,
    }
}

fn cursor_bracket_fast_parameter(parameters: &str) -> Option<bool> {
    let parameters = parameters
        .split_once(']')
        .map_or(parameters, |(inside, _)| inside);
    parameters.split(',').find_map(|parameter| {
        let (name, value) = parameter.split_once('=')?;
        match (name.trim(), value.trim()) {
            ("fast", "true") => Some(true),
            ("fast", "false") => Some(false),
            _ => None,
        }
    })
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
        assert_eq!(display("codex", "gpt-6-astra"), "GPT-6 Astra");
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
    fn current_grok_models_keep_their_versions() {
        let grok = resolve("copilot", "grok-4.6");
        assert_eq!(grok.display, "Grok 4.6");
        assert_eq!(grok.canonical_id, "grok-4.6");
        assert_eq!(grok.provider, Provider::XAI);
    }

    #[test]
    fn copilot_current_display_labels_fold_into_api_model_identities() {
        for (raw, canonical, display, provider, family) in [
            (
                "Claude Fable 5.1",
                "claude-fable-5-1",
                "Fable 5.1",
                Provider::Anthropic,
                "Fable",
            ),
            (
                "GPT-6 Astra",
                "gpt-6-astra",
                "GPT-6 Astra",
                Provider::OpenAI,
                "GPT-6",
            ),
            (
                "Gemini 3.6 Flash",
                "gemini-3.6-flash",
                "Gemini 3.6 Flash",
                Provider::Google,
                "Gemini",
            ),
            (
                "Gemini 3.7 Flash",
                "gemini-3.7-flash",
                "Gemini 3.7 Flash",
                Provider::Google,
                "Gemini",
            ),
            (
                "Gemini 3.8 Flash",
                "gemini-3.8-flash",
                "Gemini 3.8 Flash",
                Provider::Google,
                "Gemini",
            ),
            (
                "MAI-Code-1.1-Flash",
                "mai-code-1.1-flash",
                "MAI-Code-1.1-Flash",
                Provider::Other,
                "MAI-Code",
            ),
            ("Grok 4.5", "grok-4.5", "Grok 4.5", Provider::XAI, "Grok"),
            ("Grok 4.6", "grok-4.6", "Grok 4.6", Provider::XAI, "Grok"),
            ("Kimi K3", "kimi-k3", "Kimi K3", Provider::Other, "Kimi"),
        ] {
            let identity = resolve("copilot", raw);
            assert_eq!(identity.canonical_id, canonical, "{raw} canonical id");
            assert_eq!(identity.display, display, "{raw} display");
            assert_eq!(identity.provider, provider, "{raw} provider");
            assert_eq!(identity.family, family, "{raw} family");

            let api_identity = resolve("copilot", canonical);
            assert_eq!(
                api_identity.canonical_id, canonical,
                "{raw} must fold with the API-style id"
            );
        }
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

        let fable_51 = resolve("claude-code", "claude-fable-5-1");
        assert_eq!(fable_51.display, "Fable 5.1");
        assert_eq!(fable_51.canonical_id, "claude-fable-5-1");
        assert_eq!(fable_51.provider, Provider::Anthropic);

        let mythos_51 = resolve("claude-code", "claude-mythos-5.1");
        assert_eq!(mythos_51.display, "Mythos 5.1");
        assert_eq!(mythos_51.canonical_id, "claude-mythos-5-1");

        let cursor_fable = resolve("cursor", "claude-5.1-fable-thinking");
        assert_eq!(cursor_fable.display, "Fable 5.1");
        assert_eq!(cursor_fable.canonical_id, "claude-fable-5-1");
        assert_eq!(cursor_fable.provider, Provider::Anthropic);

        let cursor_mythos = resolve("cursor", "claude-5.1-mythos-high");
        assert_eq!(cursor_mythos.display, "Mythos 5.1");
        assert_eq!(cursor_mythos.canonical_id, "claude-mythos-5-1");
        assert_eq!(cursor_mythos.provider, Provider::Anthropic);

        // Claude Code's old table stopped at Opus 4.7; the shared registry
        // knows newer models regardless of which tool saw them.
        assert_eq!(display("claude-code", "claude-opus-4-8"), "Opus 4.8");
        assert_eq!(display("copilot", "claude-opus-4-8"), "Opus 4.8");

        // Opus 5 drops the point release, so it must not be shadowed by the
        // `claude-opus-4*` prefix rules that sit next to it in the registry.
        let opus_5 = resolve("claude-code", "claude-opus-5");
        assert_eq!(opus_5.display, "Opus 5");
        assert_eq!(opus_5.canonical_id, "claude-opus-5");
        assert_eq!(opus_5.family, "Opus");
        assert_eq!(display("copilot", "claude-opus-5"), "Opus 5");

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
        assert_eq!(canonical_key("gpt-5.4-2026-03-05"), "gpt-5.4");
        assert_eq!(canonical_key(" gpt-5 "), "gpt-5");
        assert_eq!(
            canonical_key("claude-4.5-sonnet-thinking-high"),
            "claude-sonnet-4-5-thinking-high"
        );
        assert_eq!(
            canonical_key("claude-5.1-fable-thinking"),
            "claude-fable-5-1-thinking"
        );
        assert_eq!(
            canonical_key("claude-5.1-mythos-high"),
            "claude-mythos-5-1-high"
        );
    }

    #[test]
    fn pricing_key_preserves_snapshot_dates() {
        assert_eq!(
            pricing_key("openai/gpt-4o-2024-05-13@production"),
            "gpt-4o-2024-05-13"
        );
        assert_eq!(
            pricing_key("anthropic/claude-opus-4-7-20250514@v1"),
            "claude-opus-4-7-20250514"
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

        let grok_46 = resolve("cursor", "grok-4-6-fast-high");
        assert_eq!(grok_46.display, "Grok 4.6 Fast");
        assert_eq!(grok_46.canonical_id, "cursor-grok-4.6");
        assert_eq!(grok_46.provider, Provider::Cursor);

        let vega = resolve("cursor", "vega-fast-xhigh");
        assert_eq!(vega.display, "Vega (Preview)");
        assert_eq!(vega.canonical_id, "cursor-vega");
    }

    #[test]
    fn cursor_gpt_standard_effort_models_fold_into_base_identities() {
        for (raw, canonical, display) in [
            ("gpt-5", "gpt-5", "GPT-5"),
            ("gpt-5-high", "gpt-5", "GPT-5"),
            ("gpt-5-low", "gpt-5", "GPT-5"),
            ("gpt-5.4-high", "gpt-5.4", "GPT-5.4"),
            ("gpt-5.5-extra-high", "gpt-5.5", "GPT-5.5"),
            ("gpt-5.6-luna-medium", "gpt-5.6-luna", "GPT-5.6 Luna"),
            ("gpt-5.6-sol-xhigh", "gpt-5.6-sol", "GPT-5.6 Sol"),
            ("gpt-5.6-terra-high", "gpt-5.6-terra", "GPT-5.6 Terra"),
        ] {
            let identity = resolve("cursor", raw);
            assert_eq!(identity.canonical_id, canonical, "{raw} canonical id");
            assert_eq!(identity.display, display, "{raw} display");
            assert_eq!(identity.provider, Provider::OpenAI, "{raw} provider");
            assert_eq!(identity.family, "GPT-5", "{raw} family");
        }
    }

    #[test]
    fn cursor_gpt_fast_effort_models_keep_fast_display_and_base_identity() {
        for (raw, canonical, display) in [
            ("gpt-5-fast", "gpt-5", "GPT-5 Fast"),
            ("gpt-5-fast-high", "gpt-5", "GPT-5 Fast"),
            ("gpt-5-high-fast", "gpt-5", "GPT-5 Fast"),
            ("gpt-5-low-fast", "gpt-5", "GPT-5 Fast"),
            ("gpt-5.4-fast", "gpt-5.4", "GPT-5.4 Fast"),
            ("gpt-5.4-fast-high", "gpt-5.4", "GPT-5.4 Fast"),
            ("gpt-5.4-high-fast", "gpt-5.4", "GPT-5.4 Fast"),
            ("gpt-5.5-fast", "gpt-5.5", "GPT-5.5 Fast"),
            ("gpt-5.5-extra-high-fast", "gpt-5.5", "GPT-5.5 Fast"),
            (
                "gpt-5.6-luna-fast-high",
                "gpt-5.6-luna",
                "GPT-5.6 Luna Fast",
            ),
            ("gpt-5.6-luna-low-fast", "gpt-5.6-luna", "GPT-5.6 Luna Fast"),
            ("gpt-5.6-sol-fast-xhigh", "gpt-5.6-sol", "GPT-5.6 Sol Fast"),
            ("gpt-5.6-sol-low-fast", "gpt-5.6-sol", "GPT-5.6 Sol Fast"),
            ("gpt-5.6-sol-medium-fast", "gpt-5.6-sol", "GPT-5.6 Sol Fast"),
            ("gpt-5.6-sol-xhigh-fast", "gpt-5.6-sol", "GPT-5.6 Sol Fast"),
            ("gpt-5.6-sol-max-fast", "gpt-5.6-sol", "GPT-5.6 Sol Fast"),
            (
                "gpt-5.6-terra-fast-high",
                "gpt-5.6-terra",
                "GPT-5.6 Terra Fast",
            ),
            (
                "gpt-5.6-terra-max-fast",
                "gpt-5.6-terra",
                "GPT-5.6 Terra Fast",
            ),
        ] {
            let identity = resolve("cursor", raw);
            assert_eq!(identity.canonical_id, canonical, "{raw} canonical id");
            assert_eq!(identity.display, display, "{raw} display");
            assert_eq!(identity.provider, Provider::OpenAI, "{raw} provider");
            assert_eq!(identity.family, "GPT-5", "{raw} family");
        }
    }

    #[test]
    fn cursor_gpt_5_6_luna_and_terra_all_efforts_keep_fast_identity() {
        for (base, canonical, display) in [
            ("gpt-5.6-luna", "gpt-5.6-luna", "GPT-5.6 Luna Fast"),
            ("gpt-5.6-terra", "gpt-5.6-terra", "GPT-5.6 Terra Fast"),
        ] {
            for effort in [
                None,
                Some("low"),
                Some("medium"),
                Some("high"),
                Some("xhigh"),
                Some("max"),
            ] {
                let raw = effort.map_or_else(
                    || format!("{base}-fast"),
                    |effort| format!("{base}-{effort}-fast"),
                );
                let identity = resolve("cursor", &raw);
                assert_eq!(identity.canonical_id, canonical, "{raw} canonical id");
                assert_eq!(identity.display, display, "{raw} display");
                assert_eq!(identity.provider, Provider::OpenAI, "{raw} provider");
            }
        }
    }

    #[test]
    fn cursor_bracket_fast_flags_select_the_matching_identity_display() {
        for (raw, canonical, display) in [
            (
                "gpt-5.4[context=272k,reasoning=medium,fast=true]",
                "gpt-5.4",
                "GPT-5.4 Fast",
            ),
            (
                "gpt-5.4-fast[context=272k,fast=false]",
                "gpt-5.4",
                "GPT-5.4",
            ),
            (
                "gpt-5.5[reasoning=extra-high,fast=true]",
                "gpt-5.5",
                "GPT-5.5 Fast",
            ),
            (
                "gpt-5.6-luna[reasoning=low,fast=true]",
                "gpt-5.6-luna",
                "GPT-5.6 Luna Fast",
            ),
            (
                "gpt-5.6-sol[reasoning=medium,fast=false]",
                "gpt-5.6-sol",
                "GPT-5.6 Sol",
            ),
            (
                "gpt-5.6-terra[reasoning=max,fast=true]",
                "gpt-5.6-terra",
                "GPT-5.6 Terra Fast",
            ),
        ] {
            let identity = resolve("cursor", raw);
            assert_eq!(identity.canonical_id, canonical, "{raw} canonical id");
            assert_eq!(identity.display, display, "{raw} display");
            assert_eq!(identity.provider, Provider::OpenAI, "{raw} provider");
        }
    }

    #[test]
    fn cursor_third_party_models_fold_base_and_effort_ids() {
        for (raw, canonical, display, family) in [
            ("glm-5.2", "glm-5.2", "GLM 5.2", "GLM"),
            ("glm-5.2-max", "glm-5.2", "GLM 5.2", "GLM"),
            ("kimi-k2.7-code", "kimi-k2.7-code", "Kimi K2.7 Code", "Kimi"),
            (
                "kimi-k2.7-code-high",
                "kimi-k2.7-code",
                "Kimi K2.7 Code",
                "Kimi",
            ),
        ] {
            let identity = resolve("cursor", raw);
            assert_eq!(identity.canonical_id, canonical, "{raw} canonical id");
            assert_eq!(identity.display, display, "{raw} display");
            assert_eq!(identity.provider, Provider::Other, "{raw} provider");
            assert_eq!(identity.family, family, "{raw} family");
        }
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
