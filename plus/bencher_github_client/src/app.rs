use std::fmt;

use bencher_valid::{
    DateTime, GitHubAppSlug, GitHubInstallationId, GitHubRepositoryId, GitHubRepositoryName, Jwt,
    NonEmpty, Secret, ValidError,
};
use jsonwebtoken::{Algorithm, EncodingKey, Header};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{
    AUTH_URL, GitHubInstallationChange, GitHubWebhookError,
    api::{GITHUB_API_URL, GitHubApi, GitHubApiError, JsonOwner},
    webhook,
};

/// Issued this far in the past, so a GitHub clock behind ours still accepts the JWT.
const JWT_DRIFT_SECS: i64 = 60;
/// GitHub refuses a JWT that expires more than 10 minutes after its own clock.
const JWT_TTL_SECS: i64 = 9 * 60;

/// The GitHub App that mints the dispatch token for a linked repository.
pub struct GitHubApp {
    client_id: NonEmpty,
    slug: GitHubAppSlug,
    key: EncodingKey,
    webhook_secret: Secret,
    api: GitHubApi,
}

#[derive(Debug, thiserror::Error)]
pub enum GitHubAppError {
    #[error("Failed to read the GitHub App private key: {0}")]
    Key(jsonwebtoken::errors::Error),
    #[error("Failed to create a reqwest client: {0}")]
    Reqwest(reqwest::Error),
    #[error("Failed to sign a GitHub App JWT: {0}")]
    Sign(jsonwebtoken::errors::Error),
    #[error("Failed to mint an installation token: {0}")]
    Mint(GitHubApiError),
    #[error("GitHub minted a token for repositories {returned:?} instead of only {requested}")]
    Repositories {
        requested: GitHubRepositoryId,
        returned: Vec<GitHubRepositoryId>,
    },
    #[error("Failed to parse the minted repository's name: {0}")]
    RepositoryName(ValidError),
}

impl GitHubApp {
    /// The app is the one that `client_id` names, which GitHub accepts as the JWT issuer.
    pub fn new(
        client_id: NonEmpty,
        slug: GitHubAppSlug,
        private_key: &Secret,
        webhook_secret: Secret,
    ) -> Result<Self, GitHubAppError> {
        let key = EncodingKey::from_rsa_pem(private_key.as_ref().as_bytes())
            .map_err(GitHubAppError::Key)?;
        let api = GitHubApi::new(GITHUB_API_URL.clone()).map_err(GitHubAppError::Reqwest)?;
        let app = Self {
            client_id,
            slug,
            key,
            webhook_secret,
            api,
        };
        // jsonwebtoken reads an RSA key only when it signs, so a key that cannot sign fails here.
        app.jwt(DateTime::now()).map_err(GitHubAppError::Key)?;
        Ok(app)
    }

    /// Sends every mint to `base` instead of GitHub.
    #[cfg(any(test, feature = "test-base-url"))]
    pub fn with_base_url(self, base: Url) -> Result<Self, GitHubAppError> {
        Ok(Self {
            api: GitHubApi::new(base).map_err(GitHubAppError::Reqwest)?,
            ..self
        })
    }

    pub fn webhook_secret(&self) -> &Secret {
        &self.webhook_secret
    }

    /// The change a webhook delivery makes, once its signature proves that the app's webhook
    /// secret signed its body. Any other event or action makes none.
    pub fn webhook(
        &self,
        signature: Option<&str>,
        event: Option<&str>,
        body: &[u8],
    ) -> Result<Option<GitHubInstallationChange>, GitHubWebhookError> {
        webhook::delivery(&self.webhook_secret, signature, event, body)
    }

    /// Where a user installs the app, with `state` to carry back to the app's first callback URL.
    pub fn install_url(&self, state: &Jwt) -> Url {
        // GitHub serves the install page on the authorize page's origin.
        let mut url = AUTH_URL.url().clone();
        if let Ok(mut segments) = url.path_segments_mut() {
            segments
                .clear()
                .extend(["apps", self.slug.as_ref(), "installations", "new"]);
        }
        url.query_pairs_mut().append_pair("state", state.as_ref());
        url
    }

    /// Mints a token that can write the contents of the one repository and nothing else.
    pub async fn mint(
        &self,
        now: DateTime,
        installation: GitHubInstallationId,
        repository: GitHubRepositoryId,
    ) -> Result<GitHubInstallationToken, GitHubAppError> {
        let jwt = self.jwt(now).map_err(GitHubAppError::Sign)?;
        let mint = JsonMint {
            repository_ids: [repository],
            permissions: JsonMintPermissions { contents: "write" },
        };
        let JsonMinted {
            token,
            repositories,
        } = self
            .api
            .post(
                &[
                    "app",
                    "installations",
                    &installation.to_string(),
                    "access_tokens",
                ],
                &jwt,
                &mint,
            )
            .await
            .map_err(GitHubAppError::Mint)?;
        if let [
            JsonMintedRepository {
                id,
                owner: JsonOwner { login },
                name,
            },
        ] = repositories.as_slice()
            && *id == repository
        {
            let full_name =
                GitHubRepositoryName::new(login, name).map_err(GitHubAppError::RepositoryName)?;
            return Ok(GitHubInstallationToken { token, full_name });
        }
        Err(GitHubAppError::Repositories {
            requested: repository,
            returned: repositories.iter().map(|minted| minted.id).collect(),
        })
    }

    /// Where a dispatch to the repository goes, on the same API as the mint.
    pub fn dispatch_url(&self, full_name: &GitHubRepositoryName) -> Url {
        self.api
            .url(&["repos", full_name.owner(), full_name.name(), "dispatches"])
    }

    fn jwt(&self, now: DateTime) -> jsonwebtoken::errors::Result<String> {
        let now = now.timestamp();
        let claims = JsonAppClaims {
            iat: now - JWT_DRIFT_SECS,
            exp: now + JWT_TTL_SECS,
            iss: &self.client_id,
        };
        jsonwebtoken::encode(&Header::new(Algorithm::RS256), &claims, &self.key)
    }
}

impl fmt::Debug for GitHubApp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GitHubApp")
            .field("client_id", &self.client_id)
            .field("slug", &self.slug)
            .finish_non_exhaustive()
    }
}

/// A one-hour installation token, and the current full name of the one repository it can write.
pub struct GitHubInstallationToken {
    pub token: Secret,
    pub full_name: GitHubRepositoryName,
}

impl fmt::Debug for GitHubInstallationToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GitHubInstallationToken")
            .field("full_name", &self.full_name)
            .finish_non_exhaustive()
    }
}

#[derive(Serialize)]
struct JsonAppClaims<'a> {
    iat: i64,
    exp: i64,
    iss: &'a NonEmpty,
}

#[derive(Serialize)]
struct JsonMint {
    repository_ids: [GitHubRepositoryId; 1],
    permissions: JsonMintPermissions,
}

#[derive(Serialize)]
struct JsonMintPermissions {
    contents: &'static str,
}

#[derive(Deserialize)]
struct JsonMinted {
    token: Secret,
    repositories: Vec<JsonMintedRepository>,
}

#[derive(Deserialize)]
struct JsonMintedRepository {
    id: GitHubRepositoryId,
    owner: JsonOwner,
    name: String,
}

#[cfg(test)]
mod tests {
    use aws_lc_rs::{
        encoding::{AsDer as _, Pkcs8V1Der},
        rsa::{KeyPair, KeySize},
        signature::KeyPair as _,
    };
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use bencher_valid::{DateTime, GitHubInstallationId, GitHubRepositoryId, Jwt, Secret};
    use jsonwebtoken::{Algorithm, DecodingKey, Validation};
    use serde::Deserialize;

    use super::{GitHubApp, GitHubAppError, GitHubInstallationToken};
    use crate::{
        GitHubApiError,
        api::USER_AGENT,
        test_server::{Server, install_crypto_provider},
    };

    const CLIENT_ID: &str = "Iv1.0123456789abcdef";
    const SLUG: &str = "bencher-test";
    const INSTALLATION_ID: u64 = 2048;
    const REPOSITORY_ID: u64 = 4096;
    const OTHER_REPOSITORY_ID: u64 = 8192;
    const TOKEN: &str = "installation-token-marker";
    const WEBHOOK_SECRET: &str = "webhook-secret-marker";
    const REPOSITORY_INACCESSIBLE: &str = "There is at least one repository that does not exist or is not accessible to the parent installation.";
    const PERMISSIONS_NOT_GRANTED: &str =
        "The permissions requested are not granted to this installation.";

    #[derive(Debug, Deserialize)]
    struct Claims {
        iat: i64,
        exp: i64,
        iss: String,
    }

    /// A fresh key pair: the private key as a PKCS#8 PEM, and the public key to verify with.
    fn key_pair() -> (Secret, DecodingKey) {
        let key_pair = KeyPair::generate(KeySize::Rsa2048).unwrap();
        let private: Pkcs8V1Der<'static> = key_pair.as_der().unwrap();
        let pem = format!(
            "-----BEGIN PRIVATE KEY-----\n{}\n-----END PRIVATE KEY-----\n",
            STANDARD.encode(private.as_ref())
        );
        let public = DecodingKey::from_rsa_der(key_pair.public_key().as_ref());
        (pem.parse().unwrap(), public)
    }

    fn app(private_key: &Secret, server: Option<&Server>) -> GitHubApp {
        install_crypto_provider();
        let app = GitHubApp::new(
            CLIENT_ID.parse().unwrap(),
            SLUG.parse().unwrap(),
            private_key,
            WEBHOOK_SECRET.parse().unwrap(),
        )
        .unwrap();
        if let Some(server) = server {
            app.with_base_url(server.base()).unwrap()
        } else {
            app
        }
    }

    fn installation() -> GitHubInstallationId {
        GitHubInstallationId::try_from(INSTALLATION_ID).unwrap()
    }

    fn repository() -> GitHubRepositoryId {
        GitHubRepositoryId::try_from(REPOSITORY_ID).unwrap()
    }

    fn minted(repositories: &str) -> (u16, String) {
        (
            201,
            format!(
                r#"{{"token": "{TOKEN}", "expires_at": "2024-07-11T07:23:00Z", "permissions": {{"contents": "write", "metadata": "read"}}, "repository_selection": "selected", "repositories": [{repositories}]}}"#
            ),
        )
    }

    fn minted_repository(id: u64) -> String {
        named_minted_repository(id, &format!("repo-{id}"))
    }

    fn named_minted_repository(id: u64, name: &str) -> String {
        format!(
            r#"{{"id": {id}, "name": "{name}", "full_name": "octo-cat/{name}", "owner": {{"login": "octo-cat", "id": 1}}}}"#
        )
    }

    /// The claims of an RS256 JWT, verified with `public`.
    fn verify(jwt: &str, public: &DecodingKey) -> Result<Claims, jsonwebtoken::errors::Error> {
        let mut validation = Validation::new(Algorithm::RS256);
        // The claims are asserted against the fixed test clock instead.
        validation.validate_exp = false;
        validation.required_spec_claims.clear();
        jsonwebtoken::decode::<Claims>(jwt, public, &validation).map(|data| data.claims)
    }

    // Kills a wrong algorithm, issuer, or signing key, and any shift of the issue or expiry time.
    #[test]
    fn the_jwt_is_signed_by_the_app_key_for_nine_minutes() {
        let (private_key, public) = key_pair();
        let (_other_private_key, other_public) = key_pair();
        let now = DateTime::TEST.timestamp();

        let jwt = app(&private_key, None).jwt(DateTime::TEST).unwrap();

        let header = jsonwebtoken::decode_header(&jwt).unwrap();
        assert_eq!(header.alg, Algorithm::RS256);
        let claims = verify(&jwt, &public).unwrap();
        assert_eq!(claims.iss, CLIENT_ID);
        assert_eq!(claims.iat, now - 60);
        assert_eq!(claims.exp, now + 9 * 60);
        verify(&jwt, &other_public).unwrap_err();
    }

    // Kills a `new` that skips the signing check, which lets a key that cannot sign fail every
    // mint instead of the boot.
    #[test]
    fn new_refuses_a_key_that_cannot_sign() {
        install_crypto_provider();
        let key_pair = KeyPair::generate(KeySize::Rsa2048).unwrap();
        let public_key = format!(
            "-----BEGIN RSA PUBLIC KEY-----\n{}\n-----END RSA PUBLIC KEY-----\n",
            STANDARD.encode(key_pair.public_key().as_ref())
        );

        let error = GitHubApp::new(
            CLIENT_ID.parse().unwrap(),
            SLUG.parse().unwrap(),
            &public_key.parse().unwrap(),
            WEBHOOK_SECRET.parse().unwrap(),
        )
        .unwrap_err();

        assert!(matches!(error, GitHubAppError::Key(_)), "{error:?}");
    }

    // Kills asking for any other repository or for more than one, asking for any other
    // permission, sending the request anywhere else or under any other JWT, sending it with no
    // user agent, and reading the owner or the name from anywhere else.
    #[tokio::test]
    async fn the_mint_asks_for_one_repository_with_contents_write() {
        let (private_key, public) = key_pair();
        let server = Server::start(vec![minted(&minted_repository(REPOSITORY_ID))]).await;

        let token = app(&private_key, Some(&server))
            .mint(DateTime::TEST, installation(), repository())
            .await
            .unwrap();

        assert_eq!(token.token.as_ref(), TOKEN);
        assert_eq!(
            (token.full_name.owner(), token.full_name.name()),
            ("octo-cat", format!("repo-{REPOSITORY_ID}").as_str())
        );
        let [request] = server.received().await.try_into().ok().unwrap();
        assert_eq!(
            request.request_line(),
            format!("POST /github/app/installations/{INSTALLATION_ID}/access_tokens HTTP/1.1")
        );
        assert_eq!(request.header("user-agent"), Some(USER_AGENT));
        assert_eq!(
            request.json(),
            serde_json::json!({
                "repository_ids": [REPOSITORY_ID],
                "permissions": { "contents": "write" },
            })
        );
        let jwt = request
            .header("authorization")
            .and_then(|authorization| authorization.strip_prefix("Bearer "))
            .unwrap();
        assert_eq!(verify(jwt, &public).unwrap().iss, CLIENT_ID);
    }

    // Kills dropping or reordering a segment, building on any API but the app's, a URL that
    // misses the base's path, and keeping the empty segment of a base's trailing slash.
    #[test]
    fn the_dispatch_url_is_on_the_app_api() {
        let (private_key, _public) = key_pair();
        let full_name = "octo-cat/hello.world".parse().unwrap();

        let github = app(&private_key, None).dispatch_url(&full_name);

        assert_eq!(
            github.as_str(),
            "https://api.github.com/repos/octo-cat/hello.world/dispatches"
        );
        for base in ["http://127.0.0.1/github", "http://127.0.0.1/github/"] {
            let fake = app(&private_key, None)
                .with_base_url(base.parse().unwrap())
                .unwrap()
                .dispatch_url(&full_name);

            assert_eq!(
                fake.as_str(),
                "http://127.0.0.1/github/repos/octo-cat/hello.world/dispatches",
                "{base}"
            );
        }
    }

    // Kills an install page on any other host or under any other app, and dropping the state.
    #[test]
    fn the_install_url_carries_the_state() {
        let (private_key, _public) = key_pair();
        let state = Jwt::test_token();

        let url = app(&private_key, None).install_url(&state);

        assert_eq!(
            url.as_str(),
            format!("https://github.com/apps/{SLUG}/installations/new?state={state}")
        );
    }

    // Kills accepting a token for no repository, for another repository, or for more than one.
    #[tokio::test]
    async fn the_mint_refuses_a_token_for_any_other_repository() {
        let (private_key, _public) = key_pair();
        for (repositories, returned) in [
            (Vec::new(), Vec::new()),
            (vec![OTHER_REPOSITORY_ID], vec![OTHER_REPOSITORY_ID]),
            (
                vec![REPOSITORY_ID, OTHER_REPOSITORY_ID],
                vec![REPOSITORY_ID, OTHER_REPOSITORY_ID],
            ),
            (
                vec![OTHER_REPOSITORY_ID, REPOSITORY_ID],
                vec![OTHER_REPOSITORY_ID, REPOSITORY_ID],
            ),
        ] {
            let listed: Vec<String> = repositories.into_iter().map(minted_repository).collect();
            let server = Server::start(vec![minted(&listed.join(", "))]).await;

            let error = app(&private_key, Some(&server))
                .mint(DateTime::TEST, installation(), repository())
                .await
                .unwrap_err();

            let GitHubAppError::Repositories {
                requested,
                returned: actual,
            } = &error
            else {
                panic!("{error:?}");
            };
            assert_eq!(*requested, repository());
            let actual: Vec<u64> = actual.iter().copied().map(u64::from).collect();
            assert_eq!(actual, returned);
            let printed = format!("{error} {error:?}");
            assert!(!printed.contains(TOKEN), "{printed}");
            server.received().await;
        }
    }

    // The dispatch URL is built from the minted name, so this kills accepting one that is not two
    // path segments.
    #[tokio::test]
    async fn the_mint_refuses_a_repository_name_that_does_not_parse() {
        let (private_key, _public) = key_pair();
        let server =
            Server::start(vec![minted(&named_minted_repository(REPOSITORY_ID, ".."))]).await;

        let error = app(&private_key, Some(&server))
            .mint(DateTime::TEST, installation(), repository())
            .await
            .unwrap_err();

        assert!(
            matches!(error, GitHubAppError::RepositoryName(_)),
            "{error:?}"
        );
        let printed = format!("{error} {error:?}");
        assert!(!printed.contains(TOKEN), "{printed}");
        server.received().await;
    }

    // Kills folding a 404 or either known 422 into any other refusal, which would keep a dead link
    // connected or disconnect a live one, reading a 422 by its status alone or the message of
    // any other status, and following a redirect.
    #[tokio::test]
    async fn the_mint_tells_each_refusal_apart() {
        let (private_key, _public) = key_pair();
        let message = |message: &str| format!(r#"{{"message": "{message}", "status": "422"}}"#);
        for (status, body, expected) in [
            (404, message("Not Found"), "NotFound"),
            (
                422,
                message(REPOSITORY_INACCESSIBLE),
                "RepositoryInaccessible",
            ),
            (
                422,
                message(PERMISSIONS_NOT_GRANTED),
                "PermissionsNotGranted",
            ),
            (422, message("Validation Failed"), "422"),
            (422, "Unprocessable".to_owned(), "422"),
            (403, message(REPOSITORY_INACCESSIBLE), "403"),
            (403, message(PERMISSIONS_NOT_GRANTED), "403"),
            (302, String::new(), "302"),
            (401, message("Bad credentials"), "401"),
            (500, message("Server Error"), "500"),
        ] {
            let server = Server::start(vec![(status, body)]).await;

            let error = app(&private_key, Some(&server))
                .mint(DateTime::TEST, installation(), repository())
                .await
                .unwrap_err();

            assert_eq!(refusal(error), expected, "{status}");
            assert_eq!(server.received().await.len(), 1, "{status}");
        }
    }

    // `Secret` prints itself in a debug build, so this kills a derived `Debug` on either type.
    #[test]
    fn debug_hides_the_key_the_webhook_secret_and_the_token() {
        let (private_key, _public) = key_pair();
        let app = app(&private_key, None);
        let token = GitHubInstallationToken {
            token: TOKEN.parse().unwrap(),
            full_name: "octo-cat/repo".parse().unwrap(),
        };

        let printed = format!("{app:?} {token:?}");

        let pem_body = private_key
            .as_ref()
            .lines()
            .nth(1)
            .unwrap()
            .get(..64)
            .unwrap();
        for marker in [pem_body, WEBHOOK_SECRET, TOKEN] {
            assert!(!printed.contains(marker), "{marker} in {printed}");
        }
        assert!(printed.contains(CLIENT_ID), "{printed}");
        assert!(printed.contains("octo-cat"), "{printed}");
    }

    /// The mint's refusal, named as the table names it.
    fn refusal(error: GitHubAppError) -> String {
        let GitHubAppError::Mint(error) = error else {
            panic!("{error:?}");
        };
        match error {
            GitHubApiError::NotFound => "NotFound".to_owned(),
            GitHubApiError::RepositoryInaccessible => "RepositoryInaccessible".to_owned(),
            GitHubApiError::PermissionsNotGranted => "PermissionsNotGranted".to_owned(),
            GitHubApiError::Status(status) => status.as_str().to_owned(),
            GitHubApiError::Request(_)
            | GitHubApiError::Json(_)
            | GitHubApiError::RepositoryName(_) => panic!("{error:?}"),
        }
    }
}
