use aws_lc_rs::{
    encoding::{AsDer as _, Pkcs8V1Der},
    hmac,
    rsa::{KeyPair, KeySize},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
pub use bencher_github_client::test_server::{Received, Server as GitHubServer};
use bencher_github_client::{GitHubApp, test_server::install_crypto_provider};

use crate::TestServer;

pub const GITHUB_CLIENT_ID: &str = "Iv1.0123456789abcdef";
pub const GITHUB_CLIENT_SECRET: &str = "client-secret-marker";
pub const GITHUB_APP_SLUG: &str = "bencher-test";
pub const GITHUB_WEBHOOK_SECRET: &str = "webhook-secret-marker";

impl TestServer {
    /// Send a GitHub webhook delivery with the signature given, if any.
    #[expect(clippy::expect_used, reason = "test helper")]
    pub async fn github_webhook(
        &self,
        event: &str,
        signature: Option<&str>,
        body: &str,
    ) -> reqwest::Response {
        let request = self
            .client
            .post(self.api_url("/v0/github/webhook"))
            .header("x-github-event", event)
            .body(body.to_owned());
        let request = if let Some(signature) = signature {
            request.header("x-hub-signature-256", signature)
        } else {
            request
        };
        request
            .send()
            .await
            .expect("Failed to send a GitHub webhook delivery")
    }
}

/// The signature that GitHub sends with `body`, by the test app's webhook secret.
pub fn github_signature(body: &str) -> String {
    let key = hmac::Key::new(hmac::HMAC_SHA256, GITHUB_WEBHOOK_SECRET.as_bytes());
    format!(
        "sha256={}",
        hex::encode(hmac::sign(&key, body.as_bytes()).as_ref())
    )
}

/// The app, with a fresh key.
#[expect(clippy::expect_used, reason = "test helper")]
pub(crate) fn app(base: url::Url) -> GitHubApp {
    install_crypto_provider();
    let key_pair = KeyPair::generate(KeySize::Rsa2048).expect("Failed to generate a key");
    let der: Pkcs8V1Der<'static> = key_pair.as_der().expect("Failed to encode the key");
    let private_key = format!(
        "-----BEGIN PRIVATE KEY-----\n{}\n-----END PRIVATE KEY-----\n",
        STANDARD.encode(der.as_ref())
    );
    GitHubApp::new(
        GITHUB_CLIENT_ID.parse().expect("Invalid client ID"),
        GITHUB_APP_SLUG.parse().expect("Invalid app slug"),
        &private_key.parse().expect("Invalid private key"),
        GITHUB_WEBHOOK_SECRET
            .parse()
            .expect("Invalid webhook secret"),
    )
    .expect("Failed to set up the GitHub App")
    .with_base_url(base)
    .expect("Failed to point the GitHub App at the base")
}
