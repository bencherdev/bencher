use std::convert::TryFrom;

use bencher_json::{Jwt, Url};
use serde::Serialize;
use tokio::time::{sleep, Duration};

use crate::{cli_eprintln, cli_println, parser::CliBackend};

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

#[derive(thiserror::Error, Debug)]
pub enum BackendError {
    #[error("Failed to parse host URL: {0}")]
    ParseHost(url::ParseError),
    #[error("Failed to parse API token: {0}")]
    ParseToken(bencher_json::ValidError),

    #[error("Failed to parse Authorization header: {0}")]
    HeaderValue(reqwest::header::InvalidHeaderValue),
    #[error("Failed to build API client: {0}")]
    BuildClient(reqwest::Error),

    #[error("Failed to deserialize response JSON: {0}")]
    DeserializeResponse(serde_json::Error),
    #[error("Failed to serialize response JSON: {0}")]
    SerializeResponse(serde_json::Error),

    #[error("Invalid request. The request did not conform to API requirements: {0}")]
    InvalidRequest(String),
    #[error("Error processing request:\n{0}")]
    ErrorResponse(ErrorResponse),
    #[error("Invalid response payload: {0}")]
    InvalidResponsePayload(reqwest::Error),
    #[error("Request succeeded with an unexpected response: {0:?}")]
    UnexpectedResponseOk(reqwest::Response),
    #[error("Request failed with an unexpected response: {0:?}")]
    UnexpectedResponseErr(reqwest::Response),

    #[error("Failed to send after {0} attempts")]
    SendTimeout(usize),
}

impl TryFrom<CliBackend> for Backend {
    type Error = BackendError;

    fn try_from(backend: CliBackend) -> Result<Self, Self::Error> {
        Ok(Self {
            host: unwrap_host(backend.host)?,
            token: map_token(backend.token)?,
            attempts: backend.attempts,
            retry_after: backend.retry_after,
        })
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
        let timeout = std::time::Duration::from_secs(15);
        let mut client_builder = reqwest::ClientBuilder::new()
            .connect_timeout(timeout)
            .timeout(timeout);

        if let Some(token) = &self.token {
            let mut headers = reqwest::header::HeaderMap::new();
            let bearer_token = reqwest::header::HeaderValue::from_str(&format!("Bearer {token}"))
                .map_err(BackendError::HeaderValue)?;
            headers.insert("Authorization", bearer_token);
            client_builder = client_builder.default_headers(headers);
        }

        let reqwest_client = client_builder.build().map_err(BackendError::BuildClient)?;
        let client = bencher_client::Client::new_with_client(self.host.as_ref(), reqwest_client);

        let attempts = self.attempts.unwrap_or(DEFAULT_ATTEMPTS);
        let max_attempts = attempts.checked_sub(1).unwrap_or_default();
        let retry_after = self.retry_after.unwrap_or(DEFAULT_RETRY_AFTER);

        for attempt in 0..attempts {
            match sender(client.clone()).await {
                Ok(response_value) => {
                    let response = response_value.into_inner();
                    let json_response =
                        Json::try_from(response).map_err(BackendError::DeserializeResponse)?;
                    if log {
                        cli_println!(
                            "{}",
                            serde_json::to_string_pretty(&json_response)
                                .map_err(BackendError::SerializeResponse)?
                        );
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
                Err(bencher_client::Error::InvalidRequest(e)) => {
                    return Err(BackendError::InvalidRequest(e))
                },
                Err(bencher_client::Error::ErrorResponse(e)) => {
                    let status = e.status();
                    let headers = e.headers().clone();
                    let http_error = e.into_inner();
                    return Err(BackendError::ErrorResponse(ErrorResponse {
                        status,
                        headers,
                        request_id: http_error.request_id,
                        error_code: http_error.error_code,
                        message: http_error.message,
                    }));
                },
                Err(bencher_client::Error::InvalidResponsePayload(e)) => {
                    return Err(BackendError::InvalidResponsePayload(e))
                },
                Err(bencher_client::Error::UnexpectedResponse(response)) => {
                    return Err(if response.status().is_success() {
                        BackendError::UnexpectedResponseOk(response)
                    } else {
                        BackendError::UnexpectedResponseErr(response)
                    })
                },
            }
        }

        Err(BackendError::SendTimeout(attempts))
    }
}

#[derive(Debug)]
pub struct ErrorResponse {
    status: reqwest::StatusCode,
    headers: reqwest::header::HeaderMap,
    request_id: String,
    error_code: Option<String>,
    message: String,
}

impl std::fmt::Display for ErrorResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Status: {}", self.status)?;
        writeln!(f, "Headers: {:?}", self.headers)?;
        writeln!(f, "Request ID: {}", self.request_id)?;
        if let Some(error_code) = &self.error_code {
            writeln!(f, "Error Code: {error_code}")?;
        }
        writeln!(f, "Message: {}", self.message)?;
        Ok(())
    }
}
