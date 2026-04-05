use crate::common;
use crate::price_providers::PriceProvider;

/// Factory for creating price provider instances.
/// Centralized location for provider instantiation logic.
pub struct PriceProviderFactory;

impl PriceProviderFactory {
    /// Creates a price provider instance based on the configuration.
    pub fn create_provider(provider_type: &str, config: &common::PriceProviderConfig) -> Box<dyn PriceProvider> {
        match provider_type {
            "elpriset_just_nu" => create_elpriset_provider(config),
            _ => {
                // Default to elpriset_just_nu if unknown provider type
                common::log(format!(
                    "Unknown provider type '{}', using elpriset_just_nu",
                    provider_type
                ));
                create_elpriset_provider(config)
            }
        }
    }
}

/// Helper function to create an ElPrisetJustNuProvider instance.
/// Extracted to avoid code duplication between match arms.
fn create_elpriset_provider(config: &common::PriceProviderConfig) -> Box<dyn PriceProvider> {
    let elpriset_config = config
        .elpriset_just_nu
        .as_ref()
        .expect("elpriset_just_nu provider requires elpriset_just_nu config");
    Box::new(crate::price_providers::ElPrisetJustNuProvider::new(
        crate::price_providers::ElPrisetJustNuConfig {
            price_zone: elpriset_config.price_zone.clone(),
        },
    ))
}
