use std::collections::HashMap;
use std::sync::OnceLock;

use serde::Deserialize;

use crate::providers::{ParsedCall, Speed};

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
    pub fn embedded() -> &'static Self {
        static TABLE: OnceLock<PriceTable> = OnceLock::new();
        TABLE.get_or_init(|| {
            let snap: Snapshot = serde_json::from_str(EMBEDDED_SNAPSHOT)
                .expect("embedded pricing snapshot must be valid JSON");
            PriceTable {
                models: snap.models,
                aliases: snap.aliases,
                fallback_key: snap.fallback,
            }
        })
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
        for (key, price) in &self.models {
            if canonical.starts_with(key) {
                return price;
            }
        }
        self.models
            .get(&self.fallback_key)
            .expect("fallback model present in snapshot")
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
    let price = PriceTable::embedded().lookup(model);
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
    fn fast_multiplier_only_applies_to_opus() {
        let mut call = ParsedCall::default();
        call.input_tokens = 1_000_000;
        let standard = cost("claude-sonnet-4-5", &call, Speed::Fast);
        let opus_std = cost("claude-opus-4-7", &call, Speed::Standard);
        let opus_fast = cost("claude-opus-4-7", &call, Speed::Fast);
        assert!((standard - 3.0).abs() < 0.001);
        assert!((opus_fast / opus_std - 6.0).abs() < 0.001);
    }
}
