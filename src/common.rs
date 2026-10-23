use serde::Deserialize;
use std::fs::File;
use std::io::Read;
use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("failed to read config.toml: {0}")]
    Read(#[from] std::io::Error),

    #[error("invalid config.toml: {0}")]
    Parse(#[from] toml::de::Error),

    #[error("unknown price provider '{0}'; expected one of: elpriset_just_nu")]
    UnknownProvider(String),

    #[error("price provider '{0}' requires a [price_provider.{0}] section")]
    MissingProviderSection(String),

    #[error("check_interval of {0} minutes is too large to schedule")]
    IntervalTooLarge(u64),
}

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    /// Minutes between price checks and frequency updates.
    pub check_interval: u64,
    pub prices: Prices,
    pub price_provider: PriceProviderConfig,
    pub bitaxes: Vec<Bitaxe>,
}

impl Config {
    /// How long to wait between checks. Total: the arithmetic that used to live in
    /// `main` could overflow, and in a release build overflow wraps rather than
    /// panics, so a bad value would have produced a silently wrong sleep.
    pub fn sleep_duration(&self) -> Result<Duration, ConfigError> {
        let seconds = self
            .check_interval
            .checked_mul(60)
            .ok_or(ConfigError::IntervalTooLarge(self.check_interval))?;

        Ok(Duration::from_secs(seconds))
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct PriceProviderConfig {
    pub provider_type: String,
    #[serde(default)]
    pub elpriset_just_nu: Option<crate::price_providers::ElPrisetJustNuConfig>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Bitaxe {
    pub host: String,
    pub expensive: i32,
    pub default: i32,
    pub cheap: i32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Prices {
    pub cheap: f64,
    pub expensive: f64,
    pub default: f64,
}

/// Parses configuration text. Kept separate from `load_config` so the rules can be
/// tested without a file on disk at a fixed path.
pub fn parse_config(contents: &str) -> Result<Config, ConfigError> {
    let config: Config = toml::from_str(contents)?;
    Ok(config)
}

pub fn load_config() -> Result<Config, ConfigError> {
    let mut file = File::open("config.toml")?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    let config = parse_config(&contents)?;
    tracing::info!(config = ?config, "Loaded config");

    Ok(config)
}
