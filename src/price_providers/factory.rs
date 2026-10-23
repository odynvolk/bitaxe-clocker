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

#[cfg(test)]
mod tests {
    use super::PriceProviderFactory;
    use crate::common::PriceProviderConfig;
    use crate::price_providers::ElPrisetJustNuConfig;

    fn config(provider_type: &str, price_zone: Option<&str>) -> PriceProviderConfig {
        PriceProviderConfig {
            provider_type: provider_type.to_string(),
            elpriset_just_nu: price_zone.map(|zone| ElPrisetJustNuConfig {
                price_zone: zone.to_string(),
            }),
        }
    }

    #[test]
    fn known_provider_with_its_subconfig_is_created() {
        let provider = PriceProviderFactory::create_provider(&config("elpriset_just_nu", Some("SE3")), 0.5);
        assert!(provider.is_ok());
    }

    // The old behaviour logged "Unknown provider type, using elpriset_just_nu" and carried on,
    // so a daemon configured for one market would silently price itself against another.
    #[test]
    fn unknown_provider_type_is_rejected_rather_than_defaulting() {
        let provider = PriceProviderFactory::create_provider(&config("nordpool", Some("SE3")), 0.5);
        assert!(provider.is_err());
    }

    // The old behaviour called `.expect()`, which panics inside the loop rather than reporting
    // a configuration problem the operator can act on.
    #[test]
    fn missing_provider_subconfig_is_rejected_rather_than_panicking() {
        let provider = PriceProviderFactory::create_provider(&config("elpriset_just_nu", None), 0.5);
        assert!(provider.is_err());
    }
}
