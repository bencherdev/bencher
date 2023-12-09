use bencher_json::{Jwt, BENCHER_API_URL};
use serde::{de::DeserializeOwned, Serialize};
use tokio::time::{sleep, Duration};

const DEFAULT_ATTEMPTS: usize = 10;
const DEFAULT_RETRY_AFTER: u64 = 1;

/// A client for the Bencher API
#[derive(Debug, Clone)]
pub struct BencherClient {
    pub host: url::Url,
    pub token: Option<Jwt>,
    pub attempts: usize,
    pub retry_after: u64,
    pub log: bool,
}

#[derive(thiserror::Error, Debug)]
pub enum ClientError {
    #[error("Failed to build. Missing `host` field.")]
    NoHost,

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
    #[error("Error upgrading request: {0}")]
    InvalidUpgrade(reqwest::Error),
    #[error("Invalid response body bytes: {0}")]
    InvalidResponseBytes(reqwest::Error),
    #[error("Invalid response payload: {1}")]
    InvalidResponsePayload(progenitor_client::Bytes, serde_json::Error),
    #[error("Request succeeded with an unexpected response: {0}")]
    UnexpectedResponseOk(reqwest::Error),
    #[error("Request failed with an unexpected response: {0:?}")]
    UnexpectedResponseErr(reqwest::Response),

    #[error("Failed to send after {0} attempts")]
    SendTimeout(usize),
}

impl BencherClient {
    /// Create a new `BencherClient` with the given parameters
    ///
    /// # Parameters
    /// - `host`: The host URL
    /// - `token`: The JWT token
    /// - `attempts`: The number of attempts to make before giving up
    /// - `retry_after`: The number of initial seconds to wait between attempts (exponential backoff)
    /// - `log`: Whether to log the response JSON to stdout
    pub fn new(
        host: Option<url::Url>,
        token: Option<Jwt>,
        attempts: Option<usize>,
        retry_after: Option<u64>,
        log: Option<bool>,
    ) -> Self {
        BencherClientBuilder {
            host,
            token,
            attempts,
            retry_after,
            log,
        }
        .build()
    }

    /// Create a new `BencherClientBuilder`
    pub fn builder() -> BencherClientBuilder {
        BencherClientBuilder::default()
    }

    /// Send a request to the Bencher API
    ///
    /// # Parameters
    ///
    /// - `sender`: A function that takes a `codegen::Client` and returns a `Future` that resolves
    ///  to a `Result` containing a `ResponseValue` or an `Error`
    /// - `log`: Whether to log the response JSON to stdout
    ///
    /// # Returns
    ///
    /// A `Result` containing the response JSON or an `Error`
    #[allow(clippy::print_stdout)]
    pub async fn send_with<F, Fut, T, Json>(&self, sender: F) -> Result<Json, ClientError>
    where
        F: Fn(crate::codegen::Client) -> Fut,
        Fut: std::future::Future<
            Output = Result<
                progenitor_client::ResponseValue<T>,
                crate::codegen::Error<crate::codegen::types::Error>,
            >,
        >,
        T: Serialize,
        Json: DeserializeOwned + Serialize + TryFrom<T, Error = serde_json::Error>,
    {
        let timeout = std::time::Duration::from_secs(15);
        let mut client_builder = reqwest::ClientBuilder::new()
            .connect_timeout(timeout)
            .timeout(timeout);

        if let Some(token) = &self.token {
            let mut headers = reqwest::header::HeaderMap::new();
            let bearer_token = reqwest::header::HeaderValue::from_str(&format!("Bearer {token}"))
                .map_err(ClientError::HeaderValue)?;
            headers.insert("Authorization", bearer_token);
            client_builder = client_builder.default_headers(headers);
        }

        let reqwest_client = client_builder.build().map_err(ClientError::BuildClient)?;
        let client = crate::codegen::Client::new_with_client(self.host.as_ref(), reqwest_client);

        let attempts = self.attempts;
        let max_attempts = attempts.checked_sub(1).unwrap_or_default();
        let mut retry_after = self.retry_after;

        for attempt in 0..attempts {
            match sender(client.clone()).await {
                #[allow(clippy::print_stdout)]
                Ok(response_value) => {
                    let response = response_value.into_inner();
                    let json_response =
                        Json::try_from(response).map_err(ClientError::DeserializeResponse)?;
                    self.log(&json_response)?;
                    return Ok(json_response);
                },
                #[allow(clippy::print_stderr)]
                Err(crate::codegen::Error::CommunicationError(e)) => {
                    eprintln!("\nSend attempt #{}/{attempts}: {e}", attempt + 1);
                    if attempt != max_attempts {
                        eprintln!("Will retry after {retry_after} second(s).");
                        sleep(Duration::from_secs(retry_after)).await;
                        retry_after *= 2;
                    }
                },
                Err(crate::codegen::Error::InvalidRequest(e)) => {
                    return Err(ClientError::InvalidRequest(e))
                },
                Err(crate::codegen::Error::ErrorResponse(e)) => {
                    let status = e.status();
                    let headers = e.headers().clone();
                    let http_error = e.into_inner();
                    return Err(ClientError::ErrorResponse(ErrorResponse {
                        status,
                        headers,
                        request_id: http_error.request_id,
                        error_code: http_error.error_code,
                        message: http_error.message,
                    }));
                },
                Err(crate::codegen::Error::InvalidUpgrade(e)) => {
                    return Err(ClientError::InvalidUpgrade(e))
                },
                Err(crate::codegen::Error::InvalidResponseBytes(e)) => {
                    return Err(ClientError::InvalidResponseBytes(e))
                },
                Err(crate::codegen::Error::InvalidResponsePayload(bytes, e)) => {
                    return if let Ok(json_response) = serde_json::from_slice(&bytes) {
                        self.log(&json_response)?;
                        Ok(json_response)
                    } else {
                        Err(ClientError::InvalidResponsePayload(bytes, e))
                    }
                },
                Err(crate::codegen::Error::UnexpectedResponse(response)) => {
                    return if response.status().is_success() {
                        match response.json().await {
                            Ok(json_response) => {
                                self.log(&json_response)?;
                                Ok(json_response)
                            },
                            Err(e) => Err(ClientError::UnexpectedResponseOk(e)),
                        }
                    } else {
                        Err(ClientError::UnexpectedResponseErr(response))
                    }
                },
            }
        }

        Err(ClientError::SendTimeout(attempts))
    }

    fn log<T>(&self, response: &T) -> Result<(), ClientError>
    where
        T: Serialize,
    {
        #[allow(clippy::print_stdout)]
        if self.log {
            println!(
                "{}",
                serde_json::to_string_pretty(&response).map_err(ClientError::SerializeResponse)?
            );
        }
        Ok(())
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
        #[allow(clippy::use_debug)]
        writeln!(f, "Headers: {:?}", self.headers)?;
        writeln!(f, "Request ID: {}", self.request_id)?;
        if let Some(error_code) = &self.error_code {
            writeln!(f, "Error Code: {error_code}")?;
        }
        writeln!(f, "Message: {}", self.message)?;
        Ok(())
    }
}

/// A builder for `BencherClient`
#[derive(Debug, Clone, Default)]
pub struct BencherClientBuilder {
    host: Option<url::Url>,
    token: Option<Jwt>,
    attempts: Option<usize>,
    retry_after: Option<u64>,
    log: Option<bool>,
}

impl BencherClientBuilder {
    #[must_use]
    /// Set the host URL
    pub fn host(mut self, host: url::Url) -> Self {
        self.host = Some(host);
        self
    }

    #[must_use]
    /// Set the JWT token
    pub fn token(mut self, token: Jwt) -> Self {
        self.token = Some(token);
        self
    }

    #[must_use]
    /// Set the number of attempts to make before giving up
    pub fn attempts(mut self, attempts: usize) -> Self {
        self.attempts = Some(attempts);
        self
    }

    #[must_use]
    /// Set the number of initial seconds to wait between attempts (exponential backoff)
    pub fn retry_after(mut self, retry_after: u64) -> Self {
        self.retry_after = Some(retry_after);
        self
    }

    #[must_use]
    /// Set the whether to log the response JSON to stdout
    pub fn log(mut self, log: bool) -> Self {
        self.log = Some(log);
        self
    }

    /// Build the `BencherClient`
    ///
    /// Default values:
    /// - `host`: `https://api.bencher.dev`
    /// - `attempts`: `10`
    /// - `retry_after`: `1`
    pub fn build(self) -> BencherClient {
        let Self {
            host,
            token,
            attempts,
            retry_after,
            log,
        } = self;
        BencherClient {
            host: host.unwrap_or_else(|| BENCHER_API_URL.clone()),
            token,
            attempts: attempts.unwrap_or(DEFAULT_ATTEMPTS),
            retry_after: retry_after.unwrap_or(DEFAULT_RETRY_AFTER),
            log: log.unwrap_or_default(),
        }
    }
}
