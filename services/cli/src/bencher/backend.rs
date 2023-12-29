use std::convert::TryFrom;

use bencher_json::{Jwt, Url};
use serde::{de::DeserializeOwned, Serialize};

use crate::parser::CliBackend;

pub const BENCHER_HOST: &str = "BENCHER_HOST";
pub const BENCHER_API_TOKEN: &str = "BENCHER_API_TOKEN";

#[derive(Debug, Clone)]
pub struct PubBackend {
    inner: Backend,
}

#[derive(Debug, Clone)]
pub struct AuthBackend {
    inner: Backend,
}

#[derive(Debug, Clone)]
pub(crate) struct Backend {
    client: bencher_client::BencherClient,
}

#[derive(thiserror::Error, Debug)]
pub enum BackendError {
    #[error("Failed to parse host URL: {0}")]
    ParseHost(url::ParseError),
    #[error("Failed to parse API token: {0}")]
    ParseToken(bencher_json::ValidError),
    #[error("Failed to find Bencher API token, and this API endpoint requires authorization. Set the `--token` flag or the `BENCHER_API_TOKEN` environment variable.")]
    NoToken,
    #[error("{0}")]
    Client(#[from] bencher_client::ClientError),
}

impl TryFrom<CliBackend> for PubBackend {
    type Error = BackendError;

    fn try_from(backend: CliBackend) -> Result<Self, Self::Error> {
        (backend, true).try_into().map(|inner| Self { inner })
    }
}

impl TryFrom<CliBackend> for AuthBackend {
    type Error = BackendError;

    fn try_from(backend: CliBackend) -> Result<Self, Self::Error> {
        (backend, false).try_into().map(|inner| Self { inner })
    }
}

impl TryFrom<(CliBackend, bool)> for Backend {
    type Error = BackendError;

    fn try_from((backend, is_public): (CliBackend, bool)) -> Result<Self, Self::Error> {
        let CliBackend {
            host,
            token,
            attempts,
            retry_after,
            strict,
        } = backend;
        let host = map_host(host)?;
        let token = map_token(token, is_public)?;
        let client = bencher_client::BencherClient::new(
            host,
            token,
            attempts,
            retry_after,
            Some(strict),
            Some(true),
        );
        Ok(Self { client })
    }
}

fn map_host(host: Option<Url>) -> Result<Option<url::Url>, BackendError> {
    host.map(Into::into)
        .or_else(|| std::env::var(BENCHER_HOST).ok())
        .as_deref()
        .map(std::str::FromStr::from_str)
        .transpose()
        .map_err(BackendError::ParseHost)
}

fn map_token(token: Option<Jwt>, is_public: bool) -> Result<Option<Jwt>, BackendError> {
    if let Some(token) = token {
        Ok(Some(token))
    } else if let Ok(env_token) = std::env::var(BENCHER_API_TOKEN) {
        Ok(Some(env_token.parse().map_err(BackendError::ParseToken)?))
    } else if is_public {
        Ok(None)
    } else {
        Err(BackendError::NoToken)
    }
}

impl AsRef<Backend> for PubBackend {
    fn as_ref(&self) -> &Backend {
        &self.inner
    }
}

impl PubBackend {
    pub fn log(mut self, log: bool) -> Self {
        self.inner.client.log = log;
        self
    }
}

impl AsRef<Backend> for AuthBackend {
    fn as_ref(&self) -> &Backend {
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
        R: std::future::Future<
            Output = Result<
                progenitor_client::ResponseValue<T>,
                bencher_client::Error<bencher_client::types::Error>,
            >,
        >,
        T: Serialize,
        E: std::error::Error + Send + Sync + 'static,
        bencher_client::JsonValue: TryFrom<T, Error = E>,
    {
        self.client.send(sender).await.map_err(Into::into)
    }

    pub async fn send_with<F, R, T, Json, E>(&self, sender: F) -> Result<Json, BackendError>
    where
        F: Fn(bencher_client::Client) -> R,
        R: std::future::Future<
            Output = Result<
                progenitor_client::ResponseValue<T>,
                bencher_client::Error<bencher_client::types::Error>,
            >,
        >,
        T: Serialize,
        Json: DeserializeOwned + Serialize + TryFrom<T, Error = E>,
        E: std::error::Error + Send + Sync + 'static,
    {
        self.client.send_with(sender).await.map_err(Into::into)
    }
}
