use bencher_endpoint::{CorsResponse, Endpoint, Get, Post, ResponseAccepted, ResponseOk};
use bencher_json::{JsonOAuthUrl, JsonOAuthUser, system::auth::JsonOAuth};
use bencher_schema::{
    HeaderMap,
    context::ApiContext,
    error::{issue_error, payment_required_error, unauthorized_error},
};
use dropshot::{HttpError, Query, RequestContext, TypedBody, endpoint};
use http::{HeaderValue, header::CACHE_CONTROL};
use slog::Logger;

use crate::oauth::{OAuthProvider, oauth_state::OAuthState};

use super::{handle_oauth_user, is_allowed_oauth};

/// The console's sign-in page that GitHub sends the user back to.
const SIGN_IN_PATH: &str = "/auth/github";

#[endpoint {
    method = OPTIONS,
    path =  "/v0/auth/github",
    tags = ["auth"]
}]
pub async fn auth_github_options(
    _rqctx: RequestContext<ApiContext>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into(), Post.into()]))
}

#[endpoint {
    method = GET,
    path =  "/v0/auth/github",
    tags = ["auth"]
}]
pub async fn auth_github_get(
    rqctx: RequestContext<ApiContext>,
    query_params: Query<OAuthState>,
) -> Result<ResponseOk<JsonOAuthUrl>, HttpError> {
    let json = get_inner(
        &rqctx.log,
        rqctx.context(),
        rqctx.request.headers(),
        query_params.into_inner(),
    )
    .await?;
    let mut response = Get::pub_response_ok(json);
    // Every start mints a state of its own, so no cache may answer one start with another's.
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(response)
}

async fn get_inner(
    log: &Logger,
    context: &ApiContext,
    headers: &HeaderMap,
    oauth_state: OAuthState,
) -> Result<JsonOAuthUrl, HttpError> {
    let Some(github_client) = &context.github_client else {
        let err = "GitHub OAuth2 is not configured";
        slog::warn!(log, "{err}");
        return Err(payment_required_error(err));
    };

    is_allowed_oauth(context).await?;

    if let Some(remote_ip) = bencher_schema::RateLimiting::remote_ip(log, headers) {
        context.rate_limiting.public_auth_attempt(remote_ip)?;
    }

    let oauth_state = OAuthState {
        authorization: crate::pending_authorization(context, oauth_state.authorization).await?,
        ..oauth_state
    };
    let state = oauth_state.encode(log, &context.token_key)?;
    let url = github_client.auth_url_with_redirect(state, sign_in_url(context)?);

    Ok(JsonOAuthUrl { url })
}

#[endpoint {
    method = POST,
    path = "/v0/auth/github",
    tags = ["auth"]
}]
pub async fn auth_github_post(
    rqctx: RequestContext<ApiContext>,
    body: TypedBody<JsonOAuth>,
) -> Result<ResponseAccepted<JsonOAuthUser>, HttpError> {
    let json = post_inner(&rqctx.log, rqctx.context(), body.into_inner()).await?;
    Ok(Post::pub_response_accepted(json))
}

async fn post_inner(
    log: &Logger,
    context: &ApiContext,
    json_oauth: JsonOAuth,
) -> Result<JsonOAuthUser, HttpError> {
    let Some(github_client) = &context.github_client else {
        let err = "GitHub OAuth2 is not configured";
        slog::info!(log, "{err}");
        return Err(payment_required_error(err));
    };
    is_allowed_oauth(context).await?;

    let oauth_state = OAuthState::decode(log, &context.token_key, &json_oauth.state)?;

    let (name, email) = github_client
        .oauth_user_with_redirect(json_oauth.code.clone(), sign_in_url(context)?)
        .await
        .map_err(unauthorized_error)?;

    handle_oauth_user(
        log,
        context,
        oauth_state,
        name,
        email,
        OAuthProvider::GitHub,
    )
    .await
}

/// Sign-in names its page, so the app's first callback URL is free for another page.
fn sign_in_url(context: &ApiContext) -> Result<url::Url, HttpError> {
    context.console_url.join(SIGN_IN_PATH).map_err(|e| {
        issue_error(
            "Failed to build the GitHub sign-in URL",
            &format!(
                "Failed to build the GitHub sign-in URL from ({}).",
                context.console_url
            ),
            e,
        )
    })
}
