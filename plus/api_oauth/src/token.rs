use bencher_endpoint::{CorsResponse, Endpoint, Post};
use bencher_json::{
    DateTime, OAuthCode, OAuthCodeHash, OAuthCodeVerifier, OAuthResource, OAuthScope, OAuthScopes,
    OAuthTokenHash,
};
use bencher_schema::{
    context::{ApiContext, DbConnection, OAuthCredential, OAuthServer},
    model::user::{
        QueryUser,
        key::QueryUserKey,
        oauth::{
            OAuthGrantId, OAuthTokenKind, OAuthTokens, QueryOAuthGrant, QueryOAuthToken,
            RefreshTokens, approver_failure, grant_failure, oauth_token_hash,
        },
        oauth_authorization::QueryOAuthAuthorization,
    },
    write_transaction,
};
use dropshot::{Body, HttpError, RequestContext, UntypedBody, endpoint};
use http::{HeaderMap, Response};
use serde::Serialize;

use crate::{
    AUTHORIZATION_CODE, REFRESH_TOKEN,
    client::{CLIENT_CREDENTIALS, CLIENT_ID, authenticates_client},
    error::{GrantRefusal, InvalidRequest, TokenError, charge_refusal},
    oauth_server,
    params::Params,
    response::json,
};

const GRANT_TYPE: &str = "grant_type";
const CODE: &str = "code";
const REDIRECT_URI: &str = "redirect_uri";
const CODE_VERIFIER: &str = "code_verifier";
const SCOPE: &str = "scope";
const RESOURCE: &str = "resource";
const PARAMETERS: [&str; 11] = [
    GRANT_TYPE,
    CODE,
    REDIRECT_URI,
    CLIENT_ID,
    CODE_VERIFIER,
    REFRESH_TOKEN,
    SCOPE,
    RESOURCE,
    CLIENT_CREDENTIALS[0],
    CLIENT_CREDENTIALS[1],
    CLIENT_CREDENTIALS[2],
];
const BEARER: &str = "Bearer";

#[endpoint {
    method = OPTIONS,
    path = "/oauth/token",
    unpublished = true,
}]
pub async fn oauth_token_options(
    rqctx: RequestContext<ApiContext>,
) -> Result<CorsResponse, HttpError> {
    oauth_server(rqctx.context())?;
    Ok(Endpoint::cors(&[Post.into()]))
}

/// OAuth token endpoint
///
/// Exchanges an authorization code, or a refresh token, for an access token.
/// A code or refresh token presented a second time revokes its grant.
#[endpoint {
    method = POST,
    path = "/oauth/token",
    unpublished = true,
}]
pub async fn oauth_token_post(
    rqctx: RequestContext<ApiContext>,
    body: UntypedBody,
) -> Result<Response<Body>, HttpError> {
    let context = rqctx.context();
    let oauth = oauth_server(context)?;
    let now = context.clock.now();
    match token(
        context,
        oauth,
        rqctx.request.headers(),
        body.as_bytes(),
        now,
    )
    .await
    {
        Ok(tokens) => json(&TokenResponse::new(&tokens, now)),
        Err(error) => error.charged(&rqctx).into_response(&rqctx.log, oauth),
    }
}

async fn token(
    context: &ApiContext,
    oauth: &OAuthServer,
    headers: &HeaderMap,
    body: &[u8],
    now: DateTime,
) -> Result<OAuthTokens, TokenError> {
    let params =
        Params::parse(body, &PARAMETERS).ok_or(TokenError::Request(InvalidRequest::Repeated))?;
    if authenticates_client(&params, headers) {
        return Err(TokenError::Client);
    }
    match params.get(GRANT_TYPE) {
        Some(AUTHORIZATION_CODE) => {
            let tokens = exchange(context, oauth, &params, now).await?;
            #[cfg(feature = "otel")]
            bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::OAuthTokenIssue(
                bencher_otel::OAuthGrantKind::AuthorizationCode,
            ));
            Ok(tokens)
        },
        Some(REFRESH_TOKEN) => {
            let tokens = refresh(context, oauth, &params, now).await?;
            #[cfg(feature = "otel")]
            bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::OAuthTokenIssue(
                bencher_otel::OAuthGrantKind::RefreshToken,
            ));
            Ok(tokens)
        },
        Some(_) => Err(TokenError::GrantType),
        None => Err(TokenError::Request(InvalidRequest::GrantType)),
    }
}

async fn exchange(
    context: &ApiContext,
    oauth: &OAuthServer,
    params: &Params<'_>,
    now: DateTime,
) -> Result<OAuthTokens, TokenError> {
    let (Some(code), Some(client_id), Some(code_verifier)) = (
        params.get(CODE),
        params.get(CLIENT_ID),
        params.get(CODE_VERIFIER),
    ) else {
        return Err(TokenError::Request(InvalidRequest::CodeParameters));
    };
    let code_verifier = code_verifier
        .parse::<OAuthCodeVerifier>()
        .map_err(|_err| TokenError::Request(InvalidRequest::CodeVerifier))?;
    let resource = requested_resource(oauth, params)?;
    let code_hash = code
        .parse::<OAuthCode>()
        .map(|code| OAuthCodeHash::from(&code))
        .map_err(|_err| TokenError::Grant(GrantRefusal::UnknownCode))?;
    let exchange = CodeExchange {
        code_hash,
        client_id,
        redirect_uri: params.get(REDIRECT_URI),
        code_verifier,
        resource,
    };
    // Judged and charged first on a read connection, so only an exchange, or a reuse that revokes
    // a live grant, takes the writer.
    {
        let mut conn = context
            .database
            .get_public_conn()
            .await
            .map_err(TokenError::pool)?;
        let conn = &mut *conn;
        let Some((authorization, query_user, query_key)) =
            QueryOAuthAuthorization::approved(conn, &exchange.code_hash)
                .map_err(TokenError::server)?
        else {
            return Err(TokenError::Grant(GrantRefusal::UnknownCode));
        };
        if let Some(error) =
            exchange.refusal(oauth, &authorization, &query_user, query_key.as_ref(), now)
        {
            let revokes = matches!(error, TokenError::Grant(GrantRefusal::CodeReused))
                && live_grant(conn, authorization.grant_id, now).map_err(TokenError::server)?;
            if !revokes {
                charge_refusal(context, OAuthCredential::Authorization(authorization.uuid))?;
                return Err(error);
            }
        } else if context.rate_limiting.user_request(query_user.uuid).is_err() {
            return Err(TokenError::Throttled);
        }
    }
    write_transaction!(context, |conn| exchange.grant(conn, oauth, now))
        .map_err(TokenError::server)?
}

async fn refresh(
    context: &ApiContext,
    oauth: &OAuthServer,
    params: &Params<'_>,
    now: DateTime,
) -> Result<OAuthTokens, TokenError> {
    let (Some(refresh_token), Some(client_id)) = (params.get(REFRESH_TOKEN), params.get(CLIENT_ID))
    else {
        return Err(TokenError::Request(InvalidRequest::RefreshParameters));
    };
    let resource = requested_resource(oauth, params)?;
    // Like an authorization request, unknown scopes (`offline_access` among them) are ignored.
    let scope = params
        .get(SCOPE)
        .unwrap_or_default()
        .split(' ')
        .filter_map(|scope| scope.parse::<OAuthScope>().ok())
        .collect::<OAuthScopes>();
    // An access token hashes too, so that one presented here is refused as the wrong kind.
    let token_hash =
        oauth_token_hash(refresh_token).ok_or(TokenError::Grant(GrantRefusal::UnknownToken))?;
    let refresh = Refresh {
        token_hash,
        client_id,
        scope,
        resource,
    };
    // Judged and charged first on a read connection, so only a refresh, or a reuse that revokes a
    // live grant, takes the writer.
    {
        let mut conn = context
            .database
            .get_public_conn()
            .await
            .map_err(TokenError::pool)?;
        let conn = &mut *conn;
        let Some((query_token, query_grant, query_user, query_key)) =
            QueryOAuthToken::with_grant(conn, &refresh.token_hash).map_err(TokenError::server)?
        else {
            return Err(TokenError::Grant(GrantRefusal::UnknownToken));
        };
        if let Some(error) = refresh.refusal(
            oauth,
            &query_token,
            &query_grant,
            &query_user,
            query_key.as_ref(),
            now,
        ) {
            if !(matches!(error, TokenError::Grant(GrantRefusal::TokenReused))
                && query_grant.is_live(now))
            {
                charge_refusal(context, OAuthCredential::Grant(query_grant.uuid))?;
                return Err(error);
            }
        } else if context.rate_limiting.user_request(query_user.uuid).is_err() {
            return Err(TokenError::Throttled);
        }
    }
    write_transaction!(context, |conn| refresh.rotate(conn, oauth, now))
        .map_err(TokenError::server)?
}

fn requested_resource(
    oauth: &OAuthServer,
    params: &Params<'_>,
) -> Result<Option<OAuthResource>, TokenError> {
    let resource = params
        .value::<OAuthResource>(RESOURCE)
        .map_err(|()| TokenError::Target)?;
    if let Some(resource) = &resource
        && !oauth.serves(resource)
    {
        return Err(TokenError::Target);
    }
    Ok(resource)
}

/// Whether `grant_id` names a grant that is still live, which a replayed code revokes.
fn live_grant(
    conn: &mut DbConnection,
    grant_id: Option<OAuthGrantId>,
    now: DateTime,
) -> diesel::QueryResult<bool> {
    Ok(grant_id
        .map(|grant_id| QueryOAuthGrant::from_id(conn, grant_id))
        .transpose()?
        .is_some_and(|query_grant| query_grant.is_live(now)))
}

/// An authorization code to exchange, with what the request binds it to.
struct CodeExchange<'p> {
    code_hash: OAuthCodeHash,
    client_id: &'p str,
    /// Compared with the authorization request's as sent, byte for byte.
    redirect_uri: Option<&'p str>,
    code_verifier: OAuthCodeVerifier,
    resource: Option<OAuthResource>,
}

impl CodeExchange<'_> {
    /// In one write transaction, judged again as it now stands: an exchanged code is reuse, which
    /// revokes the grant made from it; any other code issues a new grant's tokens if it is live and
    /// bound to this request. The read path already charged the request.
    fn grant(
        &self,
        conn: &mut DbConnection,
        oauth: &OAuthServer,
        now: DateTime,
    ) -> diesel::QueryResult<Result<OAuthTokens, TokenError>> {
        let Some((authorization, query_user, query_key)) =
            QueryOAuthAuthorization::approved(conn, &self.code_hash)?
        else {
            return Ok(Err(TokenError::Grant(GrantRefusal::Vanished)));
        };
        if let Some(error) =
            self.refusal(oauth, &authorization, &query_user, query_key.as_ref(), now)
        {
            if let Some(grant_id) = authorization.grant_id {
                QueryOAuthGrant::revoke(conn, grant_id, now)?;
            }
            return Ok(Err(error));
        }
        let query_grant = authorization.grant(conn, &query_user, query_key.as_ref(), now)?;
        let refresh = if authorization.refresh {
            RefreshTokens::Issue
        } else {
            RefreshTokens::Withhold
        };
        query_grant.issue(conn, refresh, now).map(Ok)
    }

    /// Why the code cannot be exchanged, if it cannot. An exchanged code is reuse, whatever else
    /// is wrong with the request.
    fn refusal(
        &self,
        oauth: &OAuthServer,
        authorization: &QueryOAuthAuthorization,
        query_user: &QueryUser,
        query_key: Option<&QueryUserKey>,
        now: DateTime,
    ) -> Option<TokenError> {
        let refusal = if authorization.grant_id.is_some() {
            GrantRefusal::CodeReused
        } else if authorization.is_expired(now) {
            GrantRefusal::CodeExpired
        } else if authorization.client_id.as_ref() != self.client_id {
            GrantRefusal::OtherClient
        } else if self
            .redirect_uri
            .is_some_and(|redirect_uri| redirect_uri != authorization.redirect_uri.as_ref())
        {
            GrantRefusal::RedirectUri
        } else if self
            .resource
            .as_ref()
            .is_some_and(|resource| *resource != authorization.resource)
        {
            GrantRefusal::Resource
        } else if !oauth.serves(&authorization.resource) {
            GrantRefusal::ResourceUnserved
        } else if !oauth.clients().allows(&authorization.client_id) {
            GrantRefusal::UnlistedClient
        } else if !self.code_verifier.verifies(&authorization.code_challenge) {
            GrantRefusal::CodeVerifier
        } else {
            GrantRefusal::Grant(approver_failure(
                query_user,
                authorization.user_key_id,
                query_key,
                now,
            )?)
        };
        Some(TokenError::Grant(refusal))
    }
}

/// A refresh token to exchange, with what the request asks of it.
struct Refresh<'p> {
    token_hash: OAuthTokenHash,
    client_id: &'p str,
    /// What the new access token should allow; it is never more than the grant allows.
    scope: OAuthScopes,
    resource: Option<OAuthResource>,
}

impl Refresh<'_> {
    /// In one write transaction, judged again as it now stands: a used refresh token is reuse,
    /// which revokes its grant; any other live refresh token of a live grant is used up for a new
    /// pair of tokens. The read path already charged the request.
    fn rotate(
        &self,
        conn: &mut DbConnection,
        oauth: &OAuthServer,
        now: DateTime,
    ) -> diesel::QueryResult<Result<OAuthTokens, TokenError>> {
        let Some((query_token, query_grant, query_user, query_key)) =
            QueryOAuthToken::with_grant(conn, &self.token_hash)?
        else {
            return Ok(Err(TokenError::Grant(GrantRefusal::Vanished)));
        };
        if let Some(error) = self.refusal(
            oauth,
            &query_token,
            &query_grant,
            &query_user,
            query_key.as_ref(),
            now,
        ) {
            if matches!(error, TokenError::Grant(GrantRefusal::TokenReused)) {
                QueryOAuthGrant::revoke(conn, query_grant.id, now)?;
            }
            return Ok(Err(error));
        }
        query_token.use_up(conn, now)?;
        query_grant.refreshed(conn, now)?;
        query_grant.issue(conn, RefreshTokens::Issue, now).map(Ok)
    }

    /// Why the token cannot be refreshed, if it cannot. A used refresh token is reuse, whatever
    /// else is wrong with the request.
    fn refusal(
        &self,
        oauth: &OAuthServer,
        query_token: &QueryOAuthToken,
        query_grant: &QueryOAuthGrant,
        query_user: &QueryUser,
        query_key: Option<&QueryUserKey>,
        now: DateTime,
    ) -> Option<TokenError> {
        let refusal = if query_token.kind != OAuthTokenKind::Refresh {
            GrantRefusal::TokenKind
        } else if query_token.used.is_some() {
            GrantRefusal::TokenReused
        } else if query_token.is_expired(now) {
            GrantRefusal::TokenExpired
        } else if query_grant.client_id.as_ref() != self.client_id {
            GrantRefusal::OtherClient
        } else if let Some(failure) = grant_failure(query_grant, query_user, query_key, now) {
            GrantRefusal::Grant(failure)
        } else if !oauth.serves(&query_grant.resource) {
            GrantRefusal::ResourceUnserved
        } else if !oauth.clients().allows(&query_grant.client_id) {
            GrantRefusal::UnlistedClient
        } else if self
            .resource
            .as_ref()
            .is_some_and(|resource| *resource != query_grant.resource)
        {
            GrantRefusal::Resource
        } else if !self
            .scope
            .iter()
            .all(|scope| query_grant.scope.permits(scope))
        {
            return Some(TokenError::Scope);
        } else {
            return None;
        };
        Some(TokenError::Grant(refusal))
    }
}

/// A successful token response (RFC 6749, section 5.1).
#[derive(Serialize)]
struct TokenResponse<'t> {
    access_token: &'t str,
    token_type: &'static str,
    expires_in: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    refresh_token: Option<&'t str>,
    scope: String,
}

impl<'t> TokenResponse<'t> {
    fn new(tokens: &'t OAuthTokens, now: DateTime) -> Self {
        let OAuthTokens {
            access_token,
            access_expiration,
            refresh_token,
            scope,
        } = tokens;
        Self {
            access_token: access_token.as_ref(),
            token_type: BEARER,
            // A live grant always leaves time; the floor keeps a client from reading the token
            // as already expired.
            expires_in: (access_expiration.timestamp() - now.timestamp()).max(1),
            refresh_token: refresh_token.as_ref().map(AsRef::as_ref),
            scope: scope.to_string(),
        }
    }
}
