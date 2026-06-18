use bencher_json::RunnerResourceId;
use bencher_valid::RunnerKey;
use url::Url;

use super::error::ApiClientError;

pub struct RunnerApiClient {
    host: Url,
    key: RunnerKey,
    runner: RunnerResourceId,
}

impl RunnerApiClient {
    pub fn new(host: Url, key: RunnerKey, runner: RunnerResourceId) -> Self {
        Self { host, key, runner }
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

        Url::parse(&ws_url_str).map_err(ApiClientError::Url)
    }

    pub fn key(&self) -> &str {
        self.key.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_host() -> Url {
        Url::parse("http://localhost:6610/").unwrap()
    }

    fn test_https_host() -> Url {
        Url::parse("https://api.bencher.dev/").unwrap()
    }

    fn valid_key() -> RunnerKey {
        "bencher_runner_aB3xY9mN2pQ7rS4tU8vW1zK5jL0fGh"
            .parse()
            .unwrap()
    }

    // --- RunnerApiClient::new ---

    #[test]
    fn new_stores_key() {
        let client = RunnerApiClient::new(test_host(), valid_key(), "my-runner".parse().unwrap());
        assert_eq!(
            client.key(),
            "bencher_runner_aB3xY9mN2pQ7rS4tU8vW1zK5jL0fGh"
        );
    }

    // --- channel_url ---

    #[test]
    fn channel_url_http_becomes_ws() {
        let client = RunnerApiClient::new(test_host(), valid_key(), "my-runner".parse().unwrap());
        let ws_url = client.channel_url().unwrap();
        assert_eq!(ws_url.scheme(), "ws");
        assert_eq!(
            ws_url.as_str(),
            "ws://localhost:6610/v0/runners/my-runner/channel"
        );
    }

    #[test]
    fn channel_url_https_becomes_wss() {
        let client =
            RunnerApiClient::new(test_https_host(), valid_key(), "runner-1".parse().unwrap());
        let ws_url = client.channel_url().unwrap();
        assert_eq!(ws_url.scheme(), "wss");
        assert_eq!(
            ws_url.as_str(),
            "wss://api.bencher.dev/v0/runners/runner-1/channel"
        );
    }

    #[test]
    fn channel_url_includes_runner() {
        let client = RunnerApiClient::new(test_host(), valid_key(), "slug-test".parse().unwrap());
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
