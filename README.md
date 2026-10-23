# bitaxe-clocker
A small application which adjusts the frequency on one or several Bitaxes depending on energy prices. Written in Rust.

Currently it only fetches electricity prices for Sweden, via [https://www.elprisetjustnu.se/](https://www.elprisetjustnu.se/),
but creating modules for other suppliers is possible.

## Setup

Prerequisites:

- Rust lang

The toolchain used for local development is declared in `rust-toolchain.toml` and selected
automatically by [rustup](https://rustup.rs/) — just run `cargo build`. There is no need to
install a specific version by hand. It names the `stable` channel rather than an exact
version, so nothing has to be downloaded to start work; it does not apply to the Docker
image, which pins its own.

## Development

There is no CI in this repository, so these checks are run by hand before committing:

```bash
cargo fmt -- --check                        # formatting
cargo clippy --all-targets -- -D warnings   # lints, warnings are errors
cargo test                                  # test suite
cargo build --release --locked              # release build
```

Both config files in this repo are worth knowing about before you touch them, because each
fails in the opposite way. A key clippy does not recognise aborts the entire build with an
`unknown field` error, so nothing lints and the reason looks like a code problem. A key
rustfmt recognises but does not implement on stable is accepted, silently does nothing, and
`cargo fmt` still exits 0. Re-run the checks above after editing either file, and for a
rustfmt key with visible layout effects, confirm a file really is formatted differently with
and without it.

## Upgrading

Three changes to what the application accepts and does at startup, none of which change how
it behaves once running:

- **An unrecognised `provider_type` now stops startup.** It used to log
  `Unknown provider type '<x>', using elpriset_just_nu` and carry on, so a daemon configured
  for one market would silently price itself against another. A missing
  `[price_provider.<name>]` section likewise used to panic via `.expect()`; it is now a
  reported configuration error.
- **`check_interval` must be a non-negative integer**, enforced while parsing. It used to
  accept a negative value, then panic on the way to building a `Duration` from inside the
  loop. An absurdly large value is now reported rather than silently wrapping.
- **The Docker container runs as uid 10001** rather than root, so a mounted `config.toml`
  must be world-readable.

Log output also changed shape, from `INFO msg="Checking 192.168.8.1"` to structured fields:
`INFO Checking device host=192.168.8.1`. Anything grepping the old format needs updating.

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
            other => Err(common::ConfigError::UnknownProvider(format!(
                "'{other}'; supported providers: {}",
                SUPPORTED_PROVIDERS.join(", ")
            ))),
        }
    }
}
```

A new provider also needs its name added to `SUPPORTED_PROVIDERS` in the same file, which is
what the error message reports. `listed_provider_is_actually_buildable` fails if the two
drift apart.

5. **Update the configuration** in `config.toml`:

```toml
[price_provider]
provider_type = "nordpool"

[price_provider.nordpool]
region = "SE3"
```
