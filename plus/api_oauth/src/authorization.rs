use bencher_endpoint::{CorsResponse, Endpoint, Get, Post, ResponseOk};
use bencher_json::{
    DateTime, JsonOAuthAuthorization, JsonOAuthDecision, JsonOAuthRedirect, Jwt,
    OAuthAuthorizationUuid, OAuthCode, OAuthCodeHash, OAuthScope, ProjectKey, UserKey,
};
use bencher_schema::{
    context::{ApiContext, OAuthServer},
    error::{
        conflict_error, forbidden_error, invalid_credential, issue_error, not_found_error,
        unauthorized_error,
    },
    model::user::{
        QueryUser,
        auth::{AuthCredential, AuthUser, BearerToken},
        oauth_authorization::{OAuthAnswer, QueryOAuthAuthorization},
        session::cookie::CookieCredential,
    },
    public_conn, write_transaction,
};
use diesel::OptionalExtension as _;
use dropshot::{Body, HttpError, Path, RequestContext, TypedBody, endpoint};
use http::Response;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::response::{client_redirect, json};

const NOT_FOUND: &str = "No such request to connect, or it expired";
const ANSWERED: &str = "This request to connect has already been answered";
const CLI_NEEDS_A_SESSION: &str =
    "Only a browser session can approve the Bencher CLI. Sign in to approve it.";
const CLI_NEEDS_A_SERVER_ADMIN: &str = "Only a server admin can approve the Bencher CLI as admin";
/// The refusal's code, for the consent page to tell a reader who cannot approve an admin grant.
const NOT_A_SERVER_ADMIN: &str = "not_a_server_admin";

#[derive(Deserialize, JsonSchema)]
pub struct OAuthAuthorizationParams {
    /// The UUID of an app's request to connect.
    pub authorization: OAuthAuthorizationUuid,
}

#[endpoint {
    method = OPTIONS,
    path = "/v0/oauth/authorizations/{authorization}",
    unpublished = true,
}]
pub async fn oauth_authorization_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<OAuthAuthorizationParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into(), Post.into()]))
}

/// View a request to connect
///
/// View an app's pending request to connect to a user's account, for the consent page.
#[endpoint {
    method = GET,
    path = "/v0/oauth/authorizations/{authorization}",
    unpublished = true,
}]
pub async fn oauth_authorization_get(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<OAuthAuthorizationParams>,
) -> Result<ResponseOk<JsonOAuthAuthorization>, HttpError> {
    let context = rqctx.context();
    charge_public_request(&rqctx)?;
    let OAuthAuthorizationParams { authorization } = path_params.into_inner();
    let authorization = live(context, authorization).await?;
    if authorization.is_answered() {
        return Err(conflict_error(ANSWERED));
    }
    Ok(Get::pub_response_ok(
        authorization.into_json(context.oauth.as_ref()),
    ))
}

/// A request that has not expired.
async fn live(
    context: &ApiContext,
    uuid: OAuthAuthorizationUuid,
) -> Result<QueryOAuthAuthorization, HttpError> {
    QueryOAuthAuthorization::from_uuid(public_conn!(context), uuid)
        .optional()
        .map_err(|error| {
            issue_error(
                "Failed to view an OAuth authorization request",
                "Failed to view an OAuth authorization request:",
                error,
            )
        })?
        .filter(|authorization| !authorization.is_expired(context.clock.now()))
        .ok_or_else(|| not_found_error(NOT_FOUND))
}

/// Answer a request to connect
///
/// Approve an app's pending request to connect, as the user of the session or user API key,
/// or deny it, which needs no credential. Either way, the response is where to send the browser.
#[endpoint {
    method = POST,
    path = "/v0/oauth/authorizations/{authorization}",
    unpublished = true,
}]
pub async fn oauth_authorization_post(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<OAuthAuthorizationParams>,
    body: TypedBody<JsonOAuthDecision>,
) -> Result<Response<Body>, HttpError> {
    let context = rqctx.context();
    let issuer = context.oauth.as_ref().map(OAuthServer::issuer);
    let OAuthAuthorizationParams { authorization } = path_params.into_inner();
    let JsonOAuthDecision { approve, key } = body.into_inner();
    let now = context.clock.now();

    let (answer, code) = if approve {
        let (auth_user, auth_time) = approver(&rqctx, key).await?;
        let pending = live(context, authorization).await?;
        if pending.client_id.is_bencher_cli() {
            cli_approver(context, &auth_user, &pending)?;
        }
        let code = OAuthCode::generate();
        let code_hash = OAuthCodeHash::from(&code);
        let answer = write_transaction!(context, |conn| {
            let answer = QueryOAuthAuthorization::approve_with_auth_time(
                conn,
                authorization,
                auth_user.id,
                auth_user.user_key_id(),
                auth_time,
                &code_hash,
                now,
            )?;
            // A session is only minted for an address its user proved they hold.
            if auth_user.is_session() && matches!(answer, OAuthAnswer::Recorded(_)) {
                QueryUser::verify_email(conn, auth_user.id, &auth_user.email, now)?;
            }
            Ok::<_, diesel::result::Error>(answer)
        });
        (answer, Some(code))
    } else {
        charge_public_request(&rqctx)?;
        let answer = write_transaction!(context, |conn| QueryOAuthAuthorization::deny(
            conn,
            authorization,
            now
        ));
        (answer, None)
    };
    let authorization = match answer.map_err(|error| {
        issue_error(
            "Failed to answer an OAuth authorization request",
            "Failed to answer an OAuth authorization request:",
            error,
        )
    })? {
        OAuthAnswer::Recorded(authorization) => *authorization,
        OAuthAnswer::Missing => return Err(not_found_error(NOT_FOUND)),
        OAuthAnswer::Repeated => return Err(conflict_error(ANSWERED)),
        OAuthAnswer::ForeignKey => {
            return Err(forbidden_error(
                "The user API key is not the approving user's",
            ));
        },
        OAuthAnswer::LoginRequired(authorization) => {
            #[cfg(feature = "otel")]
            bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::OAuthConsent(
                bencher_otel::OAuthConsentKind::LoginRequired,
            ));
            return redirect(
                issuer,
                &authorization,
                &[
                    ("error", "login_required"),
                    (
                        "error_description",
                        "The user has not signed in recently enough",
                    ),
                ],
            );
        },
    };

    if let Some(code) = code {
        #[cfg(feature = "otel")]
        bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::OAuthConsent(
            if authorization.user_key_id.is_some() {
                bencher_otel::OAuthConsentKind::ApproveKey
            } else {
                bencher_otel::OAuthConsentKind::ApproveSession
            },
        ));
        redirect(issuer, &authorization, &[("code", code.as_ref())])
    } else {
        #[cfg(feature = "otel")]
        bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::OAuthConsent(
            bencher_otel::OAuthConsentKind::Deny,
        ));
        redirect(
            issuer,
            &authorization,
            &[
                ("error", "access_denied"),
                ("error_description", "The user denied the request"),
            ],
        )
    }
}

/// Where to send the browser with the answer to a request: its client's redirect URI, with `params`
/// and the configured `issuer`, if any.
fn redirect(
    issuer: Option<&bencher_json::OAuthIssuer>,
    authorization: &QueryOAuthAuthorization,
    params: &[(&str, &str)],
) -> Result<Response<Body>, HttpError> {
    let url = client_redirect(
        &authorization.redirect_uri,
        params,
        authorization.state.as_ref(),
        issuer,
    );
    json(&JsonOAuthRedirect { url })
}

/// Refuse an approval of the Bencher CLI that is not a browser session signed in within the fresh
/// sign-in window, or that asks for server admin from a reader without it, which a later refresh
/// token could otherwise carry far past the session.
fn cli_approver(
    context: &ApiContext,
    auth_user: &AuthUser,
    pending: &QueryOAuthAuthorization,
) -> Result<(), HttpError> {
    if !auth_user.is_session() {
        return Err(forbidden_error(CLI_NEEDS_A_SESSION));
    }
    if pending.scope.permits(OAuthScope::Admin) && !auth_user.is_admin(&context.rbac) {
        let mut error = forbidden_error(CLI_NEEDS_A_SERVER_ADMIN);
        error.error_code = Some(NOT_A_SERVER_ADMIN.to_owned());
        return Err(error);
    }
    auth_user.check_fresh_sign_in(context)
}

/// The user approving a request, and when they last proved who they are: a user API key pasted in
/// the decision or sent as a bearer, which proves no sign in and is charged as an attempt to sign
/// in from the caller's address, or else the session cookie.
/// No credential is ever quoted back.
async fn approver(
    rqctx: &RequestContext<ApiContext>,
    pasted: Option<UserKey>,
) -> Result<(AuthUser, Option<DateTime>), HttpError> {
    let context = rqctx.context();
    let headers = rqctx.request.headers();
    let bearer_token = if let Some(key) = pasted {
        charge_attempt(rqctx)?;
        BearerToken::UserKey(key)
    } else if let Some(token) = headers
        .get(bencher_json::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(bencher_json::strip_bearer_token)
    {
        bearer(rqctx, token)?
    } else if let Some(cookie) = CookieCredential::from_request(&rqctx.request) {
        BearerToken::Session(cookie)
    } else {
        return Err(unauthorized_error(
            "Approving a request to connect requires a session or a user API key",
        ));
    };
    let auth_user = AuthUser::from_token(context, bearer_token).await?;
    let auth_time = match auth_user.credential {
        AuthCredential::Session(session) => Some(session.proved),
        AuthCredential::UserKey(_) => None,
        AuthCredential::ApiToken | AuthCredential::OAuthGrant { .. } | AuthCredential::CliGrant => {
            #[cfg(feature = "otel")]
            bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::OAuthConsentBlocked);
            return Err(forbidden_error(
                "An API token cannot approve a request to connect. Sign in, or use a user API key (bencher_user_) instead.",
            ));
        },
    };
    Ok((auth_user, auth_time))
}

/// The credential a bearer header names, refusing a project key, which cannot approve.
fn bearer(rqctx: &RequestContext<ApiContext>, token: &str) -> Result<BearerToken, HttpError> {
    let is_project_key = token.starts_with(ProjectKey::PREFIX);
    if is_project_key || token.starts_with(UserKey::PREFIX) {
        charge_attempt(rqctx)?;
    }
    if is_project_key {
        #[cfg(feature = "otel")]
        bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::OAuthConsentBlocked);
        return Err(forbidden_error(
            "A project API key (bencher_run_) cannot approve a request to connect. Sign in, or use a user API key (bencher_user_) instead.",
        ));
    }
    if token.starts_with(UserKey::PREFIX) {
        token.parse::<UserKey>().map(BearerToken::UserKey)
    } else {
        token.parse::<Jwt>().map(BearerToken::Jwt)
    }
    .map_err(|_err| invalid_credential())
}

/// Charge a pasted key as an attempt to sign in from the caller's address.
fn charge_attempt(rqctx: &RequestContext<ApiContext>) -> Result<(), HttpError> {
    let context = rqctx.context();
    if let Some(remote_ip) = context.remote_ip(&rqctx.log, rqctx.request.headers()) {
        context.rate_limiting.public_auth_attempt(remote_ip)?;
    }
    Ok(())
}

fn charge_public_request(rqctx: &RequestContext<ApiContext>) -> Result<(), HttpError> {
    if let Some(remote_ip) = rqctx
        .context()
        .remote_ip(&rqctx.log, rqctx.request.headers())
    {
        rqctx.context().rate_limiting.public_request(remote_ip)?;
    }
    Ok(())
}
