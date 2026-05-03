use std::{collections::BTreeMap, fs, path::PathBuf};

use color_eyre::{
    eyre::{eyre, Context},
    Result,
};
use serde::Deserialize;

use crate::config::{self, ConfigPaths};

#[cfg(feature = "refresh-currency")]
pub mod refresh;

const EMBEDDED_RATES: &str = include_str!("../../currency/rates.json");

#[derive(Debug, Clone, Deserialize)]
pub struct CurrencySnapshot {
    pub base: String,
    pub date: String,
    pub generated_at: String,
    pub source: SnapshotSource,
    pub rates: BTreeMap<String, f64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SnapshotSource {
    pub name: String,
    pub url: String,
    pub coverage: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RateSource {
    Embedded,
    Local(PathBuf),
}

impl RateSource {
    pub fn label(&self) -> String {
        match self {
            Self::Embedded => "embedded".into(),
            Self::Local(path) => format!("local {}", path.display()),
        }
    }

    pub fn short_label(&self) -> &'static str {
        match self {
            Self::Embedded => "embedded",
            Self::Local(_) => "local",
        }
    }
}

#[derive(Debug, Clone)]
pub struct CurrencyTable {
    snapshot: CurrencySnapshot,
    source: RateSource,
}

impl CurrencyTable {
    pub fn load(paths: &ConfigPaths) -> Result<Self> {
        if paths.currency_rates_file.exists() {
            let raw = fs::read_to_string(&paths.currency_rates_file)
                .wrap_err_with(|| format!("read {}", paths.currency_rates_file.display()))?;
            return Self::from_json_str(&raw, RateSource::Local(paths.currency_rates_file.clone()))
                .wrap_err_with(|| format!("parse {}", paths.currency_rates_file.display()));
        }

        Self::embedded()
    }

    pub fn embedded() -> Result<Self> {
        Self::from_json_str(EMBEDDED_RATES, RateSource::Embedded)
            .wrap_err("parse embedded currency rates")
    }

    pub fn from_json_str(raw: &str, source: RateSource) -> Result<Self> {
        let mut snapshot: CurrencySnapshot =
            serde_json::from_str(raw).wrap_err("parse currency rates json")?;
        validate_snapshot(&mut snapshot)?;
        Ok(Self { snapshot, source })
    }

    pub fn formatter(&self, requested: &str) -> CurrencyFormatter {
        let code = config::normalize_currency_code(requested)
            .filter(|code| self.snapshot.rates.contains_key(code))
            .unwrap_or_else(|| config::DEFAULT_CURRENCY.into());
        let rate = self.snapshot.rates.get(&code).copied().unwrap_or(1.0);
        CurrencyFormatter { code, rate }
    }

    pub fn codes(&self) -> Vec<String> {
        let mut codes: Vec<String> = self.snapshot.rates.keys().cloned().collect();
        codes.sort_by(|a, b| {
            currency_rank(a)
                .cmp(&currency_rank(b))
                .then_with(|| a.cmp(b))
        });
        codes
    }

    pub fn rate(&self, code: &str) -> Option<f64> {
        let code = config::normalize_currency_code(code)?;
        self.snapshot.rates.get(&code).copied()
    }

    pub fn source(&self) -> &RateSource {
        &self.source
    }

    pub fn date(&self) -> &str {
        &self.snapshot.date
    }

    pub fn generated_at(&self) -> &str {
        &self.snapshot.generated_at
    }

    pub fn source_name(&self) -> &str {
        &self.snapshot.source.name
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CurrencyFormatter {
    code: String,
    rate: f64,
}

impl CurrencyFormatter {
    pub fn usd() -> Self {
        Self {
            code: config::DEFAULT_CURRENCY.into(),
            rate: 1.0,
        }
    }

    pub fn code(&self) -> &str {
        &self.code
    }

    pub fn is_usd(&self) -> bool {
        self.code == config::DEFAULT_CURRENCY
    }

    pub fn format_money(&self, amount_usd: f64) -> String {
        if self.is_usd() {
            return format_usd(amount_usd);
        }

        self.format_converted(amount_usd, false)
    }

    pub fn format_money_short(&self, amount_usd: f64) -> String {
        if self.is_usd() {
            return format_usd_short(amount_usd);
        }

        self.format_converted(amount_usd, true)
    }

    fn format_converted(&self, amount_usd: f64, short: bool) -> String {
        let amount = format_non_usd(amount_usd * self.rate, short);
        match currency_prefix(&self.code) {
            Some(prefix) if prefix.compact => format!("{}{amount}", prefix.text),
            Some(prefix) => format!("{} {amount}", prefix.text),
            None => format!("{} {}", self.code, amount),
        }
    }
}

fn validate_snapshot(snapshot: &mut CurrencySnapshot) -> Result<()> {
    snapshot.base = snapshot.base.trim().to_ascii_uppercase();
    if snapshot.base != config::DEFAULT_CURRENCY {
        return Err(eyre!(
            "expected base currency {}, got {}",
            config::DEFAULT_CURRENCY,
            snapshot.base
        ));
    }

    let mut normalized = BTreeMap::new();
    for (code, rate) in &snapshot.rates {
        let code = config::normalize_currency_code(code)
            .ok_or_else(|| eyre!("invalid currency code {code:?}"))?;
        if *rate <= 0.0 || !rate.is_finite() {
            return Err(eyre!("invalid rate for {code}: {rate}"));
        }
        normalized.insert(code, *rate);
    }
    normalized.insert(config::DEFAULT_CURRENCY.into(), 1.0);
    snapshot.rates = normalized;

    Ok(())
}

fn currency_rank(code: &str) -> (u8, &str) {
    let rank = match code {
        "USD" => 0,
        "GBP" => 1,
        "EUR" => 2,
        "CAD" => 3,
        "AUD" => 4,
        "JPY" => 5,
        _ => 9,
    };
    (rank, code)
}

fn format_usd(amount: f64) -> String {
    if amount >= 1.0 {
        format!("${amount:.2}")
    } else if amount >= 0.01 {
        format!("${amount:.3}")
    } else {
        format!("${amount:.4}")
    }
}

fn format_usd_short(amount: f64) -> String {
    if amount >= 100.0 {
        format!("${amount:.0}")
    } else if amount >= 10.0 {
        format!("${amount:.1}")
    } else if amount >= 0.01 {
        format!("${amount:.2}")
    } else {
        format!("${amount:.4}")
    }
}

fn format_non_usd(amount: f64, short: bool) -> String {
    if amount >= 1000.0 {
        format!("{amount:.0}")
    } else if short && amount >= 10.0 {
        format!("{amount:.1}")
    } else if amount >= 1.0 {
        format!("{amount:.2}")
    } else if amount >= 0.01 {
        format!("{amount:.3}")
    } else {
        format!("{amount:.4}")
    }
}

struct CurrencyPrefix {
    text: &'static str,
    compact: bool,
}

fn compact(text: &'static str) -> CurrencyPrefix {
    CurrencyPrefix {
        text,
        compact: true,
    }
}

fn spaced(text: &'static str) -> CurrencyPrefix {
    CurrencyPrefix {
        text,
        compact: false,
    }
}

fn currency_prefix(code: &str) -> Option<CurrencyPrefix> {
    match code {
        "AED" => Some(spaced("د.إ")),
        "AFN" => Some(compact("؋")),
        "ALL" => Some(spaced("L")),
        "AMD" => Some(compact("֏")),
        "ANG" | "AWG" => Some(compact("ƒ")),
        "ARS" => Some(compact("AR$")),
        "AUD" => Some(compact("A$")),
        "AZN" => Some(compact("₼")),
        "BAM" => Some(spaced("KM")),
        "BBD" | "BMD" | "BSD" | "BZD" | "KYD" | "XCD" => Some(compact("$")),
        "BDT" => Some(compact("৳")),
        "BGN" => Some(spaced("лв")),
        "BHD" => Some(spaced("BD")),
        "BND" => Some(compact("B$")),
        "BOB" => Some(spaced("Bs")),
        "BRL" => Some(compact("R$")),
        "CAD" => Some(compact("C$")),
        "CHF" => Some(spaced("CHF")),
        "CLP" => Some(compact("CLP$")),
        "CNH" | "CNY" => Some(compact("¥")),
        "COP" => Some(compact("COL$")),
        "CRC" => Some(compact("₡")),
        "CUP" => Some(compact("$")),
        "CZK" => Some(spaced("Kč")),
        "DKK" | "NOK" | "SEK" => Some(spaced("kr")),
        "DOP" => Some(compact("RD$")),
        "EGP" | "FKP" | "GIP" | "GGP" | "IMP" | "JEP" | "LBP" | "SHP" | "SYP" => Some(compact("£")),
        "EUR" => Some(compact("€")),
        "FJD" => Some(compact("FJ$")),
        "GBP" => Some(compact("£")),
        "GEL" => Some(compact("₾")),
        "GHS" => Some(compact("₵")),
        "GTQ" => Some(spaced("Q")),
        "GYD" => Some(compact("G$")),
        "HKD" => Some(compact("HK$")),
        "HNL" => Some(spaced("L")),
        "HRK" => Some(spaced("kn")),
        "HUF" => Some(spaced("Ft")),
        "IDR" => Some(spaced("Rp")),
        "ILS" => Some(compact("₪")),
        "INR" => Some(compact("₹")),
        "IRR" | "OMR" | "QAR" | "SAR" | "YER" => Some(compact("﷼")),
        "ISK" => Some(spaced("kr")),
        "JMD" => Some(compact("J$")),
        "JPY" => Some(compact("¥")),
        "KHR" => Some(compact("៛")),
        "KRW" => Some(compact("₩")),
        "KZT" => Some(compact("₸")),
        "LAK" => Some(compact("₭")),
        "LKR" => Some(spaced("Rs")),
        "LYD" => Some(spaced("LD")),
        "MAD" => Some(spaced("DH")),
        "MDL" | "RON" => Some(spaced("lei")),
        "MKD" | "RSD" => Some(spaced("дин")),
        "MNT" => Some(compact("₮")),
        "MOP" => Some(compact("MOP$")),
        "MUR" | "NPR" | "PKR" | "SCR" => Some(spaced("Rs")),
        "MXN" => Some(compact("MX$")),
        "MYR" => Some(spaced("RM")),
        "NGN" => Some(compact("₦")),
        "NZD" => Some(compact("NZ$")),
        "PEN" => Some(compact("S/")),
        "PHP" => Some(compact("₱")),
        "PLN" => Some(spaced("zł")),
        "PYG" => Some(compact("₲")),
        "RUB" => Some(compact("₽")),
        "SGD" => Some(compact("S$")),
        "THB" => Some(compact("฿")),
        "TRY" => Some(compact("₺")),
        "TTD" => Some(compact("TT$")),
        "TWD" => Some(compact("NT$")),
        "UAH" => Some(compact("₴")),
        "USD" => Some(compact("$")),
        "UYU" => Some(compact("$U")),
        "VES" => Some(spaced("Bs")),
        "VND" => Some(compact("₫")),
        "XAF" | "XOF" => Some(spaced("CFA")),
        "XPF" => Some(spaced("CFPF")),
        "ZAR" => Some(spaced("R")),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_usd_format_as_default() {
        let usd = CurrencyFormatter::usd();

        assert_eq!(usd.format_money(65.87), "$65.87");
        assert_eq!(usd.format_money(0.624), "$0.624");
        assert_eq!(usd.format_money_short(59.03), "$59.0");
    }

    #[test]
    fn converts_from_usd_base_rate() {
        let formatter = CurrencyFormatter {
            code: "GBP".into(),
            rate: 0.74092,
        };

        assert_eq!(formatter.format_money(10.0), "£7.41");
        assert_eq!(formatter.format_money_short(100.0), "£74.1");
    }

    #[test]
    fn uses_prefixed_symbols_for_ambiguous_dollar_codes() {
        let formatter = CurrencyFormatter {
            code: "CAD".into(),
            rate: 1.3656,
        };

        assert_eq!(formatter.format_money(10.0), "C$13.66");
    }

    #[test]
    fn falls_back_to_code_when_no_symbol_is_known() {
        let formatter = CurrencyFormatter {
            code: "XYZ".into(),
            rate: 2.0,
        };

        assert_eq!(formatter.format_money(10.0), "XYZ 20.00");
    }

    #[test]
    fn spaces_alphabetic_currency_prefixes() {
        let formatter = CurrencyFormatter {
            code: "CHF".into(),
            rate: 0.78969,
        };

        assert_eq!(formatter.format_money(10.0), "CHF 7.90");
    }

    #[test]
    fn embedded_rates_include_usd_and_common_codes() {
        let table = CurrencyTable::embedded().unwrap();

        assert_eq!(table.rate("USD"), Some(1.0));
        assert!(table.rate("GBP").unwrap() > 0.0);
        assert!(table.rate("EUR").unwrap() > 0.0);
    }
}
