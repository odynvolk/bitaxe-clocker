# bitaxe-clocker
A small application which adjusts the frequency on one or several Bitaxes depending on energy prices. Written in Rust.

Currently it only fetches electricity prices for Sweden, via [https://www.elprisetjustnu.se/](https://www.elprisetjustnu.se/),
but creating modules for other suppliers is possible.

## Setup

Prerequisites:

- Rust lang

The required toolchain is pinned in `rust-toolchain.toml` and is selected automatically by
[rustup](https://rustup.rs/) — just run `cargo build`. There is no need to install a specific
version by hand.

## Development

There is no CI in this repository, so these checks are run by hand before committing:

```bash
cargo fmt -- --check                        # formatting
cargo clippy --all-targets -- -D warnings   # lints, warnings are errors
NODE_ENV=test cargo test                    # test suite
cargo build --release                       # release build
```

Both config files in this repo are worth knowing about before you touch them: a key name that
clippy or rustfmt does not recognise does **not** produce a warning. Clippy aborts the entire
build with an `unknown field` error, and stable rustfmt silently ignores the key and carries on.
Re-run the two commands above after any edit to `rust-toolchain.toml` or `.rustfmt.toml` to
confirm the change is actually in effect.

## Usage
1) Create a `config.toml` from the `config.example.toml` and place it in the main directory.

2) Build a release.

```bash
cargo build -r
```

3) Run the application.

```bash
./target/release/bitaxe-clocker
```

## Docker

Build and run the application using Docker Compose:

```bash
docker compose build
docker compose up -d   # run in background
docker compose down    # stop
```

This mounts your local `config.toml` into the container. The container will restart automatically unless you explicitly stop it.

The container runs as a non-root user (uid 10001), so a mounted `config.toml` must be
world-readable — the usual `644` is fine, `600` is not.

Alternatively, build and run with plain Docker:

```bash
docker build -t bitaxe-clocker .
docker run -v $(pwd)/config.toml:/app/config.toml:ro bitaxe-clocker
```

The image ships with `config.example.toml` as the default. Mount your own `config.toml` to override it.

The image includes a Docker `HEALTHCHECK` (every 30s, 3 retries). Because the application only
makes outbound connections and exposes no inbound port, the healthcheck verifies process liveness
by signalling PID 1 (`kill -0 1`), the `bitaxe-clocker` process itself. The container reports
`unhealthy` if that process no longer responds.

## Configuration

The application is configured via `config.toml`. Below is a reference of all available options.

### Global Settings

| Key | Type | Description |
|-----|------|-------------|
| `check_interval` | `u64` | How often to check prices and update frequency, in minutes. Must not be negative. |

### Prices (under `[prices]`)

These options control the electricity-price thresholds that decide when the Bitaxe switches between frequency modes. Only `cheap` and `expensive` are actual mode triggers; `default` is a fallback price.

| Key | Type | Description |
|-----|------|-------------|
| `cheap` | `f64` | Price below this threshold triggers the **cheap** (turbo) frequency mode. |
| `default` | `f64` | Fallback price used when live prices can't be fetched (not a mode trigger). |
| `expensive` | `f64` | Price at or above this threshold triggers the **expensive** (slow) frequency mode. |

### Price Provider

| Key | Type | Description |
|-----|------|-------------|
| `provider_type` | `String` | Name of the price provider (e.g. `elpriset_just_nu`). |
| `elpriset_just_nu.price_zone` | `String` | Swedish price zone (e.g. `SE3`). |

### Bitaxe Devices

Each Bitaxe entry defines the frequency (in GH/s) for each price mode:

| Key | Type | Description |
|-----|------|-------------|
| `host` | `String` | IP or hostname of the Bitaxe. |
| `expensive` | `i32` | Frequency to use when electricity is expensive. |
| `default` | `i32` | Frequency to use when electricity is at a normal price. |
| `cheap` | `i32` | Frequency to use when electricity is cheap. |

### Example

```toml
check_interval = 1

[prices]
cheap = 0.1        # Below this, run at cheap (turbo) frequency
expensive = 0.8    # At or above this, run at expensive (slow) frequency
default = 0.5      # Fallback price

[price_provider]
provider_type = "elpriset_just_nu"

[price_provider.elpriset_just_nu]
price_zone = "SE3"

[[bitaxes]]
host = "192.168.8.1"
expensive = 50     # Low frequency when electricity is expensive
default = 500      # Normal frequency for average prices
cheap = 520        # High frequency when electricity is cheap
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
    /// Returned when the price API is unreachable, so a pricing outage degrades to a
    /// configured frequency instead of stalling the loop.
    fallback_price: f64,
}

impl NordpoolProvider {
    pub fn new(config: NordpoolConfig, fallback_price: f64) -> Self {
        Self { config, fallback_price }
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

3. **Add a `nordpool` field to the price-provider config struct** in `src/common.rs`:

```rust
pub struct PriceProviderConfig {
    pub provider_type: String,
    #[serde(default)]
    pub elpriset_just_nu: Option<crate::price_providers::ElPrisetJustNuConfig>,
    #[serde(default)]
    pub nordpool: Option<crate::price_providers::NordpoolConfig>,
}
```

4. **Add the provider to the factory** in `src/price_providers/factory.rs`:

```rust
impl PriceProviderFactory {
    pub fn create_provider(
        config: &common::PriceProviderConfig,
        fallback_price: f64,
    ) -> Result<Box<dyn PriceProvider>, common::ConfigError> {
        match config.provider_type.as_str() {
            "elpriset_just_nu" => {
                // existing implementation
            }
            "nordpool" => {
                let nordpool_config = config.nordpool.as_ref().ok_or_else(|| {
                    common::ConfigError::MissingProviderSection("nordpool".to_string())
                })?;

                Ok(Box::new(NordpoolProvider::new(
                    nordpool_config.clone(),
                    fallback_price,
                )))
            }
            other => Err(common::ConfigError::UnknownProvider(other.to_string())),
        }
    }
}
```

A new provider needs a matching arm in `ConfigError::UnknownProvider`'s message so the
error names the providers that are actually available.

5. **Update the configuration** in `config.toml`:

```toml
[price_provider]
provider_type = "nordpool"

[price_provider.nordpool]
region = "SE3"
```
