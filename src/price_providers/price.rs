use reqwest::Client;
use std::future::Future;
use std::pin::Pin;

#[derive(Debug, thiserror::Error)]
pub enum PriceError {
    /// Transport-level failure, with the `reqwest` error kept as the source.
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("Failed to parse datetime")]
    ParseError,

    #[error("Invalid price value")]
    InvalidPrice,

    #[error("DateTime parse error: {0}")]
    DateTimeParse(#[from] chrono::ParseError),
}

/// Trait that all price providers must implement.
/// This allows plugging in new providers without changing the core logic.
pub trait PriceProvider: Send + Sync {
    /// Fetch the current electricity price.
    /// Returns the price in SEK/kWh or an error.
    fn get_current_price<'a>(
        &'a self,
        client: &'a Client,
    ) -> Pin<Box<dyn Future<Output = Result<f64, PriceError>> + Send + 'a>>;
}

/// Parse price data from JSON response.
pub fn parse_price_data(json: &serde_json::Value, now: chrono::DateTime<chrono::Local>) -> Result<f64, PriceError> {
    let items = json.as_array().ok_or(PriceError::InvalidPrice)?;

    for item in items {
        let time_start =
            chrono::DateTime::parse_from_rfc3339(item["time_start"].as_str().ok_or(PriceError::ParseError)?)?;
        let time_end = chrono::DateTime::parse_from_rfc3339(item["time_end"].as_str().ok_or(PriceError::ParseError)?)?;

        if (time_start < now) && (time_end > now) {
            let price = item["SEK_per_kWh"].as_f64().ok_or(PriceError::InvalidPrice)?;
            return Ok(price);
        }
    }

    Err(PriceError::InvalidPrice)
}
