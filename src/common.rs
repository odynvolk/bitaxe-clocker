use serde::Deserialize;
use std::fs::File;
use std::io::Read;
use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error(
        "no config.toml in the working directory.\n\
         Local: copy config.example.toml to config.toml and edit it.\n\
         Docker: mount one, e.g. -v ./config.toml:/app/config.toml:ro (docker compose already does)"
    )]
    NotFound,

    #[error(
        "config.toml is a directory, not a file.\n\
         A Docker bind mount does this when the host path does not exist - it creates the \
         directory rather than failing. Create ./config.toml on the host, then re-run."
    )]
    IsADirectory,

    #[error("failed to read config.toml: {0}")]
    Read(#[from] std::io::Error),

    #[error("invalid config.toml: {0}")]
    Parse(#[from] toml::de::Error),

    #[error("unknown price provider: {0}")]
    UnknownProvider(String),

    #[error("price provider '{0}' requires a [price_provider.{0}] section")]
    MissingProviderSection(String),

    #[error("check_interval of {0} minutes is too large to schedule")]
    IntervalTooLarge(u64),
}

/// Turn an io failure into one an operator can act on. The two kinds worth naming both
/// arise from the config simply not being there, but only one of them looks like it: a
/// missing bind-mount source becomes a directory, which Docker creates without complaining.
///
/// Applied to both the open and the read, because which stage reports a failure is
/// platform-dependent - opening a directory succeeds on both platforms tested and the
/// `EISDIR` surfaces from `read`. Attaching this to the open call alone was verified
/// end-to-end to leave `Is a directory` unmapped.
fn classify_io_error(err: std::io::Error) -> ConfigError {
    match err.kind() {
        std::io::ErrorKind::NotFound => ConfigError::NotFound,
        std::io::ErrorKind::IsADirectory => ConfigError::IsADirectory,
        _ => ConfigError::Read(err),
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    /// Minutes between price checks and frequency updates.
    pub check_interval: u64,
    pub prices: Prices,
    pub price_provider: PriceProviderConfig,
    pub bitaxes: Vec<Bitaxe>,
}

impl Config {
    /// How long to wait between checks. Total: the arithmetic that used to live in
    /// `main` could overflow, and in a release build overflow wraps rather than
    /// panics, so a bad value would have produced a silently wrong sleep.
    pub fn sleep_duration(&self) -> Result<Duration, ConfigError> {
        let seconds = self
            .check_interval
            .checked_mul(60)
            .ok_or(ConfigError::IntervalTooLarge(self.check_interval))?;

        Ok(Duration::from_secs(seconds))
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct PriceProviderConfig {
    pub provider_type: String,
    #[serde(default)]
    pub elpriset_just_nu: Option<crate::price_providers::ElPrisetJustNuConfig>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Bitaxe {
    pub host: String,
    pub expensive: i32,
    pub default: i32,
    pub cheap: i32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Prices {
    pub cheap: f64,
    pub expensive: f64,
    pub default: f64,
}

/// The image ships no config; this file must exist. It is a runtime default rather
/// than a search path, so a missing mount fails at startup instead of quietly using
/// someone else's hosts - which for this application means reconfiguring devices
/// nobody pointed it at.
const CONFIG_PATH: &str = "config.toml";

/// Parses configuration text. Kept separate from `load_config` so the rules can be
/// tested without a file on disk at a fixed path.
pub fn parse_config(contents: &str) -> Result<Config, ConfigError> {
    let config: Config = toml::from_str(contents)?;
    Ok(config)
}

pub fn load_config() -> Result<Config, ConfigError> {
    let config = load_config_from(CONFIG_PATH)?;
    tracing::info!(config = ?config, "Loaded config");

    Ok(config)
}

/// Read and parse a named file. Split out from `load_config` so the path handling can be
/// tested by passing a path, rather than by calling `set_current_dir` - the working
/// directory is process-wide and these tests run on parallel threads.
pub fn load_config_from(path: &str) -> Result<Config, ConfigError> {
    let mut file = File::open(path).map_err(classify_io_error)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents).map_err(classify_io_error)?;

    parse_config(&contents)
}

#[cfg(test)]
mod tests {
    use super::*;

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

    /// The image ships `config.example.toml` as `/app/config.toml`, so this file is a
    /// runtime default, not documentation. Without a mounted config the container parses
    /// this one, and until now nothing checked that it parses.
    #[test]
    fn example_config_shipped_in_the_docker_image_is_valid() {
        let example = include_str!("../config.example.toml");

        parse_config(example).expect("config.example.toml must be loadable");
    }

    fn io_err(kind: std::io::ErrorKind) -> std::io::Error {
        kind.into()
    }

    #[test]
    fn missing_config_file_names_the_fix_rather_than_the_errno() {
        assert!(matches!(
            classify_io_error(io_err(std::io::ErrorKind::NotFound)),
            ConfigError::NotFound
        ));
    }

    /// The trap this exists for: a Docker bind mount whose host path is absent is not
    /// reported by Docker, which creates the directory and starts regardless.
    #[test]
    fn config_that_is_a_directory_explains_the_bind_mount_trap() {
        assert!(matches!(
            classify_io_error(io_err(std::io::ErrorKind::IsADirectory)),
            ConfigError::IsADirectory
        ));
    }

    #[test]
    fn other_io_failures_keep_the_underlying_error_as_source() {
        match classify_io_error(io_err(std::io::ErrorKind::PermissionDenied)) {
            ConfigError::Read(err) => {
                assert_eq!(err.kind(), std::io::ErrorKind::PermissionDenied)
            }
            other => panic!("expected the io error to be retained, got {other:?}"),
        }
    }

    #[test]
    fn a_missing_path_reports_the_missing_config_message() {
        let outcome = load_config_from("/nonexistent/bitaxe-clocker-absent-config.toml");
        assert!(
            matches!(outcome, Err(ConfigError::NotFound)),
            "expected the missing-config message, got {outcome:?}"
        );
    }

    /// End-to-end counterpart of the mapping test above, and the one that actually matters:
    /// a real directory passed as the config path must reach the bind-mount message. The
    /// mapping test alone let a wrong wiring through, because the classifier was attached
    /// to `File::open`, which succeeds on a directory - the error comes from `read`.
    #[test]
    fn a_directory_as_config_reaches_the_bind_mount_message() {
        let dir = std::env::temp_dir().join(format!("bitaxe-clocker-eisdir-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("could not create a scratch directory");

        let outcome = load_config_from(&dir.to_string_lossy());

        let _ = std::fs::remove_dir_all(&dir);

        assert!(
            matches!(outcome, Err(ConfigError::IsADirectory)),
            "expected the bind-mount message, got {outcome:?}"
        );
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
}
