use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Bitaxe {
    pub host: String,
    pub slow: i32,
    pub normal: i32,
    pub turbo: i32,
}
