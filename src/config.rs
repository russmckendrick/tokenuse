use std::{collections::BTreeMap, fs, path::PathBuf};

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

const CONFIG_FILE_NAME: &str = "config.json";
const LOCAL_RATES_FILE_NAME: &str = "rates.json";
const LOCAL_PRICING_FILE_NAME: &str = "pricing-snapshot.json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigPaths {
    pub dir: PathBuf,
    pub config_file: PathBuf,
    pub currency_rates_file: PathBuf,
    pub pricing_snapshot_file: PathBuf,
}

impl ConfigPaths {
    pub fn new(dir: PathBuf) -> Self {
        Self {
            config_file: dir.join(CONFIG_FILE_NAME),
            currency_rates_file: dir.join(LOCAL_RATES_FILE_NAME),
            pricing_snapshot_file: dir.join(LOCAL_PRICING_FILE_NAME),
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
    pub overrides: BTreeMap<String, Value>,
}

impl Default for UserConfig {
    fn default() -> Self {
        Self {
            currency: DEFAULT_CURRENCY.into(),
            overrides: BTreeMap::new(),
        }
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

fn default_config_dir() -> PathBuf {
    let base = dirs::config_dir()
        .or_else(|| dirs::home_dir().map(|home| home.join(".config")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join(APP_ID)
}
