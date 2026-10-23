use crate::logger;
use serde::Deserialize;
use std::fs::File;
use std::io::Read;

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
}

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub check_interval: i32,
    pub prices: Prices,
    pub price_provider: PriceProviderConfig,
    pub bitaxes: Vec<Bitaxe>,
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

pub fn load_config() -> Result<Config, ConfigError> {
    let mut file = File::open("config.toml")?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    let config: Config = toml::from_str(&contents)?;
    logger::info(&format!("Config: {:?}", config));

    Ok(config)
}
