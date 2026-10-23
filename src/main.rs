use bitaxe_clocker::logger;
use bitaxe_clocker::price_providers::PriceProviderFactory;
use reqwest::Client;
use std::{
    sync::atomic::{AtomicBool, Ordering},
    sync::Arc,
    time as std_time,
};
use tokio::{signal, time as tokio_time};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Installed first, on purpose: `tracing` events emitted before a subscriber is
    // installed are discarded rather than buffered, and `load_config` emits one.
    bitaxe_clocker::logger::init();

    let config = bitaxe_clocker::common::load_config()?;

    logger::info(&format!("Bitaxe Clocker Config {:?}", config.bitaxes));

    // Atomic flag to track shutdown request, wrapped in Arc for sharing
    let shutdown_requested = Arc::new(AtomicBool::new(false));

    // Set up Ctrl-C signal handler
    let shutdown_clone = Arc::clone(&shutdown_requested);
    tokio::spawn(async move {
        if let Err(_err) = signal::ctrl_c().await {
            logger::error("Failed to set up Ctrl-C handler");
        }
        shutdown_clone.store(true, Ordering::SeqCst);
        logger::info("Shutdown signal received");
    });

    let check_interval = 1000 * 60 * config.check_interval;
    let client = Client::builder().timeout(std_time::Duration::from_secs(10)).build()?;
    let price_provider = PriceProviderFactory::create_provider(&config.price_provider, config.prices.default)?;

    loop {
        if shutdown_requested.load(Ordering::SeqCst) {
            logger::info("Shutting down...");
            break;
        }

        for bitaxe in &config.bitaxes {
            logger::info(&format!("Checking {}", bitaxe.host));
            if let Err(e) = async {
                let current_price: f64 = price_provider.get_current_price(&client).await?;
                let switch_frequency_to: i32 =
                    bitaxe_clocker::bitaxe::should_switch_frequency_to(&client, bitaxe, current_price, &config.prices)
                        .await?;
                if switch_frequency_to != -1 {
                    bitaxe_clocker::bitaxe::switch_frequency(&client, bitaxe, switch_frequency_to).await?;
                }
                Ok::<(), Box<dyn std::error::Error>>(())
            }
            .await
            {
                logger::error(&format!("Error processing {}: {}", bitaxe.host, e));
            }
        }

        logger::info(&format!("Sleeping for {:?} minutes", config.check_interval));
        let ten_millis = std_time::Duration::from_millis(check_interval.try_into().unwrap());
        tokio_time::sleep(ten_millis).await;
    }

    logger::info("Goodbye!");
    Ok(())
}
