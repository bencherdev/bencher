//! OCI-specific test helpers

use bencher_json::RunnerUuid;
use bencher_token::OciAction;
use sha2::{Digest as _, Sha256};

use crate::{TestProject, TestServer, TestUser};

/// Compute the SHA-256 digest of the given data, returning an OCI-format digest string.
pub fn compute_digest(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let hash = hasher.finalize();
    format!("sha256:{}", hex::encode(hash))
}

impl TestServer {
    /// Generate an OCI token for a user with the specified repository and actions.
    ///
    /// This creates a valid OCI JWT token that can be used in Bearer authentication.
    #[expect(clippy::expect_used)]
    pub fn oci_token(&self, user: &TestUser, repository: &str, actions: &[OciAction]) -> String {
        self.token_key()
            .new_oci(
                user.email.clone(),
                u32::MAX,
                Some(repository.to_owned()),
                actions.to_vec(),
            )
            .expect("Failed to create OCI token")
            .to_string()
    }

    /// Generate an OCI token for pull access to a project.
    pub fn oci_pull_token(&self, user: &TestUser, project: &TestProject) -> String {
        self.oci_token(user, project.slug.as_ref(), &[OciAction::Pull])
    }

    /// Generate an OCI token for push access to a project.
    pub fn oci_push_token(&self, user: &TestUser, project: &TestProject) -> String {
        self.oci_token(user, project.slug.as_ref(), &[OciAction::Push])
    }

    /// Generate an OCI token for pull and push access to a project.
    pub fn oci_pull_push_token(&self, user: &TestUser, project: &TestProject) -> String {
        self.oci_token(
            user,
            project.slug.as_ref(),
            &[OciAction::Pull, OciAction::Push],
        )
    }

    /// Generate a runner OCI token for pull access to a project.
    ///
    /// Creates a valid runner OCI JWT token scoped to the given project slug.
    #[expect(clippy::expect_used)]
    pub fn oci_runner_pull_token(&self, runner_uuid: RunnerUuid, project_slug: &str) -> String {
        self.token_key()
            .new_oci_runner(
                runner_uuid,
                u32::MAX,
                Some(project_slug.to_owned()),
                vec![OciAction::Pull],
            )
            .expect("Failed to create runner OCI token")
            .to_string()
    }

    /// Generate a runner OCI token with the specified repository and actions.
    #[expect(clippy::expect_used)]
    pub fn oci_runner_token(
        &self,
        runner_uuid: RunnerUuid,
        repository: &str,
        actions: &[OciAction],
    ) -> String {
        self.token_key()
            .new_oci_runner(
                runner_uuid,
                u32::MAX,
                Some(repository.to_owned()),
                actions.to_vec(),
            )
            .expect("Failed to create runner OCI token")
            .to_string()
    }

    /// Upload a single blob and return its digest.
    #[expect(clippy::expect_used)]
    pub async fn upload_blob(&self, project_slug: &str, auth_token: &str, data: &[u8]) -> String {
        let digest = compute_digest(data);
        let resp = self
            .client
            .put(self.api_url(&format!("/v2/{project_slug}/blobs/uploads?digest={digest}")))
            .header(
                bencher_json::AUTHORIZATION,
                bencher_json::bearer_header(auth_token),
            )
            .body(data.to_vec())
            .send()
            .await
            .expect("Blob upload failed");
        assert_eq!(
            resp.status(),
            http::StatusCode::CREATED,
            "Blob upload should succeed"
        );
        digest
    }
}
