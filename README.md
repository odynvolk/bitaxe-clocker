# bitaxe-clocker
A small application which adjusts the frequency on one or several Bitaxes depending on energy prices. Written in Rust.

Currently it only fetches electricity prices for Sweden, via [https://www.elprisetjustnu.se/](https://www.elprisetjustnu.se/),
but creating modules for other suppliers is possible.

## Setup

Prerequisites:

- Rust lang

An easy way to setup Rust is to use [asdf](https://asdf-vm.com/). Then run `asdf install` to get the proper version specified in the project.

## Usage
1) Create a `config.toml` from the `config.example.toml` and place it in the main diretory.

2) Build a release.

```bash
cargo build -r
```

3) Run the application.

```bash
./target/release/bitaxe-clocker
```

## Adding a Custom Price Provider

The application uses a `PriceProvider` trait to fetch electricity prices, making it easy to add support for new providers. All price provider code is located in the `src/price_providers/` directory.

### Module Structure

```
src/price_providers/
├── mod.rs          # Module exports and re-exports
├── price.rs        # PriceProvider trait and PriceError enum
├── factory.rs      # PriceProviderFactory for creating provider instances
└── elpriset_just_nu.rs  # Implementation for elpriset-just-nu.se
```

### Implementation Steps

1. **Create a new provider file** in `src/price_providers/`. For example, `nordpool.rs`:

```rust
use crate::price_providers::{PriceProvider, PriceError};
use reqwest::Client;
use serde::Deserialize;
use std::pin::Pin;
use std::future::Future;

/// Configuration for the Nordpool provider.
#[derive(Debug, Deserialize)]
pub struct NordpoolConfig {
    pub region: String,
}

/// Price provider implementation for Nordpool.
pub struct NordpoolProvider {
    config: NordpoolConfig,
}

impl NordpoolProvider {
    pub fn new(config: NordpoolConfig) -> Self {
        Self { config }
    }
}

impl PriceProvider for NordpoolProvider {
    fn get_current_price<'a>(&'a self, client: &'a Client) -> Pin<Box<dyn Future<Output = Result<f64, PriceError>> + Send + 'a>> {
        Box::pin(async move {
            let now = chrono::Local::now();
            let url = format!(
                "https://www.nordpoolgroup.com/api/marketdata/r/1/price/countries/SE/area/SE3/currency/SEK/years/{}/month/{}", 
                now.year(),
                now.month()
            );
            
            let response = client.get(&url).send().await?;
            let json: serde_json::Value = response.json().await?;
            
            // Parse the Nordpool response format and extract current price
            // ... parsing logic ...
            
            Ok(price)
        })
    }
}
```

2. **Register the new module** in `src/price_providers/mod.rs`:

```rust
pub use elpriset_just_nu::{ElPrisetJustNuConfig, ElPrisetJustNuProvider};
pub use factory::PriceProviderFactory;
pub use price::{parse_price_data, PriceError, PriceProvider};
pub use nordpool::{NordpoolConfig, NordpoolProvider};  // Add this line

mod elpriset_just_nu;
mod factory;
mod nordpool;  // Add this line
mod price;
```

3. **Add the provider to the factory** in `src/price_providers/factory.rs`:

```rust
impl PriceProviderFactory {
    pub fn create_provider(provider_type: &str, config: &common::PriceProviderConfig) -> Box<dyn PriceProvider> {
        match provider_type {
            "elpriset_just_nu" => {
                // existing implementation
            }
            "nordpool" => {
                let nordpool_config = config.nordpool.as_ref()
                    .expect("nordpool provider requires nordpool config");
                Box::new(NordpoolProvider::new(NordpoolConfig {
                    region: nordpool_config.region.clone(),
                }))
            }
            _ => {
                // default fallback
            }
        }
    }
}
```

4. **Update the configuration** in `config.toml`:

```toml
[price_provider]
provider_type = "nordpool"

[price_provider.nordpool]
region = "SE3"
```
