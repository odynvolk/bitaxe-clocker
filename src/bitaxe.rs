use reqwest::StatusCode;
use serde_json::Value;
use std::collections::HashMap;

use crate::common;

#[derive(Debug, thiserror::Error)]
pub enum BitaxeError {
    /// Transport-level failure. The `reqwest` error is retained as the source, so the
    /// actual reason (refused, DNS, timeout) survives into the log rather than being
    /// flattened into a string at conversion time.
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("Invalid frequency value")]
    InvalidFrequency,

    /// The device answered, but rejected the frequency update.
    #[error("Failed to update Bitaxe")]
    UpdateRejected,
}

/// Frequency this device should run at for the given price: below `prices.cheap`
/// the device's cheap frequency, below `prices.expensive` its default, at or above
/// `prices.expensive` its expensive one.
pub fn target_frequency(current_price: f64, bitaxe: &common::Bitaxe, prices: &common::Prices) -> i32 {
    if current_price < prices.cheap {
        bitaxe.cheap
    } else if current_price < prices.expensive {
        bitaxe.default
    } else {
        bitaxe.expensive
    }
}

/// The frequency to change the device to, or `None` when it is already running it.
pub async fn frequency_to_switch_to(
    client: &reqwest::Client,
    bitaxe: &common::Bitaxe,
    current_price: f64,
    prices: &common::Prices,
) -> Result<Option<i32>, BitaxeError> {
    let running = running_frequency(client, bitaxe).await?;
    let target = target_frequency(current_price, bitaxe, prices);

    Ok((running != target).then_some(target))
}

pub fn parse_running_frequency(json: &Value) -> Result<i32, BitaxeError> {
    let frequency: i32 = json["frequency"].as_f64().ok_or(BitaxeError::InvalidFrequency)? as i32;
    Ok(frequency)
}

/// The frequency the device reports as currently running.
pub async fn running_frequency(client: &reqwest::Client, bitaxe: &common::Bitaxe) -> Result<i32, BitaxeError> {
    let url = format!("http://{}/api/system/info", bitaxe.host);
    tracing::info!(url = %url, "Getting Bitaxe info");

    let response = client.get(url).send().await?;
    let json: Value = response.json().await?;

    let frequency = parse_running_frequency(&json)?;
    tracing::info!(frequency, "Running frequency");

    Ok(frequency)
}

pub async fn switch_frequency(
    client: &reqwest::Client,
    bitaxe: &common::Bitaxe,
    frequency: i32,
) -> Result<(), BitaxeError> {
    tracing::info!(frequency, "Switching frequency");
    let mut body = HashMap::new();
    body.insert("frequency", frequency);
    let response = client
        .patch(format!("http://{}/api/system", bitaxe.host))
        .json(&body)
        .send()
        .await?;

    if response.status() == StatusCode::OK {
        tracing::info!("Restarting!");
        client
            .post(format!("http://{}/api/system/restart", bitaxe.host))
            .send()
            .await?;
        Ok(())
    } else {
        tracing::error!(host = %bitaxe.host, "Something went wrong when updating");
        Err(BitaxeError::UpdateRejected)
    }
}
