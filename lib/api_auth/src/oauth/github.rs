use bencher_endpoint::{CorsResponse, Endpoint, Get, Post, ResponseAccepted, ResponseOk};
use bencher_json::{JsonOAuthUrl, JsonOAuthUser, system::auth::JsonOAuth};
use bencher_schema::{
    context::ApiContext,
    error::{payment_required_error, unauthorized_error},
};
use dropshot::{HttpError, Query, RequestContext, TypedBody, endpoint};
use slog::Logger;

use crate::oauth::{OAuthProvider, oauth_state::OAuthState};

use super::{handle_oauth_user, is_allowed_oauth};

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
    let json = get_inner(&rqctx.log, rqctx.context(), query_params.into_inner()).await?;
    Ok(Get::pub_response_ok(json))
}

async fn get_inner(
    log: &Logger,
    context: &ApiContext,
    oauth_state: OAuthState,
) -> Result<JsonOAuthUrl, HttpError> {
    let Some(github_client) = &context.github_client else {
        let err = "GitHub OAuth2 is not configured";
        slog::warn!(log, "{err}");
        return Err(payment_required_error(err));
    };
    is_allowed_oauth(context).await?;

    let state = oauth_state.encode(log, &context.token_key)?;
    let url = github_client.auth_url(state);

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
        .oauth_user(json_oauth.code.clone())
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
