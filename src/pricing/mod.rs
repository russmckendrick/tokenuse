use std::collections::HashMap;
use std::fs;
use std::sync::OnceLock;

use serde::Deserialize;

use crate::tools::{ParsedCall, Speed};

#[cfg(feature = "refresh-prices")]
pub mod refresh;

const EMBEDDED_SNAPSHOT: &str = include_str!("snapshot.json");

#[derive(Debug, Deserialize, Clone, Default)]
pub struct ModelPrice {
    #[serde(default)]
    pub input: f64,
    #[serde(default)]
    pub output: f64,
    #[serde(default)]
    pub cache_write: f64,
    #[serde(default)]
    pub cache_read: f64,
    #[serde(default)]
    pub web_search: f64,
    #[serde(default)]
    pub fast_multiplier: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct Snapshot {
    models: HashMap<String, ModelPrice>,
    aliases: HashMap<String, String>,
    fallback: String,
}

pub struct PriceTable {
    models: HashMap<String, ModelPrice>,
    aliases: HashMap<String, String>,
    fallback_key: String,
}

impl PriceTable {
    pub fn configured() -> &'static Self {
        static TABLE: OnceLock<PriceTable> = OnceLock::new();
        TABLE.get_or_init(|| Self::local().unwrap_or_else(Self::from_embedded))
    }

    pub fn embedded() -> &'static Self {
        static TABLE: OnceLock<PriceTable> = OnceLock::new();
        TABLE.get_or_init(Self::from_embedded)
    }

    pub fn lookup(&self, model: &str) -> &ModelPrice {
        let canonical = canonicalize(model);
        if let Some(price) = self.models.get(&canonical) {
            return price;
        }
        if let Some(target) = self.aliases.get(&canonical) {
            if let Some(price) = self.models.get(target) {
                return price;
            }
        }
        if let Some((_, price)) = self
            .models
            .iter()
            .filter(|(key, _)| canonical.starts_with(*key))
            .max_by_key(|(key, _)| key.len())
        {
            return price;
        }
        self.models
            .get(&self.fallback_key)
            .expect("fallback model present in snapshot")
    }

    pub fn cache_read_rate_label(&self, model: &str) -> String {
        let price = self.lookup(model);
        rate_label(price.cache_read, price.input)
    }

    pub fn cache_write_rate_label(&self, model: &str) -> String {
        let price = self.lookup(model);
        rate_label(price.cache_write, price.input)
    }

    fn local() -> Option<Self> {
        let paths = crate::config::ConfigPaths::default();
        let raw = fs::read_to_string(paths.pricing_snapshot_file).ok()?;
        Self::from_json(&raw).ok()
    }

    fn from_embedded() -> Self {
        Self::from_json(EMBEDDED_SNAPSHOT).expect("embedded pricing snapshot must be valid JSON")
    }

    fn from_json(raw: &str) -> Result<Self, serde_json::Error> {
        let snap: Snapshot = serde_json::from_str(raw)?;
        Ok(PriceTable {
            models: snap.models,
            aliases: snap.aliases,
            fallback_key: snap.fallback,
        })
    }
}

pub fn cache_read_rate_label(model: &str) -> String {
    #[cfg(test)]
    let table = PriceTable::embedded();
    #[cfg(not(test))]
    let table = PriceTable::configured();
    table.cache_read_rate_label(model)
}

pub fn cache_write_rate_label(model: &str) -> String {
    #[cfg(test)]
    let table = PriceTable::embedded();
    #[cfg(not(test))]
    let table = PriceTable::configured();
    table.cache_write_rate_label(model)
}

fn rate_label(rate: f64, input: f64) -> String {
    if rate <= 0.0 || input <= 0.0 {
        return "-".into();
    }
    let pct = (rate / input) * 100.0;
    if (pct - pct.round()).abs() < 0.05 {
        format!("{:.0}%", pct)
    } else {
        format!("{pct:.1}%")
    }
}

fn canonicalize(model: &str) -> String {
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
    s
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

pub fn cost(model: &str, call: &ParsedCall, speed: Speed) -> f64 {
    #[cfg(test)]
    let price = PriceTable::embedded().lookup(model);
    #[cfg(not(test))]
    let price = PriceTable::configured().lookup(model);
    let multiplier = match (speed, price.fast_multiplier) {
        (Speed::Fast, Some(m)) => m,
        _ => 1.0,
    };

    let input = call.input_tokens as f64;
    let output = call.output_tokens as f64;
    let cache_w = call.cache_creation_input_tokens as f64;
    let cache_r = call.cache_read_input_tokens as f64;
    let web = call.web_search_requests as f64;

    multiplier
        * (input * price.input
            + output * price.output
            + cache_w * price.cache_write
            + cache_r * price.cache_read
            + web * price.web_search)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opus_47_resolves_with_date_suffix_and_pin() {
        let p = PriceTable::embedded().lookup("anthropic/claude-opus-4-7-20250514@v1");
        assert!(p.input > 0.0);
        assert_eq!(p.fast_multiplier, Some(6.0));
    }

    #[test]
    fn cursor_auto_alias_resolves() {
        let p = PriceTable::embedded().lookup("cursor-auto");
        assert!(p.input > 0.0);
        assert!(p.fast_multiplier.is_none());
    }

    #[test]
    fn claude_cache_rates_match_prompt_cache_pricing() {
        let table = PriceTable::embedded();

        assert_eq!(table.cache_read_rate_label("claude-sonnet-4-6"), "10%");
        assert_eq!(table.cache_write_rate_label("claude-sonnet-4-6"), "125%");
    }

    #[test]
    fn current_gpt_codex_cache_reads_are_ten_percent() {
        let table = PriceTable::embedded();

        assert_eq!(table.cache_read_rate_label("gpt-5.3-codex"), "10%");
        assert_eq!(table.cache_read_rate_label("gpt-5.4"), "10%");
    }

    #[test]
    fn codex_mini_cache_read_is_twenty_five_percent() {
        let table = PriceTable::embedded();

        assert_eq!(table.cache_read_rate_label("codex-mini-latest"), "25%");
    }

    #[test]
    fn cursor_auto_cache_read_is_twenty_percent() {
        let table = PriceTable::embedded();

        assert_eq!(table.cache_read_rate_label("cursor-auto"), "20%");
        assert_eq!(table.cache_write_rate_label("cursor-auto"), "100%");
    }

    #[test]
    fn gemini_pro_cache_read_is_ten_percent() {
        let table = PriceTable::embedded();

        assert_eq!(table.cache_read_rate_label("gemini-2.5-pro"), "10%");
    }

    #[test]
    fn older_gpt_4o_cache_read_is_fifty_percent() {
        let table = PriceTable::embedded();

        assert_eq!(table.cache_read_rate_label("gpt-4o"), "50%");
    }

    #[test]
    fn fast_multiplier_only_applies_to_opus() {
        let call = ParsedCall {
            input_tokens: 1_000_000,
            ..ParsedCall::default()
        };
        let standard = cost("claude-sonnet-4-5", &call, Speed::Fast);
        let opus_std = cost("claude-opus-4-7", &call, Speed::Standard);
        let opus_fast = cost("claude-opus-4-7", &call, Speed::Fast);
        assert!((standard - 3.0).abs() < 0.001);
        assert!((opus_fast / opus_std - 6.0).abs() < 0.001);
    }
}
