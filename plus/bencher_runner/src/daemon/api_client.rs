use std::time::Duration;

use bencher_json::JsonClaimedJob;
use serde::Serialize;
use url::Url;
use uuid::Uuid;

use super::error::ApiClientError;

const TOKEN_PREFIX: &str = "bencher_runner_";

/// HTTP agent timeout.
///
/// This must be greater than the server's `MAX_POLL_TIMEOUT` (from `bencher_json`)
/// to avoid the client timing out before the server responds during long-polling.
/// We add a 30-second margin for network latency and server processing.
const AGENT_TIMEOUT_MARGIN_SECS: u64 = 30;
const AGENT_TIMEOUT: Duration =
    Duration::from_secs(bencher_json::MAX_POLL_TIMEOUT as u64 + AGENT_TIMEOUT_MARGIN_SECS);

pub struct RunnerApiClient {
    agent: ureq::Agent,
    host: Url,
    token: String,
    runner: String,
}

#[derive(Serialize)]
pub struct ClaimRequest {
    pub poll_timeout: u32,
}

impl RunnerApiClient {
    pub fn new(host: Url, token: String, runner: String) -> Result<Self, ApiClientError> {
        if !token.starts_with(TOKEN_PREFIX) {
            return Err(ApiClientError::InvalidToken);
        }

        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(AGENT_TIMEOUT))
            .build()
            .new_agent();

        Ok(Self {
            agent,
            host,
            token,
            runner,
        })
    }

    pub fn claim_job(
        &self,
        request: &ClaimRequest,
    ) -> Result<Option<JsonClaimedJob>, ApiClientError> {
        let url = format!("{}v0/runners/{}/jobs", self.host.as_str(), self.runner);

        let body = serde_json::to_value(request)
            .map_err(|e| ApiClientError::Http(format!("Failed to serialize claim request: {e}")))?;

        let response = self
            .agent
            .post(&url)
            .header("Authorization", &format!("Bearer {}", self.token))
            .header("Content-Type", "application/json")
            .send_json(&body)
            .map_err(|e| match classify_ureq_error(&e) {
                Some(err) => err,
                None => ApiClientError::Http(e.to_string()),
            })?;

        let claimed: Option<JsonClaimedJob> = response
            .into_body()
            .read_json()
            .map_err(|e| ApiClientError::Http(format!("Failed to parse claim response: {e}")))?;

        Ok(claimed)
    }

    pub fn websocket_url(&self, job_uuid: &Uuid) -> Result<Url, ApiClientError> {
        let scheme = match self.host.scheme() {
            "https" => "wss",
            _ => "ws",
        };

        let host_str = self.host.as_str();
        // Strip the scheme prefix and rebuild with ws(s)
        let without_scheme = host_str
            .strip_prefix(self.host.scheme())
            .unwrap_or(host_str);
        let ws_url_str = format!(
            "{scheme}{without_scheme}v0/runners/{}/jobs/{job_uuid}",
            self.runner
        );

        Url::parse(&ws_url_str)
            .map_err(|e| ApiClientError::Http(format!("Failed to build WebSocket URL: {e}")))
    }

    #[cfg(test)]
    pub fn token(&self) -> &str {
        &self.token
    }
}

fn classify_ureq_error(err: &ureq::Error) -> Option<ApiClientError> {
    if let ureq::Error::StatusCode(code) = *err {
        match code {
            401 | 403 => Some(ApiClientError::Unauthorized),
            409 => Some(ApiClientError::RunnerLocked),
            _ => Some(ApiClientError::UnexpectedStatus {
                status: code,
                body: err.to_string(),
            }),
        }
    } else {
        None
    }
}

#[cfg(test)]
#[expect(clippy::indexing_slicing, reason = "Test assertions on JSON values")]
mod tests {
    use super::*;

    fn test_host() -> Url {
        Url::parse("http://localhost:61016/").unwrap()
    }

    fn test_https_host() -> Url {
        Url::parse("https://api.bencher.dev/").unwrap()
    }

    fn valid_token() -> String {
        "bencher_runner_abc123".to_owned()
    }

    // --- RunnerApiClient::new ---

    #[test]
    fn new_accepts_valid_token() {
        let client = RunnerApiClient::new(test_host(), valid_token(), "my-runner".to_owned());
        assert!(client.is_ok());
    }

    #[test]
    fn new_rejects_empty_token() {
        let result = RunnerApiClient::new(test_host(), String::new(), "r".to_owned());
        assert!(matches!(result, Err(ApiClientError::InvalidToken)));
    }

    #[test]
    fn new_rejects_wrong_prefix() {
        let result = RunnerApiClient::new(test_host(), "bearer_abc123".to_owned(), "r".to_owned());
        assert!(matches!(result, Err(ApiClientError::InvalidToken)));
    }

    #[test]
    fn new_rejects_partial_prefix() {
        let result = RunnerApiClient::new(test_host(), "bencher_runne".to_owned(), "r".to_owned());
        assert!(matches!(result, Err(ApiClientError::InvalidToken)));
    }

    #[test]
    fn new_stores_token() {
        let client =
            RunnerApiClient::new(test_host(), valid_token(), "my-runner".to_owned()).unwrap();
        assert_eq!(client.token(), "bencher_runner_abc123");
    }

    // --- websocket_url ---

    #[test]
    fn websocket_url_http_becomes_ws() {
        let client =
            RunnerApiClient::new(test_host(), valid_token(), "my-runner".to_owned()).unwrap();
        let uuid = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let ws_url = client.websocket_url(&uuid).unwrap();
        assert_eq!(ws_url.scheme(), "ws");
        assert_eq!(
            ws_url.as_str(),
            "ws://localhost:61016/v0/runners/my-runner/jobs/550e8400-e29b-41d4-a716-446655440000"
        );
    }

    #[test]
    fn websocket_url_https_becomes_wss() {
        let client =
            RunnerApiClient::new(test_https_host(), valid_token(), "runner-1".to_owned()).unwrap();
        let uuid = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let ws_url = client.websocket_url(&uuid).unwrap();
        assert_eq!(ws_url.scheme(), "wss");
        assert!(
            ws_url.as_str().starts_with("wss://api.bencher.dev/"),
            "URL was: {ws_url}",
        );
    }

    #[test]
    fn websocket_url_includes_runner_and_job() {
        let client =
            RunnerApiClient::new(test_host(), valid_token(), "slug-test".to_owned()).unwrap();
        let uuid = Uuid::parse_str("11111111-2222-3333-4444-555555555555").unwrap();
        let ws_url = client.websocket_url(&uuid).unwrap();
        let path = ws_url.path();
        assert!(
            path.contains("slug-test"),
            "path should contain runner slug: {path}"
        );
        assert!(
            path.contains("11111111-2222-3333-4444-555555555555"),
            "path should contain job UUID: {path}"
        );
    }

    // --- ClaimRequest serialization ---

    #[test]
    fn claim_request_serializes() {
        let req = ClaimRequest { poll_timeout: 55 };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["poll_timeout"], 55);
    }

    #[test]
    fn claim_request_zero_timeout() {
        let req = ClaimRequest { poll_timeout: 0 };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["poll_timeout"], 0);
    }
}
