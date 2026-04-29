use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use chrono::{SecondsFormat, Utc};
use color_eyre::{
    eyre::{eyre, Context},
    Result,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const BASE_CURRENCY: &str = "USD";
const EXCLUDED_NON_FIAT_CODES: [&str; 5] = ["XAG", "XAU", "XDR", "XPD", "XPT"];

#[derive(Debug, Deserialize)]
struct FrankfurterRate {
    date: String,
    base: String,
    quote: String,
    rate: f64,
}

#[derive(Debug, Serialize)]
struct CurrencySnapshot {
    base: &'static str,
    date: String,
    generated_at: String,
    source: SnapshotSource,
    rates: BTreeMap<String, f64>,
}

#[derive(Debug, Serialize)]
struct SnapshotSource {
    name: &'static str,
    url: &'static str,
    coverage: &'static str,
}

pub fn run(output: &Path) -> Result<()> {
    let rows: Vec<FrankfurterRate> = ureq::get(crate::config::FRANKFURTER_RATES_URL)
        .call()
        .map_err(|e| eyre!("fetch frankfurter currency rates: {e}"))?
        .into_json()
        .map_err(|e| eyre!("parse frankfurter currency json: {e}"))?;

    let generated_at = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    let snapshot = build_snapshot(rows, generated_at)?;
    write_snapshot(output, &snapshot)
}

pub fn download_published_snapshot(output: &Path) -> Result<()> {
    let raw = ureq::get(crate::config::CURRENCY_RATES_URL)
        .call()
        .map_err(|e| eyre!("fetch published currency rates: {e}"))?
        .into_string()
        .map_err(|e| eyre!("read published currency rates: {e}"))?;

    super::CurrencyTable::from_json_str(&raw, super::RateSource::Local(output.to_path_buf()))?;

    let parsed: Value =
        serde_json::from_str(&raw).map_err(|e| eyre!("parse published currency json: {e}"))?;
    write_json_value(output, &parsed)
}

fn build_snapshot(rows: Vec<FrankfurterRate>, generated_at: String) -> Result<CurrencySnapshot> {
    if rows.is_empty() {
        return Err(eyre!("frankfurter response did not include any rate rows"));
    }

    let mut latest_date: Option<String> = None;
    let mut rates = BTreeMap::from([(BASE_CURRENCY.to_string(), 1.0)]);

    for row in rows {
        let base = normalize_code(&row.base)?;
        if base != BASE_CURRENCY {
            return Err(eyre!("expected base currency USD, got {}", row.base));
        }

        let quote = normalize_code(&row.quote)?;
        if is_excluded_non_fiat(&quote) {
            continue;
        }
        if row.rate <= 0.0 || !row.rate.is_finite() {
            return Err(eyre!("invalid rate for {quote}: {}", row.rate));
        }

        latest_date = Some(
            latest_date
                .map(|existing| existing.max(row.date.clone()))
                .unwrap_or_else(|| row.date.clone()),
        );

        rates.insert(quote, row.rate);
    }

    if rates.len() <= 1 {
        return Err(eyre!(
            "frankfurter response did not include usable fiat rates"
        ));
    }

    Ok(CurrencySnapshot {
        base: BASE_CURRENCY,
        date: latest_date.ok_or_else(|| eyre!("frankfurter response did not include a date"))?,
        generated_at,
        source: SnapshotSource {
            name: "Frankfurter",
            url: crate::config::FRANKFURTER_RATES_URL,
            coverage: "fiat",
        },
        rates,
    })
}

fn write_snapshot(output: &Path, snapshot: &CurrencySnapshot) -> Result<()> {
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).wrap_err_with(|| format!("create {}", parent.display()))?;
    }
    let mut pretty = serde_json::to_string_pretty(snapshot)?;
    pretty.push('\n');
    fs::write(output, pretty).wrap_err_with(|| format!("write {}", output.display()))?;
    Ok(())
}

fn write_json_value(output: &Path, value: &Value) -> Result<()> {
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).wrap_err_with(|| format!("create {}", parent.display()))?;
    }
    let mut pretty = serde_json::to_string_pretty(value)?;
    pretty.push('\n');
    fs::write(output, pretty).wrap_err_with(|| format!("write {}", output.display()))?;
    Ok(())
}

fn normalize_code(code: &str) -> Result<String> {
    let normalized = code.trim().to_ascii_uppercase();
    if normalized.len() != 3 || !normalized.chars().all(|c| c.is_ascii_uppercase()) {
        return Err(eyre!("invalid currency code {code:?}"));
    }
    Ok(normalized)
}

fn is_excluded_non_fiat(code: &str) -> bool {
    EXCLUDED_NON_FIAT_CODES.contains(&code)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(date: &str, base: &str, quote: &str, rate: f64) -> FrankfurterRate {
        FrankfurterRate {
            date: date.into(),
            base: base.into(),
            quote: quote.into(),
            rate,
        }
    }

    #[test]
    fn builds_usd_snapshot_from_frankfurter_rows() {
        let snapshot = build_snapshot(
            vec![
                row("2026-04-29", "USD", "eur", 0.85496),
                row("2026-04-29", "USD", "GBP", 0.74073),
                row("2026-04-29", "USD", "JPY", 159.72),
            ],
            "2026-04-29T12:00:00Z".into(),
        )
        .unwrap();

        assert_eq!(snapshot.base, "USD");
        assert_eq!(snapshot.date, "2026-04-29");
        assert_eq!(snapshot.rates.get("USD"), Some(&1.0));
        assert_eq!(snapshot.rates.get("EUR"), Some(&0.85496));
        assert_eq!(snapshot.rates.get("GBP"), Some(&0.74073));
    }

    #[test]
    fn rejects_non_usd_base() {
        let err = build_snapshot(
            vec![row("2026-04-29", "EUR", "USD", 1.168)],
            "2026-04-29T12:00:00Z".into(),
        )
        .unwrap_err();

        assert!(err.to_string().contains("expected base currency USD"));
    }

    #[test]
    fn normalizes_codes_and_filters_non_fiat_assets() {
        let snapshot = build_snapshot(
            vec![
                row("2026-04-29", "usd", "xau", 0.00022),
                row("2026-04-29", "usd", "XAG", 0.01317),
                row("2026-04-29", "usd", "XDR", 0.72927),
                row("2026-04-29", "usd", "xaf", 567.43),
                row("2026-04-29", "usd", "eur", 0.85496),
            ],
            "2026-04-29T12:00:00Z".into(),
        )
        .unwrap();

        assert!(!snapshot.rates.contains_key("XAU"));
        assert!(!snapshot.rates.contains_key("XAG"));
        assert!(!snapshot.rates.contains_key("XDR"));
        assert_eq!(snapshot.rates.get("XAF"), Some(&567.43));
        assert_eq!(snapshot.rates.get("EUR"), Some(&0.85496));
    }

    #[test]
    fn serializes_rates_in_deterministic_order() {
        let snapshot = build_snapshot(
            vec![
                row("2026-04-29", "USD", "JPY", 159.72),
                row("2026-04-29", "USD", "EUR", 0.85496),
                row("2026-04-29", "USD", "GBP", 0.74073),
            ],
            "2026-04-29T12:00:00Z".into(),
        )
        .unwrap();

        let keys: Vec<&str> = snapshot.rates.keys().map(String::as_str).collect();
        assert_eq!(keys, vec!["EUR", "GBP", "JPY", "USD"]);
    }

    #[test]
    fn uses_latest_included_rate_date() {
        let snapshot = build_snapshot(
            vec![
                row("2026-04-28", "USD", "BMD", 1.0),
                row("2026-04-29", "USD", "EUR", 0.85496),
            ],
            "2026-04-29T12:00:00Z".into(),
        )
        .unwrap();

        assert_eq!(snapshot.date, "2026-04-29");
    }

    #[test]
    fn rejects_invalid_or_missing_rates() {
        let err = build_snapshot(
            vec![row("2026-04-29", "USD", "EUR", 0.0)],
            "2026-04-29T12:00:00Z".into(),
        )
        .unwrap_err();
        assert!(err.to_string().contains("invalid rate for EUR"));

        let err = build_snapshot(
            vec![row("2026-04-29", "USD", "XAU", 0.00022)],
            "2026-04-29T12:00:00Z".into(),
        )
        .unwrap_err();
        assert!(err.to_string().contains("usable fiat rates"));
    }
}
