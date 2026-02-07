//! OCI-specific test helpers

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
}
