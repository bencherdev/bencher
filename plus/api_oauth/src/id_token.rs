use bencher_json::{DateTime, OAuthIssuer, OAuthScope};
use bencher_schema::{
    context::OAuthServer,
    error::issue_error,
    model::user::{QueryUser, oauth_authorization::QueryOAuthAuthorization},
};
use bencher_token::OAuthSigningKey;
use serde::Serialize;

use crate::{error::TokenError, userinfo::Identity};

/// How long an ID token lasts, in seconds: a client reads it once, when the user signs in.
const LIFETIME: i64 = 10 * 60;

/// Who signed in to approve a grant with `openid`, read with the grant and signed after the grant
/// is written, so the signature never holds the writer.
pub(crate) struct SignIn<'o> {
    signing_key: &'o OAuthSigningKey,
    issuer: &'o OAuthIssuer,
    authorization: QueryOAuthAuthorization,
    query_user: QueryUser,
}

impl<'o> SignIn<'o> {
    /// Only for an approved request with `openid`, on a server that signs ID tokens.
    pub(crate) fn new(
        oauth: &'o OAuthServer,
        authorization: &QueryOAuthAuthorization,
        query_user: &QueryUser,
    ) -> Option<Self> {
        let signing_key = oauth.signing_key()?;
        authorization
            .scope
            .permits(OAuthScope::Openid)
            .then(|| Self {
                signing_key,
                issuer: oauth.issuer(),
                authorization: authorization.clone(),
                query_user: query_user.clone(),
            })
    }

    /// The ID token (OIDC Core, section 2), issued at `now` for the client.
    pub(crate) fn id_token(&self, now: DateTime) -> Result<String, TokenError> {
        let Self {
            signing_key,
            issuer,
            authorization,
            query_user,
        } = self;
        let claims = IdTokenClaims {
            iss: issuer,
            aud: authorization.client_id.as_ref(),
            exp: now.timestamp() + LIFETIME,
            iat: now.timestamp(),
            auth_time: authorization.approval_time().timestamp(),
            nonce: authorization.nonce.as_ref().map(AsRef::as_ref),
            identity: Identity::new(query_user, &authorization.scope),
        };
        signing_key.sign(&claims).map_err(|error| {
            let _err = issue_error(
                "Failed to sign an OAuth ID token",
                "Failed to sign an OAuth ID token:",
                error,
            );
            TokenError::Server
        })
    }
}

#[derive(Serialize)]
struct IdTokenClaims<'c> {
    iss: &'c OAuthIssuer,
    aud: &'c str,
    exp: i64,
    iat: i64,
    auth_time: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    nonce: Option<&'c str>,
    #[serde(flatten)]
    identity: Identity<'c>,
}
