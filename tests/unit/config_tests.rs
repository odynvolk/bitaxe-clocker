use bitaxe_clocker::common::{parse_config, ConfigError};

fn config_with_check_interval(value: &str) -> String {
    format!(
        r#"
check_interval = {value}

[prices]
cheap = 0.1
expensive = 0.8
default = 0.5

[price_provider]
provider_type = "elpriset_just_nu"

[price_provider.elpriset_just_nu]
price_zone = "SE3"

[[bitaxes]]
host = "192.168.8.1"
expensive = 50
default = 500
cheap = 520
"#
    )
}

#[test]
fn parses_a_valid_configuration() {
    let config = parse_config(&config_with_check_interval("60")).unwrap();
    assert_eq!(config.check_interval, 60);
}

// `check_interval` used to be `i32`, so `-1` deserialised successfully and then panicked
// on the way to `Duration::from_millis`, from inside the loop, at runtime.
#[test]
fn negative_check_interval_is_rejected_by_the_parser() {
    let result = parse_config(&config_with_check_interval("-1"));
    assert!(
        matches!(result, Err(ConfigError::Parse(_))),
        "a negative interval should never reach the runtime"
    );
}

// The old arithmetic was `1000 * 60 * interval` on an `i32`, which wrapped, and a
// wrapped-negative then hit `try_into().unwrap()`. In a release build overflow wraps
// silently instead of panicking, so the old code could have slept for a wrong duration
// with nothing to indicate it.
//
// `i64::MAX` is the ceiling: TOML integers are signed 64-bit, so the parser has already
// rejected anything larger before this arithmetic is reached.
#[test]
fn absurd_check_interval_is_reported_rather_than_overflowing() {
    let config = parse_config(&config_with_check_interval(&i64::MAX.to_string())).unwrap();

    assert!(
        matches!(config.sleep_duration(), Err(ConfigError::IntervalTooLarge(_))),
        "overflow must not be reachable in release builds, where it wraps rather than panics"
    );
}

#[test]
fn converts_minutes_to_a_duration() {
    let config = parse_config(&config_with_check_interval("2")).unwrap();
    assert_eq!(config.sleep_duration().unwrap(), std::time::Duration::from_secs(120));
}
