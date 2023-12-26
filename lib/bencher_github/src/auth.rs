use oauth2::basic::BasicClient;
use oauth2::reqwest::http_client;
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, RedirectUrl, Scope,
    TokenResponse, TokenUrl,
};
use once_cell::sync::Lazy;
use url::Url;

use crate::GitHub;

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

impl GitHub {
    pub fn authorize_url(&self) -> Url {
        // Set up the config for the Github OAuth2 process.
        let client = BasicClient::new(
            self.client_id.clone(),
            Some(self.client_secret.clone()),
            AUTH_URL.clone(),
            Some(TOKEN_URL.clone()),
        )
        .set_redirect_uri(self.redirect_url.clone());

        let (authorize_url, _csrf_state) = client
            .authorize_url(CsrfToken::new_random)
            .add_scope(USER_EMAIL_SCOPE.clone())
            .url();

        authorize_url
    }
}
