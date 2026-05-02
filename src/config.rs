use std::{collections::BTreeMap, fs, path::PathBuf, time::Duration};

use color_eyre::{eyre::Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const APP_ID: &str = "tokenuse";
pub const DEFAULT_CURRENCY: &str = "USD";

pub const CURRENCY_RATES_URL: &str =
    "https://raw.githubusercontent.com/russmckendrick/tokenuse/refs/heads/main/currency/rates.json";
pub const FRANKFURTER_RATES_URL: &str = "https://api.frankfurter.dev/v2/rates?base=USD";
pub const LITELLM_PRICING_URL: &str =
    "https://raw.githubusercontent.com/BerriAI/litellm/main/model_prices_and_context_window.json";

pub const DEFAULT_BACKGROUND_ALERT_MIN_COST_USD: f64 = 1.0;
pub const DEFAULT_BACKGROUND_ALERT_MIN_TOKENS: i64 = 100_000;
pub const DEFAULT_BACKGROUND_ALERT_MIN_CALLS: i64 = 25;
pub const DEFAULT_BACKGROUND_ALERT_COOLDOWN_MINUTES: i64 = 30;

const CONFIG_FILE_NAME: &str = "config.json";
const LOCAL_RATES_FILE_NAME: &str = "rates.json";
const LOCAL_PRICING_FILE_NAME: &str = "pricing-snapshot.json";
const ARCHIVE_DB_FILE_NAME: &str = "archive.db";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigPaths {
    pub dir: PathBuf,
    pub config_file: PathBuf,
    pub currency_rates_file: PathBuf,
    pub pricing_snapshot_file: PathBuf,
    pub archive_db_file: PathBuf,
}

impl ConfigPaths {
    pub fn new(dir: PathBuf) -> Self {
        Self {
            config_file: dir.join(CONFIG_FILE_NAME),
            currency_rates_file: dir.join(LOCAL_RATES_FILE_NAME),
            pricing_snapshot_file: dir.join(LOCAL_PRICING_FILE_NAME),
            archive_db_file: dir.join(ARCHIVE_DB_FILE_NAME),
            dir,
        }
    }

    pub fn ensure_dir(&self) -> Result<()> {
        fs::create_dir_all(&self.dir).wrap_err_with(|| format!("create {}", self.dir.display()))
    }
}

impl Default for ConfigPaths {
    fn default() -> Self {
        Self::new(default_config_dir())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UserConfig {
    #[serde(default = "default_currency")]
    pub currency: String,
    #[serde(default)]
    pub background_alerts: BackgroundAlertsConfig,
    #[serde(default)]
    pub overrides: BTreeMap<String, Value>,
}

impl Default for UserConfig {
    fn default() -> Self {
        Self {
            currency: DEFAULT_CURRENCY.into(),
            background_alerts: BackgroundAlertsConfig::default(),
            overrides: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BackgroundAlertsConfig {
    #[serde(default = "default_background_alerts_enabled")]
    pub enabled: bool,
    #[serde(default = "default_background_alert_min_cost_usd")]
    pub min_cost_usd: f64,
    #[serde(default = "default_background_alert_min_tokens")]
    pub min_tokens: i64,
    #[serde(default = "default_background_alert_min_calls")]
    pub min_calls: i64,
    #[serde(default = "default_background_alert_cooldown_minutes")]
    pub cooldown_minutes: i64,
}

impl Default for BackgroundAlertsConfig {
    fn default() -> Self {
        Self {
            enabled: default_background_alerts_enabled(),
            min_cost_usd: default_background_alert_min_cost_usd(),
            min_tokens: default_background_alert_min_tokens(),
            min_calls: default_background_alert_min_calls(),
            cooldown_minutes: default_background_alert_cooldown_minutes(),
        }
    }
}

impl BackgroundAlertsConfig {
    pub fn normalize(&mut self) {
        if !self.min_cost_usd.is_finite() || self.min_cost_usd <= 0.0 {
            self.min_cost_usd = default_background_alert_min_cost_usd();
        }
        if self.min_tokens <= 0 {
            self.min_tokens = default_background_alert_min_tokens();
        }
        if self.min_calls <= 0 {
            self.min_calls = default_background_alert_min_calls();
        }
        if self.cooldown_minutes <= 0 {
            self.cooldown_minutes = default_background_alert_cooldown_minutes();
        }
    }

    pub fn min_tokens(&self) -> u64 {
        self.min_tokens.max(1) as u64
    }

    pub fn min_calls(&self) -> u64 {
        self.min_calls.max(1) as u64
    }

    pub fn cooldown(&self) -> Duration {
        Duration::from_secs((self.cooldown_minutes.max(1) as u64).saturating_mul(60))
    }
}

impl UserConfig {
    pub fn load(paths: &ConfigPaths) -> Result<Self> {
        if !paths.config_file.exists() {
            return Ok(Self::default());
        }

        let raw = fs::read_to_string(&paths.config_file)
            .wrap_err_with(|| format!("read {}", paths.config_file.display()))?;
        let mut config: Self = serde_json::from_str(&raw)
            .wrap_err_with(|| format!("parse {}", paths.config_file.display()))?;
        config.normalize();
        Ok(config)
    }

    pub fn load_or_create(paths: &ConfigPaths) -> Result<Self> {
        paths.ensure_dir()?;
        let config = Self::load(paths)?;
        if !paths.config_file.exists() {
            config.save(paths)?;
        }
        Ok(config)
    }

    pub fn save(&self, paths: &ConfigPaths) -> Result<()> {
        paths.ensure_dir()?;
        let mut config = self.clone();
        config.normalize();
        let mut pretty = serde_json::to_string_pretty(&config)?;
        pretty.push('\n');
        fs::write(&paths.config_file, pretty)
            .wrap_err_with(|| format!("write {}", paths.config_file.display()))?;
        Ok(())
    }

    pub fn set_currency(&mut self, code: &str) {
        self.currency = normalize_currency_code(code).unwrap_or_else(|| DEFAULT_CURRENCY.into());
    }

    fn normalize(&mut self) {
        self.currency =
            normalize_currency_code(&self.currency).unwrap_or_else(|| DEFAULT_CURRENCY.into());
        self.background_alerts.normalize();
    }
}

pub fn normalize_currency_code(code: &str) -> Option<String> {
    let normalized = code.trim().to_ascii_uppercase();
    if normalized.len() == 3 && normalized.chars().all(|c| c.is_ascii_uppercase()) {
        Some(normalized)
    } else {
        None
    }
}

fn default_currency() -> String {
    DEFAULT_CURRENCY.into()
}

fn default_background_alerts_enabled() -> bool {
    true
}

fn default_background_alert_min_cost_usd() -> f64 {
    DEFAULT_BACKGROUND_ALERT_MIN_COST_USD
}

fn default_background_alert_min_tokens() -> i64 {
    DEFAULT_BACKGROUND_ALERT_MIN_TOKENS
}

fn default_background_alert_min_calls() -> i64 {
    DEFAULT_BACKGROUND_ALERT_MIN_CALLS
}

fn default_background_alert_cooldown_minutes() -> i64 {
    DEFAULT_BACKGROUND_ALERT_COOLDOWN_MINUTES
}

fn default_config_dir() -> PathBuf {
    let base = dirs::config_dir()
        .or_else(|| dirs::home_dir().map(|home| home.join(".config")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join(APP_ID)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_paths(name: &str) -> ConfigPaths {
        let dir = std::env::temp_dir().join(format!(
            "tokenuse-config-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            name
        ));
        ConfigPaths::new(dir)
    }

    #[test]
    fn default_config_includes_background_alert_thresholds() {
        let config = UserConfig::default();

        assert!(config.background_alerts.enabled);
        assert_eq!(
            config.background_alerts.min_cost_usd,
            DEFAULT_BACKGROUND_ALERT_MIN_COST_USD
        );
        assert_eq!(
            config.background_alerts.min_tokens,
            DEFAULT_BACKGROUND_ALERT_MIN_TOKENS
        );
        assert_eq!(
            config.background_alerts.min_calls,
            DEFAULT_BACKGROUND_ALERT_MIN_CALLS
        );
        assert_eq!(
            config.background_alerts.cooldown_minutes,
            DEFAULT_BACKGROUND_ALERT_COOLDOWN_MINUTES
        );
    }

    #[test]
    fn custom_background_alert_config_loads_from_file() {
        let paths = temp_paths("custom-alerts");
        paths.ensure_dir().unwrap();
        std::fs::write(
            &paths.config_file,
            r#"{
  "currency": "GBP",
  "background_alerts": {
    "enabled": false,
    "min_cost_usd": 2.5,
    "min_tokens": 250000,
    "min_calls": 50,
    "cooldown_minutes": 45
  }
}
"#,
        )
        .unwrap();

        let config = UserConfig::load(&paths).unwrap();

        assert_eq!(config.currency, "GBP");
        assert!(!config.background_alerts.enabled);
        assert_eq!(config.background_alerts.min_cost_usd, 2.5);
        assert_eq!(config.background_alerts.min_tokens, 250_000);
        assert_eq!(config.background_alerts.min_calls, 50);
        assert_eq!(config.background_alerts.cooldown_minutes, 45);

        let _ = std::fs::remove_dir_all(paths.dir);
    }

    #[test]
    fn invalid_background_alert_values_fall_back_to_defaults() {
        let paths = temp_paths("invalid-alerts");
        paths.ensure_dir().unwrap();
        std::fs::write(
            &paths.config_file,
            r#"{
  "currency": "usd",
  "background_alerts": {
    "enabled": true,
    "min_cost_usd": -1.0,
    "min_tokens": -20,
    "min_calls": 0,
    "cooldown_minutes": -5
  }
}
"#,
        )
        .unwrap();

        let config = UserConfig::load(&paths).unwrap();

        assert_eq!(config.currency, "USD");
        assert_eq!(
            config.background_alerts.min_cost_usd,
            DEFAULT_BACKGROUND_ALERT_MIN_COST_USD
        );
        assert_eq!(
            config.background_alerts.min_tokens,
            DEFAULT_BACKGROUND_ALERT_MIN_TOKENS
        );
        assert_eq!(
            config.background_alerts.min_calls,
            DEFAULT_BACKGROUND_ALERT_MIN_CALLS
        );
        assert_eq!(
            config.background_alerts.cooldown_minutes,
            DEFAULT_BACKGROUND_ALERT_COOLDOWN_MINUTES
        );

        let _ = std::fs::remove_dir_all(paths.dir);
    }
}
