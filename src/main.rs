use chrono::DateTime;
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

fn read_config() -> Result<Config, Box<dyn std::error::Error>> {
    let mut file = File::open("config.toml")?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    let config: Config = toml::from_str(&contents)?;

    Ok(config)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let hour_in_milliseconds = 1000 * 60 * 60;

    let config = read_config()?;
    println!("Config: {:?}", config);

    let mut is_running_normal: bool = true;
    let mut switch_frequency: bool = false;

    while true {
        println!("Is running normal {}", is_running_normal);

        let now = chrono::Local::now();
        let url = format!(
            "https://www.elprisetjustnu.se/api/v1/prices/{}_SE3.json",
            now.format("%Y/%m-%d")
        );
        println!("URL {}", url);

        let client = Client::new();
        let response = client.get(url).send().await?;
        let json: Value = response.json().await?;

        for item in json.as_array().unwrap() {
            let time_start =
                DateTime::parse_from_rfc3339(item["time_start"].as_str().unwrap()).unwrap();
            let time_end =
                DateTime::parse_from_rfc3339(item["time_end"].as_str().unwrap()).unwrap();

            if (time_start < now) && (time_end > now) {
                if item["SEK_per_kWh"].as_f64() > Some(config.price_limit) && is_running_normal {
                    switch_frequency = true;
                    break;
                }
            }
        }

        if switch_frequency {
            for bitaxe in &config.bitaxes {
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

                if (response.status() == 200) {
                    println!("Restarting");
                    client
                        .post(format!("http://{}/api/system/restart", bitaxe.host))
                        .send()
                        .await?;
                } else {
                    println!("Something went wrong when updating {}", bitaxe.host);
                }

                switch_frequency = false;
                is_running_normal = if is_running_normal { false } else { true };
            }
        }

        println!("Sleeping for 1 hour");
        let ten_millis = time::Duration::from_millis(hour_in_milliseconds);
        thread::sleep(ten_millis);
    }

    Ok(())
}
