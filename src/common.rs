use once_cell::sync::OnceCell;
use serde::Deserialize;
use std::fs::File;
use std::io::Read;
use crate::logger;

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
    pub slow: i32,
    pub normal: i32,
    pub turbo: i32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Prices {
    pub cheap: f64,
    pub expensive: f64,
    pub default: f64,
}

pub static CONFIG: OnceCell<Config> = OnceCell::new();

pub fn load_config() -> Result<Config, Box<dyn std::error::Error>> {
    let mut file = File::open("config.toml")?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    let config: Config = toml::from_str(&contents)?;
    logger::info(&format!("Config: {:?}", config));

    if !CONFIG.set(config).is_ok() {
        logger::info("Config already initialized");
    }

    Ok(CONFIG.get().ok_or_else(|| Box::<dyn std::error::Error>::from("Config not initialized"))?.clone())
}
