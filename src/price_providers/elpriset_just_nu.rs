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
    /// Returned when the price API answers with a non-OK status, so a pricing outage
    /// degrades to a configured frequency instead of stalling the loop. A host that is
    /// genuinely unreachable does *not* use this - that fails at request time and
    /// propagates `PriceError::Network`.
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
            tracing::info!(url = %url, "Getting electricity price");

            // `url.as_str()` rather than `url`, so the request below does not consume
            // the value this block's error line still wants to report.
            let response = client.get(url.as_str()).send().await?;
            let current_price = if response.status() == StatusCode::OK {
                let json: serde_json::Value = response.json().await?;
                parse_price_data(&json, now)?
            } else {
                tracing::error!(
                    url = %url,
                    status = %response.status(),
                    fallback_price = self.fallback_price,
                    "Price request failed, using configured default"
                );
                self.fallback_price
            };

            tracing::info!(sek_per_kwh = current_price, "Current electricity price");
            Ok(current_price)
        })
    }
}
