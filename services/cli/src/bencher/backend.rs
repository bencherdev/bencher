use std::convert::TryFrom;

use bencher_json::{Jwt, Url};
use serde::Serialize;

use crate::parser::CliBackend;

pub const BENCHER_API_TOKEN: &str = "BENCHER_API_TOKEN";
pub const BENCHER_HOST: &str = "BENCHER_HOST";
#[cfg(debug_assertions)]
pub const DEFAULT_HOST: &str = "http://localhost:61016";
#[cfg(not(debug_assertions))]
pub const DEFAULT_HOST: &str = "https://api.bencher.dev";

#[derive(Debug, Clone)]
pub struct Backend(bencher_client::BencherClient);

#[derive(thiserror::Error, Debug)]
pub enum BackendError {
    #[error("Failed to parse host URL: {0}")]
    ParseHost(url::ParseError),
    #[error("Failed to parse API token: {0}")]
    ParseToken(bencher_json::ValidError),
    #[error("{0}")]
    Client(#[from] bencher_client::ClientError),
}

impl TryFrom<CliBackend> for Backend {
    type Error = BackendError;

    fn try_from(backend: CliBackend) -> Result<Self, Self::Error> {
        let client = bencher_client::BencherClient {
            host: unwrap_host(backend.host)?,
            token: map_token(backend.token)?,
            attempts: backend.attempts,
            retry_after: backend.retry_after,
        };
        Ok(Self(client))
    }
}

fn unwrap_host(host: Option<Url>) -> Result<url::Url, BackendError> {
    if let Some(url) = host {
        url.into()
    } else if let Ok(env_url) = std::env::var(BENCHER_HOST) {
        env_url
    } else {
        DEFAULT_HOST.into()
    }
    .parse()
    .map_err(BackendError::ParseHost)
}

fn map_token(token: Option<Jwt>) -> Result<Option<Jwt>, BackendError> {
    Ok(if let Some(token) = token {
        Some(token)
    } else if let Ok(env_token) = std::env::var(BENCHER_API_TOKEN) {
        Some(env_token.parse().map_err(BackendError::ParseToken)?)
    } else {
        None
    })
}

impl Backend {
    pub async fn send_with<F, Fut, T, Json>(
        &self,
        sender: F,
        log: bool,
    ) -> Result<Json, BackendError>
    where
        F: Fn(bencher_client::Client) -> Fut,
        Fut: std::future::Future<
            Output = Result<
                progenitor_client::ResponseValue<T>,
                bencher_client::Error<bencher_client::types::Error>,
            >,
        >,
        T: Serialize,
        Json: Serialize + TryFrom<T, Error = serde_json::Error>,
    {
        self.0.send_with(sender, log).await.map_err(Into::into)
    }
}
