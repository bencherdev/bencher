use std::fmt;

use bencher_valid::{GitHubAppSlug, NonEmpty, Sanitize, Secret};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonGitHub {
    pub client_id: NonEmpty,
    pub client_secret: Secret,
    /// The GitHub App that mints the dispatch token for a linked repository.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app: Option<JsonGitHubApp>,
}

impl Sanitize for JsonGitHub {
    fn sanitize(&mut self) {
        let Self {
            client_id: _,
            client_secret,
            app,
        } = self;
        client_secret.sanitize();
        app.sanitize();
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonGitHubApp {
    /// The app's name in its GitHub URLs, as in `https://github.com/apps/{slug}`.
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
