use bencher_json::{
    DateTime, Email, EmailChangeSecret, InviteSecret, JsonNewOrganization, JsonNewProject,
    JsonNewUserKey, JsonOrganization, JsonProject, JsonUserKeyCreated, Jwt, OrganizationSlug,
    OrganizationUuid, ProjectSlug, ProjectUuid, ResourceName, SessionMethod, SessionToken, UserKey,
    UserName, UserSlug, UserUuid, organization::member::OrganizationRole,
};
#[cfg(feature = "plus")]
use bencher_json::{JsonProjectKeyCreated, ProjectKey};
#[cfg(feature = "plus")]
use bencher_schema::model::organization::sso::QuerySso;
use bencher_schema::{
    context::DbConnection,
    model::{
        organization::{QueryOrganization, invite::InsertInvite},
        user::{
            InsertUser, QueryUser,
            email_change::InsertEmailChange,
            key::InsertUserKey,
            session::{
                InsertSession, QuerySession,
                cookie::{PendingCookie, SessionCookie},
            },
        },
    },
    schema,
};
use bencher_token::Claims;
use diesel::RunQueryDsl as _;
use jsonwebtoken::Header;
use reqwest::{
    StatusCode,
    header::{CONTENT_TYPE, COOKIE, HeaderMap, HeaderValue, SET_COOKIE},
};

use crate::{LogCapture, TestServer};

/// The `User-Agent` a test session signs in with.
const SESSION_USER_AGENT: &str =
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 14.6; rv:131.0) Gecko/20100101 Firefox/131.0";
/// The name of the user API key every test user is made with.
pub const TEST_USER_KEY_NAME: &str = "Test User Key";
/// When an old session JWT expires: 2159-12-06, past any clock a test sets, so nothing but its
/// audience can be why it is refused.
const OLD_SESSION_EXPIRATION: i64 = 5_993_643_609;

/// A test user, with the user API key their bearer requests send.
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

#[cfg(feature = "plus")]
pub struct TestProjectKey {
    pub key: ProjectKey,
}

impl TestServer {
    /// Sign up a new user, the server admin if they are the first, with a user API key.
    /// The account is made as a sign-in that proves the email makes one, on the API's own writer,
    /// and then joins the SSO organization at the email's domain if it is Enterprise, as a sign-in
    /// does once its write transaction is in.
    #[expect(clippy::expect_used, reason = "test helper for user signup flow")]
    pub async fn signup(&self, name: &str, email: &str) -> TestUser {
        let email: Email = email.parse().expect("Invalid email");
        let name: UserName = name.parse().expect("Invalid name");
        let now = self.context().clock.now();
        let (user, key) = {
            let mut conn = self.context().database.connection.lock().await;
            let user = sign_up(&mut conn, name, &email, now);
            let (insert_key, key) = InsertUserKey::from_json(
                user.id,
                JsonNewUserKey {
                    name: TEST_USER_KEY_NAME.parse().expect("Invalid user key name"),
                    ttl: None,
                },
                now,
            )
            .expect("Failed to make a user key");
            diesel::insert_into(schema::user_key::table)
                .values(&insert_key)
                .execute(&mut *conn)
                .expect("Failed to insert the user key");
            (user, key)
        };
        // Boxed, since the read of whether the organization is Enterprise may call Stripe.
        #[cfg(feature = "plus")]
        Box::pin(QuerySso::join_domain(
            self.log(),
            self.context(),
            &user,
            &email.domain(),
        ))
        .await
        .expect("Failed to join by SSO");

        TestUser {
            uuid: user.uuid,
            name: user.name,
            slug: user.slug,
            email: user.email,
            token: key.into(),
        }
    }

    /// A session JWT for the user, signed with the server's key as the old sign-in's confirmation
    /// signed one, for the tests that pin that the API refuses one.
    #[expect(clippy::expect_used, reason = "test helper forging a session JWT")]
    pub fn old_session_jwt(&self, user: &TestUser) -> Jwt {
        let token_key = self.token_key();
        let claims = Claims {
            aud: "client".to_owned(),
            exp: OLD_SESSION_EXPIRATION,
            iat: self.context().clock.now().timestamp(),
            iss: token_key.issuer.clone(),
            sub: user.email.clone(),
            oci: None,
        };
        jsonwebtoken::encode(&Header::default(), &claims, &token_key.encoding)
            .expect("Failed to sign a session JWT")
            .parse()
            .expect("Invalid session JWT")
    }

    /// Sign the user in to a new browser session now, returning the session cookie's value.
    pub fn session(&self, user: &TestUser) -> SessionToken {
        self.session_at(user, self.context().clock.now())
    }

    /// The headers of a request from the console in a session the user signed in to now, which may
    /// do what needs a fresh sign-in.
    #[expect(clippy::expect_used, reason = "test helper building headers")]
    pub fn fresh_session(&self, user: &TestUser) -> HeaderMap {
        let token = self.session(user);
        let mut headers = HeaderMap::new();
        headers.insert(
            COOKIE,
            session_cookie(&token)
                .parse()
                .expect("Invalid session cookie"),
        );
        headers.insert("Sec-Fetch-Site", HeaderValue::from_static("same-origin"));
        headers
    }

    /// Sign the user in to a new browser session at `now`, returning the session cookie's value.
    #[expect(clippy::expect_used, reason = "test helper inserting a session")]
    pub fn session_at(&self, user: &TestUser, now: DateTime) -> SessionToken {
        let mut conn = self.db_conn();
        let user_id = QueryUser::get_id(&mut conn, user.uuid).expect("Failed to get user ID");
        let (session, token) =
            InsertSession::new(user_id, SessionMethod::Email, Some(SESSION_USER_AGENT), now)
                .expect("Failed to create session");
        session
            .insert(&mut conn, now)
            .expect("Failed to insert session");
        token
    }

    /// Sign `email` in from the console by the code in the sign-in email, which the server logs to
    /// `log` in place of sending it, and return the session's cookie value.
    #[expect(clippy::expect_used, reason = "test helper signing in")]
    pub async fn sign_in_by_email(&self, email: &str, log: &LogCapture) -> String {
        let resp = self
            .client
            .post(self.api_url("/v0/auth/email"))
            .header("Sec-Fetch-Site", "same-origin")
            .header(CONTENT_TYPE, "application/json")
            .body(format!(r#"{{"email":"{email}"}}"#))
            .send()
            .await
            .expect("Failed to start a sign-in");
        assert_eq!(resp.status(), StatusCode::ACCEPTED, "Sign-in not started");
        let pending = PendingCookie::new(&self.context().console_url);
        let binding =
            cookie_value(resp.headers(), pending.name()).expect("No pending sign-in cookie");
        let (code, _link) = log.sign_in();
        let resp = self
            .client
            .post(self.api_url("/v0/auth/code"))
            .header(COOKIE, format!("{}={binding}", pending.name()))
            .header("Sec-Fetch-Site", "same-origin")
            .header(CONTENT_TYPE, "application/json")
            .body(format!(r#"{{"code":"{}"}}"#, code.as_ref()))
            .send()
            .await
            .expect("Failed to send the code");
        assert_eq!(resp.status(), StatusCode::OK, "Code not taken");
        cookie_value(resp.headers(), SessionCookie::Host.name()).expect("No session cookie")
    }

    /// Invite `email` to the organization from `inviter` now, as an invitation makes its row, and
    /// return the secret its link carries.
    pub fn invite(&self, inviter: &TestUser, org: &TestOrg, email: &str) -> InviteSecret {
        self.invite_at(inviter, org, email, self.context().clock.now())
    }

    /// [`Self::invite`] at `now`.
    #[expect(clippy::expect_used, reason = "test helper inserting an invitation")]
    pub fn invite_at(
        &self,
        inviter: &TestUser,
        org: &TestOrg,
        email: &str,
        now: DateTime,
    ) -> InviteSecret {
        let mut conn = self.db_conn();
        let organization_id =
            QueryOrganization::get_id(&mut conn, org.uuid).expect("Failed to get organization ID");
        let inviter_id = QueryUser::get_id(&mut conn, inviter.uuid).expect("Failed to get user ID");
        let (_invite, secret) = InsertInvite::invite(
            &mut conn,
            organization_id,
            email.parse().expect("Invalid email"),
            OrganizationRole::Leader,
            inviter_id,
            now,
        )
        .expect("Failed to invite");
        secret
    }

    /// Ask to change the user's email to `new_email` now, from `session` if any, as a user's own
    /// change makes its row, and return the secret its link carries.
    pub fn email_change(
        &self,
        user: &TestUser,
        session: Option<&SessionToken>,
        new_email: &str,
    ) -> EmailChangeSecret {
        self.email_change_at(user, session, new_email, self.context().clock.now())
    }

    /// [`Self::email_change`] at `now`.
    #[expect(clippy::expect_used, reason = "test helper inserting an email change")]
    pub fn email_change_at(
        &self,
        user: &TestUser,
        session: Option<&SessionToken>,
        new_email: &str,
        now: DateTime,
    ) -> EmailChangeSecret {
        let mut conn = self.db_conn();
        let user_id = QueryUser::get_id(&mut conn, user.uuid).expect("Failed to get user ID");
        let query_user = QueryUser::get(&mut conn, user_id).expect("Failed to get user");
        let session_id = session.map(|session| {
            QuerySession::from_token(&mut conn, session)
                .expect("Failed to find the session")
                .1
                .id
        });
        let (change, secret) = InsertEmailChange::new(
            query_user.id,
            session_id,
            query_user.email,
            new_email.parse().expect("Invalid email"),
            now,
        )
        .expect("Failed to make an email change");
        change
            .upsert(&mut conn)
            .expect("Failed to store an email change");
        secret
    }

    /// Create a new organization for the given user
    #[expect(clippy::expect_used, reason = "test helper for organization creation")]
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
            .header(
                bencher_json::AUTHORIZATION,
                bencher_json::bearer_header(&user.token),
            )
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
    #[expect(clippy::expect_used, reason = "test helper for project creation")]
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
            .header(
                bencher_json::AUTHORIZATION,
                bencher_json::bearer_header(&user.token),
            )
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

    #[cfg(feature = "plus")]
    #[expect(clippy::expect_used, reason = "test helper for project key creation")]
    pub async fn create_project_key(
        &self,
        user: &TestUser,
        project: &TestProject,
        name: &str,
    ) -> TestProjectKey {
        let project_slug: &str = project.slug.as_ref();

        let resp = self
            .client
            .post(self.api_url(&format!("/v0/projects/{project_slug}/keys")))
            .headers(self.fresh_session(user))
            .json(&serde_json::json!({"name": name}))
            .send()
            .await
            .expect("Failed to create project key");

        assert!(
            resp.status().is_success(),
            "Create project key failed: {}",
            resp.text().await.unwrap_or_default()
        );

        let key_created: JsonProjectKeyCreated =
            resp.json().await.expect("Failed to parse project key");

        TestProjectKey {
            key: key_created.key,
        }
    }

    /// Create a user API key in a session the user signed in to now.
    #[expect(clippy::expect_used, reason = "test helper for user key creation")]
    pub async fn create_user_key(&self, user: &TestUser, name: &str) -> UserKey {
        let user_slug: &str = user.slug.as_ref();
        let body = JsonNewUserKey {
            name: name.parse().expect("Invalid user key name"),
            ttl: None,
        };

        let resp = self
            .client
            .post(self.api_url(&format!("/v0/users/{user_slug}/keys")))
            .headers(self.fresh_session(user))
            .json(&body)
            .send()
            .await
            .expect("Failed to create user key");

        assert!(
            resp.status().is_success(),
            "Create user key failed: {}",
            resp.text().await.unwrap_or_default()
        );

        let key_created: JsonUserKeyCreated = resp.json().await.expect("Failed to parse user key");
        key_created.key
    }
}

/// Make the account a sign-in that proves `email` makes: the user, the server admin if the first,
/// who leads a personal organization, with the email verified.
#[expect(clippy::expect_used, reason = "test helper making an account")]
fn sign_up(conn: &mut DbConnection, name: UserName, email: &Email, now: DateTime) -> QueryUser {
    let (_insert_user, user) =
        InsertUser::sign_up(conn, name, None, email.clone()).expect("Failed to sign up");
    InsertUser::personal_organization(conn, &user)
        .expect("Failed to make the personal organization");
    QueryUser::verify_email(conn, user.id, email, now).expect("Failed to verify the email");
    user
}

/// The `Cookie` header that presents a session, in the form the test server's console URL calls for.
pub fn session_cookie(token: &SessionToken) -> String {
    format!("{}={}", SessionCookie::Host.name(), token.as_ref())
}

/// The value a `Set-Cookie` header in `headers` gives the cookie `name`.
fn cookie_value(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get_all(SET_COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .find_map(|cookie| {
            cookie
                .split(';')
                .next()?
                .strip_prefix(name)?
                .strip_prefix('=')
                .map(ToOwned::to_owned)
        })
}

/// The headers of a request that presents `token` as its bearer, as [`TestServer::fresh_session`]
/// gives the headers of one that presents a session.
#[expect(clippy::expect_used, reason = "test helper building headers")]
pub fn bearer(token: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        bencher_json::AUTHORIZATION,
        bencher_json::bearer_header(token)
            .parse()
            .expect("Invalid bearer header"),
    );
    headers
}
