use std::sync::LazyLock;

use bencher_valid::{Email, Jwt, NonEmpty, Secret, UserName};
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, EndpointNotSet, EndpointSet,
    RedirectUrl, Scope, TokenResponse as _, TokenUrl, basic::BasicClient,
};
use serde::Deserialize;
use url::Url;

#[expect(clippy::expect_used)]
static AUTH_URL: LazyLock<AuthUrl> = LazyLock::new(|| {
    AuthUrl::new("https://accounts.google.com/o/oauth2/v2/auth".to_owned())
        .expect("Invalid authorization endpoint URL")
});

#[expect(clippy::expect_used)]
static TOKEN_URL: LazyLock<TokenUrl> = LazyLock::new(|| {
    TokenUrl::new("https://oauth2.googleapis.com/token".to_owned())
        .expect("Invalid token endpoint URL")
});

// Replaced deprecated plus.me scope with OpenID Connect scopes
static OPENID_SCOPE: LazyLock<Scope> = LazyLock::new(|| Scope::new("openid".to_owned()));
static EMAIL_SCOPE: LazyLock<Scope> = LazyLock::new(|| Scope::new("email".to_owned()));
static PROFILE_SCOPE: LazyLock<Scope> = LazyLock::new(|| Scope::new("profile".to_owned()));

#[derive(Debug, Clone)]
pub struct GoogleClient {
    oauth2_client:
        BasicClient<EndpointSet, EndpointNotSet, EndpointNotSet, EndpointNotSet, EndpointSet>,
}

#[derive(Debug, thiserror::Error)]
pub enum GoogleClientError {
    #[error("Failed to create a reqwest client: {0}")]
    Reqwest(reqwest::Error),
    #[error("Failed to exchange code for access token: {0}")]
    Exchange(
        oauth2::RequestTokenError<
            oauth2::HttpClientError<reqwest::Error>,
            oauth2::StandardErrorResponse<oauth2::basic::BasicErrorResponseType>,
        >,
    ),
    #[error("Failed to call Google UserInfo endpoint: {0}")]
    UserInfoReq(reqwest::Error),
    #[error("Failed to parse Google UserInfo response: {0}")]
    UserInfoJson(reqwest::Error),
    #[error("Google account email is not verified")]
    NotEmailVerified,
    #[error("Missing email in Google UserInfo response")]
    NoEmail,
    #[error("Failed to parse user name: {0}")]
    BadUserName(bencher_valid::ValidError),
    #[error("Failed to parse email: {0}")]
    BadEmail(bencher_valid::ValidError),
}

impl GoogleClient {
    pub fn new(client_id: NonEmpty, client_secret: Secret, redirect_url: Url) -> Self {
        let client_id = ClientId::new(client_id.into());
        let client_secret = ClientSecret::new(client_secret.into());
        let redirect_url = RedirectUrl::from_url(redirect_url);

        let oauth2_client = BasicClient::new(client_id)
            .set_client_secret(client_secret)
            .set_auth_uri(AUTH_URL.clone())
            .set_token_uri(TOKEN_URL.clone())
            .set_redirect_uri(redirect_url);

        Self { oauth2_client }
    }

    pub fn auth_url(&self, state: Jwt) -> Url {
        let state_fn = || CsrfToken::new(state.into());
        let (auth_url, _csrf_token) = self
            .oauth2_client
            .authorize_url(state_fn)
            .add_scope(OPENID_SCOPE.clone())
            .add_scope(PROFILE_SCOPE.clone())
            .add_scope(EMAIL_SCOPE.clone())
            .url();
        auth_url
    }

    pub async fn oauth_user(&self, code: Secret) -> Result<(UserName, Email), GoogleClientError> {
        let code = AuthorizationCode::new(code.into());
        let http_client = reqwest::ClientBuilder::new()
            // Following redirects opens the client up to SSRF vulnerabilities.
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(GoogleClientError::Reqwest)?;
        let token = self
            .oauth2_client
            .exchange_code(code)
            .request_async(&http_client)
            .await
            .map_err(GoogleClientError::Exchange)?;
        let access_token = token.access_token().secret();

        // Call OpenID Connect UserInfo endpoint
        let user_info_resp = http_client
            .get("https://openidconnect.googleapis.com/v1/userinfo")
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(GoogleClientError::UserInfoReq)?;

        let user_info: GoogleUserInfo = user_info_resp
            .json()
            .await
            .map_err(GoogleClientError::UserInfoJson)?;

        if !user_info.email_verified {
            return Err(GoogleClientError::NotEmailVerified);
        }

        let email_str = user_info
            .email
            .as_deref()
            .ok_or(GoogleClientError::NoEmail)?;
        let email: Email = email_str.parse().map_err(GoogleClientError::BadEmail)?;

        // Choose a username: name > given_name > email local-part
        let candidate_name = user_info
            .name
            .as_deref()
            .or(user_info.given_name.as_deref())
            .or(user_info.family_name.as_deref())
            .unwrap_or_else(|| email_str.split('@').next().unwrap_or(email_str));
        let user_name: UserName = candidate_name
            .parse()
            .map_err(GoogleClientError::BadUserName)?;

        Ok((user_name, email))
    }
}

/// Google `UserInfo` response
/// <https://openid.net/specs/openid-connect-core-1_0.html#UserInfo>
#[derive(Debug, Deserialize)]
struct GoogleUserInfo {
    name: Option<String>,
    given_name: Option<String>,
    family_name: Option<String>,
    email: Option<String>,
    email_verified: bool,
}
