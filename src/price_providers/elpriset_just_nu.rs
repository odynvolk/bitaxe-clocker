use crate::logger;
use crate::price_providers::{parse_price_data, PriceError, PriceProvider};
use reqwest::Client;
use reqwest::StatusCode;
use serde::Deserialize;
use std::future::Future;
use std::pin::Pin;

/// Configuration for the ElPrisetJustNu provider.
#[derive(Debug, Deserialize, Clone)]
pub struct ElPrisetJustNuConfig {
    pub price_zone: String,
}

/// Price provider implementation for elpriset-just-nu.se.
pub struct ElPrisetJustNuProvider {
    config: ElPrisetJustNuConfig,
    /// Used when the price API is unreachable, so a pricing outage degrades to a
    /// configured frequency instead of stalling the loop.
    fallback_price: f64,
}

impl ElPrisetJustNuProvider {
    pub fn new(config: ElPrisetJustNuConfig, fallback_price: f64) -> Self {
        Self { config, fallback_price }
    }
}

impl PriceProvider for ElPrisetJustNuProvider {
    fn get_current_price<'a>(
        &'a self,
        client: &'a Client,
    ) -> Pin<Box<dyn Future<Output = Result<f64, PriceError>> + Send + 'a>> {
        Box::pin(async move {
            let now = chrono::Local::now();
            let url = format!(
                "https://www.elprisetjustnu.se/api/v1/prices/{}_{}.json",
                now.format("%Y/%m-%d"),
                self.config.price_zone
            );
            logger::info(&format!("Getting electricity price from URL {}", url));

            let response = client.get(url).send().await?;
            let current_price = if response.status() == StatusCode::OK {
                let json: serde_json::Value = response.json().await?;
                parse_price_data(&json, now)?
            } else {
                logger::error("Error getting price using default");
                self.fallback_price
            };

            logger::info(&format!("Current electricity price {:?}", current_price));
            Ok(current_price)
        })
    }
}
