use serde_json::Value;
use std::collections::HashMap;

use crate::common;
use crate::logger;

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

/// Determines the target frequency based on current price and thresholds.
/// Returns: cheap if price < cheap, default if cheap <= price < expensive, expensive if price >= expensive
pub fn determine_target_mode(
    current_price: f64,
    bitaxe: &common::Bitaxe,
    cheap_threshold: f64,
    expensive_threshold: f64,
) -> i32 {
    if current_price < cheap_threshold {
        bitaxe.cheap
    } else if current_price < expensive_threshold {
        bitaxe.default
    } else {
        bitaxe.expensive
    }
}

pub async fn should_switch_frequency_to(
    client: &reqwest::Client,
    bitaxe: &common::Bitaxe,
    current_price: f64,
) -> Result<i32, Box<dyn std::error::Error>> {
    let running_mode: i32 = get_running_mode(client, bitaxe).await?;
    let switch_to_mode: i32 = determine_target_mode(
        current_price,
        bitaxe,
        common::CONFIG.get().unwrap().prices.cheap,
        common::CONFIG.get().unwrap().prices.expensive,
    );
    let switch_frequency_to: i32 = if running_mode != switch_to_mode {
        switch_to_mode
    } else {
        -1
    };

    Ok(switch_frequency_to)
}

pub fn parse_running_mode(json: &Value) -> Result<i32, BitaxeError> {
    let running_mode: i32 = json["frequency"].as_f64().ok_or(BitaxeError::InvalidFrequency)? as i32;
    Ok(running_mode)
}

pub async fn get_running_mode(client: &reqwest::Client, bitaxe: &common::Bitaxe) -> Result<i32, BitaxeError> {
    let url = format!("http://{}/api/system/info", bitaxe.host);
    logger::info(&format!("Getting Bitaxe info from URL {}", url));

    let response = client.get(url).send().await?;
    let json: Value = response.json().await?;

    let running_mode = parse_running_mode(&json)?;
    logger::info(&format!("Running mode {}", running_mode));

    Ok(running_mode)
}

pub async fn switch_frequency(
    client: &reqwest::Client,
    bitaxe: &common::Bitaxe,
    switch_frequency_to: i32,
) -> Result<(), BitaxeError> {
    logger::info(&format!("Switching frequency to {}", switch_frequency_to));
    let mut body = HashMap::new();
    body.insert("frequency", switch_frequency_to);
    let response = client
        .patch(format!("http://{}/api/system", bitaxe.host))
        .json(&body)
        .send()
        .await?;

    if response.status() == 200 {
        logger::info("Restarting!");
        client
            .post(format!("http://{}/api/system/restart", bitaxe.host))
            .send()
            .await?;
        Ok(())
    } else {
        logger::error(&format!("Something went wrong when updating {}", bitaxe.host));
        Err(BitaxeError::UpdateRejected)
    }
}
