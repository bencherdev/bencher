//! OCI-specific test helpers

use crate::{TestProject, TestServer, TestUser};

impl TestServer {
    /// Generate an OCI token for a user with the specified repository and actions.
    ///
    /// This creates a valid OCI JWT token that can be used in Bearer authentication.
    #[expect(clippy::expect_used)]
    pub fn oci_token(&self, user: &TestUser, repository: &str, actions: &[&str]) -> String {
        self.token_key()
            .new_oci(
                user.email.clone(),
                u32::MAX,
                Some(repository.to_owned()),
                actions.iter().map(|s| (*s).to_owned()).collect(),
            )
            .expect("Failed to create OCI token")
            .to_string()
    }

    /// Generate an OCI token for pull access to a project.
    pub fn oci_pull_token(&self, user: &TestUser, project: &TestProject) -> String {
        self.oci_token(user, project.slug.as_ref(), &["pull"])
    }

    /// Generate an OCI token for push access to a project.
    pub fn oci_push_token(&self, user: &TestUser, project: &TestProject) -> String {
        self.oci_token(user, project.slug.as_ref(), &["push"])
    }

    /// Generate an OCI token for pull and push access to a project.
    pub fn oci_pull_push_token(&self, user: &TestUser, project: &TestProject) -> String {
        self.oci_token(user, project.slug.as_ref(), &["pull", "push"])
    }
}
