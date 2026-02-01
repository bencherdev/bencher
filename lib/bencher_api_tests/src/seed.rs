use bencher_json::{
    Email, JsonConfirm, JsonNewOrganization, JsonNewProject, JsonOrganization, JsonProject,
    JsonSignup, OrganizationSlug, OrganizationUuid, ProjectSlug, ProjectUuid, ResourceName,
    UserName, UserSlug, UserUuid, system::auth::JsonAuthUser,
};

use crate::TestServer;

/// A test user with their JWT token
pub struct TestUser {
    pub uuid: UserUuid,
    pub name: UserName,
    pub slug: UserSlug,
    pub email: Email,
    pub token: String,
}

/// A test organization
pub struct TestOrg {
    pub uuid: OrganizationUuid,
    pub name: ResourceName,
    pub slug: OrganizationSlug,
}

/// A test project
pub struct TestProject {
    pub uuid: ProjectUuid,
    pub name: ResourceName,
    pub slug: ProjectSlug,
}

impl TestServer {
    /// Sign up a new user and return their info with a valid token.
    /// This creates the user via the signup endpoint and confirms them.
    #[expect(clippy::expect_used)]
    pub async fn signup(&self, name: &str, email: &str) -> TestUser {
        let email: Email = email.parse().expect("Invalid email");
        let name: UserName = name.parse().expect("Invalid name");

        #[cfg(feature = "plus")]
        let body = JsonSignup {
            name: name.clone(),
            slug: None,
            email: email.clone(),
            plan: None,
            invite: None,
            claim: None,
            i_agree: true,
            recaptcha_token: None,
        };
        #[cfg(not(feature = "plus"))]
        let body = JsonSignup {
            name: name.clone(),
            slug: None,
            email: email.clone(),
            invite: None,
            claim: None,
            i_agree: true,
        };

        let resp = self
            .client
            .post(self.api_url("/v0/auth/signup"))
            .json(&body)
            .send()
            .await
            .expect("Failed to send signup request");

        assert!(
            resp.status().is_success(),
            "Signup failed: {}",
            resp.text().await.unwrap_or_default()
        );

        // Generate an auth token to confirm the user
        let auth_token = self
            .token_key()
            .new_auth(email.clone(), u32::MAX)
            .expect("Failed to generate auth token");

        // Confirm the user to get their actual data and a client token
        let confirm_body = JsonConfirm { token: auth_token };

        let confirm_resp = self
            .client
            .post(self.api_url("/v0/auth/confirm"))
            .json(&confirm_body)
            .send()
            .await
            .expect("Failed to send confirm request");

        assert!(
            confirm_resp.status().is_success(),
            "Confirm failed: {}",
            confirm_resp.text().await.unwrap_or_default()
        );

        let auth_user: JsonAuthUser = confirm_resp
            .json()
            .await
            .expect("Failed to parse confirm response");

        TestUser {
            uuid: auth_user.user.uuid,
            name: auth_user.user.name,
            slug: auth_user.user.slug,
            email: auth_user.user.email,
            token: auth_user.token.to_string(),
        }
    }

    /// Create a new organization for the given user
    #[expect(clippy::expect_used)]
    pub async fn create_org(&self, user: &TestUser, name: &str) -> TestOrg {
        let name: ResourceName = name.parse().expect("Invalid org name");
        let slug_str = name.as_ref().to_lowercase().replace(' ', "-");
        let slug: OrganizationSlug = slug_str.parse().expect("Invalid org slug");

        let body = JsonNewOrganization {
            name: name.clone(),
            slug: Some(slug.clone()),
        };

        let resp = self
            .client
            .post(self.api_url("/v0/organizations"))
            .header("Authorization", self.bearer(&user.token))
            .json(&body)
            .send()
            .await
            .expect("Failed to send create org request");

        assert!(
            resp.status().is_success(),
            "Create org failed: {}",
            resp.text().await.unwrap_or_default()
        );

        let org: JsonOrganization = resp.json().await.expect("Failed to parse org response");

        TestOrg {
            uuid: org.uuid,
            name: org.name,
            slug: org.slug,
        }
    }

    /// Create a new project in the given organization
    #[expect(clippy::expect_used)]
    pub async fn create_project(&self, user: &TestUser, org: &TestOrg, name: &str) -> TestProject {
        let name: ResourceName = name.parse().expect("Invalid project name");
        let slug_str = name.as_ref().to_lowercase().replace(' ', "-");
        let slug: ProjectSlug = slug_str.parse().expect("Invalid project slug");

        let body = JsonNewProject {
            name: name.clone(),
            slug: Some(slug.clone()),
            url: None,
            visibility: None,
        };

        let org_slug: &str = org.slug.as_ref();
        let resp = self
            .client
            .post(self.api_url(&format!("/v0/organizations/{org_slug}/projects")))
            .header("Authorization", self.bearer(&user.token))
            .json(&body)
            .send()
            .await
            .expect("Failed to send create project request");

        assert!(
            resp.status().is_success(),
            "Create project failed: {}",
            resp.text().await.unwrap_or_default()
        );

        let project: JsonProject = resp.json().await.expect("Failed to parse project response");

        TestProject {
            uuid: project.uuid,
            name: project.name,
            slug: project.slug,
        }
    }

    /// Create a user with a confirmed account (bypassing email confirmation).
    /// This inserts directly into the database for testing authenticated endpoints.
    pub async fn create_confirmed_user(&self, name: &str, email: &str) -> TestUser {
        // For now, use signup flow
        // In a full implementation, we'd insert directly into DB and skip email confirmation
        self.signup(name, email).await
    }
}
