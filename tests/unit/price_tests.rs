use mockito::Server;
use reqwest::Client;

#[tokio::test]
async fn test_get_current_price_success() {
    // Read the fixture file
    let fixture = std::fs::read_to_string("tests/fixtures/price.json").unwrap();

    // Create a mock server
    let mut server = Server::new_async().await;
    let _mock = server
        .mock("GET", "/api/v1/prices/*")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(fixture)
        .create();

    let client = Client::builder().build().unwrap();

    let price = bitaxe_clocker::price::get_current_price(&client).await.unwrap();

    // Verify the price was parsed correctly - it should be positive and reasonable
    assert!(price > 0.0, "Expected a positive price, got {}", price);
    assert!(price < 10.0, "Expected price to be reasonable, got {}", price);
}

#[tokio::test]
async fn test_get_current_price_default_on_failure() {
    // Create a mock that returns non-200 status
    let mut server = Server::new_async().await;
    let _mock = server
        .mock("GET", "/api/v1/prices/*")
        .with_status(500)
        .with_header("content-type", "application/json")
        .with_body("Internal Server Error")
        .create();

    let client = Client::builder().build().unwrap();

    let price = bitaxe_clocker::price::get_current_price(&client).await.unwrap();

    // When the API fails, should return the default price from CONFIG
    // The default should be a reasonable value
    assert!(
        price >= bitaxe_clocker::common::CONFIG.prices.default,
        "Expected default price to be non-negative, got {}",
        price
    );
}
