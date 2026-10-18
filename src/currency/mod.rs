use std::{collections::BTreeMap, fs, path::PathBuf};

use color_eyre::{
    eyre::{eyre, Context},
    Result,
};
use serde::Deserialize;

use crate::config::{self, ConfigPaths};

#[cfg(feature = "refresh-currency")]
pub mod refresh;

const EMBEDDED_RATES: &str = include_str!("../../costs/exchange-rates.json");

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
        if paths.legacy_currency_rates_file.exists() {
            let raw = fs::read_to_string(&paths.legacy_currency_rates_file)
                .wrap_err_with(|| format!("read {}", paths.legacy_currency_rates_file.display()))?;
            return Self::from_json_str(
                &raw,
                RateSource::Local(paths.legacy_currency_rates_file.clone()),
            )
            .wrap_err_with(|| format!("parse {}", paths.legacy_currency_rates_file.display()));
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

    /// Display money: two decimals with thousands separators, `£0` for an
    /// exact zero, and `<£0.01` for non-zero amounts below the smallest unit.
    pub fn format_money(&self, amount_usd: f64) -> String {
        self.format_amount(amount_usd * self.rate, false)
    }

    /// Compact money for narrow mixes and labels: whole units from 100, one
    /// decimal from 10, otherwise the same rules as [`Self::format_money`].
    pub fn format_money_short(&self, amount_usd: f64) -> String {
        self.format_amount(amount_usd * self.rate, true)
    }

    /// Full-precision money for tooltips behind a rounded or `<£0.01` value.
    pub fn format_money_exact(&self, amount_usd: f64) -> String {
        let amount = amount_usd * self.rate;
        if amount == 0.0 || !amount.is_finite() {
            return self.with_prefix("0");
        }
        let decimals = if amount.abs() >= 0.01 { 4 } else { 6 };
        let sign = if amount < 0.0 { "-" } else { "" };
        format!(
            "{sign}{}",
            self.with_prefix(&group_thousands(amount.abs(), decimals))
        )
    }

    fn format_amount(&self, amount: f64, short: bool) -> String {
        if amount == 0.0 || !amount.is_finite() {
            return self.with_prefix("0");
        }

        let minor_decimals = if zero_decimal_currency(&self.code) {
            0
        } else {
            2
        };
        let smallest = 10f64.powi(-(minor_decimals as i32));
        if amount.abs() < smallest {
            let floor = format!("{smallest:.minor_decimals$}");
            return format!("<{}", self.with_prefix(&floor));
        }

        let decimals = match amount.abs() {
            _ if minor_decimals == 0 => 0,
            value if short && value >= 100.0 => 0,
            value if short && value >= 10.0 => 1,
            _ => minor_decimals,
        };
        let sign = if amount < 0.0 { "-" } else { "" };
        format!(
            "{sign}{}",
            self.with_prefix(&group_thousands(amount.abs(), decimals))
        )
    }

    fn with_prefix(&self, amount: &str) -> String {
        if self.is_usd() {
            return format!("${amount}");
        }
        match currency_prefix(&self.code) {
            Some(prefix) if prefix.compact => format!("{}{amount}", prefix.text),
            Some(prefix) => format!("{} {amount}", prefix.text),
            None => format!("{} {}", self.code, amount),
        }
    }
}

/// ISO 4217 currencies with no minor unit; showing pence-style decimals for
/// these would invent precision the currency does not have.
fn zero_decimal_currency(code: &str) -> bool {
    matches!(
        code,
        "BIF"
            | "CLP"
            | "DJF"
            | "GNF"
            | "ISK"
            | "JPY"
            | "KMF"
            | "KRW"
            | "PYG"
            | "RWF"
            | "UGX"
            | "VND"
            | "VUV"
            | "XAF"
            | "XOF"
            | "XPF"
    )
}

/// Format a non-negative amount with `decimals` places and `,` thousands
/// separators.
fn group_thousands(amount: f64, decimals: usize) -> String {
    let fixed = format!("{amount:.decimals$}");
    let (int_part, frac_part) = match fixed.split_once('.') {
        Some((int_part, frac_part)) => (int_part, Some(frac_part)),
        None => (fixed.as_str(), None),
    };
    let mut grouped = String::with_capacity(int_part.len() + int_part.len() / 3);
    for (idx, digit) in int_part.chars().enumerate() {
        if idx > 0 && (int_part.len() - idx) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(digit);
    }
    match frac_part {
        Some(frac_part) => format!("{grouped}.{frac_part}"),
        None => grouped,
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
    use crate::config::ConfigPaths;

    fn temp_paths(name: &str) -> ConfigPaths {
        let dir = std::env::temp_dir().join(format!(
            "tokenuse-currency-{}-{}",
            crate::tools::paths::test_run_id(),
            name
        ));
        std::fs::create_dir_all(&dir).unwrap();
        ConfigPaths::new(dir)
    }

    #[test]
    fn keeps_usd_format_as_default() {
        let usd = CurrencyFormatter::usd();

        assert_eq!(usd.format_money(65.87), "$65.87");
        assert_eq!(usd.format_money(0.624), "$0.62");
        assert_eq!(usd.format_money_short(59.03), "$59.0");
    }

    #[test]
    fn groups_thousands_and_keeps_two_decimals() {
        let gbp = CurrencyFormatter {
            code: "GBP".into(),
            rate: 1.0,
        };

        assert_eq!(gbp.format_money(928.891), "£928.89");
        assert_eq!(gbp.format_money(1088.0), "£1,088.00");
        assert_eq!(gbp.format_money(2859.456), "£2,859.46");
        assert_eq!(gbp.format_money(1_234_567.8), "£1,234,567.80");
        assert_eq!(gbp.format_money(-1088.5), "-£1,088.50");
        assert_eq!(gbp.format_money_short(2859.456), "£2,859");
        assert_eq!(gbp.format_money_short(12.34), "£12.3");
        assert_eq!(
            CurrencyFormatter::usd().format_money(12_345.6),
            "$12,345.60"
        );
    }

    #[test]
    fn shows_zero_and_sub_penny_amounts_plainly() {
        let gbp = CurrencyFormatter {
            code: "GBP".into(),
            rate: 1.0,
        };

        assert_eq!(gbp.format_money(0.0), "£0");
        assert_eq!(gbp.format_money_short(0.0), "£0");
        assert_eq!(gbp.format_money(0.007), "<£0.01");
        assert_eq!(gbp.format_money_short(0.0001), "<£0.01");
        assert_eq!(gbp.format_money(0.01), "£0.01");
        assert_eq!(gbp.format_money_exact(0.007), "£0.007000");
        assert_eq!(gbp.format_money_exact(1088.12345), "£1,088.1235");
        assert_eq!(CurrencyFormatter::usd().format_money(0.0), "$0");
        assert_eq!(CurrencyFormatter::usd().format_money(0.004), "<$0.01");

        let chf = CurrencyFormatter {
            code: "CHF".into(),
            rate: 1.0,
        };
        assert_eq!(chf.format_money(0.004), "<CHF 0.01");
    }

    #[test]
    fn zero_decimal_currencies_use_whole_units() {
        let jpy = CurrencyFormatter {
            code: "JPY".into(),
            rate: 150.0,
        };

        assert_eq!(jpy.format_money(10.0), "¥1,500");
        assert_eq!(jpy.format_money(0.001), "<¥1");
        assert_eq!(jpy.format_money(0.0), "¥0");
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

    #[test]
    fn load_uses_legacy_local_rates_when_exchange_rates_are_missing() {
        let paths = temp_paths("legacy-rates");
        std::fs::write(&paths.legacy_currency_rates_file, EMBEDDED_RATES).unwrap();

        let table = CurrencyTable::load(&paths).unwrap();

        assert_eq!(
            table.source(),
            &RateSource::Local(paths.legacy_currency_rates_file.clone())
        );

        let _ = std::fs::remove_dir_all(paths.dir);
    }
}
