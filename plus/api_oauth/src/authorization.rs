use bencher_endpoint::{CorsResponse, Endpoint, Get, Post, ResponseOk};
use bencher_json::{
    DateTime, JsonOAuthAuthorization, JsonOAuthDecision, JsonOAuthRedirect, Jwt,
    OAuthAuthorizationUuid, OAuthCode, OAuthCodeHash, ProjectKey, UserKey,
};
use bencher_schema::{
    context::{ApiContext, OAuthServer},
    error::{conflict_error, forbidden_error, issue_error, not_found_error, unauthorized_error},
    model::user::{
        QueryUser,
        auth::{AuthCredential, AuthUser, BearerToken},
        oauth_authorization::{OAuthAnswer, QueryOAuthAuthorization},
    },
    public_conn, write_transaction,
};
use diesel::OptionalExtension as _;
use dropshot::{Body, HttpError, Path, RequestContext, TypedBody, endpoint};
use http::Response;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::{
    oauth_server,
    response::{client_redirect, json},
};

const NOT_FOUND: &str = "No such request to connect, or it expired";
const ANSWERED: &str = "This request to connect has already been answered";
const INVALID_CREDENTIAL: &str = "Invalid session or user API key";

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
    rqctx: RequestContext<ApiContext>,
    _path_params: Path<OAuthAuthorizationParams>,
) -> Result<CorsResponse, HttpError> {
    oauth_server(rqctx.context())?;
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
    let oauth = oauth_server(context)?;
    charge_public_request(&rqctx)?;
    let OAuthAuthorizationParams { authorization } = path_params.into_inner();
    let authorization = QueryOAuthAuthorization::from_uuid(public_conn!(context), authorization)
        .optional()
        .map_err(|error| {
            issue_error(
                "Failed to view an OAuth authorization request",
                "Failed to view an OAuth authorization request:",
                error,
            )
        })?
        .filter(|authorization| !authorization.is_expired(context.clock.now()))
        .ok_or_else(|| not_found_error(NOT_FOUND))?;
    if authorization.is_answered() {
        return Err(conflict_error(ANSWERED));
    }
    Ok(Get::pub_response_ok(authorization.into_json(oauth)))
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
    let oauth = oauth_server(context)?;
    let OAuthAuthorizationParams { authorization } = path_params.into_inner();
    let JsonOAuthDecision { approve } = body.into_inner();
    let now = context.clock.now();

    let (answer, code) = if approve {
        let (auth_user, auth_time) = approver(&rqctx).await?;
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
                oauth,
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
        redirect(oauth, &authorization, &[("code", code.as_ref())])
    } else {
        #[cfg(feature = "otel")]
        bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::OAuthConsent(
            bencher_otel::OAuthConsentKind::Deny,
        ));
        redirect(
            oauth,
            &authorization,
            &[
                ("error", "access_denied"),
                ("error_description", "The user denied the request"),
            ],
        )
    }
}

/// Where to send the browser with the answer to a request: its client's redirect URI, with `params`.
fn redirect(
    oauth: &OAuthServer,
    authorization: &QueryOAuthAuthorization,
    params: &[(&str, &str)],
) -> Result<Response<Body>, HttpError> {
    let url = client_redirect(
        &authorization.redirect_uri,
        params,
        authorization.state.as_ref(),
        oauth.issuer(),
    );
    json(&JsonOAuthRedirect { url })
}

/// The user approving a request, and when they signed in: a session JWT, or a pasted user API key,
/// which proves no sign in and is charged as an attempt to sign in from the caller's address.
/// No credential is ever quoted back.
async fn approver(
    rqctx: &RequestContext<ApiContext>,
) -> Result<(AuthUser, Option<DateTime>), HttpError> {
    let context = rqctx.context();
    let headers = rqctx.request.headers();
    let Some(token) = headers
        .get(bencher_json::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(bencher_json::strip_bearer_token)
    else {
        return Err(unauthorized_error(
            "Approving a request to connect requires a session or a user API key",
        ));
    };
    let is_project_key = token.starts_with(ProjectKey::PREFIX);
    if (is_project_key || token.starts_with(UserKey::PREFIX))
        && let Some(remote_ip) = context.remote_ip(&rqctx.log, headers)
    {
        context.rate_limiting.public_auth_attempt(remote_ip)?;
    }
    if is_project_key {
        #[cfg(feature = "otel")]
        bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::OAuthConsentBlocked);
        return Err(forbidden_error(
            "A project API key (bencher_run_) cannot approve a request to connect. Sign in, or use a user API key (bencher_user_) instead.",
        ));
    }
    let bearer_token = if token.starts_with(UserKey::PREFIX) {
        token.parse::<UserKey>().map(BearerToken::UserKey)
    } else {
        token.parse::<Jwt>().map(BearerToken::Jwt)
    }
    .map_err(|_err| unauthorized_error(INVALID_CREDENTIAL))?;
    let auth_time = if let BearerToken::Jwt(jwt) = &bearer_token {
        context
            .token_key
            .validate_client(jwt)
            .ok()
            .map(|claims| claims.issued_at())
    } else {
        None
    };
    let auth_user = AuthUser::from_token(context, bearer_token).await?;
    if !matches!(
        auth_user.credential,
        AuthCredential::Session(_) | AuthCredential::UserKey(_)
    ) {
        #[cfg(feature = "otel")]
        bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::OAuthConsentBlocked);
        return Err(forbidden_error(
            "An API token cannot approve a request to connect. Sign in, or use a user API key (bencher_user_) instead.",
        ));
    }
    Ok((auth_user, auth_time))
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
