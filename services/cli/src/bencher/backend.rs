use std::convert::TryFrom;

use bencher_json::{Jwt, Url};
use serde::Serialize;

use crate::parser::CliBackend;

pub const BENCHER_HOST: &str = "BENCHER_HOST";
pub const BENCHER_API_TOKEN: &str = "BENCHER_API_TOKEN";

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
        let CliBackend {
            host,
            token,
            attempts,
            retry_after,
        } = backend;
        let host = map_host(host)?;
        let token = map_token(token)?;
        Ok(Self(bencher_client::BencherClient::new(
            host,
            token,
            attempts,
            retry_after,
        )))
    }
}

fn map_host(host: Option<Url>) -> Result<Option<url::Url>, BackendError> {
    if let Some(url) = host {
        Some(url.into())
    } else if let Ok(env_url) = std::env::var(BENCHER_HOST) {
        Some(env_url)
    } else {
        None
    }
    .as_deref()
    .map(std::str::FromStr::from_str)
    .transpose()
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
