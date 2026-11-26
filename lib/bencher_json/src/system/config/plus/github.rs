use std::fmt;

use bencher_valid::{GitHubAppSlug, NonEmpty, Sanitize, Secret, Url};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonGitHub {
    /// The web root of the GitHub instance, as GitHub Actions names it in `GITHUB_SERVER_URL`:
    /// `https://github.com` by default, or a GitHub Enterprise Server such as
    /// `https://github.example.com`, which needs a Bencher Plus Enterprise license.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_url: Option<Url>,
    pub client_id: NonEmpty,
    pub client_secret: Secret,
    /// The GitHub App that links a repository and posts to it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app: Option<JsonGitHubApp>,
}

impl Sanitize for JsonGitHub {
    fn sanitize(&mut self) {
        let Self {
            server_url,
            client_id: _,
            client_secret,
            app,
        } = self;
        if let Some(server_url) = server_url {
            sanitize_credentials(server_url);
        }
        client_secret.sanitize();
        app.sanitize();
    }
}

/// The config logs itself before it refuses a server URL with a user name or a password.
fn sanitize_credentials(server_url: &mut Url) {
    if let Ok(mut url) = url::Url::try_from(server_url.clone())
        && (!url.username().is_empty() || url.password().is_some())
        && url.set_username("").is_ok()
        && url.set_password(None).is_ok()
    {
        *server_url = url.into();
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonGitHubApp {
    /// The app's name in its GitHub URLs, as in `https://github.com/apps/{slug}`, or
    /// `{server_url}/github-apps/{slug}` on GitHub Enterprise Server.
    pub slug: GitHubAppSlug,
    /// The app's private key, in PEM.
    pub private_key: Secret,
    pub webhook_secret: Secret,
}

impl fmt::Debug for JsonGitHubApp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("JsonGitHubApp").finish_non_exhaustive()
    }
}

impl Sanitize for JsonGitHubApp {
    fn sanitize(&mut self) {
        let Self {
            slug: _,
            private_key,
            webhook_secret,
        } = self;
        private_key.sanitize();
        webhook_secret.sanitize();
    }
}

#[cfg(test)]
mod tests {
    use bencher_valid::Sanitize as _;

    use super::JsonGitHub;

    const PRIVATE_KEY: &str = "private-key-marker";
    const WEBHOOK_SECRET: &str = "webhook-secret-marker";

    fn github() -> JsonGitHub {
        serde_json::from_str(&format!(
            r#"{{"client_id": "Iv1.0123456789abcdef", "client_secret": "client-secret", "app": {{"slug": "bencher-test", "private_key": "{PRIVATE_KEY}", "webhook_secret": "{WEBHOOK_SECRET}"}}}}"#
        ))
        .unwrap()
    }

    // Kills dropping the client secret's sanitize, the `app` sanitize, or either secret's within
    // it.
    #[test]
    fn sanitize_hides_the_app_secrets() {
        let mut github = github();

        github.sanitize();

        assert_eq!(github.client_secret.as_ref(), "************");
        let app = github.app.unwrap();
        assert_eq!(app.private_key.as_ref(), "************");
        assert_eq!(app.webhook_secret.as_ref(), "************");
    }

    // Kills a sanitize that skips the server URL, that clears only its user name or only its
    // password, or that drops the port or the path. The URL with no credentials kills a sanitize
    // that rewrites every URL.
    #[test]
    fn sanitize_hides_the_server_url_credentials() {
        for (server_url, sanitized) in [
            (
                "https://muriel@github.example.com:8443/ghes",
                "https://github.example.com:8443/ghes",
            ),
            (
                "https://muriel:hunter2@github.example.com:8443/ghes",
                "https://github.example.com:8443/ghes",
            ),
            (
                "https://:hunter2@github.example.com:8443/ghes",
                "https://github.example.com:8443/ghes",
            ),
            ("https://github.example.com", "https://github.example.com"),
        ] {
            let mut github = github();
            github.server_url = Some(server_url.parse().unwrap());

            github.sanitize();

            assert_eq!(
                github.server_url.as_ref().map(AsRef::as_ref),
                Some(sanitized),
                "{server_url}"
            );
        }
    }

    // `Secret` prints itself in a debug build, so this kills a derived `Debug` on the app.
    #[test]
    fn debug_hides_the_app_secrets() {
        let debug = format!("{:?}", github());

        assert!(debug.contains("JsonGitHubApp"), "{debug}");
        for marker in [PRIVATE_KEY, WEBHOOK_SECRET] {
            assert!(!debug.contains(marker), "{marker} in {debug}");
        }
    }
}
