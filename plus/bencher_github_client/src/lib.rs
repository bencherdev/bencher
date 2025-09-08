use std::sync::LazyLock;

use bencher_valid::{Email, Jwt, NonEmpty, Secret, UserName};
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, EndpointNotSet, EndpointSet,
    TokenResponse as _, TokenUrl, basic::BasicClient, reqwest,
};
use octocrab::Octocrab;
use serde::Deserialize;
use url::Url;

#[expect(clippy::expect_used)]
static AUTH_URL: LazyLock<AuthUrl> = LazyLock::new(|| {
    AuthUrl::new("https://github.com/login/oauth/authorize".to_owned())
        .expect("Invalid authorization endpoint URL")
});

#[expect(clippy::expect_used)]
static TOKEN_URL: LazyLock<TokenUrl> = LazyLock::new(|| {
    TokenUrl::new("https://github.com/login/oauth/access_token".to_owned())
        .expect("Invalid token endpoint URL")
});

#[derive(Debug, Clone)]
pub struct GitHubClient {
    oauth2_client:
        BasicClient<EndpointSet, EndpointNotSet, EndpointNotSet, EndpointNotSet, EndpointSet>,
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
}

impl GitHubClient {
    pub fn new(client_id: NonEmpty, client_secret: Secret) -> Self {
        let client_id = ClientId::new(client_id.into());
        let client_secret = ClientSecret::new(client_secret.into());

        let oauth2_client = BasicClient::new(client_id)
            .set_client_secret(client_secret)
            .set_auth_uri(AUTH_URL.clone())
            .set_token_uri(TOKEN_URL.clone());

        Self { oauth2_client }
    }

    pub fn auth_url(&self, state: Jwt) -> Url {
        let state_fn = || CsrfToken::new(state.into());
        let (auth_url, _csrf_token) = self.oauth2_client.authorize_url(state_fn).url();
        auth_url
    }

    pub async fn oauth_user(&self, code: Secret) -> Result<(UserName, Email), GitHubClientError> {
        let code = AuthorizationCode::new(code.into());
        let http_client = reqwest::ClientBuilder::new()
            // Following redirects opens the client up to SSRF vulnerabilities.
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(GitHubClientError::Reqwest)?;
        let token = self
            .oauth2_client
            .exchange_code(code)
            .request_async(&http_client)
            .await
            .map_err(GitHubClientError::Exchange)?;

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
            .oauth(oauth)
            .build()
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
}

#[derive(Debug, Deserialize)]
struct GitHubUserEmail {
    email: String,
    verified: bool,
    primary: bool,
    #[expect(dead_code)]
    visibility: Option<String>,
}
