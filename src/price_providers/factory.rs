use crate::common;
use crate::price_providers::{ElPrisetJustNuProvider, PriceProvider};

/// Factory for creating price provider instances.
/// Centralized location for provider instantiation logic.
pub struct PriceProviderFactory;

impl PriceProviderFactory {
    /// Builds the provider named by `config.provider_type`.
    ///
    /// Every failure here is a configuration mistake, so it is returned rather than
    /// guessed at: the previous behaviour was to log and fall back to
    /// `elpriset_just_nu` for any unrecognised name, which means a daemon set up for
    /// one market would quietly price itself against another.
    pub fn create_provider(
        config: &common::PriceProviderConfig,
        fallback_price: f64,
    ) -> Result<Box<dyn PriceProvider>, common::ConfigError> {
        match config.provider_type.as_str() {
            "elpriset_just_nu" => {
                let elpriset_config = config
                    .elpriset_just_nu
                    .as_ref()
                    .ok_or_else(|| common::ConfigError::MissingProviderSection("elpriset_just_nu".to_string()))?;

                Ok(Box::new(ElPrisetJustNuProvider::new(
                    elpriset_config.clone(),
                    fallback_price,
                )))
            }
            other => Err(common::ConfigError::UnknownProvider(other.to_string())),
        }
    }
}
