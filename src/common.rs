use serde::Deserialize;
use std::fs::File;
use std::io::Read;
use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("failed to read config.toml: {0}")]
    Read(#[from] std::io::Error),

    #[error("invalid config.toml: {0}")]
    Parse(#[from] toml::de::Error),

    #[error("unknown price provider '{0}'; expected one of: elpriset_just_nu")]
    UnknownProvider(String),

    #[error("price provider '{0}' requires a [price_provider.{0}] section")]
    MissingProviderSection(String),

    #[error("check_interval of {0} minutes is too large to schedule")]
    IntervalTooLarge(u64),
}

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    /// Minutes between price checks and frequency updates.
    pub check_interval: u64,
    pub prices: Prices,
    pub price_provider: PriceProviderConfig,
    pub bitaxes: Vec<Bitaxe>,
}

impl Config {
    /// How long to wait between checks. Total: the arithmetic that used to live in
    /// `main` could overflow, and in a release build overflow wraps rather than
    /// panics, so a bad value would have produced a silently wrong sleep.
    pub fn sleep_duration(&self) -> Result<Duration, ConfigError> {
        let seconds = self
            .check_interval
            .checked_mul(60)
            .ok_or(ConfigError::IntervalTooLarge(self.check_interval))?;

        Ok(Duration::from_secs(seconds))
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct PriceProviderConfig {
    pub provider_type: String,
    #[serde(default)]
    pub elpriset_just_nu: Option<crate::price_providers::ElPrisetJustNuConfig>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Bitaxe {
    pub host: String,
    pub expensive: i32,
    pub default: i32,
    pub cheap: i32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Prices {
    pub cheap: f64,
    pub expensive: f64,
    pub default: f64,
}

/// Parses configuration text. Kept separate from `load_config` so the rules can be
/// tested without a file on disk at a fixed path.
pub fn parse_config(contents: &str) -> Result<Config, ConfigError> {
    let config: Config = toml::from_str(contents)?;
    Ok(config)
}

pub fn load_config() -> Result<Config, ConfigError> {
    let mut file = File::open("config.toml")?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    let config = parse_config(&contents)?;
    tracing::info!(config = ?config, "Loaded config");

    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config_with_check_interval(value: &str) -> String {
        format!(
            r#"
    check_interval = {value}

    [prices]
    cheap = 0.1
    expensive = 0.8
    default = 0.5

    [price_provider]
    provider_type = "elpriset_just_nu"

    [price_provider.elpriset_just_nu]
    price_zone = "SE3"

    [[bitaxes]]
    host = "192.168.8.1"
    expensive = 50
    default = 500
    cheap = 520
    "#
        )
    }

    #[test]
    fn parses_a_valid_configuration() {
        let config = parse_config(&config_with_check_interval("60")).unwrap();
        assert_eq!(config.check_interval, 60);
    }

    // `check_interval` used to be `i32`, so `-1` deserialised successfully and then panicked
    // on the way to `Duration::from_millis`, from inside the loop, at runtime.
    #[test]
    fn negative_check_interval_is_rejected_by_the_parser() {
        let result = parse_config(&config_with_check_interval("-1"));
        assert!(
            matches!(result, Err(ConfigError::Parse(_))),
            "a negative interval should never reach the runtime"
        );
    }

    // The old arithmetic was `1000 * 60 * interval` on an `i32`, which wrapped, and a
    // wrapped-negative then hit `try_into().unwrap()`. In a release build overflow wraps
    // silently instead of panicking, so the old code could have slept for a wrong duration
    // with nothing to indicate it.
    //
    // `i64::MAX` is the ceiling: TOML integers are signed 64-bit, so the parser has already
    // rejected anything larger before this arithmetic is reached.
    #[test]
    fn absurd_check_interval_is_reported_rather_than_overflowing() {
        let config = parse_config(&config_with_check_interval(&i64::MAX.to_string())).unwrap();

        assert!(
            matches!(config.sleep_duration(), Err(ConfigError::IntervalTooLarge(_))),
            "overflow must not be reachable in release builds, where it wraps rather than panics"
        );
    }

    #[test]
    fn converts_minutes_to_a_duration() {
        let config = parse_config(&config_with_check_interval("2")).unwrap();
        assert_eq!(config.sleep_duration().unwrap(), std::time::Duration::from_secs(120));
    }
}
