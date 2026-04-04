use bitaxe_clocker::common::Bitaxe;

#[test]
fn test_determine_target_mode_turbo() {
    let bitaxe = Bitaxe {
        host: "192.168.8.227".to_string(),
        slow: 50,
        normal: 550,
        turbo: 590,
    };

    let mode = bitaxe_clocker::bitaxe::determine_target_mode(0.05, &bitaxe, 0.1, 0.8);
    assert_eq!(mode, 590);
}

#[test]
fn test_determine_target_mode_normal() {
    let bitaxe = Bitaxe {
        host: "192.168.8.227".to_string(),
        slow: 50,
        normal: 550,
        turbo: 590,
    };

    let mode = bitaxe_clocker::bitaxe::determine_target_mode(0.5, &bitaxe, 0.1, 0.8);
    assert_eq!(mode, 550);
}

#[test]
fn test_determine_target_mode_slow() {
    let bitaxe = Bitaxe {
        host: "192.168.8.227".to_string(),
        slow: 50,
        normal: 550,
        turbo: 590,
    };

    let mode = bitaxe_clocker::bitaxe::determine_target_mode(1.0, &bitaxe, 0.1, 0.8);
    assert_eq!(mode, 50);
}

#[test]
fn test_determine_target_mode_boundary_cheap() {
    let bitaxe = Bitaxe {
        host: "192.168.8.227".to_string(),
        slow: 50,
        normal: 550,
        turbo: 590,
    };

    // Price exactly at cheap threshold should be normal (not turbo)
    let mode = bitaxe_clocker::bitaxe::determine_target_mode(0.1, &bitaxe, 0.1, 0.8);
    assert_eq!(mode, 550);
}

#[test]
fn test_determine_target_mode_boundary_expensive() {
    let bitaxe = Bitaxe {
        host: "192.168.8.227".to_string(),
        slow: 50,
        normal: 550,
        turbo: 590,
    };

    // Price exactly at expensive threshold should be slow (not normal)
    let mode = bitaxe_clocker::bitaxe::determine_target_mode(0.8, &bitaxe, 0.1, 0.8);
    assert_eq!(mode, 50);
}

#[tokio::test]
async fn test_get_running_mode_success() {
    let fixture = std::fs::read_to_string("tests/fixtures/bitaxe_info.json").unwrap();
    let mut mock_server = mockito::Server::new_async().await;
    let _mock = mock_server
        .mock("GET", "/api/system/info")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(fixture)
        .create();

    let client = reqwest::Client::builder().build().unwrap();
    let bitaxe = Bitaxe {
        host: mock_server.url().replace("http://", ""),
        slow: 50,
        normal: 550,
        turbo: 590,
    };

    let running_mode = bitaxe_clocker::bitaxe::get_running_mode(&client, &bitaxe)
        .await
        .unwrap();
    assert_eq!(running_mode, 500);
}

#[tokio::test]
async fn test_switch_frequency_success() {
    let mut mock_server = mockito::Server::new_async().await;
    let _mock = mock_server
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
        slow: 50,
        normal: 550,
        turbo: 590,
    };

    let result = bitaxe_clocker::bitaxe::switch_frequency(&client, &bitaxe, 590).await;
    assert!(result.is_ok());
    let _ = mock_server;
    let _ = mock_restart;
}

#[tokio::test]
async fn test_switch_frequency_failure() {
    let mut mock_server = mockito::Server::new_async().await;
    let _mock = mock_server
        .mock("PATCH", "/api/system")
        .with_status(500)
        .with_header("content-type", "application/json")
        .create();

    let client = reqwest::Client::builder().build().unwrap();
    let bitaxe = Bitaxe {
        host: mock_server.url().replace("http://", ""),
        slow: 50,
        normal: 550,
        turbo: 590,
    };

    let result = bitaxe_clocker::bitaxe::switch_frequency(&client, &bitaxe, 590).await;
    // With proper error handling, should now return an error on failure
    assert!(result.is_err());
    match result {
        Err(bitaxe_clocker::bitaxe::BitaxeError::NetworkMessage(msg)) => {
            assert_eq!(msg, "Failed to update Bitaxe");
        }
        _ => panic!("Expected NetworkError variant"),
    }
}

#[cfg(test)]
mod parse_running_mode_tests {
    use serde_json::json;

    #[test]
    fn test_parse_running_mode_success() {
        let json = json!({
            "frequency": 500,
            "hashrate": 125.5,
            "temperature": 45.2
        });
        let result = bitaxe_clocker::bitaxe::parse_running_mode(&json);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 500);
    }

    #[test]
    fn test_parse_running_mode_with_fixture() {
        let fixture_content = include_str!("../../tests/fixtures/bitaxe_info.json");
        let json: serde_json::Value = serde_json::from_str(fixture_content).unwrap();
        let result = bitaxe_clocker::bitaxe::parse_running_mode(&json);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 500);
    }

    #[test]
    fn test_parse_running_mode_missing_frequency() {
        let json = json!({
            "hashrate": 125.5,
            "temperature": 45.2
        });
        let result = bitaxe_clocker::bitaxe::parse_running_mode(&json);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            bitaxe_clocker::bitaxe::BitaxeError::InvalidFrequency
        );
    }

    #[test]
    fn test_parse_running_mode_invalid_frequency_type() {
        let json = json!({
            "frequency": "not_a_number"
        });
        let result = bitaxe_clocker::bitaxe::parse_running_mode(&json);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            bitaxe_clocker::bitaxe::BitaxeError::InvalidFrequency
        );
    }

    #[test]
    fn test_parse_running_mode_zero_frequency() {
        let json = json!({
            "frequency": 0
        });
        let result = bitaxe_clocker::bitaxe::parse_running_mode(&json);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    }

    #[test]
    fn test_parse_running_mode_large_frequency() {
        let json = json!({
            "frequency": 999999
        });
        let result = bitaxe_clocker::bitaxe::parse_running_mode(&json);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 999999);
    }

    #[test]
    fn test_parse_running_mode_float_frequency() {
        let json = json!({
            "frequency": 550.7
        });
        let result = bitaxe_clocker::bitaxe::parse_running_mode(&json);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 550);
    }

    #[test]
    fn test_parse_running_mode_empty_object() {
        let json = json!({});
        let result = bitaxe_clocker::bitaxe::parse_running_mode(&json);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            bitaxe_clocker::bitaxe::BitaxeError::InvalidFrequency
        );
    }

    #[test]
    fn test_parse_running_mode_null_frequency() {
        let json = json!({
            "frequency": null
        });
        let result = bitaxe_clocker::bitaxe::parse_running_mode(&json);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            bitaxe_clocker::bitaxe::BitaxeError::InvalidFrequency
        );
    }

    #[test]
    fn test_parse_running_mode_negative_frequency() {
        let json = json!({
            "frequency": -100
        });
        let result = bitaxe_clocker::bitaxe::parse_running_mode(&json);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), -100);
    }
}
