//! Subscriber setup only.
//!
//! There is deliberately no `info()`/`error()` wrapper here. The previous one took
//! `&str`, which forced every call site to pre-format its own message
//! (`logger::info(&format!("Checking {}", host))`), discarding the typed fields
//! `tracing` would otherwise record. Call sites use the `tracing` macros directly.

use tracing_subscriber::fmt::time::ChronoLocal;
use tracing_subscriber::{fmt, prelude::*};

pub fn init() {
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(
            fmt::layer()
                .with_target(false)
                .with_timer(ChronoLocal::new("%H:%M:%S".to_string())),
        )
        .with(env_filter)
        .init();
}
