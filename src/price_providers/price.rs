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

#[cfg(test)]
mod tests {
    use super::{parse_price_data, PriceError};
    use chrono::DateTime;
    use serde_json::json;
    use serde_json::Value;

    fn fixture_price_data() -> Value {
        serde_json::from_str(include_str!("../../tests/fixtures/price.json")).unwrap()
    }

    #[test]
    fn test_parse_price_data_success() {
        let json = fixture_price_data();
        // Time within the first interval: 2026-04-03T00:00:00+02:00 to 2026-04-03T00:15:00+02:00
        let now: DateTime<chrono::Local> = "2026-04-03T00:07:00+02:00".parse().unwrap();

        let result = parse_price_data(&json, now);

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0.5929);
    }

    #[test]
    fn test_parse_price_data_second_interval() {
        let json = fixture_price_data();
        // Time within the second interval: 2026-04-03T00:15:00+02:00 to 2026-04-03T00:30:00+02:00
        let now: DateTime<chrono::Local> = "2026-04-03T00:22:00+02:00".parse().unwrap();

        let result = parse_price_data(&json, now);

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0.59859);
    }

    #[test]
    fn test_parse_price_data_before_range() {
        let json = fixture_price_data();
        // Time before the first interval
        let now: DateTime<chrono::Local> = "2026-04-02T23:00:00+02:00".parse().unwrap();

        let result = parse_price_data(&json, now);

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), PriceError::InvalidPrice));
    }

    #[test]
    fn test_parse_price_data_after_range() {
        let json = fixture_price_data();
        // Time after the last interval
        let now: DateTime<chrono::Local> = "2026-04-03T01:00:00+02:00".parse().unwrap();

        let result = parse_price_data(&json, now);

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), PriceError::InvalidPrice));
    }

    #[test]
    fn test_parse_price_data_empty_array() {
        let json = json!([]);
        let now: DateTime<chrono::Local> = "2026-04-03T00:07:00+02:00".parse().unwrap();

        let result = parse_price_data(&json, now);

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), PriceError::InvalidPrice));
    }

    #[test]
    fn test_parse_price_data_not_array() {
        let json = json!({"invalid": "structure"});
        let now: DateTime<chrono::Local> = "2026-04-03T00:07:00+02:00".parse().unwrap();

        let result = parse_price_data(&json, now);

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), PriceError::InvalidPrice));
    }

    #[test]
    fn test_parse_price_data_missing_time_start() {
        let json = json!([{
            "SEK_per_kWh": 0.5929,
            "time_end": "2026-04-03T00:15:00+02:00"
        }]);
        let now: DateTime<chrono::Local> = "2026-04-03T00:07:00+02:00".parse().unwrap();

        let result = parse_price_data(&json, now);

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), PriceError::ParseError));
    }

    #[test]
    fn test_parse_price_data_missing_time_end() {
        let json = json!([{
            "SEK_per_kWh": 0.5929,
            "time_start": "2026-04-03T00:00:00+02:00"
        }]);
        let now: DateTime<chrono::Local> = "2026-04-03T00:07:00+02:00".parse().unwrap();

        let result = parse_price_data(&json, now);

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), PriceError::ParseError));
    }

    #[test]
    fn test_parse_price_data_invalid_datetime_format() {
        let json = json!([{
            "SEK_per_kWh": 0.5929,
            "time_start": "invalid-date",
            "time_end": "2026-04-03T00:15:00+02:00"
        }]);
        let now: DateTime<chrono::Local> = "2026-04-03T00:07:00+02:00".parse().unwrap();

        let result = parse_price_data(&json, now);

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), PriceError::DateTimeParse(_)));
    }

    #[test]
    fn test_parse_price_data_missing_price_value() {
        let json = json!([{
            "time_start": "2026-04-03T00:00:00+02:00",
            "time_end": "2026-04-03T00:15:00+02:00"
        }]);
        let now: DateTime<chrono::Local> = "2026-04-03T00:07:00+02:00".parse().unwrap();

        let result = parse_price_data(&json, now);

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), PriceError::InvalidPrice));
    }

    #[test]
    fn test_parse_price_data_price_not_number() {
        let json = json!([{
            "SEK_per_kWh": "not_a_number",
            "time_start": "2026-04-03T00:00:00+02:00",
            "time_end": "2026-04-03T00:15:00+02:00"
        }]);
        let now: DateTime<chrono::Local> = "2026-04-03T00:07:00+02:00".parse().unwrap();

        let result = parse_price_data(&json, now);

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), PriceError::InvalidPrice));
    }
}
