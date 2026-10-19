use bencher_endpoint::{CorsResponse, Endpoint, Post};
use bencher_json::{DateTime, OAuthTokenHash};
use bencher_schema::{
    context::{ApiContext, DbConnection, OAuthCredential},
    model::user::oauth::{OAuthTokenKind, QueryOAuthGrant, QueryOAuthToken, oauth_token_hash},
    write_transaction,
};
use dropshot::{Body, HttpError, RequestContext, UntypedBody, endpoint};
use http::{HeaderMap, Response};

use crate::{
    client::{CLIENT_CREDENTIALS, CLIENT_ID, authenticates_client},
    error::{InvalidRequest, TokenError, charge_address, charge_refusal},
    oauth_server,
    params::Params,
    response::empty,
};

const TOKEN: &str = "token";
const TOKEN_TYPE_HINT: &str = "token_type_hint";
const PARAMETERS: [&str; 6] = [
    TOKEN,
    TOKEN_TYPE_HINT,
    CLIENT_ID,
    CLIENT_CREDENTIALS[0],
    CLIENT_CREDENTIALS[1],
    CLIENT_CREDENTIALS[2],
];

#[endpoint {
    method = OPTIONS,
    path = "/oauth/revoke",
    unpublished = true,
}]
pub async fn oauth_revoke_options(
    rqctx: RequestContext<ApiContext>,
) -> Result<CorsResponse, HttpError> {
    oauth_server(rqctx.context())?;
    Ok(Endpoint::cors(&[Post.into()]))
}

/// OAuth token revocation endpoint (RFC 7009)
///
/// A refresh token revokes its whole grant; an access token is deleted.
/// Any token that is unknown, or was not issued to the client, is left as it is,
/// and the answer is the same either way.
#[endpoint {
    method = POST,
    path = "/oauth/revoke",
    unpublished = true,
}]
pub async fn oauth_revoke_post(
    rqctx: RequestContext<ApiContext>,
    body: UntypedBody,
) -> Result<Response<Body>, HttpError> {
    let context = rqctx.context();
    let oauth = oauth_server(context)?;
    let outcome = revoke(context, rqctx.request.headers(), body.as_bytes()).await;
    match outcome.and_then(|revocation| revocation.record(&rqctx)) {
        Ok(()) => empty(),
        Err(error) => error.charged(&rqctx).into_response(&rqctx.log, oauth),
    }
}

async fn revoke(
    context: &ApiContext,
    headers: &HeaderMap,
    body: &[u8],
) -> Result<Revocation, TokenError> {
    let params =
        Params::parse(body, &PARAMETERS).ok_or(TokenError::Request(InvalidRequest::Repeated))?;
    if authenticates_client(&params, headers) {
        return Err(TokenError::Client);
    }
    let (Some(token), Some(client_id)) = (params.get(TOKEN), params.get(CLIENT_ID)) else {
        return Err(TokenError::Request(InvalidRequest::RevokeParameters));
    };
    // The hint may be wrong, so the token's own prefix decides what it is.
    let Some(token_hash) = oauth_token_hash(token) else {
        return Ok(Revocation::Unknown);
    };
    let now = context.clock.now();
    // Judged and charged first on a read connection, so only a revocation that changes something
    // takes the writer.
    {
        let mut conn = context
            .database
            .get_public_conn()
            .await
            .map_err(TokenError::pool)?;
        let Some((query_token, query_grant, query_user, _query_key)) =
            QueryOAuthToken::with_grant(&mut conn, &token_hash).map_err(TokenError::server)?
        else {
            return Ok(Revocation::Unknown);
        };
        if let Some(revocation) = settled(&query_token, &query_grant, client_id, now) {
            charge_refusal(context, OAuthCredential::Grant(query_grant.uuid))?;
            return Ok(revocation);
        }
        if context.rate_limiting.user_request(query_user.uuid).is_err() {
            return Err(TokenError::Throttled);
        }
    }
    write_transaction!(context, |conn| revoke_token(
        conn,
        &token_hash,
        client_id,
        now
    ))
    .map_err(TokenError::server)
}

/// Revoke what a token of `client_id` stands for, in one write transaction, judged again as it
/// now stands. The read path already charged the request, and a token gone since it was read
/// changes nothing.
fn revoke_token(
    conn: &mut DbConnection,
    token_hash: &OAuthTokenHash,
    client_id: &str,
    now: DateTime,
) -> diesel::QueryResult<Revocation> {
    let Some((query_token, query_grant, _query_user, _query_key)) =
        QueryOAuthToken::with_grant(conn, token_hash)?
    else {
        return Ok(Revocation::Ignored);
    };
    if let Some(revocation) = settled(&query_token, &query_grant, client_id, now) {
        return Ok(revocation);
    }
    Ok(match query_token.kind {
        OAuthTokenKind::Refresh => {
            QueryOAuthGrant::revoke(conn, query_grant.id, now)?;
            Revocation::Grant
        },
        OAuthTokenKind::Access => {
            query_token.delete(conn)?;
            Revocation::AccessToken
        },
    })
}

/// A revocation that changes nothing: another client's token, or a refresh token whose grant is
/// already dead.
fn settled(
    query_token: &QueryOAuthToken,
    query_grant: &QueryOAuthGrant,
    client_id: &str,
    now: DateTime,
) -> Option<Revocation> {
    let unchanged = query_grant.client_id.as_ref() != client_id
        || (query_token.kind == OAuthTokenKind::Refresh && !query_grant.is_live(now));
    unchanged.then_some(Revocation::Ignored)
}

/// What a revocation request changed.
#[derive(Debug, Clone, Copy)]
enum Revocation {
    Grant,
    AccessToken,
    /// Nothing, for a known token: another client's, or a dead grant's.
    Ignored,
    /// Nothing, for a token this server does not know.
    Unknown,
}

impl Revocation {
    /// Only a request that named no known token is anonymous, so only it is charged to its address.
    fn record(self, rqctx: &RequestContext<ApiContext>) -> Result<(), TokenError> {
        if matches!(self, Self::Unknown) {
            charge_address(rqctx)?;
        }
        #[cfg(feature = "otel")]
        bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::OAuthRevoke(self.into()));
        Ok(())
    }
}

#[cfg(feature = "otel")]
impl From<Revocation> for bencher_otel::OAuthRevokeKind {
    fn from(revocation: Revocation) -> Self {
        match revocation {
            Revocation::Grant => Self::Grant,
            Revocation::AccessToken => Self::AccessToken,
            Revocation::Ignored => Self::Ignored,
            Revocation::Unknown => Self::Unknown,
        }
    }
}
