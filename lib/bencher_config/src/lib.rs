use std::sync::LazyLock;
use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    num::NonZeroU32,
    ops::Deref,
};

use bencher_json::{
    BENCHER_API_PORT, JsonConfig, Sanitize as _, sanitize_json,
    system::config::{
        JsonConsole, JsonDatabase, JsonLogging, JsonSecurity, JsonServer, LogLevel, ServerLog,
    },
};
use bencher_token::DEFAULT_SECRET_KEY;
use slog::{Logger, error, info};
use url::Url;

mod config_tx;
mod plus;

pub use config_tx::{ConfigTx, ConfigTxError};
#[cfg(feature = "plus")]
pub use plus::{Plus, PlusError};

pub const API_NAME: &str = "Bencher API";

pub const BENCHER_CONFIG: &str = "BENCHER_CONFIG";
pub const BENCHER_CONFIG_PATH: &str = "BENCHER_CONFIG_PATH";

#[cfg(debug_assertions)]
const DEFAULT_CONFIG_PATH: &str = "etc/bencher.json";
#[cfg(not(debug_assertions))]
const DEFAULT_CONFIG_PATH: &str = "/etc/bencher/bencher.json";
const DEFAULT_IP: IpAddr = IpAddr::V4(Ipv4Addr::UNSPECIFIED);

// 4 mebibytes or 4_194_304 bytes (OCI distribution-spec recommended manifest limit)
pub const DEFAULT_MAX_BODY_SIZE: usize = 1 << 22;
#[cfg(debug_assertions)]
const DEFAULT_DB_PATH: &str = "data/bencher.db";
#[cfg(not(debug_assertions))]
const DEFAULT_DB_PATH: &str = "/var/lib/bencher/data/bencher.db";
pub const DEFAULT_SMTP_PORT: u16 = 587;

#[cfg(debug_assertions)]
const DEFAULT_LOG_LEVEL: LogLevel = LogLevel::Debug;
#[cfg(not(debug_assertions))]
const DEFAULT_LOG_LEVEL: LogLevel = LogLevel::Info;

const DEFAULT_BUSY_TIMEOUT: u32 = 5_000;

// 64 MiB, in KiB. Large report deletions and ingests dirty far more pages
// than the SQLite default (2 MiB); a bigger writer cache avoids re-reading
// evicted pages mid-operation.
const DEFAULT_CACHE_SIZE: NonZeroU32 = match NonZeroU32::new(0x1_0000) {
    Some(cache_size) => cache_size,
    None => panic!("default cache size is zero"),
};

const DEFAULT_CONSOLE_URL_STR: &str = "http://localhost:3000";
#[expect(clippy::panic, reason = "compile-time constant URL must be valid")]
static DEFAULT_CONSOLE_URL: LazyLock<Url> = LazyLock::new(|| {
    DEFAULT_CONSOLE_URL_STR.parse().unwrap_or_else(|e| {
        panic!("Failed to parse default console URL \"{DEFAULT_CONSOLE_URL_STR}\": {e}")
    })
});

static DEFAULT_BIND_ADDRESS: LazyLock<SocketAddr> =
    LazyLock::new(|| SocketAddr::new(DEFAULT_IP, BENCHER_API_PORT));

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
                info!(
                    log,
                    "Failed to find \"{BENCHER_CONFIG_PATH}\" environment variable defaulting to \"{DEFAULT_CONFIG_PATH}\": {e}"
                );
                let config_file = match tokio::fs::read(DEFAULT_CONFIG_PATH).await {
                    Ok(config_file) => config_file,
                    Err(e) => {
                        info!(
                            log,
                            "Failed to open config file at default path \"{DEFAULT_CONFIG_PATH}\": {e}"
                        );
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

    /// Consume the config and return it with every secret masked.
    ///
    /// Used by the `GET /v0/server/config` endpoint so an admin can inspect the
    /// server configuration without exposing plaintext secrets (security,
    /// database, SMTP, OAuth, replica, and Litestream credentials). Unlike
    /// [`sanitize_json`], this masks unconditionally in both debug and release
    /// builds.
    pub fn sanitized(self) -> JsonConfig {
        let mut json = self.0;
        json.sanitize();
        json
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
                busy_timeout: None,
                cache_size: None,
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

#[cfg(test)]
mod tests {
    use bencher_json::{
        Sanitize as _, Secret,
        system::config::{DataStore, JsonSmtp},
    };

    use super::Config;

    // Mirror `bencher_valid::secret::SANITIZED_SECRET` (not publicly re-exported)
    // by deriving the mask at runtime, so this test tracks the crate's mask
    // instead of duplicating the literal.
    fn mask() -> String {
        let mut secret = secret("placeholder");
        secret.sanitize();
        String::from(secret)
    }

    fn secret(value: &str) -> Secret {
        value.parse().expect("valid secret")
    }

    // `GET /v0/server/config` must never leak plaintext secrets. Sanitizing the
    // base (non-plus) config masks the security, database, and SMTP secrets.
    #[test]
    fn sanitized_masks_base_secrets() {
        let mut config = Config::default();
        config.0.security.secret_key = secret("PLAINTEXT_SECURITY");
        config.0.database.data_store = Some(DataStore::AwsS3 {
            access_key_id: "access-key-id".to_owned(),
            secret_access_key: secret("PLAINTEXT_DB"),
            access_point: "access-point".to_owned(),
        });
        config.0.smtp = Some(JsonSmtp {
            hostname: "smtp.example.com".parse().unwrap(),
            port: None,
            insecure_host: None,
            starttls: None,
            username: "smtp-user".parse().unwrap(),
            secret: secret("PLAINTEXT_SMTP"),
            from_name: "Bencher".parse().unwrap(),
            from_email: "bencher@example.com".parse().unwrap(),
        });

        let json = config.sanitized();
        let serialized = serde_json::to_string(&json).unwrap();

        for plaintext in ["PLAINTEXT_SECURITY", "PLAINTEXT_DB", "PLAINTEXT_SMTP"] {
            assert!(
                !serialized.contains(plaintext),
                "leaked {plaintext}: {serialized}"
            );
        }
        assert!(
            serialized.contains(&mask()),
            "no mask present: {serialized}"
        );
    }

    // The plus config carries the OAuth, Litestream, and replica secrets flagged
    // in the review. Sanitizing must mask every one of them.
    #[cfg(feature = "plus")]
    #[test]
    fn sanitized_masks_plus_secrets() {
        use bencher_json::system::config::{
            JsonGitHub, JsonGoogle, JsonLitestream, JsonPlus, JsonReplica, JsonReplication,
            ReplicationTarget,
        };

        let mut config = Config::default();
        config.0.plus = Some(JsonPlus {
            rate_limiting: None,
            github: Some(JsonGitHub {
                client_id: "github-client-id".parse().unwrap(),
                client_secret: secret("PLAINTEXT_GITHUB"),
            }),
            google: Some(JsonGoogle {
                client_id: "google-client-id".parse().unwrap(),
                client_secret: secret("PLAINTEXT_GOOGLE"),
            }),
            litestream: Some(JsonLitestream {
                replica: JsonReplica::S3 {
                    bucket: "litestream-bucket".to_owned(),
                    path: None,
                    endpoint: None,
                    region: None,
                    access_key_id: "access-key-id".to_owned(),
                    secret_access_key: secret("PLAINTEXT_LITESTREAM"),
                    sync_interval: None,
                },
                snapshot: None,
                validation: None,
                checkpoint: None,
                metrics_port: None,
            }),
            replica: Some(JsonReplication {
                target: ReplicationTarget::S3 {
                    bucket: "replica-bucket".to_owned(),
                    path: None,
                    endpoint: None,
                    region: None,
                    access_key_id: "access-key-id".to_owned(),
                    secret_access_key: secret("PLAINTEXT_REPLICA"),
                },
                sync_interval_secs: None,
                checkpoint_interval_secs: None,
                min_checkpoint_pages: None,
                snapshot_interval_secs: None,
                snapshot_throttle_mib: None,
                retention_generations: None,
                verification_interval_secs: None,
                shutdown_sync_timeout_secs: None,
            }),
            stats: None,
            cloud: None,
            registry: None,
            runners: None,
        });

        let json = config.sanitized();
        let serialized = serde_json::to_string(&json).unwrap();

        for plaintext in [
            "PLAINTEXT_GITHUB",
            "PLAINTEXT_GOOGLE",
            "PLAINTEXT_LITESTREAM",
            "PLAINTEXT_REPLICA",
        ] {
            assert!(
                !serialized.contains(plaintext),
                "leaked {plaintext}: {serialized}"
            );
        }
        assert!(
            serialized.contains(&mask()),
            "no mask present: {serialized}"
        );
    }
}
