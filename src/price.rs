use chrono::DateTime;
use reqwest::Client;
use serde_json::Value;

use crate::common;

pub async fn get_current_price(client: &Client) -> Result<f64, Box<dyn std::error::Error>> {
    let mut current_price: f64 = 0.0;
    let now = chrono::Local::now();
    let url = format!(
        "https://www.elprisetjustnu.se/api/v1/prices/{}_{}.json",
        now.format("%Y/%m-%d"),
        common::CONFIG.elpriset_just_nu.price_zone
    );
    common::log(format!("Getting electricity price from URL {}", url));

    let response = client.get(url).send().await?;
    if response.status() == reqwest::StatusCode::OK {
        let json: Value = response.json().await?;

        for item in json.as_array().unwrap() {
            let time_start = DateTime::parse_from_rfc3339(item["time_start"].as_str().unwrap()).unwrap();
            let time_end = DateTime::parse_from_rfc3339(item["time_end"].as_str().unwrap()).unwrap();

            if (time_start < now) && (time_end > now) {
                current_price = item["SEK_per_kWh"].as_f64().unwrap();
                break;
            }
        }

        common::log(format!("Current electricity price {:?}", current_price));
    } else {
        current_price = common::CONFIG.prices.default;
        common::log(format!("Error getting price using default {:?}", current_price));
    }

    Ok(current_price)
}
