use crate::common;
use crate::price_providers::{parse_price_data, PriceError, PriceProvider};
use reqwest::Client;
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
}

impl ElPrisetJustNuProvider {
    pub fn new(config: ElPrisetJustNuConfig) -> Self {
        Self { config }
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
            common::log(format!("Getting electricity price from URL {}", url));

            let response = client.get(url).send().await?;
            let current_price = if response.status() == reqwest::StatusCode::OK {
                let json: serde_json::Value = response.json().await?;
                parse_price_data(&json, now)?
            } else {
                common::log(format!("Error getting price using default"));
                common::CONFIG.get().ok_or(PriceError::InvalidPrice)?.prices.default
            };

            common::log(format!("Current electricity price {:?}", current_price));
            Ok(current_price)
        })
    }
}
