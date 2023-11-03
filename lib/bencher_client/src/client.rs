use bencher_json::Jwt;
use serde::Serialize;
use tokio::time::{sleep, Duration};

const DEFAULT_ATTEMPTS: usize = 10;
const DEFAULT_RETRY_AFTER: u64 = 3;

#[derive(Debug, Clone)]
pub struct BencherClient {
    pub host: url::Url,
    pub token: Option<Jwt>,
    pub attempts: Option<usize>,
    pub retry_after: Option<u64>,
}

#[derive(thiserror::Error, Debug)]
pub enum ClientError {
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

impl BencherClient {
    pub async fn send_with<F, Fut, T, Json>(
        &self,
        sender: F,
        log: bool,
    ) -> Result<Json, ClientError>
    where
        F: Fn(crate::codegen::Client) -> Fut,
        Fut: std::future::Future<
            Output = Result<
                progenitor_client::ResponseValue<T>,
                crate::codegen::Error<crate::codegen::types::Error>,
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
                .map_err(ClientError::HeaderValue)?;
            headers.insert("Authorization", bearer_token);
            client_builder = client_builder.default_headers(headers);
        }

        let reqwest_client = client_builder.build().map_err(ClientError::BuildClient)?;
        let client = crate::codegen::Client::new_with_client(self.host.as_ref(), reqwest_client);

        let attempts = self.attempts.unwrap_or(DEFAULT_ATTEMPTS);
        let max_attempts = attempts.checked_sub(1).unwrap_or_default();
        let retry_after = self.retry_after.unwrap_or(DEFAULT_RETRY_AFTER);

        for attempt in 0..attempts {
            match sender(client.clone()).await {
                #[allow(clippy::print_stdout)]
                Ok(response_value) => {
                    let response = response_value.into_inner();
                    let json_response =
                        Json::try_from(response).map_err(ClientError::DeserializeResponse)?;
                    if log {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&json_response)
                                .map_err(ClientError::SerializeResponse)?
                        );
                    }
                    return Ok(json_response);
                },
                #[allow(clippy::print_stderr)]
                Err(crate::codegen::Error::CommunicationError(e)) => {
                    eprintln!("\nSend attempt #{}/{attempts}: {e}", attempt + 1);
                    if attempt != max_attempts {
                        eprintln!("Will retry after {retry_after} second(s).");
                        sleep(Duration::from_secs(retry_after)).await;
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
                Err(crate::codegen::Error::InvalidResponsePayload(e)) => {
                    return Err(ClientError::InvalidResponsePayload(e))
                },
                Err(crate::codegen::Error::UnexpectedResponse(response)) => {
                    return Err(if response.status().is_success() {
                        ClientError::UnexpectedResponseOk(response)
                    } else {
                        ClientError::UnexpectedResponseErr(response)
                    })
                },
            }
        }

        Err(ClientError::SendTimeout(attempts))
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
