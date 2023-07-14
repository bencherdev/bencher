use std::convert::TryFrom;

use bencher_json::{Jwt, Url};
use serde::Serialize;
use tokio::time::{sleep, Duration};

use crate::{cli_eprintln, cli_println, parser::CliBackend, CliError};

pub const BENCHER_API_TOKEN: &str = "BENCHER_API_TOKEN";
pub const BENCHER_HOST: &str = "BENCHER_HOST";
#[cfg(debug_assertions)]
pub const DEFAULT_HOST: &str = "http://localhost:61016";
#[cfg(not(debug_assertions))]
pub const DEFAULT_HOST: &str = "https://api.bencher.dev";
const DEFAULT_ATTEMPTS: usize = 10;
const DEFAULT_RETRY_AFTER: u64 = 3;

#[derive(Debug, Clone)]
pub struct Backend {
    pub host: url::Url,
    pub token: Option<Jwt>,
    pub attempts: Option<usize>,
    pub retry_after: Option<u64>,
}

impl TryFrom<CliBackend> for Backend {
    type Error = CliError;

    fn try_from(backend: CliBackend) -> Result<Self, Self::Error> {
        Ok(Self {
            host: unwrap_host(backend.host)?,
            token: map_token(backend.token)?,
            attempts: backend.attempts,
            retry_after: backend.retry_after,
        })
    }
}

fn unwrap_host(host: Option<Url>) -> Result<url::Url, CliError> {
    if let Some(url) = host {
        url.into()
    } else if let Ok(env_url) = std::env::var(BENCHER_HOST) {
        env_url
    } else {
        DEFAULT_HOST.into()
    }
    .parse()
    .map_err(Into::into)
}

fn map_token(token: Option<Jwt>) -> Result<Option<Jwt>, CliError> {
    Ok(if let Some(token) = token {
        Some(token)
    } else if let Ok(env_token) = std::env::var(BENCHER_API_TOKEN) {
        Some(env_token.parse()?)
    } else {
        None
    })
}

impl Backend {
    pub async fn send_with<F, Fut, T, Json>(&self, sender: F, log: bool) -> Result<Json, CliError>
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
        let timeout = std::time::Duration::from_secs(15);
        let mut client_builder = reqwest::ClientBuilder::new()
            .connect_timeout(timeout)
            .timeout(timeout);
        if let Some(token) = &self.token {
            let mut headers = reqwest::header::HeaderMap::new();
            headers.insert(
                "Authorization",
                reqwest::header::HeaderValue::from_str(&format!("Bearer {token}"))?,
            );

            client_builder = client_builder.default_headers(headers)
        }
        let reqwest_client = client_builder.build()?;
        let client = bencher_client::Client::new_with_client(self.host.as_ref(), reqwest_client);

        let attempts = self.attempts.unwrap_or(DEFAULT_ATTEMPTS);
        let max_attempts = attempts.checked_sub(1).ok_or(CliError::BadMath)?;
        let retry_after = self.retry_after.unwrap_or(DEFAULT_RETRY_AFTER);

        for attempt in 0..attempts {
            match sender(client.clone()).await {
                Ok(response_value) => {
                    let response = response_value.into_inner();
                    let json_response = Json::try_from(response)?;
                    if log {
                        cli_println!("{}", serde_json::to_string_pretty(&json_response)?);
                    }
                    return Ok(json_response);
                },
                Err(bencher_client::Error::CommunicationError(e)) => {
                    cli_eprintln!("\nSend attempt #{}/{attempts}: {e}", attempt + 1);
                    if attempt != max_attempts {
                        cli_eprintln!("Will retry after {retry_after} second(s).");
                        sleep(Duration::from_secs(retry_after)).await;
                    }
                },
                Err(e) => return Err(e.into()),
            }
        }

        Err(CliError::Send(attempts))
    }
}
