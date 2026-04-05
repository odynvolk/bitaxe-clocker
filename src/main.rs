mod bitaxe;
mod common;
mod price_providers;
use common::{log, CONFIG};
use price_providers::PriceProviderFactory;
use reqwest::Client;
use std::{
    sync::atomic::{AtomicBool, Ordering},
    sync::Arc,
    thread, time,
};
use tokio::signal;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    common::log(format!("Bitaxe Clocker Config {:?}", &common::CONFIG.bitaxes));

    // Atomic flag to track shutdown request, wrapped in Arc for sharing
    let shutdown_requested = Arc::new(AtomicBool::new(false));

    // Set up Ctrl-C signal handler
    let shutdown_clone = Arc::clone(&shutdown_requested);
    tokio::spawn(async move {
        if let Err(_err) = signal::ctrl_c().await {
            log("Failed to set up Ctrl-C handler".to_string());
        }
        shutdown_clone.store(true, Ordering::SeqCst);
        log("Shutdown signal received".to_string());
    });

    let check_interval = 1000 * 60 * &CONFIG.check_interval;
    let client = Client::builder().timeout(time::Duration::from_secs(10)).build()?;

    // Create the price provider based on configuration
    let price_provider =
        PriceProviderFactory::create_provider(&CONFIG.price_provider.provider_type, &CONFIG.price_provider);

    loop {
        // Check if shutdown was requested
        if shutdown_requested.load(Ordering::SeqCst) {
            log("Shutting down...".to_string());
            break;
        }

        for bitaxe in &CONFIG.bitaxes {
            log(format!("Checking {}", bitaxe.host));
            let current_price: f64 = price_provider.get_current_price(&client).await?;
            let switch_frequency_to: i32 = bitaxe::should_switch_frequency_to(&client, bitaxe, current_price).await?;
            if switch_frequency_to != -1 {
                bitaxe::switch_frequency(&client, bitaxe, switch_frequency_to).await?;
            }
        }

        log(format!("Sleeping for {:?} minutes", &CONFIG.check_interval).to_owned());
        let ten_millis = time::Duration::from_millis(check_interval.try_into().unwrap());
        thread::sleep(ten_millis);
    }

    log("Goodbye!".to_string());
    Ok(())
}
