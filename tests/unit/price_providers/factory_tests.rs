use bitaxe_clocker::common::PriceProviderConfig;
use bitaxe_clocker::price_providers::ElPrisetJustNuConfig;
use bitaxe_clocker::price_providers::PriceProviderFactory;

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
