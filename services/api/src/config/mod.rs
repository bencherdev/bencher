use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    ops::Deref,
};

use bencher_json::{
    sanitize_json,
    system::config::{
        JsonConsole, JsonDatabase, JsonLogging, JsonSecurity, JsonServer, LogLevel, ServerLog,
    },
    JsonConfig, BENCHER_API_PORT,
};
use bencher_token::DEFAULT_SECRET_KEY;
use once_cell::sync::Lazy;
use slog::{error, info, Logger};
use url::Url;

pub mod config_tx;
#[cfg(feature = "plus")]
pub mod plus;

pub const API_NAME: &str = "Bencher API";

pub const BENCHER_CONFIG: &str = "BENCHER_CONFIG";
pub const BENCHER_CONFIG_PATH: &str = "BENCHER_CONFIG_PATH";

#[cfg(debug_assertions)]
const DEFAULT_CONFIG_PATH: &str = "bencher.json";
#[cfg(not(debug_assertions))]
const DEFAULT_CONFIG_PATH: &str = "/etc/bencher/bencher.json";
const DEFAULT_IP: IpAddr = IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0));

// 1 megabyte or 1_048_576 bytes
const DEFAULT_MAX_BODY_SIZE: usize = 2 << 19;
#[cfg(debug_assertions)]
const DEFAULT_DB_PATH: &str = "data/bencher.db";
#[cfg(not(debug_assertions))]
const DEFAULT_DB_PATH: &str = "/var/lib/bencher/data/bencher.db";
const DEFAULT_SMTP_PORT: u16 = 587;

#[cfg(debug_assertions)]
const DEFAULT_LOG_LEVEL: LogLevel = LogLevel::Debug;
#[cfg(not(debug_assertions))]
const DEFAULT_LOG_LEVEL: LogLevel = LogLevel::Info;

const DEFAULT_CONSOLE_URL_STR: &str = "http://localhost:3000";
#[allow(clippy::panic)]
static DEFAULT_CONSOLE_URL: Lazy<Url> = Lazy::new(|| {
    DEFAULT_CONSOLE_URL_STR.parse().unwrap_or_else(|e| {
        panic!("Failed to parse default console URL \"{DEFAULT_CONSOLE_URL_STR}\": {e}")
    })
});

static DEFAULT_BIND_ADDRESS: Lazy<SocketAddr> =
    Lazy::new(|| SocketAddr::new(DEFAULT_IP, BENCHER_API_PORT));

#[derive(Debug, Clone)]
pub struct Config(JsonConfig);

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Failed to parse config string: {0}")]
    ParseStr(String),
    #[error("Failed to open config file ({0}): {1}")]
    OpenFile(String, std::io::Error),
    #[error("Failed to parse config file ({0}): {1}")]
    ParseFile(String, serde_json::Error),
    #[error("Failed to write config file ({0}): {1}")]
    WriteFile(String, std::io::Error),
    #[error("Failed to parse default config ({0:?}): {1}")]
    ParseDefault(Box<JsonConfig>, serde_json::Error),
}

impl Config {
    pub async fn load_or_default(log: &Logger) -> Result<Self, ConfigError> {
        if let Some(config) = Self::load_env(log).await? {
            return Ok(config);
        }

        if let Some(config) = Self::load_file(log).await? {
            return Ok(config);
        }

        let config = Self::default();
        info!(log, "Using default config: {}", sanitize_json(&config.0));
        let config_str = if cfg!(debug_assertions) {
            serde_json::to_string_pretty(&config.0)
        } else {
            serde_json::to_string(&config.0)
        }
        .map_err(|e| {
            let err = ConfigError::ParseDefault(Box::new(config.0.clone()), e);
            error!(log, "{err}");
            debug_assert!(false, "{err}");
            err
        })?;
        Self::write(log, config_str.as_bytes()).await?;

        Ok(config)
    }

    pub async fn load_env(log: &Logger) -> Result<Option<Self>, ConfigError> {
        // If the env var is set then failing to read or parse the config is an error
        // However, if it isn't set then just return None
        let config_str = match std::env::var(BENCHER_CONFIG) {
            Ok(config_str) => config_str,
            Err(e) => {
                info!(
                    log,
                    "Failed to find \"{BENCHER_CONFIG}\" environment variable: {e}"
                );
                return Ok(None);
            },
        };

        let json_config = serde_json::from_str(&config_str).map_err(|e| {
            error!(
                log,
                "Failed to parse config string from \"{BENCHER_CONFIG}\": {e}"
            );
            ConfigError::ParseStr(config_str.clone())
        })?;
        info!(
            log,
            "Loaded config from env var \"{BENCHER_CONFIG}\": {}",
            sanitize_json(&json_config)
        );

        #[cfg(debug_assertions)]
        Self::write(log, config_str.as_bytes()).await?;

        Ok(Some(Self(json_config)))
    }

    pub async fn load_file(log: &Logger) -> Result<Option<Self>, ConfigError> {
        // If the env var is set then failing to read or parse the config is an error
        // However, if it isn't set then just try the default path
        // If there is a file to read at the default path, then that config is expected to parse
        // Otherwise, just return None if there is no file to read a the default path
        let (path, config_file) = match std::env::var(BENCHER_CONFIG_PATH) {
            Ok(path) => {
                let config_file = tokio::fs::read(&path).await.map_err(|e| {
                    error!(log, "Failed to open config file at {path}: {e}");
                    ConfigError::OpenFile(path.clone(), e)
                })?;
                (path, config_file)
            },
            Err(e) => {
                info!(log, "Failed to find \"{BENCHER_CONFIG_PATH}\" environment variable defaulting to \"{DEFAULT_CONFIG_PATH}\": {e}");
                let config_file = match tokio::fs::read(DEFAULT_CONFIG_PATH).await {
                    Ok(config_file) => config_file,
                    Err(e) => {
                        info!(log, "Failed to open config file at default path \"{DEFAULT_CONFIG_PATH}\": {e}");
                        return Ok(None);
                    },
                };
                (DEFAULT_CONFIG_PATH.into(), config_file)
            },
        };

        let json_config = serde_json::from_slice(&config_file).map_err(|e| {
            error!(log, "Failed to parse config file at {path}: {e}");
            ConfigError::ParseFile(path.clone(), e)
        })?;
        info!(
            log,
            "Loaded config from file {path}: {}",
            sanitize_json(&json_config)
        );

        Ok(Some(Self(json_config)))
    }

    pub async fn write<C>(log: &Logger, config: C) -> Result<(), ConfigError>
    where
        C: AsRef<[u8]>,
    {
        let path = std::env::var(BENCHER_CONFIG_PATH).unwrap_or_else(|e| {
            info!(log, "Failed to find \"{BENCHER_CONFIG_PATH}\" environment variable defaulting to \"{DEFAULT_CONFIG_PATH}\": {e}");
            DEFAULT_CONFIG_PATH.into()
        });

        tokio::fs::write(&path, config).await.map_err(|e| {
            error!(log, "Failed to write config file at {path}: {e}");
            ConfigError::WriteFile(path, e)
        })
    }

    pub fn into_inner(self) -> JsonConfig {
        self.0
    }
}

impl Default for Config {
    fn default() -> Self {
        Self(JsonConfig {
            console: JsonConsole {
                url: DEFAULT_CONSOLE_URL.clone().into(),
            },
            security: JsonSecurity {
                issuer: Some(DEFAULT_CONSOLE_URL.to_string()),
                secret_key: DEFAULT_SECRET_KEY.clone(),
            },
            server: JsonServer {
                bind_address: *DEFAULT_BIND_ADDRESS,
                request_body_max_bytes: DEFAULT_MAX_BODY_SIZE,
                tls: None,
            },
            database: JsonDatabase {
                file: DEFAULT_DB_PATH.into(),
                data_store: None,
            },
            smtp: None,
            logging: JsonLogging {
                name: API_NAME.into(),
                log: ServerLog::StderrTerminal {
                    level: DEFAULT_LOG_LEVEL,
                },
            },
            #[cfg(feature = "plus")]
            plus: None,
        })
    }
}

impl From<Config> for JsonConfig {
    fn from(config: Config) -> Self {
        config.0
    }
}

impl Deref for Config {
    type Target = JsonConfig;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
