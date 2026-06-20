use serde_json::Value;
use std::collections::HashMap;
use std::fmt;

use crate::common;
use crate::logger;

#[derive(Debug)]
pub enum BitaxeError {
    NetworkError(reqwest::Error),
    NetworkMessage(String),
    JsonError(serde_json::Error),
    InvalidFrequency,
}

impl fmt::Display for BitaxeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            BitaxeError::NetworkError(e) => write!(f, "Network error: {}", e),
            BitaxeError::NetworkMessage(msg) => write!(f, "Network error: {}", msg),
            BitaxeError::JsonError(e) => write!(f, "JSON parsing error: {}", e),
            BitaxeError::InvalidFrequency => write!(f, "Invalid frequency value"),
        }
    }
}

impl PartialEq for BitaxeError {
    fn eq(&self, other: &Self) -> bool {
        use BitaxeError::*;
        match (self, other) {
            (NetworkError(_), NetworkError(_)) => true,
            (NetworkMessage(a), NetworkMessage(b)) => a == b,
            (JsonError(_), JsonError(_)) => true,
            (InvalidFrequency, InvalidFrequency) => true,
            _ => false,
        }
    }
}

impl std::error::Error for BitaxeError {}

impl From<reqwest::Error> for BitaxeError {
    fn from(err: reqwest::Error) -> Self {
        BitaxeError::NetworkError(err)
    }
}

impl From<serde_json::Error> for BitaxeError {
    fn from(err: serde_json::Error) -> Self {
        BitaxeError::JsonError(err)
    }
}

impl From<std::io::Error> for BitaxeError {
    fn from(err: std::io::Error) -> Self {
        BitaxeError::NetworkMessage(format!("IO error: {}", err))
    }
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
        Err(BitaxeError::NetworkMessage("Failed to update Bitaxe".to_string()))
    }
}
