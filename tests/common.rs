use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Bitaxe {
    #[allow(dead_code)]
    pub host: String,
    #[allow(dead_code)]
    pub slow: i32,
    #[allow(dead_code)]
    pub normal: i32,
    #[allow(dead_code)]
    pub turbo: i32,
}
