use std::{fmt, ops::Deref, time::Duration};

use bencher_json::{BENCHER_API_URL, BENCHER_URL, JsonApiVersion, JsonConsole};
use serde::{Serialize, de::DeserializeOwned};

use crate::{CLI_VERSION, cli_eprintln_quietable, parser::CliBackend};

#[derive(Debug, Clone)]
pub struct PubBackend {
    inner: Backend,
}

#[derive(Debug, Clone)]
pub struct AuthBackend {
    inner: Backend,
}

#[derive(Debug, Clone)]
pub struct Backend {
    client: bencher_client::BencherClient,
}

#[derive(Clone, Copy)]
enum BackendKind {
    Pub,
    Auth,
}

#[derive(thiserror::Error, Debug)]
pub enum BackendError {
    #[error("Failed to parse host URL: {0}")]
    ParseHost(bencher_json::ValidError),
    #[error("Failed to parse API token: {0}")]
    ParseToken(bencher_json::ValidError),
    #[error(
        "Failed to find Bencher API credential, and this API endpoint requires authorization. Set `--token`/`BENCHER_API_TOKEN` or `--key`/`BENCHER_API_KEY`."
    )]
    NoCredential,
    #[error("Failed to get API server version: {0}")]
    ApiVersion(bencher_client::ClientError),
    #[error("{err}\nHint: This may be due to a version mismatch. {mismatch}")]
    ClientMismatch {
        mismatch: Box<VersionMismatch>,
        err: bencher_client::ClientError,
    },
    #[error("{0}")]
    Client(#[from] bencher_client::ClientError),
    #[error("Invalid console URL: {0}")]
    BadConsoleUrl(bencher_json::ValidError),
}

impl TryFrom<CliBackend> for PubBackend {
    type Error = BackendError;

    fn try_from(backend: CliBackend) -> Result<Self, Self::Error> {
        (backend, BackendKind::Pub)
            .try_into()
            .map(|inner| Self { inner })
    }
}

impl TryFrom<CliBackend> for AuthBackend {
    type Error = BackendError;

    fn try_from(backend: CliBackend) -> Result<Self, Self::Error> {
        (backend, BackendKind::Auth)
            .try_into()
            .map(|inner| Self { inner })
    }
}

impl TryFrom<(CliBackend, BackendKind)> for Backend {
    type Error = BackendError;

    fn try_from((backend, kind): (CliBackend, BackendKind)) -> Result<Self, Self::Error> {
        let CliBackend {
            host,
            token,
            key,
            insecure_host,
            native_tls,
            timeout,
            attempts,
            retry_after,
            max_retry_after,
            strict,
        } = backend;
        let host = host.try_into().map_err(BackendError::ParseHost)?;
        let mut builder = bencher_client::BencherClient::builder()
            .host(host)
            .insecure_host(insecure_host)
            .native_tls(native_tls)
            .timeout(Duration::from_secs(timeout.into()))
            .attempts(attempts.into())
            .retry_after(retry_after.into())
            .max_retry_after(max_retry_after.into())
            .strict(strict)
            .log(true);

        // `--key` takes precedence: if a key is supplied, send it as the bearer.
        // Otherwise fall back to the JWT `--token`. The CLI's `bencher_credential`
        // ArgGroup already enforces that the two are mutually exclusive, so at
        // most one is set in practice.
        if let Some(key) = key {
            builder = builder.key(key);
        } else if let Some(token) = token {
            builder = builder.token(token);
        } else if matches!(kind, BackendKind::Auth) {
            return Err(BackendError::NoCredential);
        }

        Ok(Self {
            client: builder.build(),
        })
    }
}

impl Deref for PubBackend {
    type Target = Backend;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl PubBackend {
    pub fn log(mut self, log: bool) -> Self {
        self.inner.client.log = log;
        self
    }
}

impl Deref for AuthBackend {
    type Target = Backend;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl AuthBackend {
    pub fn log(mut self, log: bool) -> Self {
        self.inner.client.log = log;
        self
    }
}

impl Backend {
    pub async fn send<F, R, T, E>(&self, sender: F) -> Result<serde_json::Value, BackendError>
    where
        F: Fn(bencher_client::Client) -> R,
        R: Future<
            Output = Result<
                progenitor_client::ResponseValue<T>,
                bencher_client::Error<bencher_client::types::Error>,
            >,
        >,
        T: Serialize,
        E: std::error::Error + Send + Sync + 'static,
        bencher_client::JsonValue: TryFrom<T, Error = E>,
    {
        let mismatch = self.check_version().await?;
        self.client.send(sender).await.map_err(|err| {
            if let Some(mismatch) = mismatch {
                BackendError::ClientMismatch {
                    mismatch: Box::new(mismatch),
                    err,
                }
            } else {
                err.into()
            }
        })
    }

    pub async fn send_with<F, R, T, Json, E>(&self, sender: F) -> Result<Json, BackendError>
    where
        F: Fn(bencher_client::Client) -> R,
        R: Future<
            Output = Result<
                progenitor_client::ResponseValue<T>,
                bencher_client::Error<bencher_client::types::Error>,
            >,
        >,
        T: Serialize,
        Json: DeserializeOwned + Serialize + TryFrom<T, Error = E>,
        E: std::error::Error + Send + Sync + 'static,
    {
        let mismatch = self.check_version().await?;
        self.client.send_with(sender).await.map_err(|err| {
            if let Some(mismatch) = mismatch {
                BackendError::ClientMismatch {
                    mismatch: Box::new(mismatch),
                    err,
                }
            } else {
                err.into()
            }
        })
    }

    pub async fn check_version(&self) -> Result<Option<VersionMismatch>, BackendError> {
        let json_api_version: JsonApiVersion = self
            .client
            .clone()
            .into_builder()
            .log(false)
            .build()
            .send_with(|client| async move { client.server_version_get().send().await })
            .await
            .map_err(BackendError::ApiVersion)?;
        let api_version = json_api_version.version;
        let mismatch = VersionMismatch::check(&self.client.host, api_version);
        if let Some(mismatch) = &mismatch {
            cli_eprintln_quietable!(self.client.log, "Warning: {mismatch}",);
        }
        Ok(mismatch)
    }

    pub async fn get_console_url(&self) -> Result<url::Url, BackendError> {
        if self.client.host == *BENCHER_API_URL {
            return Ok(BENCHER_URL.clone());
        }

        let json_console: JsonConsole = self
            .send_with(|client| async move { client.server_config_console_get().send().await })
            .await?;
        json_console
            .url
            .try_into()
            .map_err(BackendError::BadConsoleUrl)
    }
}

#[derive(Debug)]
pub struct VersionMismatch {
    pub host: url::Url,
    pub api_version: String,
    pub cli_version: String,
}

impl fmt::Display for VersionMismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "The Bencher API server version is {}, but this CLI version is {}.\n{}",
            self.api_version,
            self.cli_version,
            if self.host == *BENCHER_API_URL {
                "You should use the latest version of the Bencher CLI when using Bencher Cloud."
            } else {
                "You should use the same version of the Bencher CLI as your Bencher Self-Hosted server."
            }
        )
    }
}

impl VersionMismatch {
    pub fn check(host: &url::Url, api_version: String) -> Option<Self> {
        (api_version != CLI_VERSION).then(|| Self {
            host: host.clone(),
            api_version,
            cli_version: CLI_VERSION.into(),
        })
    }
}
