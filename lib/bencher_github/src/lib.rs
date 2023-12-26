use bencher_valid::{NonEmpty, Secret};
use oauth2::{
    basic::BasicClient, reqwest::AsyncHttpClientError, AuthUrl, AuthorizationCode, ClientId,
    ClientSecret, CsrfToken, RedirectUrl, Scope, TokenResponse, TokenUrl,
};
use once_cell::sync::Lazy;
use url::Url;

#[allow(clippy::expect_used)]
static AUTH_URL: Lazy<AuthUrl> = Lazy::new(|| {
    AuthUrl::new("https://github.com/login/oauth/authorize".to_owned())
        .expect("Invalid authorization endpoint URL")
});

#[allow(clippy::expect_used)]
static TOKEN_URL: Lazy<TokenUrl> = Lazy::new(|| {
    TokenUrl::new("https://github.com/login/oauth/access_token".to_owned())
        .expect("Invalid token endpoint URL")
});

#[allow(clippy::expect_used)]
static USER_EMAIL_SCOPE: Lazy<Scope> = Lazy::new(|| Scope::new("user:email".to_owned()));

#[derive(Debug, Clone)]
pub struct GitHub {
    oauth2_client: BasicClient,
}

#[derive(Debug, thiserror::Error)]
pub enum GitHubError {
    #[error("Failed to exchange code for access token: {0}")]
    Exchange(
        #[from]
        oauth2::RequestTokenError<
            AsyncHttpClientError,
            oauth2::StandardErrorResponse<oauth2::basic::BasicErrorResponseType>,
        >,
    ),
}

impl GitHub {
    pub fn new(endpoint: &Url, client_id: NonEmpty, client_secret: Secret) -> Self {
        let client_id = ClientId::new(client_id.into());
        let client_secret = ClientSecret::new(client_secret.into());
        let mut endpoint = endpoint.clone();
        endpoint.set_path("/auth/github");
        let redirect_url = RedirectUrl::from_url(endpoint);

        let oauth2_client = BasicClient::new(
            client_id,
            Some(client_secret),
            AUTH_URL.clone(),
            Some(TOKEN_URL.clone()),
        )
        .set_redirect_uri(redirect_url);

        Self { oauth2_client }
    }

    pub fn authorize_url(&self) -> Url {
        self.oauth2_client
            .authorize_url(CsrfToken::new_random)
            .add_scope(USER_EMAIL_SCOPE.clone())
            .url()
            .0
    }

    pub async fn access_token(&self, code: NonEmpty) -> Result<NonEmpty, GitHubError> {
        let code = AuthorizationCode::new(code.into());
        let token = self
            .oauth2_client
            .exchange_code(code)
            .request_async(oauth2::reqwest::async_http_client)
            .await?;
        let access_token = token.access_token();
        todo!();
    }
}
