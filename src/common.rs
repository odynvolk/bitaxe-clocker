use chrono::{DateTime, Local};
use lazy_static::lazy_static;
use serde::Deserialize;
use std::fs::File;
use std::io::Read;

#[derive(Debug, Deserialize)]
pub struct Config {
    #[allow(dead_code)]
    pub check_interval: i32,
    #[allow(dead_code)]
    pub prices: Prices,
    #[allow(dead_code)]
    pub elpriset_just_nu: ElPrisetJustNu,
    #[allow(dead_code)]
    pub bitaxes: Vec<Bitaxe>,
}

#[derive(Debug, Deserialize)]
pub struct Bitaxe {
    #[allow(dead_code)]
    pub host: String,
    #[allow(dead_code)]
    pub slow: i32,
    #[allow(dead_code)]
    pub normal: i32,
    #[allow(dead_code)]
    pub turbo: i32,
}

#[derive(Debug, Deserialize)]
pub struct ElPrisetJustNu {
    #[allow(dead_code)]
    pub price_zone: String,
}

#[derive(Debug, Deserialize)]
pub struct Prices {
    #[allow(dead_code)]
    pub cheap: f64,
    #[allow(dead_code)]
    pub expensive: f64,
    #[allow(dead_code)]
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
