use bitaxe_clocker::common::Bitaxe;
use bitaxe_clocker::common::Prices;

#[test]
fn test_target_frequency_cheap() {
    let bitaxe = Bitaxe {
        host: "192.168.8.227".to_string(),
        expensive: 50,
        default: 550,
        cheap: 590,
    };

    let mode = bitaxe_clocker::bitaxe::target_frequency(0.05, &bitaxe, &prices());
    assert_eq!(mode, 590);
}

#[test]
fn test_target_frequency_default() {
    let bitaxe = Bitaxe {
        host: "192.168.8.227".to_string(),
        expensive: 50,
        default: 550,
        cheap: 590,
    };

    let mode = bitaxe_clocker::bitaxe::target_frequency(0.5, &bitaxe, &prices());
    assert_eq!(mode, 550);
}

#[test]
fn test_target_frequency_expensive() {
    let bitaxe = Bitaxe {
        host: "192.168.8.227".to_string(),
        expensive: 50,
        default: 550,
        cheap: 590,
    };

    let mode = bitaxe_clocker::bitaxe::target_frequency(1.0, &bitaxe, &prices());
    assert_eq!(mode, 50);
}

#[test]
fn test_target_frequency_boundary_cheap() {
    let bitaxe = Bitaxe {
        host: "192.168.8.227".to_string(),
        expensive: 50,
        default: 550,
        cheap: 590,
    };

    // Price exactly at cheap threshold should be default (not cheap)
    let mode = bitaxe_clocker::bitaxe::target_frequency(0.1, &bitaxe, &prices());
    assert_eq!(mode, 550);
}

#[test]
fn test_target_frequency_boundary_expensive() {
    let bitaxe = Bitaxe {
        host: "192.168.8.227".to_string(),
        expensive: 50,
        default: 550,
        cheap: 590,
    };

    // Price exactly at expensive threshold should be expensive (not default)
    let mode = bitaxe_clocker::bitaxe::target_frequency(0.8, &bitaxe, &prices());
    assert_eq!(mode, 50);
}

#[tokio::test]
async fn test_running_frequency_success() {
    let fixture = std::fs::read_to_string("tests/fixtures/bitaxe_info.json").unwrap();
    let mut mock_server = mockito::Server::new_async().await;
    let mock_info = mock_server
        .mock("GET", "/api/system/info")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(fixture)
        .create();

    let client = reqwest::Client::builder().build().unwrap();
    let bitaxe = Bitaxe {
        host: mock_server.url().replace("http://", ""),
        expensive: 50,
        default: 550,
        cheap: 590,
    };

    let frequency = bitaxe_clocker::bitaxe::running_frequency(&client, &bitaxe)
        .await
        .unwrap();
    assert_eq!(frequency, 500);
    mock_info.assert();
}

#[tokio::test]
async fn network_failure_keeps_the_transport_error_as_source() {
    use std::error::Error;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()
        .unwrap();
    // Nothing is listening on port 1, so the request fails at the transport layer.
    let bitaxe = Bitaxe {
        host: "127.0.0.1:1".to_string(),
        expensive: 50,
        default: 550,
        cheap: 590,
    };

    let err = bitaxe_clocker::bitaxe::running_frequency(&client, &bitaxe)
        .await
        .unwrap_err();

    assert!(
        err.source().is_some(),
        "transport error was flattened into a string; the underlying cause is lost"
    );
}

// These three are the reason the global CONFIG had to go: `frequency_to_switch_to`
// read its thresholds out of a `OnceCell` that can only be set once per process, so it
// could not be driven from a test at all.

fn prices() -> Prices {
    Prices {
        cheap: 0.1,
        expensive: 0.8,
        default: 0.5,
    }
}

/// Stands up a device reporting `frequency` as its current running frequency.
/// Returns the mock so each test can assert the info endpoint was actually queried.
async fn device_running_at(server: &mut mockito::Server, frequency: i32) -> (Bitaxe, mockito::Mock) {
    let mock = server
        .mock("GET", "/api/system/info")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(format!(r#"{{"frequency": {frequency}}}"#))
        .create();

    let bitaxe = Bitaxe {
        host: server.url().replace("http://", ""),
        expensive: 50,
        default: 550,
        cheap: 590,
    };

    (bitaxe, mock)
}

#[tokio::test]
async fn requests_no_change_when_device_already_runs_target_frequency() {
    let mut server = mockito::Server::new_async().await;
    let (bitaxe, info) = device_running_at(&mut server, 590).await;
    let client = reqwest::Client::builder().build().unwrap();

    // 0.05 is below the cheap threshold, and this device's cheap frequency is 590 -
    // the same it is already running, so no change is wanted.
    let switch_to = bitaxe_clocker::bitaxe::frequency_to_switch_to(&client, &bitaxe, 0.05, &prices())
        .await
        .unwrap();

    assert_eq!(switch_to, None, "no restart should be requested");
    info.assert();
}

#[tokio::test]
async fn requests_the_target_frequency_when_the_device_is_running_something_else() {
    let mut server = mockito::Server::new_async().await;
    let (bitaxe, info) = device_running_at(&mut server, 500).await;
    let client = reqwest::Client::builder().build().unwrap();

    let switch_to = bitaxe_clocker::bitaxe::frequency_to_switch_to(&client, &bitaxe, 0.05, &prices())
        .await
        .unwrap();

    assert_eq!(switch_to, Some(590));
    info.assert();
}

#[tokio::test]
async fn throttles_down_when_price_reaches_the_expensive_threshold() {
    let mut server = mockito::Server::new_async().await;
    let (bitaxe, info) = device_running_at(&mut server, 550).await;
    let client = reqwest::Client::builder().build().unwrap();

    // Exactly at the expensive threshold, which the boundary tests already pin as
    // the expensive branch.
    let switch_to = bitaxe_clocker::bitaxe::frequency_to_switch_to(&client, &bitaxe, 0.8, &prices())
        .await
        .unwrap();

    assert_eq!(switch_to, Some(50));
    info.assert();
}

#[tokio::test]
async fn test_switch_frequency_success() {
    let mut mock_server = mockito::Server::new_async().await;
    let mock_patch = mock_server
        .mock("PATCH", "/api/system")
        .with_status(200)
        .with_header("content-type", "application/json")
        .create();

    let mock_restart = mock_server
        .mock("POST", "/api/system/restart")
        .with_status(200)
        .with_header("content-type", "application/json")
        .create();

    let client = reqwest::Client::builder().build().unwrap();
    let bitaxe = Bitaxe {
        host: mock_server.url().replace("http://", ""),
        expensive: 50,
        default: 550,
        cheap: 590,
    };

    let result = bitaxe_clocker::bitaxe::switch_frequency(&client, &bitaxe, 590).await;
    assert!(result.is_ok());
    mock_patch.assert();
    mock_restart.assert();
}

#[tokio::test]
async fn test_switch_frequency_failure() {
    let mut mock_server = mockito::Server::new_async().await;
    let mock_patch = mock_server
        .mock("PATCH", "/api/system")
        .with_status(500)
        .with_header("content-type", "application/json")
        .create();

    // A failed frequency update must never reach the restart endpoint.
    let mock_restart = mock_server
        .mock("POST", "/api/system/restart")
        .with_status(200)
        .expect_at_most(0)
        .create();

    let client = reqwest::Client::builder().build().unwrap();
    let bitaxe = Bitaxe {
        host: mock_server.url().replace("http://", ""),
        expensive: 50,
        default: 550,
        cheap: 590,
    };

    let result = bitaxe_clocker::bitaxe::switch_frequency(&client, &bitaxe, 590).await;
    // With proper error handling, should now return an error on failure
    assert!(result.is_err());
    assert!(
        matches!(result, Err(bitaxe_clocker::bitaxe::BitaxeError::UpdateRejected)),
        "expected the device-rejection variant"
    );
    mock_patch.assert();
    mock_restart.assert();
}

#[cfg(test)]
mod parse_running_frequency_tests {
    use serde_json::json;

    #[test]
    fn test_parse_running_frequency_success() {
        let json = json!({
            "frequency": 500,
            "hashrate": 125.5,
            "temperature": 45.2
        });
        let result = bitaxe_clocker::bitaxe::parse_running_frequency(&json);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 500);
    }

    #[test]
    fn test_parse_running_frequency_with_fixture() {
        let fixture_content = include_str!("../../tests/fixtures/bitaxe_info.json");
        let json: serde_json::Value = serde_json::from_str(fixture_content).unwrap();
        let result = bitaxe_clocker::bitaxe::parse_running_frequency(&json);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 500);
    }

    #[test]
    fn test_parse_running_frequency_missing_frequency() {
        let json = json!({
            "hashrate": 125.5,
            "temperature": 45.2
        });
        let result = bitaxe_clocker::bitaxe::parse_running_frequency(&json);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            bitaxe_clocker::bitaxe::BitaxeError::InvalidFrequency
        ));
    }

    #[test]
    fn test_parse_running_frequency_invalid_frequency_type() {
        let json = json!({
            "frequency": "not_a_number"
        });
        let result = bitaxe_clocker::bitaxe::parse_running_frequency(&json);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            bitaxe_clocker::bitaxe::BitaxeError::InvalidFrequency
        ));
    }

    #[test]
    fn test_parse_running_frequency_zero_frequency() {
        let json = json!({
            "frequency": 0
        });
        let result = bitaxe_clocker::bitaxe::parse_running_frequency(&json);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    }

    #[test]
    fn test_parse_running_frequency_large_frequency() {
        let json = json!({
            "frequency": 999999
        });
        let result = bitaxe_clocker::bitaxe::parse_running_frequency(&json);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 999999);
    }

    #[test]
    fn test_parse_running_frequency_float_frequency() {
        let json = json!({
            "frequency": 550.7
        });
        let result = bitaxe_clocker::bitaxe::parse_running_frequency(&json);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 550);
    }

    #[test]
    fn test_parse_running_frequency_empty_object() {
        let json = json!({});
        let result = bitaxe_clocker::bitaxe::parse_running_frequency(&json);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            bitaxe_clocker::bitaxe::BitaxeError::InvalidFrequency
        ));
    }

    #[test]
    fn test_parse_running_frequency_null_frequency() {
        let json = json!({
            "frequency": null
        });
        let result = bitaxe_clocker::bitaxe::parse_running_frequency(&json);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            bitaxe_clocker::bitaxe::BitaxeError::InvalidFrequency
        ));
    }

    #[test]
    fn test_parse_running_frequency_negative_frequency() {
        let json = json!({
            "frequency": -100
        });
        let result = bitaxe_clocker::bitaxe::parse_running_frequency(&json);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), -100);
    }
}
