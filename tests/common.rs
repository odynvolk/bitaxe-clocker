use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Bitaxe {
    pub host: String,
    pub expensive: i32,
    pub default: i32,
    pub cheap: i32,
}
