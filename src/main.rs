mod bitaxe;
mod common;
mod price_providers;
use common::{log, CONFIG};
use price_providers::PriceProviderFactory;
use reqwest::Client;
use std::{
    sync::atomic::{AtomicBool, Ordering},
    sync::Arc,
    time as std_time,
};
use tokio::{signal, time as tokio_time};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _config = common::load_config().map_err(|e| format!("Failed to load config: {}", e))?;

    common::log(format!(
        "Bitaxe Clocker Config {:?}",
        &common::CONFIG.get().unwrap().bitaxes
    ));

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

    let check_interval = 1000 * 60 * &CONFIG.get().unwrap().check_interval;
    let client = Client::builder().timeout(std_time::Duration::from_secs(10)).build()?;
    let price_provider = PriceProviderFactory::create_provider(
        &CONFIG.get().unwrap().price_provider.provider_type,
        &CONFIG.get().unwrap().price_provider,
    );

    loop {
        if shutdown_requested.load(Ordering::SeqCst) {
            log("Shutting down...".to_string());
            break;
        }

        for bitaxe in &CONFIG.get().unwrap().bitaxes {
            log(format!("Checking {}", bitaxe.host));
            if let Err(e) = async {
                let current_price: f64 = price_provider.get_current_price(&client).await?;
                let switch_frequency_to: i32 = bitaxe::should_switch_frequency_to(&client, bitaxe, current_price).await?;
                if switch_frequency_to != -1 {
                    bitaxe::switch_frequency(&client, bitaxe, switch_frequency_to).await?;
                }
                Ok::<(), Box<dyn std::error::Error>>(())
            }.await {
                log(format!("Error processing {}: {}", bitaxe.host, e));
            }
        }

        log(format!("Sleeping for {:?} minutes", &CONFIG.get().unwrap().check_interval).to_owned());
        let ten_millis = std_time::Duration::from_millis(check_interval.try_into().unwrap());
        tokio_time::sleep(ten_millis).await;
    }

    log("Goodbye!".to_string());
    Ok(())
}
