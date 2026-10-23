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

    let sleep_duration = config.sleep_duration()?;
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
                if let Some(frequency) =
                    bitaxe_clocker::bitaxe::frequency_to_switch_to(&client, bitaxe, current_price, &config.prices)
                        .await?
                {
                    bitaxe_clocker::bitaxe::switch_frequency(&client, bitaxe, frequency).await?;
                }
                Ok::<(), Box<dyn std::error::Error>>(())
            }
            .await
            {
                logger::error(&format!("Error processing {}: {}", bitaxe.host, e));
            }
        }

        logger::info(&format!("Sleeping for {:?} minutes", config.check_interval));
        tokio_time::sleep(sleep_duration).await;
    }

    logger::info("Goodbye!");
    Ok(())
}
