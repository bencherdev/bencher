use aws_lc_rs::hmac;
pub use bencher_github_client::test_server::{
    Answer, HOLD, NO_ANSWER, Received, Server as GitHubServer,
};
use bencher_github_client::{
    GitHubApp,
    test_server::{app_key, install_crypto_provider},
};

use bencher_json::{DateTime, GitHubInstallationId, GitHubRepositoryId};
use bencher_schema::{model::project::linked_repository::QueryLinkedRepository, schema};
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};

use crate::{TestProject, TestServer, TestUser};

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

impl TestServer {
    /// Link `project` to a repository in an installation, connected, as a `PUT` with a grant
    /// from `user` would, and return the link.
    #[expect(clippy::expect_used, reason = "test helper")]
    pub fn link_repository(
        &self,
        project: &TestProject,
        user: &TestUser,
        installation: u64,
        repository: u64,
        full_name: &str,
    ) -> QueryLinkedRepository {
        let mut conn = self.db_conn();
        let project_id = schema::project::table
            .filter(schema::project::uuid.eq(project.uuid))
            .select(schema::project::id)
            .first(&mut conn)
            .expect("No project");
        let user_id = schema::user::table
            .filter(schema::user::uuid.eq(user.uuid))
            .select(schema::user::id)
            .first(&mut conn)
            .expect("No user");
        let link = QueryLinkedRepository::connected(
            project_id,
            GitHubInstallationId::try_from(installation).expect("Invalid installation"),
            GitHubRepositoryId::try_from(repository).expect("Invalid repository"),
            full_name.parse().expect("Invalid full name"),
            user_id,
            DateTime::TEST,
        );
        link.replace(&mut conn).expect("Failed to write the link");
        link
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
    let (private_key, _public_key) = app_key();
    GitHubApp::new(
        GITHUB_CLIENT_ID.parse().expect("Invalid client ID"),
        GITHUB_APP_SLUG.parse().expect("Invalid app slug"),
        &private_key,
        GITHUB_WEBHOOK_SECRET
            .parse()
            .expect("Invalid webhook secret"),
    )
    .expect("Failed to set up the GitHub App")
    .with_base_url(base)
    .expect("Failed to point the GitHub App at the base")
}
