mod bitaxe;
mod common;
mod price;
use common::{log, CONFIG};
use reqwest::Client;
use std::{thread, time};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    common::log(format!("Bitaxe Clocker Config {:?}", &common::CONFIG.bitaxes));

    let check_interval = 1000 * 60 * &CONFIG.check_interval;
    let client = Client::builder().timeout(time::Duration::from_secs(10)).build()?;

    loop {
        for bitaxe in &CONFIG.bitaxes {
            log(format!("Checking {}", bitaxe.host));
            let current_price: f64 = price::get_current_price(&client).await?;
            let switch_frequency_to: i32 = bitaxe::should_switch_frequency_to(&client, bitaxe, current_price).await?;
            if switch_frequency_to != -1 {
                bitaxe::switch_frequency(&client, bitaxe, switch_frequency_to).await?;
            }
        }

        log(format!("Sleeping for {:?} minutes", &CONFIG.check_interval).to_owned());
        let ten_millis = time::Duration::from_millis(check_interval.try_into().unwrap());
        thread::sleep(ten_millis);
    }
}
