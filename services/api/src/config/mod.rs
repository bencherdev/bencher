use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::PathBuf,
};

use bencher_json::{
    config::{JsonDatabase, JsonLogging, JsonServer, LogLevel, ServerLog},
    JsonConfig,
};
use tracing::info;
use url::Url;

use crate::ApiError;

mod server;

pub const API_NAME: &str = "Bencher API";

pub const BENCHER_CONFIG_PATH: &str = "BENCHER_CONFIG_PATH";

const DEFAULT_CONFIG_PATH: &str = "bencher.json";
#[cfg(debug_assertions)]
const DEFAULT_ENDPOINT_STR: &str = "http://localhost:3000";
#[cfg(not(debug_assertions))]
const DEFAULT_ENDPOINT_STR: &str = "https://bencher.dev";
// Dynamic and/or Private Ports (49152-65535)
// https://www.iana.org/assignments/service-names-port-numbers/service-names-port-numbers.xhtml?search=61016
const DEFAULT_PORT: u16 = 61016;
// 1 megabyte or 1_048_576 bytes
const DEFAULT_MAX_BODY_SIZE: usize = 1 << 20;
const DEFAULT_DB_PATH: &str = "data/bencher.db";

lazy_static::lazy_static! {
    static ref DEFAULT_ENDPOINT: Url = DEFAULT_ENDPOINT_STR.parse().expect(&format!("Failed to parse default endpoint: {DEFAULT_ENDPOINT_STR}"));
    static ref DEFAULT_BIND_ADDRESS: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), DEFAULT_PORT);
}

#[cfg(debug_assertions)]
lazy_static::lazy_static! {
    static ref DEFAULT_SECRET_KEY: String = "DO_NOT_USE_THIS_IN_PRODUCTION".into();
}
#[cfg(not(debug_assertions))]
lazy_static::lazy_static! {
    static ref DEFAULT_SECRET_KEY: String = uuid::Uuid::new_v4().to_string();
}

#[derive(Debug, Clone)]
pub struct Config(pub JsonConfig);

impl Config {
    pub fn path() -> PathBuf {
        std::env::var(BENCHER_CONFIG_PATH)
            .unwrap_or_else(|e| {
                info!("Failed to find \"{BENCHER_CONFIG_PATH}\": {e}");
                info!("Defaulting \"{BENCHER_CONFIG_PATH}\" to: {DEFAULT_CONFIG_PATH}");
                DEFAULT_CONFIG_PATH.into()
            })
            .into()
    }

    pub async fn load() -> Result<Self, ApiError> {
        let path = Self::path();

        let config_file = tokio::fs::read(&path).await.map_err(|e| {
            info!("Failed to open config file at {}: {e}", path.display());
            ApiError::OpenConfig(path.clone())
        })?;

        Ok(Self(serde_json::from_slice(&config_file).map_err(|e| {
            info!("Failed to parse config file at {}: {e}", path.display());
            ApiError::ParseConfig(path)
        })?))
    }

    pub async fn load_or_default() -> Self {
        Self::load().await.unwrap_or_default()
    }
}

impl Default for Config {
    fn default() -> Self {
        Self(JsonConfig {
            endpoint: DEFAULT_ENDPOINT.clone(),
            secret_key: Some(DEFAULT_SECRET_KEY.clone()),
            server: JsonServer {
                bind_address: *DEFAULT_BIND_ADDRESS,
                request_body_max_bytes: DEFAULT_MAX_BODY_SIZE,
                tls: None,
            },
            database: JsonDatabase {
                file: DEFAULT_DB_PATH.into(),
            },
            smtp: None,
            logging: JsonLogging {
                name: API_NAME.into(),
                log: ServerLog::StderrTerminal {
                    level: LogLevel::Info,
                },
            },
        })
    }
}
