use chrono::{DateTime, Local};
use lazy_static::lazy_static;
use serde::Deserialize;
use std::fs::File;
use std::io::Read;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub check_interval: i32,
    pub prices: Prices,
    pub price_provider: PriceProviderConfig,
    pub bitaxes: Vec<Bitaxe>,
}

#[derive(Debug, Deserialize)]
pub struct PriceProviderConfig {
    pub provider_type: String,
    #[serde(default)]
    pub elpriset_just_nu: Option<crate::price_providers::ElPrisetJustNuConfig>,
}

#[derive(Debug, Deserialize)]
pub struct Bitaxe {
    pub host: String,
    pub slow: i32,
    pub normal: i32,
    pub turbo: i32,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct ElPrisetJustNu {
    pub price_zone: String,
}

#[derive(Debug, Deserialize)]
pub struct Prices {
    pub cheap: f64,
    pub expensive: f64,
    pub default: f64,
}

lazy_static! {
    pub static ref CONFIG: Config = load_config().unwrap();
}

pub fn load_config() -> Result<Config, Box<dyn std::error::Error>> {
    let mut file = File::open("config.toml")?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    let config: Config = toml::from_str(&contents)?;
    log(format!("Config: {:?}", config));

    Ok(config)
}

pub fn log(message: String) {
    let current_local: DateTime<Local> = Local::now();
    let custom_format = current_local.format("%Y-%m-%d %H:%M:%S");
    println!("{} - {}", custom_format, message);
}
