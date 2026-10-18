use std::{borrow::Cow, sync::LazyLock};

use bencher_valid::{Email, Jwt, NonEmpty, Secret, UserName};
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, EndpointNotSet, EndpointSet,
    RedirectUrl, TokenResponse as _, TokenUrl,
    basic::{BasicClient, BasicTokenResponse},
};
use oauth2_reqwest::ReqwestClient;
use octocrab::Octocrab;
use serde::Deserialize;
use url::Url;

mod api;
mod app;
#[cfg(test)]
mod test_server;

#[cfg(any(test, feature = "test-base-url"))]
use api::join_path;
use api::{GITHUB_API_URL, GitHubApi};
pub use api::{GitHubApiError, GitHubInstallationPermission, GitHubRepository};
pub use app::{GitHubApp, GitHubAppError, GitHubInstallationToken};

#[expect(
    clippy::expect_used,
    reason = "constant URL literal, infallible at runtime"
)]
static AUTH_URL: LazyLock<AuthUrl> = LazyLock::new(|| {
    AuthUrl::new("https://github.com/login/oauth/authorize".to_owned())
        .expect("Invalid authorization endpoint URL")
});

#[expect(
    clippy::expect_used,
    reason = "constant URL literal, infallible at runtime"
)]
static TOKEN_URL: LazyLock<TokenUrl> = LazyLock::new(|| {
    TokenUrl::new("https://github.com/login/oauth/access_token".to_owned())
        .expect("Invalid token endpoint URL")
});

#[derive(Debug, Clone)]
pub struct GitHubClient {
    oauth2_client:
        BasicClient<EndpointSet, EndpointNotSet, EndpointNotSet, EndpointNotSet, EndpointSet>,
    api_url: Url,
}

#[derive(Debug, thiserror::Error)]
pub enum GitHubClientError {
    #[error("Failed to create a reqwest client: {0}")]
    Reqwest(reqwest::Error),
    #[error("Failed to exchange code for access token: {0}")]
    Exchange(
        oauth2::RequestTokenError<
            oauth2::HttpClientError<reqwest::Error>,
            oauth2::StandardErrorResponse<oauth2::basic::BasicErrorResponseType>,
        >,
    ),
    #[error("Failed to authenticate using access token: {0}")]
    Auth(octocrab::Error),
    #[error("Failed to get current authenticated user: {0}")]
    User(octocrab::Error),
    #[error("Failed to parse the current authenticated user login name: {0}")]
    BadLogin(bencher_valid::ValidError),
    #[error("Failed to get emails for the current authenticated user: {0}")]
    Emails(octocrab::Error),
    #[error("Failed to get a verified primary email for the current authenticated user")]
    NoPrimaryEmail,
    #[error("Failed to parse the verified primary email for the current authenticated user: {0}")]
    BadEmail(bencher_valid::ValidError),
    #[error("Failed to list the repositories of the current authenticated user: {0}")]
    Repositories(GitHubApiError),
}

impl GitHubClient {
    pub fn new(client_id: NonEmpty, client_secret: Secret) -> Self {
        let client_id = ClientId::new(client_id.into());
        let client_secret = ClientSecret::new(client_secret.into());

        let oauth2_client = BasicClient::new(client_id)
            .set_client_secret(client_secret)
            .set_auth_uri(AUTH_URL.clone())
            .set_token_uri(TOKEN_URL.clone());

        Self {
            oauth2_client,
            api_url: GITHUB_API_URL.clone(),
        }
    }

    /// Sends every code exchange and every API read to `base` instead of GitHub.
    #[cfg(any(test, feature = "test-base-url"))]
    #[must_use]
    pub fn with_base_url(self, base: Url) -> Self {
        let token_url = TokenUrl::from_url(join_path(&base, &["login", "oauth", "access_token"]));
        Self {
            oauth2_client: self.oauth2_client.set_token_uri(token_url),
            api_url: base,
        }
    }

    pub fn auth_url(&self, state: Jwt) -> Url {
        let state_fn = || CsrfToken::new(state.into());
        let (auth_url, _csrf_token) = self.oauth2_client.authorize_url(state_fn).url();
        auth_url
    }

    /// The authorize URL, with GitHub told to send the user back to `redirect_url`.
    pub fn auth_url_with_redirect(&self, state: Jwt, redirect_url: Url) -> Url {
        let state_fn = || CsrfToken::new(state.into());
        let (auth_url, _csrf_token) = self
            .oauth2_client
            .authorize_url(state_fn)
            .set_redirect_uri(Cow::Owned(RedirectUrl::from_url(redirect_url)))
            .url();
        auth_url
    }

    pub async fn oauth_user(&self, code: Secret) -> Result<(UserName, Email), GitHubClientError> {
        let token = self.exchange(code, None).await?;

        let oauth = octocrab::auth::OAuth {
            access_token: token.access_token().secret().clone().into(),
            token_type: token.token_type().as_ref().to_owned(),
            scope: token
                .scopes()
                .map(|s| s.iter().map(AsRef::as_ref).map(ToOwned::to_owned).collect())
                .unwrap_or_default(),
            expires_in: None,
            refresh_token: None,
            refresh_token_expires_in: None,
        };
        let github_client = Octocrab::builder()
            .base_uri(self.api_url.as_str())
            .and_then(|builder| builder.oauth(oauth).build())
            .map_err(GitHubClientError::Auth)?;

        let user_name = github_client
            .current()
            .user()
            .await
            .map_err(GitHubClientError::User)
            .and_then(|user| user.login.parse().map_err(GitHubClientError::BadLogin))?;

        let email = github_client
            .get::<Vec<GitHubUserEmail>, _, &str>("/user/emails", None)
            .await
            .map_err(GitHubClientError::Emails)?
            .into_iter()
            .find_map(|email| (email.primary && email.verified).then_some(email.email))
            .ok_or(GitHubClientError::NoPrimaryEmail)
            .and_then(|email| email.parse().map_err(GitHubClientError::BadEmail))?;

        Ok((user_name, email))
    }

    /// Every repository the user can push to, in every installation of the GitHub App the user
    /// can see that is not suspended. `redirect_url` is the one the authorize URL named. The
    /// user's token is dropped before this returns.
    pub async fn oauth_repositories(
        &self,
        code: Secret,
        redirect_url: Url,
    ) -> Result<Vec<GitHubRepository>, GitHubClientError> {
        let token = self.exchange(code, Some(redirect_url)).await?;
        GitHubApi::new(self.api_url.clone())
            .map_err(GitHubClientError::Reqwest)?
            .user_repositories(token.access_token())
            .await
            .map_err(GitHubClientError::Repositories)
    }

    async fn exchange(
        &self,
        code: Secret,
        redirect_url: Option<Url>,
    ) -> Result<BasicTokenResponse, GitHubClientError> {
        let code = AuthorizationCode::new(code.into());
        let http_client = ReqwestClient::from(
            reqwest::ClientBuilder::new()
                // Following redirects opens the client up to SSRF vulnerabilities.
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .map_err(GitHubClientError::Reqwest)?,
        );
        let request = self.oauth2_client.exchange_code(code);
        // RFC 6749 requires the exchange to repeat the redirect that the authorize request named.
        let request = if let Some(redirect_url) = redirect_url {
            request.set_redirect_uri(Cow::Owned(RedirectUrl::from_url(redirect_url)))
        } else {
            request
        };
        request
            .request_async(&http_client)
            .await
            .map_err(GitHubClientError::Exchange)
    }
}

#[derive(Debug, Deserialize)]
struct GitHubUserEmail {
    email: String,
    primary: bool,
    verified: bool,
    #[expect(dead_code, reason = "deserialized from GitHub API response")]
    visibility: Option<String>,
}

#[cfg(test)]
mod tests {
    use bencher_valid::{GitHubInstallationId, GitHubRepositoryId, Jwt};
    use url::Url;

    use super::{GitHubClient, GitHubInstallationPermission, GitHubRepository};
    use crate::test_server::Server;

    const REDIRECT_URL: &str = "https://console.example/github/return";
    const CODE: &str = "code-marker";
    const USER_TOKEN: &str = "user-token-marker";

    fn client() -> GitHubClient {
        GitHubClient::new(
            "Iv1.0123456789abcdef".parse().unwrap(),
            "client-secret".parse().unwrap(),
        )
    }

    fn exchanged() -> (u16, String) {
        (
            200,
            format!(r#"{{"access_token": "{USER_TOKEN}", "token_type": "bearer", "scope": ""}}"#),
        )
    }

    fn query(url: &Url, key: &str) -> Option<String> {
        url.query_pairs()
            .find_map(|(k, v)| (k == key).then(|| v.into_owned()))
    }

    // Kills dropping the redirect from the authorize URL, or adding one to sign-in's.
    #[test]
    fn only_the_redirect_auth_url_names_a_redirect() {
        let client = client();
        let state = Jwt::test_token();

        let with_redirect =
            client.auth_url_with_redirect(state.clone(), REDIRECT_URL.parse().unwrap());
        let without = client.auth_url(state.clone());

        assert_eq!(
            query(&with_redirect, "redirect_uri").as_deref(),
            Some(REDIRECT_URL)
        );
        assert_eq!(query(&without, "redirect_uri"), None);
        for url in [&with_redirect, &without] {
            assert_eq!(query(url, "state").as_deref(), Some(state.as_ref()));
        }
    }

    // Kills sending sign-in's user or email read anywhere but the base, or under any token but
    // the one the exchange returned.
    #[tokio::test]
    async fn sign_in_reads_the_user_from_the_base() {
        let profile = "https://github.example/octo-cat";
        let server = Server::start(vec![
            exchanged(),
            (
                200,
                format!(
                    r#"{{"login": "octo-cat", "id": 1, "node_id": "U_1", "avatar_url": "{profile}", "gravatar_id": "", "url": "{profile}", "html_url": "{profile}", "followers_url": "{profile}", "following_url": "{profile}", "gists_url": "{profile}", "starred_url": "{profile}", "subscriptions_url": "{profile}", "organizations_url": "{profile}", "repos_url": "{profile}", "events_url": "{profile}", "received_events_url": "{profile}", "type": "User", "site_admin": false}}"#
                ),
            ),
            (
                200,
                r#"[{"email": "octo-cat@example.com", "primary": true, "verified": true, "visibility": null}]"#
                    .to_owned(),
            ),
        ])
        .await;
        let client = client().with_base_url(server.base());

        let (user_name, email) = client.oauth_user(CODE.parse().unwrap()).await.unwrap();

        assert_eq!(
            (user_name.as_ref(), email.as_ref()),
            ("octo-cat", "octo-cat@example.com")
        );
        let [exchange, user, emails] = server.received().await.try_into().ok().unwrap();
        assert_eq!(
            exchange.request_line(),
            "POST /github/login/oauth/access_token HTTP/1.1"
        );
        assert_eq!(user.request_line(), "GET /github/user HTTP/1.1");
        assert_eq!(emails.request_line(), "GET /github/user/emails HTTP/1.1");
        for request in [&user, &emails] {
            assert_eq!(
                request.header("authorization"),
                Some(format!("bearer {USER_TOKEN}").as_str())
            );
        }
    }

    // Kills dropping the redirect from the exchange, moving the exchange or the listing anywhere
    // but the base, and listing under any token but the one the exchange returned.
    #[tokio::test]
    async fn the_repositories_exchange_repeats_the_redirect() {
        let server = Server::start(vec![
            exchanged(),
            (
                200,
                r#"{"total_count": 1, "installations": [{"id": 11, "suspended_at": null, "permissions": {"contents": "write"}}]}"#
                    .to_owned(),
            ),
            (
                200,
                r#"{"total_count": 1, "repositories": [{"id": 1, "name": "repo", "owner": {"login": "octo-cat"}, "permissions": {"push": true}}]}"#
                    .to_owned(),
            ),
        ])
        .await;
        let client = client().with_base_url(server.base());

        let repositories = client
            .oauth_repositories(CODE.parse().unwrap(), REDIRECT_URL.parse().unwrap())
            .await
            .unwrap();

        assert_eq!(
            repositories,
            [GitHubRepository {
                installation: GitHubInstallationId::try_from(11).unwrap(),
                permission: GitHubInstallationPermission::Granted,
                id: GitHubRepositoryId::try_from(1).unwrap(),
                full_name: "octo-cat/repo".parse().unwrap(),
            }]
        );
        let [exchange, installations, listed] = server.received().await.try_into().ok().unwrap();
        assert_eq!(
            exchange.request_line(),
            "POST /github/login/oauth/access_token HTTP/1.1"
        );
        let form = exchange.form();
        assert_eq!(form.get("code").map(String::as_str), Some(CODE));
        assert_eq!(
            form.get("redirect_uri").map(String::as_str),
            Some(REDIRECT_URL)
        );
        for request in [&installations, &listed] {
            assert_eq!(
                request.header("authorization"),
                Some(format!("Bearer {USER_TOKEN}").as_str())
            );
        }
    }
}
