use std::collections::HashMap;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use url::Url;
use uuid::Uuid;

use super::error::ApiClientError;

const TOKEN_PREFIX: &str = "bencher_runner_";
const AGENT_TIMEOUT: Duration = Duration::from_secs(90);

pub struct RunnerApiClient {
    agent: ureq::Agent,
    host: Url,
    token: String,
    runner: String,
}

#[derive(Serialize)]
pub struct ClaimRequest {
    pub labels: Vec<String>,
    pub poll_timeout_seconds: u32,
}

#[derive(Deserialize)]
pub struct ClaimResponse {
    pub job: Option<ClaimedJob>,
}

#[derive(Debug, Deserialize)]
pub struct ClaimedJob {
    pub uuid: Uuid,
    pub spec: JobSpec,
    pub timeout_seconds: u32,
}

/// OCI-based job specification.
///
/// This struct mirrors `JsonJobSpec` from `bencher_json` but is kept local
/// to avoid tight coupling between the runner and the main JSON types.
#[derive(Debug, Deserialize)]
pub struct JobSpec {
    /// The OCI registry URL where the image is hosted.
    pub registry: Url,
    /// The project UUID that owns the image.
    pub project: Uuid,
    /// The image digest (e.g., "sha256:...").
    pub digest: String,
    /// Optional entrypoint override for the container.
    #[serde(default)]
    pub entrypoint: Option<Vec<String>>,
    /// Optional command override for the container.
    #[serde(default)]
    pub cmd: Option<Vec<String>>,
    /// Optional environment variables for the container.
    #[serde(default)]
    pub env: Option<HashMap<String, String>>,
    /// Number of vCPUs to allocate.
    pub vcpu: u32,
    /// Memory size in bytes.
    pub memory: u64,
    /// Disk size in bytes.
    pub disk: u64,
    /// Timeout in seconds.
    pub timeout: u32,
    /// Whether to enable network access.
    pub network: bool,
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

    pub fn claim_job(&self, request: &ClaimRequest) -> Result<Option<ClaimedJob>, ApiClientError> {
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

        let claim: ClaimResponse = response
            .into_body()
            .read_json()
            .map_err(|e| ApiClientError::Http(format!("Failed to parse claim response: {e}")))?;

        Ok(claim.job)
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
            "{scheme}{without_scheme}v0/runners/{}/jobs/{job_uuid}/channel",
            self.runner
        );

        Url::parse(&ws_url_str)
            .map_err(|e| ApiClientError::Http(format!("Failed to build WebSocket URL: {e}")))
    }

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
            "ws://localhost:61016/v0/runners/my-runner/jobs/550e8400-e29b-41d4-a716-446655440000/channel"
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
            "URL was: {}",
            ws_url
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
        let req = ClaimRequest {
            labels: vec!["gpu".to_owned(), "arm64".to_owned()],
            poll_timeout_seconds: 55,
        };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["labels"], serde_json::json!(["gpu", "arm64"]));
        assert_eq!(json["poll_timeout_seconds"], 55);
    }

    #[test]
    fn claim_request_empty_labels() {
        let req = ClaimRequest {
            labels: vec![],
            poll_timeout_seconds: 30,
        };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["labels"], serde_json::json!([]));
    }

    // --- ClaimResponse deserialization ---

    #[test]
    fn claim_response_with_no_job() {
        let json = r#"{"job": null}"#;
        let resp: ClaimResponse = serde_json::from_str(json).unwrap();
        assert!(resp.job.is_none());
    }

    #[test]
    fn claim_response_with_job() {
        let json = r#"{
            "job": {
                "uuid": "550e8400-e29b-41d4-a716-446655440000",
                "timeout_seconds": 600,
                "spec": {
                    "registry": "https://registry.bencher.dev",
                    "project": "11111111-2222-3333-4444-555555555555",
                    "digest": "sha256:a665a45920422f9d417e4867efdc4fb8a04a1f3fff1fa07e998e86f7f7a27ae3",
                    "vcpu": 2,
                    "memory": 1073741824,
                    "disk": 10737418240,
                    "timeout": 300,
                    "network": false
                }
            }
        }"#;
        let resp: ClaimResponse = serde_json::from_str(json).unwrap();
        let job = resp.job.unwrap();
        assert_eq!(
            job.uuid,
            Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap()
        );
        assert_eq!(job.timeout_seconds, 600);
        assert_eq!(job.spec.vcpu, 2);
        assert_eq!(job.spec.memory, 0x4000_0000); // 1 GiB
    }

    // --- JobSpec deserialization ---

    #[test]
    fn job_spec_minimal() {
        let json = r#"{
            "registry": "https://registry.bencher.dev",
            "project": "11111111-2222-3333-4444-555555555555",
            "digest": "sha256:a665a45920422f9d417e4867efdc4fb8a04a1f3fff1fa07e998e86f7f7a27ae3",
            "vcpu": 1,
            "memory": 536870912,
            "disk": 1073741824,
            "timeout": 60,
            "network": false
        }"#;
        let spec: JobSpec = serde_json::from_str(json).unwrap();
        assert_eq!(spec.vcpu, 1);
        assert_eq!(spec.memory, 536_870_912); // 512 MiB
        assert_eq!(spec.disk, 0x4000_0000); // 1 GiB
        assert_eq!(spec.timeout, 60);
        assert!(!spec.network);
        assert!(spec.entrypoint.is_none());
        assert!(spec.cmd.is_none());
        assert!(spec.env.is_none());
    }

    #[test]
    fn job_spec_full() {
        let json = r#"{
            "registry": "https://registry.bencher.dev",
            "project": "11111111-2222-3333-4444-555555555555",
            "digest": "sha256:a665a45920422f9d417e4867efdc4fb8a04a1f3fff1fa07e998e86f7f7a27ae3",
            "entrypoint": ["/bin/sh"],
            "cmd": ["-c", "cargo bench"],
            "env": {"RUST_LOG": "debug", "CI": "true"},
            "vcpu": 4,
            "memory": 2147483648,
            "disk": 10737418240,
            "timeout": 300,
            "network": true
        }"#;
        let spec: JobSpec = serde_json::from_str(json).unwrap();
        assert_eq!(
            spec.entrypoint.as_deref(),
            Some(&["/bin/sh".to_owned()][..])
        );
        assert_eq!(
            spec.cmd.as_deref(),
            Some(&["-c".to_owned(), "cargo bench".to_owned()][..])
        );
        let env = spec.env.as_ref().unwrap();
        assert_eq!(env.len(), 2);
        assert_eq!(env.get("RUST_LOG").unwrap(), "debug");
        assert_eq!(spec.vcpu, 4);
        assert_eq!(spec.memory, 0x8000_0000); // 2 GiB
        assert!(spec.network);
    }
}
