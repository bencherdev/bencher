#![cfg(feature = "plus")]

use bencher_endpoint::{CorsResponse, Endpoint, Post, ResponseAccepted};
use bencher_json::{JsonAuthUser, system::auth::JsonOAuth};
use bencher_schema::{
    context::ApiContext,
    error::{payment_required_error, unauthorized_error},
};
use dropshot::{HttpError, RequestContext, TypedBody, endpoint};
use slog::Logger;

use super::{handle_oauth2_user, is_allowed_oauth2};

pub const GITHUB_OAUTH2: &str = "GitHub OAuth2";

#[endpoint {
    method = OPTIONS,
    path =  "/v0/auth/github",
    tags = ["auth"]
}]
pub async fn auth_github_options(
    _rqctx: RequestContext<ApiContext>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Post.into()]))
}

#[endpoint {
    method = POST,
    path = "/v0/auth/github",
    tags = ["auth"]
}]
pub async fn auth_github_post(
    rqctx: RequestContext<ApiContext>,
    body: TypedBody<JsonOAuth>,
) -> Result<ResponseAccepted<JsonAuthUser>, HttpError> {
    let json = post_inner(&rqctx.log, rqctx.context(), body.into_inner()).await?;
    Ok(Post::pub_response_accepted(json))
}

async fn post_inner(
    log: &Logger,
    context: &ApiContext,
    json_oauth: JsonOAuth,
) -> Result<JsonAuthUser, HttpError> {
    let Some(github_client) = &context.github_client else {
        let err = "GitHub OAuth2 is not configured";
        slog::info!(log, "{err}");
        return Err(payment_required_error(err));
    };
    is_allowed_oauth2(context).await?;

    let (name, email) = github_client
        .oauth_user(json_oauth.code.clone())
        .await
        .map_err(unauthorized_error)?;

    handle_oauth2_user(log, context, json_oauth, name, email, GITHUB_OAUTH2).await
}
