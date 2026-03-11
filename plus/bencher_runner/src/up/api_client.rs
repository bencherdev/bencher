use url::Url;

use super::error::ApiClientError;

const TOKEN_PREFIX: &str = "bencher_runner_";

pub struct RunnerApiClient {
    host: Url,
    token: String,
    runner: String,
}

impl RunnerApiClient {
    pub fn new(host: Url, token: String, runner: String) -> Result<Self, ApiClientError> {
        if !token.starts_with(TOKEN_PREFIX) {
            return Err(ApiClientError::InvalidToken);
        }

        Ok(Self {
            host,
            token,
            runner,
        })
    }

    /// Build the WebSocket URL for the persistent runner channel.
    pub fn channel_url(&self) -> Result<Url, ApiClientError> {
        let scheme = match self.host.scheme() {
            "https" => "wss",
            _ => "ws",
        };

        let host_str = self.host.as_str();
        // Strip the scheme prefix and rebuild with ws(s)
        let without_scheme = host_str
            .strip_prefix(self.host.scheme())
            .unwrap_or(host_str);
        let ws_url_str = format!("{scheme}{without_scheme}v0/runners/{}/channel", self.runner);

        Url::parse(&ws_url_str)
            .map_err(|e| ApiClientError::Http(format!("Failed to build WebSocket URL: {e}")))
    }

    pub fn token(&self) -> &str {
        &self.token
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

    // --- channel_url ---

    #[test]
    fn channel_url_http_becomes_ws() {
        let client =
            RunnerApiClient::new(test_host(), valid_token(), "my-runner".to_owned()).unwrap();
        let ws_url = client.channel_url().unwrap();
        assert_eq!(ws_url.scheme(), "ws");
        assert_eq!(
            ws_url.as_str(),
            "ws://localhost:61016/v0/runners/my-runner/channel"
        );
    }

    #[test]
    fn channel_url_https_becomes_wss() {
        let client =
            RunnerApiClient::new(test_https_host(), valid_token(), "runner-1".to_owned()).unwrap();
        let ws_url = client.channel_url().unwrap();
        assert_eq!(ws_url.scheme(), "wss");
        assert_eq!(
            ws_url.as_str(),
            "wss://api.bencher.dev/v0/runners/runner-1/channel"
        );
    }

    #[test]
    fn channel_url_includes_runner() {
        let client =
            RunnerApiClient::new(test_host(), valid_token(), "slug-test".to_owned()).unwrap();
        let ws_url = client.channel_url().unwrap();
        let path = ws_url.path();
        assert!(
            path.contains("slug-test"),
            "path should contain runner slug: {path}"
        );
        assert!(
            path.ends_with("/channel"),
            "path should end with /channel: {path}"
        );
    }
}
