use chrono::DateTime;
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
    price_limit: f64,
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
}

lazy_static! {
    static ref CONFIG: Config = load_config().unwrap();
}

fn load_config() -> Result<Config, Box<dyn std::error::Error>> {
    let mut file = File::open("config.toml")?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    let config: Config = toml::from_str(&contents)?;
    println!("Config: {:?}", config);

    Ok(config)
}

async fn get_current_price(client: &Client) -> Result<f64, Box<dyn std::error::Error>> {
    let mut current_price: f64 = 0.0;
    let now = chrono::Local::now();
    let url = format!(
        "https://www.elprisetjustnu.se/api/v1/prices/{}_SE3.json",
        now.format("%Y/%m-%d")
    );
    println!("URL {}", url);

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

    println!("Current price {}", current_price);
    Ok(current_price)
}

async fn should_switch_frequency(
    client: &Client,
    is_running_normal: bool,
) -> Result<bool, Box<dyn std::error::Error>> {
    let current_price: f64 = get_current_price(&client).await?;
    let switch_frequency: bool = if current_price > CONFIG.price_limit && is_running_normal {
        true
    } else {
        false
    };

    Ok(switch_frequency)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let hour_in_milliseconds = 1000 * 60 * 60;

    let mut is_running_normal: bool = true;
    let client = Client::new();

    loop {
        println!("CONFIG {:?}", &CONFIG.bitaxes);
        println!("Bitaxes running normal {}", is_running_normal);

        let switch_frequency: bool = should_switch_frequency(&client, is_running_normal).await?;

        if switch_frequency {
            for bitaxe in &CONFIG.bitaxes {
                println!("Uppdating {}", bitaxe.host);
                let frequency_to_use = if is_running_normal {
                    println!("Switching frequency to slow");
                    bitaxe.slow
                } else {
                    println!("Switching frequency to fast");
                    bitaxe.normal
                };

                let mut body = HashMap::new();
                body.insert("frequency", frequency_to_use);

                let response = client
                    .patch(format!("http://{}/api/system", bitaxe.host))
                    .json(&body)
                    .send()
                    .await?;

                if response.status() == 200 {
                    println!("Restarting");
                    client
                        .post(format!("http://{}/api/system/restart", bitaxe.host))
                        .send()
                        .await?;
                } else {
                    println!("Something went wrong when updating {}", bitaxe.host);
                }
            }

            is_running_normal = if is_running_normal { false } else { true };
        }

        println!("Sleeping for 1 hour");
        let ten_millis = time::Duration::from_millis(hour_in_milliseconds);
        thread::sleep(ten_millis);
    }
}
