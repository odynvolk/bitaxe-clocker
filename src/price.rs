use chrono::DateTime;
use reqwest::Client;
use serde_json::Value;
use std::fmt;

use crate::common;

#[derive(Debug)]
pub enum PriceError {
    NetworkError(reqwest::Error),
    JsonError(serde_json::Error),
    ParseError,
    InvalidPrice,
    DateTimeParseError(chrono::ParseError),
}

impl fmt::Display for PriceError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            PriceError::NetworkError(e) => write!(f, "Network error: {}", e),
            PriceError::JsonError(e) => write!(f, "JSON parsing error: {}", e),
            PriceError::ParseError => write!(f, "Failed to parse datetime"),
            PriceError::InvalidPrice => write!(f, "Invalid price value"),
            PriceError::DateTimeParseError(e) => write!(f, "DateTime parse error: {}", e),
        }
    }
}

impl PartialEq for PriceError {
    fn eq(&self, other: &Self) -> bool {
        use PriceError::*;
        match (self, other) {
            (NetworkError(_), NetworkError(_)) => true,
            (JsonError(_), JsonError(_)) => true,
            (ParseError, ParseError) => true,
            (InvalidPrice, InvalidPrice) => true,
            (DateTimeParseError(_), DateTimeParseError(_)) => true,
            _ => false,
        }
    }
}

impl std::error::Error for PriceError {}

impl From<reqwest::Error> for PriceError {
    fn from(err: reqwest::Error) -> Self {
        PriceError::NetworkError(err)
    }
}

impl From<serde_json::Error> for PriceError {
    fn from(err: serde_json::Error) -> Self {
        PriceError::JsonError(err)
    }
}

impl From<chrono::ParseError> for PriceError {
    fn from(err: chrono::ParseError) -> Self {
        PriceError::DateTimeParseError(err)
    }
}

pub fn parse_price_data(json: &Value, now: DateTime<chrono::Local>) -> Result<f64, PriceError> {
    let items = json.as_array().ok_or(PriceError::InvalidPrice)?;

    for item in items {
        let time_start = DateTime::parse_from_rfc3339(item["time_start"].as_str().ok_or(PriceError::ParseError)?)?;
        let time_end = DateTime::parse_from_rfc3339(item["time_end"].as_str().ok_or(PriceError::ParseError)?)?;

        if (time_start < now) && (time_end > now) {
            let price = item["SEK_per_kWh"].as_f64().ok_or(PriceError::InvalidPrice)?;
            return Ok(price);
        }
    }

    Err(PriceError::InvalidPrice)
}

pub async fn get_current_price(client: &Client) -> Result<f64, PriceError> {
    let now = chrono::Local::now();
    let url = format!(
        "https://www.elprisetjustnu.se/api/v1/prices/{}_{}.json",
        now.format("%Y/%m-%d"),
        common::CONFIG.elpriset_just_nu.price_zone
    );
    common::log(format!("Getting electricity price from URL {}", url));

    let response = client.get(url).send().await?;
    let current_price = if response.status() == reqwest::StatusCode::OK {
        let json: Value = response.json().await?;
        parse_price_data(&json, now)?
    } else {
        common::log(format!("Error getting price using default"));
        common::CONFIG.prices.default
    };

    common::log(format!("Current electricity price {:?}", current_price));
    Ok(current_price)
}
