use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::{OnceLock, RwLock};

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::tools::{ParsedCall, Speed};

#[cfg(feature = "refresh-prices")]
pub mod refresh;

pub(crate) const SOURCES_CONFIG: &str = include_str!("../../costs/pricing-sources.json");
const EMBEDDED_UPSTREAM: &str = include_str!("../../costs/pricing-upstream.json");
const EMBEDDED_OVERRIDES: &str = include_str!("../../costs/pricing-overrides.json");
const LEGACY_EMBEDDED_SNAPSHOT: &str = include_str!("snapshot.json");

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishedBookUrls {
    pub upstream: String,
    pub overrides: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PricingBookSource {
    LocalBooks,
    LegacySnapshot,
    EmbeddedBooks,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PricingBookStatus {
    pub source: PricingBookSource,
    pub date: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PricingSourcesManifest {
    published_books: PublishedBooksManifest,
}

#[derive(Debug, Deserialize)]
struct PublishedBooksManifest {
    upstream_url: String,
    overrides_url: String,
}

pub fn published_book_urls() -> Result<PublishedBookUrls, String> {
    let manifest: PricingSourcesManifest = serde_json::from_str(SOURCES_CONFIG)
        .map_err(|e| format!("parse pricing sources config: {e}"))?;
    Ok(PublishedBookUrls {
        upstream: manifest.published_books.upstream_url,
        overrides: manifest.published_books.overrides_url,
    })
}

pub fn configured_book_status(paths: &crate::config::ConfigPaths) -> PricingBookStatus {
    if paths.pricing_upstream_file.exists() && paths.pricing_overrides_file.exists() {
        let date = read_pricing_book_date(&[
            paths.pricing_upstream_file.as_path(),
            paths.pricing_overrides_file.as_path(),
        ]);
        return PricingBookStatus {
            source: PricingBookSource::LocalBooks,
            date,
        };
    }

    if paths.pricing_snapshot_file.exists() {
        let date = read_pricing_book_date(&[paths.pricing_snapshot_file.as_path()]);
        return PricingBookStatus {
            source: PricingBookSource::LegacySnapshot,
            date,
        };
    }

    PricingBookStatus {
        source: PricingBookSource::EmbeddedBooks,
        date: pricing_book_date_from_raw(&[EMBEDDED_UPSTREAM, EMBEDDED_OVERRIDES])
            .or_else(|| pricing_book_date_from_raw(&[LEGACY_EMBEDDED_SNAPSHOT])),
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, Default, PartialEq)]
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
    #[serde(default)]
    pub effective_from: Option<NaiveDate>,
    #[serde(default)]
    pub provenance: Option<PriceProvenance>,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
pub struct PriceProvenance {
    pub source_name: String,
    pub source_url: String,
    pub checked_at: String,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpstreamBook {
    #[serde(default)]
    models: HashMap<String, ModelPrice>,
}

#[derive(Debug, Deserialize, Default)]
struct OverrideBook {
    #[serde(default)]
    models: HashMap<String, ModelPrice>,
    #[serde(default)]
    tool_models: HashMap<String, HashMap<String, ModelPrice>>,
    #[serde(default)]
    aliases: HashMap<String, String>,
    #[serde(default)]
    tool_aliases: HashMap<String, HashMap<String, String>>,
    fallback: String,
}

#[derive(Debug, Deserialize)]
struct LegacySnapshot {
    models: HashMap<String, ModelPrice>,
    aliases: HashMap<String, String>,
    fallback: String,
}

#[derive(Debug)]
pub struct PriceTable {
    models: HashMap<String, Vec<ModelPrice>>,
    tool_models: HashMap<String, HashMap<String, Vec<ModelPrice>>>,
    aliases: HashMap<String, String>,
    tool_aliases: HashMap<String, HashMap<String, String>>,
    fallback_key: String,
}

impl PriceTable {
    pub fn configured() -> &'static RwLock<Self> {
        configured_table()
    }

    pub fn reload_configured() -> Result<(), String> {
        let table = Self::local().unwrap_or_else(Self::from_embedded);
        let mut configured = configured_table()
            .write()
            .map_err(|_| "pricing table lock poisoned".to_string())?;
        *configured = table;
        Ok(())
    }

    pub fn embedded() -> &'static Self {
        static TABLE: OnceLock<PriceTable> = OnceLock::new();
        TABLE.get_or_init(Self::from_embedded)
    }

    pub fn lookup(&self, model: &str) -> &ModelPrice {
        self.lookup_for("", model, None)
    }

    pub fn lookup_for(
        &self,
        tool: &str,
        model: &str,
        timestamp: Option<DateTime<Utc>>,
    ) -> &ModelPrice {
        let date = effective_date(timestamp);
        let tool_key = tool.trim().to_ascii_lowercase();
        let canonical = canonicalize(model);
        let tool_target = self
            .tool_aliases
            .get(&tool_key)
            .and_then(|aliases| aliases.get(&canonical))
            .map(String::as_str)
            .unwrap_or(canonical.as_str());

        if let Some(price) = self.lookup_tool(&tool_key, tool_target, date) {
            return price;
        }
        if let Some(price) = self.lookup_global(tool_target, date) {
            return price;
        }
        self.lookup_global(&self.fallback_key, date)
            .expect("fallback model present in price books")
    }

    pub fn cache_read_rate_label(&self, model: &str) -> String {
        let price = self.lookup(model);
        rate_label(price.cache_read, price.input)
    }

    pub fn cache_read_rate_label_for(
        &self,
        tool: &str,
        model: &str,
        timestamp: Option<DateTime<Utc>>,
    ) -> String {
        let price = self.lookup_for(tool, model, timestamp);
        rate_label(price.cache_read, price.input)
    }

    pub fn cache_write_rate_label(&self, model: &str) -> String {
        let price = self.lookup(model);
        rate_label(price.cache_write, price.input)
    }

    pub fn cache_write_rate_label_for(
        &self,
        tool: &str,
        model: &str,
        timestamp: Option<DateTime<Utc>>,
    ) -> String {
        let price = self.lookup_for(tool, model, timestamp);
        rate_label(price.cache_write, price.input)
    }

    pub fn local_from_paths(paths: &crate::config::ConfigPaths) -> Result<Self, String> {
        if paths.pricing_upstream_file.exists() || paths.pricing_overrides_file.exists() {
            if !paths.pricing_upstream_file.exists() {
                return Err(format!("missing {}", paths.pricing_upstream_file.display()));
            }
            if !paths.pricing_overrides_file.exists() {
                return Err(format!(
                    "missing {}",
                    paths.pricing_overrides_file.display()
                ));
            }
            let upstream = fs::read_to_string(&paths.pricing_upstream_file)
                .map_err(|e| format!("read {}: {e}", paths.pricing_upstream_file.display()))?;
            let overrides = fs::read_to_string(&paths.pricing_overrides_file)
                .map_err(|e| format!("read {}: {e}", paths.pricing_overrides_file.display()))?;
            return Self::from_books(&upstream, &overrides);
        }

        let raw = fs::read_to_string(&paths.pricing_snapshot_file)
            .map_err(|e| format!("read {}: {e}", paths.pricing_snapshot_file.display()))?;
        Self::from_legacy_json(&raw)
    }

    fn local() -> Option<Self> {
        let paths = crate::config::ConfigPaths::default();
        Self::local_from_paths(&paths).ok()
    }

    fn from_embedded() -> Self {
        Self::from_books(EMBEDDED_UPSTREAM, EMBEDDED_OVERRIDES)
            .or_else(|_| Self::from_legacy_json(LEGACY_EMBEDDED_SNAPSHOT))
            .expect("embedded pricing books must be valid JSON")
    }

    pub(crate) fn from_books(upstream_raw: &str, overrides_raw: &str) -> Result<Self, String> {
        let upstream: UpstreamBook =
            serde_json::from_str(upstream_raw).map_err(|e| format!("parse upstream book: {e}"))?;
        let overrides: OverrideBook =
            serde_json::from_str(overrides_raw).map_err(|e| format!("parse override book: {e}"))?;
        let fallback_key = canonicalize(&overrides.fallback);
        if fallback_key.is_empty() {
            return Err("pricing fallback cannot be empty".into());
        }

        let mut table = PriceTable {
            models: HashMap::new(),
            tool_models: HashMap::new(),
            aliases: normalize_aliases(overrides.aliases),
            tool_aliases: normalize_tool_aliases(overrides.tool_aliases),
            fallback_key,
        };

        for (key, price) in upstream.models {
            table.insert_global_price(&key, price)?;
        }
        for (key, price) in overrides.models {
            table.insert_global_price(&key, price)?;
        }
        for (tool, models) in overrides.tool_models {
            for (key, price) in models {
                table.insert_tool_price(&tool, &key, price)?;
            }
        }
        table.validate_fallback()?;
        Ok(table)
    }

    fn from_legacy_json(raw: &str) -> Result<Self, String> {
        let snap: LegacySnapshot =
            serde_json::from_str(raw).map_err(|e| format!("parse legacy pricing snapshot: {e}"))?;
        let fallback_key = canonicalize(&snap.fallback);
        let mut table = PriceTable {
            models: HashMap::new(),
            tool_models: HashMap::new(),
            aliases: normalize_aliases(snap.aliases),
            tool_aliases: HashMap::new(),
            fallback_key,
        };
        for (key, price) in snap.models {
            table.insert_global_price(&key, price)?;
        }
        table.validate_fallback()?;
        Ok(table)
    }

    fn insert_global_price(&mut self, key: &str, price: ModelPrice) -> Result<(), String> {
        let key = canonicalize(key);
        if key.is_empty() {
            return Err("pricing model key cannot be empty".into());
        }
        insert_price(&mut self.models, key, price)
    }

    fn insert_tool_price(
        &mut self,
        tool: &str,
        key: &str,
        price: ModelPrice,
    ) -> Result<(), String> {
        let tool = tool.trim().to_ascii_lowercase();
        if tool.is_empty() {
            return Err("tool-scoped pricing key cannot be empty".into());
        }
        let key = canonicalize(key);
        if key.is_empty() {
            return Err("tool-scoped pricing model key cannot be empty".into());
        }
        insert_price(self.tool_models.entry(tool).or_default(), key, price)
    }

    fn lookup_tool(&self, tool: &str, model: &str, date: NaiveDate) -> Option<&ModelPrice> {
        let models = self.tool_models.get(tool)?;
        lookup_in_models(models, model, date)
    }

    fn lookup_global(&self, model: &str, date: NaiveDate) -> Option<&ModelPrice> {
        let target = self.aliases.get(model).map(String::as_str).unwrap_or(model);
        lookup_in_models(&self.models, target, date)
    }

    fn validate_fallback(&self) -> Result<(), String> {
        let earliest = NaiveDate::from_ymd_opt(1970, 1, 1).expect("valid fallback date");
        self.lookup_global(&self.fallback_key, earliest)
            .map(|_| ())
            .ok_or_else(|| {
                format!(
                    "fallback model {} not present in price books",
                    self.fallback_key
                )
            })
    }
}

fn configured_table() -> &'static RwLock<PriceTable> {
    static TABLE: OnceLock<RwLock<PriceTable>> = OnceLock::new();
    TABLE.get_or_init(|| RwLock::new(PriceTable::local().unwrap_or_else(PriceTable::from_embedded)))
}

#[cfg(not(test))]
fn with_configured<R>(f: impl FnOnce(&PriceTable) -> R) -> R {
    let table = configured_table()
        .read()
        .expect("pricing table lock must not be poisoned");
    f(&table)
}

fn insert_price(
    models: &mut HashMap<String, Vec<ModelPrice>>,
    key: String,
    price: ModelPrice,
) -> Result<(), String> {
    validate_price(&key, &price)?;
    let effective_from = price.effective_from;
    let entries = models.entry(key).or_default();
    if let Some(existing) = entries
        .iter_mut()
        .find(|entry| entry.effective_from == effective_from)
    {
        *existing = price;
    } else {
        entries.push(price);
    }
    entries.sort_by_key(|entry| entry.effective_from);
    Ok(())
}

fn validate_price(key: &str, price: &ModelPrice) -> Result<(), String> {
    for (field, value) in [
        ("input", price.input),
        ("output", price.output),
        ("cache_write", price.cache_write),
        ("cache_read", price.cache_read),
        ("web_search", price.web_search),
    ] {
        if value < 0.0 || !value.is_finite() {
            return Err(format!("invalid {field} price for {key}: {value}"));
        }
    }
    if let Some(multiplier) = price.fast_multiplier {
        if multiplier <= 0.0 || !multiplier.is_finite() {
            return Err(format!("invalid fast_multiplier for {key}: {multiplier}"));
        }
    }
    Ok(())
}

fn lookup_in_models<'a>(
    models: &'a HashMap<String, Vec<ModelPrice>>,
    model: &str,
    date: NaiveDate,
) -> Option<&'a ModelPrice> {
    if let Some(price) = models
        .get(model)
        .and_then(|entries| effective_entry(entries, date))
    {
        return Some(price);
    }

    models
        .iter()
        .filter(|(key, _)| model.starts_with(key.as_str()))
        .filter_map(|(key, entries)| effective_entry(entries, date).map(|price| (key, price)))
        .max_by_key(|(key, _)| key.len())
        .map(|(_, price)| price)
}

fn effective_entry(entries: &[ModelPrice], date: NaiveDate) -> Option<&ModelPrice> {
    entries
        .iter()
        .filter(|entry| entry.effective_from.map(|d| d <= date).unwrap_or(true))
        .max_by_key(|entry| entry.effective_from)
}

fn normalize_aliases(aliases: HashMap<String, String>) -> HashMap<String, String> {
    aliases
        .into_iter()
        .map(|(key, value)| (canonicalize(&key), canonicalize(&value)))
        .collect()
}

fn normalize_tool_aliases(
    aliases: HashMap<String, HashMap<String, String>>,
) -> HashMap<String, HashMap<String, String>> {
    aliases
        .into_iter()
        .map(|(tool, entries)| (tool.to_ascii_lowercase(), normalize_aliases(entries)))
        .collect()
}

pub fn cache_read_rate_label(model: &str) -> String {
    #[cfg(test)]
    let table = PriceTable::embedded();
    #[cfg(not(test))]
    return with_configured(|table| table.cache_read_rate_label(model));
    #[cfg(test)]
    table.cache_read_rate_label(model)
}

pub fn cache_read_rate_label_for(
    tool: &str,
    model: &str,
    timestamp: Option<DateTime<Utc>>,
) -> String {
    #[cfg(test)]
    let table = PriceTable::embedded();
    #[cfg(not(test))]
    return with_configured(|table| table.cache_read_rate_label_for(tool, model, timestamp));
    #[cfg(test)]
    table.cache_read_rate_label_for(tool, model, timestamp)
}

pub fn cache_write_rate_label(model: &str) -> String {
    #[cfg(test)]
    let table = PriceTable::embedded();
    #[cfg(not(test))]
    return with_configured(|table| table.cache_write_rate_label(model));
    #[cfg(test)]
    table.cache_write_rate_label(model)
}

pub fn cache_write_rate_label_for(
    tool: &str,
    model: &str,
    timestamp: Option<DateTime<Utc>>,
) -> String {
    #[cfg(test)]
    let table = PriceTable::embedded();
    #[cfg(not(test))]
    return with_configured(|table| table.cache_write_rate_label_for(tool, model, timestamp));
    #[cfg(test)]
    table.cache_write_rate_label_for(tool, model, timestamp)
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

fn read_pricing_book_date(paths: &[&Path]) -> Option<String> {
    let raws: Vec<String> = paths
        .iter()
        .filter_map(|path| fs::read_to_string(path).ok())
        .collect();
    let raw_refs: Vec<&str> = raws.iter().map(String::as_str).collect();
    pricing_book_date_from_raw(&raw_refs)
}

fn pricing_book_date_from_raw(raws: &[&str]) -> Option<String> {
    let latest = raws
        .iter()
        .filter_map(|raw| serde_json::from_str::<Value>(raw).ok())
        .flat_map(|value| {
            let mut dates = Vec::new();
            collect_pricing_book_dates(&value, &mut dates);
            dates
        })
        .max();

    latest.map(|date| date.format("%Y-%m-%d").to_string())
}

fn collect_pricing_book_dates(value: &Value, dates: &mut Vec<NaiveDate>) {
    match value {
        Value::Object(map) => {
            for key in ["checked_at", "generated_at"] {
                if let Some(date) = map
                    .get(key)
                    .and_then(Value::as_str)
                    .and_then(parse_pricing_book_date)
                {
                    dates.push(date);
                }
            }
            for child in map.values() {
                collect_pricing_book_dates(child, dates);
            }
        }
        Value::Array(values) => {
            for child in values {
                collect_pricing_book_dates(child, dates);
            }
        }
        _ => {}
    }
}

fn parse_pricing_book_date(value: &str) -> Option<NaiveDate> {
    let date = value.trim().get(..10)?;
    NaiveDate::parse_from_str(date, "%Y-%m-%d").ok()
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

fn effective_date(timestamp: Option<DateTime<Utc>>) -> NaiveDate {
    timestamp
        .map(|dt| dt.date_naive())
        .unwrap_or_else(|| Utc::now().date_naive())
}

pub fn cost(model: &str, call: &ParsedCall, speed: Speed) -> f64 {
    #[cfg(test)]
    let price = PriceTable::embedded().lookup_for(call.tool, model, call.timestamp);
    #[cfg(not(test))]
    let price = with_configured(|table| table.lookup_for(call.tool, model, call.timestamp).clone());
    #[cfg(test)]
    let price = price.clone();
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
    use chrono::TimeZone;

    use super::*;

    fn call_at(tool: &'static str, model: &str, date: (i32, u32, u32)) -> ParsedCall {
        ParsedCall {
            tool,
            model: model.into(),
            input_tokens: 1_000_000,
            output_tokens: 1_000_000,
            cache_read_input_tokens: 1_000_000,
            timestamp: Some(
                Utc.with_ymd_and_hms(date.0, date.1, date.2, 12, 0, 0)
                    .unwrap(),
            ),
            ..ParsedCall::default()
        }
    }

    #[test]
    fn opus_47_resolves_with_date_suffix_and_pin() {
        let p = PriceTable::embedded().lookup("anthropic/claude-opus-4-7-20250514@v1");
        assert!(p.input > 0.0);
        assert!(p.fast_multiplier.is_none());
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
    fn published_book_urls_are_loaded_from_sources_config() {
        let urls = published_book_urls().unwrap();

        assert!(urls.upstream.ends_with("/pricing-upstream.json"));
        assert!(urls.overrides.ends_with("/pricing-overrides.json"));
    }

    #[test]
    fn embedded_book_status_reports_latest_checked_date() {
        let status = configured_book_status(&crate::config::ConfigPaths::new(
            std::path::PathBuf::from("/tmp/tokenuse-pricing-status-missing"),
        ));

        assert_eq!(status.source, PricingBookSource::EmbeddedBooks);
        assert!(status.date.is_some());
    }

    #[test]
    fn pricing_book_date_uses_latest_checked_or_generated_date() {
        let raw = r#"{
          "_metadata": {"generated_at": "2026-05-01T10:00:00Z"},
          "models": {
            "a": {"provenance": {"checked_at": "2026-05-06"}},
            "b": {"provenance": {"checked_at": "2026-04-30"}}
          }
        }"#;

        assert_eq!(
            pricing_book_date_from_raw(&[raw]),
            Some("2026-05-06".into())
        );
    }

    #[test]
    fn fast_multiplier_only_applies_to_configured_opus_fast_mode() {
        let call = ParsedCall {
            input_tokens: 1_000_000,
            ..ParsedCall::default()
        };
        let standard = cost("claude-sonnet-4-5", &call, Speed::Fast);
        let opus_46_std = cost("claude-opus-4-6", &call, Speed::Standard);
        let opus_46_fast = cost("claude-opus-4-6", &call, Speed::Fast);
        let opus_47_std = cost("claude-opus-4-7", &call, Speed::Standard);
        let opus_47_fast = cost("claude-opus-4-7", &call, Speed::Fast);

        assert!((standard - 3.0).abs() < 0.001);
        assert!((opus_46_fast / opus_46_std - 6.0).abs() < 0.001);
        assert!((opus_47_fast - opus_47_std).abs() < 0.001);
    }

    #[test]
    fn copilot_pricing_is_gated_until_june_2026() {
        let before = call_at("copilot", "Grok Code Fast 1", (2026, 5, 31));
        let after = call_at("copilot", "Grok Code Fast 1", (2026, 6, 1));
        let codex_after = call_at("codex", "Grok Code Fast 1", (2026, 6, 1));

        let before_cost = cost(&before.model, &before, Speed::Standard);
        let after_cost = cost(&after.model, &after, Speed::Standard);
        let codex_cost = cost(&codex_after.model, &codex_after, Speed::Standard);

        assert!((after_cost - 1.72).abs() < 0.001);
        assert!((before_cost - after_cost).abs() > 0.001);
        assert!((codex_cost - before_cost).abs() < 0.001);
    }

    #[test]
    fn tool_scoped_aliases_do_not_override_other_tools() {
        let copilot = call_at("copilot", "Claude Opus 4.7", (2026, 6, 1));
        let claude = call_at("claude-code", "Claude Opus 4.7", (2026, 6, 1));

        let copilot_cost = cost(&copilot.model, &copilot, Speed::Standard);
        let claude_cost = cost(&claude.model, &claude, Speed::Standard);

        assert!((copilot_cost - 30.5).abs() < 0.001);
        assert!((claude_cost - 18.3).abs() < 0.001);
        assert_eq!(
            PriceTable::embedded().cache_read_rate_label_for(
                "copilot",
                "GPT-4.1",
                copilot.timestamp
            ),
            "25%"
        );
    }

    #[test]
    fn legacy_snapshot_loader_still_works() {
        let table = PriceTable::from_legacy_json(LEGACY_EMBEDDED_SNAPSHOT).unwrap();
        assert_eq!(table.cache_read_rate_label("cursor-auto"), "20%");
    }

    #[test]
    fn rejects_invalid_prices() {
        let upstream = r#"{"models":{"bad":{"input":-1.0}}}"#;
        let overrides = r#"{"fallback":"bad"}"#;
        let err = PriceTable::from_books(upstream, overrides).unwrap_err();
        assert!(err.contains("invalid input price"));
    }
}
