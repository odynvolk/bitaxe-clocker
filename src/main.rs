use chrono::{DateTime, Local};
use lazy_static::lazy_static;
use reqwest::{self, Client};
use serde::Deserialize;
use serde_json::Value;
use toml;

use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::{thread, time};

#[derive(Debug, Deserialize)]
struct Config {
    #[allow(dead_code)]
    check_interval: i32,
    #[allow(dead_code)]
    prices: Prices,
    #[allow(dead_code)]
    elpriset_just_nu: ElPrisetJustNu,
    #[allow(dead_code)]
    bitaxes: Vec<Bitaxe>,
}

#[derive(Debug, Deserialize)]
struct Bitaxe {
    #[allow(dead_code)]
    host: String,
    #[allow(dead_code)]
    slow: i32,
    #[allow(dead_code)]
    normal: i32,
    #[allow(dead_code)]
    turbo: i32,
}

#[derive(Debug, Deserialize)]
struct ElPrisetJustNu {
    #[allow(dead_code)]
    price_zone: String,
}

#[derive(Debug, Deserialize)]
struct Prices {
    #[allow(dead_code)]
    cheap: f64,
    #[allow(dead_code)]
    expensive: f64,
}

lazy_static! {
    static ref CONFIG: Config = load_config().unwrap();
}

fn load_config() -> Result<Config, Box<dyn std::error::Error>> {
    let mut file = File::open("config.toml")?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    let config: Config = toml::from_str(&contents)?;
    log(format!("Config: {:?}", config));

    Ok(config)
}

async fn get_current_price(client: &Client) -> Result<f64, Box<dyn std::error::Error>> {
    let mut current_price: f64 = 0.0;
    let now = chrono::Local::now();
    let url = format!(
        "https://www.elprisetjustnu.se/api/v1/prices/{}_{}.json",
        now.format("%Y/%m-%d"), CONFIG.elpriset_just_nu.price_zone
    );
    log(format!("Getting electricity price from URL {}", url));

    let response = client.get(url).send().await?;
    let json: Value = response.json().await?;

    for item in json.as_array().unwrap() {
        let time_start =
            DateTime::parse_from_rfc3339(item["time_start"].as_str().unwrap()).unwrap();
        let time_end = DateTime::parse_from_rfc3339(item["time_end"].as_str().unwrap()).unwrap();

        if (time_start < now) && (time_end > now) {
            current_price = item["SEK_per_kWh"].as_f64().unwrap();
            break;
        }
    }

    log(format!("Current electricity price {}", current_price));

    Ok(current_price)
}

async fn should_switch_frequency_to(
    client: &Client,
    bitaxe: &Bitaxe,
    current_price: f64,
) -> Result<i32, Box<dyn std::error::Error>> {
    let running_mode: i32 = get_running_mode(&client, &bitaxe).await?;

    let switch_frequency_to: i32 = if current_price > CONFIG.prices.expensive && running_mode > bitaxe.slow {
        bitaxe.slow
    } else if current_price < CONFIG.prices.expensive && current_price > CONFIG.prices.cheap && running_mode != bitaxe.normal {
        bitaxe.normal
    } else if current_price < CONFIG.prices.cheap && running_mode != bitaxe.turbo {
        bitaxe.turbo
    } else {
        -1
    };

    Ok(switch_frequency_to)
}

async fn get_running_mode(
    client: &Client,
    bitaxe: &Bitaxe,
) -> Result<i32, Box<dyn std::error::Error>> {
    let url = format!("http://{}/api/system/info", bitaxe.host);
    log(format!("Getting Bitaxe info from URL {}", url));

    let response = client.get(url).send().await?;
    let json: Value = response.json().await?;

    let running_mode: i32 = json["frequency"].as_f64().unwrap() as i32;
    log(format!("Running mode {}", running_mode));

    Ok(running_mode)
}

fn log(message: String) {
  let current_local: DateTime<Local> = Local::now();
  let custom_format = current_local.format("%Y-%m-%d %H:%M:%S");
  println!("{} - {}", custom_format, message);
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    log(format!("Bitaxe Clocker Config {:?}", &CONFIG.bitaxes));

    let check_interval = 1000 * 60 * &CONFIG.check_interval;
    let client = Client::new();

    loop {
        let current_price: f64 = get_current_price(&client).await?;
        for bitaxe in &CONFIG.bitaxes {
            log(format!("Checking {}", bitaxe.host));
            let switch_frequency_to: i32 =
                should_switch_frequency_to(&client, bitaxe, current_price).await?;
            if switch_frequency_to != -1 {
                log(format!("Switching frequency to {}", switch_frequency_to));
                let mut body = HashMap::new();
                body.insert("frequency", switch_frequency_to);
                let response = client
                    .patch(format!("http://{}/api/system", bitaxe.host))
                    .json(&body)
                    .send()
                    .await?;

                if response.status() == 200 {
                    log("Restarting!".to_owned());
                    client
                        .post(format!("http://{}/api/system/restart", bitaxe.host))
                        .send()
                        .await?;
                } else {
                    log(format!("Something went wrong when updating {}", bitaxe.host));
                }
            }
        }

        log(format!("Sleeping for {} minutes", &CONFIG.check_interval).to_owned());
        let ten_millis = time::Duration::from_millis(check_interval.try_into().unwrap());
        thread::sleep(ten_millis);
    }
}
