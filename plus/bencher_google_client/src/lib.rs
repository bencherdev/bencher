use std::{sync::LazyLock, time::Duration};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use bencher_valid::{
    DateTime, Email, NonEmpty, PkceVerifier, ProviderAccountId, ProviderNonce, ProviderState,
    Secret, UserName,
};
use oauth2::{
    AuthUrl, AuthorizationCode, Client, ClientId, ClientSecret, CsrfToken, EndpointNotSet,
    EndpointSet, ExtraTokenFields, PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, Scope,
    StandardRevocableToken, StandardTokenResponse, TokenUrl,
    basic::{
        BasicErrorResponse, BasicRevocationErrorResponse, BasicTokenIntrospectionResponse,
        BasicTokenType,
    },
};
use oauth2_reqwest::ReqwestClient;
use serde::{Deserialize, Serialize};
use url::Url;

#[expect(
    clippy::expect_used,
    reason = "constant URL literal, infallible at runtime"
)]
static AUTH_URL: LazyLock<AuthUrl> = LazyLock::new(|| {
    AuthUrl::new("https://accounts.google.com/o/oauth2/v2/auth".to_owned())
        .expect("Invalid authorization endpoint URL")
});

#[expect(
    clippy::expect_used,
    reason = "constant URL literal, infallible at runtime"
)]
static TOKEN_URL: LazyLock<TokenUrl> = LazyLock::new(|| {
    TokenUrl::new("https://oauth2.googleapis.com/token".to_owned())
        .expect("Invalid token endpoint URL")
});

static OPENID_SCOPE: LazyLock<Scope> = LazyLock::new(|| Scope::new("openid".to_owned()));
static EMAIL_SCOPE: LazyLock<Scope> = LazyLock::new(|| Scope::new("email".to_owned()));
static PROFILE_SCOPE: LazyLock<Scope> = LazyLock::new(|| Scope::new("profile".to_owned()));

/// The issuers Google's ID tokens name.
/// <https://developers.google.com/identity/openid-connect/openid-connect#validatinganidtoken>
const ISSUERS: [&str; 2] = ["https://accounts.google.com", "accounts.google.com"];
/// How long the exchange waits for Google before it gives up.
const TIMEOUT: Duration = Duration::from_secs(10);

type GoogleTokenResponse = StandardTokenResponse<IdTokenFields, BasicTokenType>;

type OAuthClient = Client<
    BasicErrorResponse,
    GoogleTokenResponse,
    BasicTokenIntrospectionResponse,
    StandardRevocableToken,
    BasicRevocationErrorResponse,
    EndpointSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointSet,
>;

#[derive(Debug, Clone)]
pub struct GoogleClient {
    oauth2_client: OAuthClient,
}

#[derive(Debug, thiserror::Error)]
pub enum GoogleClientError {
    #[error("Failed to create a reqwest client: {0}")]
    Reqwest(reqwest::Error),
    #[error("Failed to exchange code for tokens: {0}")]
    Exchange(
        oauth2::RequestTokenError<
            oauth2::HttpClientError<reqwest::Error>,
            oauth2::StandardErrorResponse<oauth2::basic::BasicErrorResponseType>,
        >,
    ),
    #[error("Google's ID token is not a JSON Web Token")]
    IdToken,
    #[error("Failed to read the claims of Google's ID token: {0}")]
    IdTokenClaims(serde_json::Error),
    #[error("Google's ID token names another issuer")]
    Issuer,
    #[error("Google's ID token is for another client")]
    Audience,
    #[error("Google's ID token has expired")]
    Expired,
    #[error("Google's ID token carries another nonce")]
    Nonce,
    #[error("Google account email is not verified")]
    NotEmailVerified,
    #[error("Google's ID token carries no email")]
    NoEmail,
    #[error("Failed to parse email: {0}")]
    BadEmail(bencher_valid::ValidError),
    #[error("Failed to parse the account's ID: {0}")]
    BadAccount(bencher_valid::ValidError),
}

impl GoogleClient {
    pub fn new(client_id: NonEmpty, client_secret: Secret, redirect_url: Url) -> Self {
        let client_id = ClientId::new(client_id.into());
        let client_secret = ClientSecret::new(client_secret.into());
        let redirect_url = RedirectUrl::from_url(redirect_url);

        let oauth2_client = Client::new(client_id)
            .set_client_secret(client_secret)
            .set_auth_uri(AUTH_URL.clone())
            .set_token_uri(TOKEN_URL.clone())
            .set_redirect_uri(redirect_url);

        Self { oauth2_client }
    }

    /// Sends every code exchange to `base` instead of Google.
    #[cfg(any(test, feature = "test-base-url"))]
    #[must_use]
    pub fn with_base_url(self, base: &Url) -> Self {
        let mut token_url = base.clone();
        token_url.set_path("/token");
        Self {
            oauth2_client: self
                .oauth2_client
                .set_token_uri(TokenUrl::from_url(token_url)),
        }
    }

    /// The authorize URL that starts a sign-in: the identity scopes of `OpenID` Connect, the
    /// `S256` challenge of `verifier`, and the nonce its ID token must carry.
    pub fn sign_in_url(
        &self,
        state: &ProviderState,
        verifier: &PkceVerifier,
        nonce: &ProviderNonce,
    ) -> Url {
        let state_fn = || CsrfToken::new(state.as_ref().to_owned());
        let (auth_url, _csrf_token) = self
            .oauth2_client
            .authorize_url(state_fn)
            .add_scope(OPENID_SCOPE.clone())
            .add_scope(EMAIL_SCOPE.clone())
            .add_scope(PROFILE_SCOPE.clone())
            .set_pkce_challenge(PkceCodeChallenge::from_code_verifier_sha256(
                &pkce_verifier(verifier),
            ))
            .add_extra_param("nonce", nonce.as_ref())
            .url();
        auth_url
    }

    /// The account a sign-in's `code` names, by the ID token of an exchange that proves
    /// `verifier`. The token comes straight from Google's token endpoint over TLS, so its claims
    /// are checked in place of its signature.
    pub async fn sign_in_user(
        &self,
        code: Secret,
        verifier: &PkceVerifier,
        nonce: &ProviderNonce,
        now: DateTime,
    ) -> Result<GoogleSignIn, GoogleClientError> {
        let http_client = ReqwestClient::from(
            reqwest::ClientBuilder::new()
                // Following redirects opens the client up to SSRF vulnerabilities.
                .redirect(reqwest::redirect::Policy::none())
                .timeout(TIMEOUT)
                .build()
                .map_err(GoogleClientError::Reqwest)?,
        );
        let token = self
            .oauth2_client
            .exchange_code(AuthorizationCode::new(code.into()))
            .set_pkce_verifier(pkce_verifier(verifier))
            .request_async(&http_client)
            .await
            .map_err(GoogleClientError::Exchange)?;
        self.signed_in(&token.extra_fields().id_token, nonce, now)
    }

    /// Who an ID token names, once its issuer, its audience, its expiry, its nonce, and its
    /// verified email check out.
    fn signed_in(
        &self,
        id_token: &str,
        nonce: &ProviderNonce,
        now: DateTime,
    ) -> Result<GoogleSignIn, GoogleClientError> {
        let claims = id_token
            .split('.')
            .nth(1)
            .and_then(|payload| URL_SAFE_NO_PAD.decode(payload).ok())
            .ok_or(GoogleClientError::IdToken)?;
        let IdTokenClaims {
            iss,
            aud,
            exp,
            nonce: token_nonce,
            sub,
            email,
            email_verified,
            hd,
            name,
            given_name,
            family_name,
        } = serde_json::from_slice(&claims).map_err(GoogleClientError::IdTokenClaims)?;
        if !ISSUERS.contains(&iss.as_str()) {
            return Err(GoogleClientError::Issuer);
        }
        if aud != self.oauth2_client.client_id().as_str() {
            return Err(GoogleClientError::Audience);
        }
        if exp <= now.timestamp() {
            return Err(GoogleClientError::Expired);
        }
        if token_nonce.as_deref() != Some(nonce.as_ref()) {
            return Err(GoogleClientError::Nonce);
        }
        if !email_verified {
            return Err(GoogleClientError::NotEmailVerified);
        }
        let email: Email = email
            .ok_or(GoogleClientError::NoEmail)?
            .parse()
            .map_err(GoogleClientError::BadEmail)?;
        Ok(GoogleSignIn {
            sub: sub.parse().map_err(GoogleClientError::BadAccount)?,
            email,
            name: [name, given_name, family_name]
                .into_iter()
                .flatten()
                .find_map(|name| name.parse().ok()),
            hd: hd.and_then(|hd| hd.parse().ok()),
        })
    }
}

/// Who signed in with Google: the account's `sub`, its verified email, its name if it gives one
/// Bencher can keep, and the hosted domain of a Google Workspace account.
#[derive(Debug, Clone)]
pub struct GoogleSignIn {
    pub sub: ProviderAccountId,
    pub email: Email,
    pub name: Option<UserName>,
    pub hd: Option<NonEmpty>,
}

fn pkce_verifier(verifier: &PkceVerifier) -> PkceCodeVerifier {
    PkceCodeVerifier::new(verifier.as_ref().to_owned())
}

/// The ID token in the token endpoint's answer.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct IdTokenFields {
    id_token: String,
}

impl ExtraTokenFields for IdTokenFields {}

/// The claims of a Google ID token that a sign-in reads.
/// <https://openid.net/specs/openid-connect-core-1_0.html#IDToken>
#[derive(Deserialize)]
struct IdTokenClaims {
    iss: String,
    aud: String,
    exp: i64,
    nonce: Option<String>,
    sub: String,
    email: Option<String>,
    #[serde(default)]
    email_verified: bool,
    hd: Option<String>,
    name: Option<String>,
    given_name: Option<String>,
    family_name: Option<String>,
}

#[cfg(test)]
mod tests {
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
    use bencher_valid::{DateTime, ProviderNonce};
    use serde_json::{Value, json};
    use url::Url;

    use super::{GoogleClient, GoogleClientError};

    const CLIENT_ID: &str = "client-id.apps.googleusercontent.com";
    const STATE: &str = "c3RhdGVzdGF0ZXN0YXRlc3RhdGVzdGF0ZXN0YXRlc3Q";
    const VERIFIER: &str = "dmVyaWZpZXJ2ZXJpZmllcnZlcmlmaWVydmVyaWZpZXI";
    const NONCE: &str = "bm9uY2Utbm9uY2Vub25jZS1ub25jZW5vbmNlLW5vbmM";
    const OTHER_NONCE: &str = "YmluZGluZ2JpbmRpbmdiaW5kaW5nYmluZGluZ2JpbmQ";
    const NOW: i64 = 1_789_344_000;

    fn client() -> GoogleClient {
        GoogleClient::new(
            CLIENT_ID.parse().unwrap(),
            "client-secret".parse().unwrap(),
            "https://console.example/auth/google".parse().unwrap(),
        )
    }

    fn query(url: &Url, key: &str) -> Option<String> {
        url.query_pairs()
            .find_map(|(k, v)| (k == key).then(|| v.into_owned()))
    }

    fn claims() -> Value {
        json!({
            "iss": "https://accounts.google.com",
            "aud": CLIENT_ID,
            "exp": NOW + 60,
            "nonce": NONCE,
            "sub": "110169484474386276334",
            "email": "ada@example.com",
            "email_verified": true,
            "hd": "example.com",
            "name": "Ada Lovelace",
        })
    }

    fn id_token(claims: &Value) -> String {
        let segment = |value: &Value| URL_SAFE_NO_PAD.encode(value.to_string());
        format!(
            "{}.{}.c2lnbmF0dXJl",
            segment(&json!({"alg": "RS256", "typ": "JWT"})),
            segment(claims)
        )
    }

    fn nonce() -> ProviderNonce {
        NONCE.parse().unwrap()
    }

    fn now() -> DateTime {
        DateTime::try_from(NOW).unwrap()
    }

    fn signed_in(claims: &Value) -> Result<super::GoogleSignIn, GoogleClientError> {
        client().signed_in(&id_token(claims), &nonce(), now())
    }

    // Kills a sign-in URL without OpenID Connect's identity scopes, the S256 challenge of its own
    // verifier, its nonce, its state, or the registered redirect.
    #[test]
    fn sign_in_url_asks_for_an_id_token_with_its_challenge_and_nonce() {
        let url = client().sign_in_url(
            &STATE.parse().unwrap(),
            &VERIFIER.parse().unwrap(),
            &NONCE.parse().unwrap(),
        );

        let challenge = {
            use aws_lc_rs::digest::{SHA256, digest};
            URL_SAFE_NO_PAD.encode(digest(&SHA256, VERIFIER.as_bytes()))
        };
        assert_eq!(query(&url, "code_challenge"), Some(challenge));
        assert_eq!(
            query(&url, "code_challenge_method").as_deref(),
            Some("S256")
        );
        assert_eq!(query(&url, "nonce").as_deref(), Some(NONCE));
        assert_eq!(query(&url, "state").as_deref(), Some(STATE));
        assert_eq!(
            query(&url, "scope").as_deref(),
            Some("openid email profile")
        );
        assert_eq!(
            query(&url, "redirect_uri").as_deref(),
            Some("https://console.example/auth/google")
        );
    }

    // Kills an ID token read without its claims: who it names, and each check that refuses it.
    #[test]
    fn id_token_claims_name_the_account() {
        let google = signed_in(&claims()).unwrap();
        assert_eq!(
            (
                google.sub.as_ref(),
                google.email.as_ref(),
                google.name.as_ref().map(AsRef::as_ref),
                google.hd.as_ref().map(AsRef::as_ref)
            ),
            (
                "110169484474386276334",
                "ada@example.com",
                Some("Ada Lovelace"),
                Some("example.com")
            )
        );
        let mut consumer = claims();
        consumer["hd"] = Value::Null;
        assert_eq!(signed_in(&consumer).unwrap().hd, None);

        let mut short_issuer = claims();
        short_issuer["iss"] = json!("accounts.google.com");
        signed_in(&short_issuer).unwrap();
    }

    // Kills each claim check: a token from another issuer, for another client, expired at or
    // before now, with another nonce or none, or with an email Google has not verified, is refused.
    #[test]
    fn id_token_with_a_wrong_claim_is_refused() {
        for (claim, value) in [
            ("iss", json!("https://accounts.google.example")),
            ("aud", json!("another-client.apps.googleusercontent.com")),
            ("exp", json!(NOW)),
            ("nonce", json!(OTHER_NONCE)),
            ("nonce", Value::Null),
            ("email_verified", json!(false)),
            ("email_verified", Value::Null),
            ("email", Value::Null),
        ] {
            let mut wrong = claims();
            wrong[claim] = value.clone();
            assert!(signed_in(&wrong).is_err(), "{claim}: {value}");
        }
        assert!(matches!(
            client().signed_in("not-a-token", &nonce(), now()),
            Err(GoogleClientError::IdToken)
        ));
    }
}
