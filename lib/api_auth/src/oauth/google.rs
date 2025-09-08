use bencher_endpoint::{CorsResponse, Endpoint, Get, Post, ResponseAccepted, ResponseOk};
use bencher_json::{
    JsonOAuthUrl, JsonOAuthUser, Jwt, OrganizationUuid, PlanLevel, system::auth::JsonOAuth,
};
use bencher_schema::{
    context::ApiContext,
    error::{payment_required_error, unauthorized_error},
};
use dropshot::{HttpError, Query, RequestContext, TypedBody, endpoint};
use schemars::JsonSchema;
use serde::Deserialize;
use slog::Logger;

use crate::oauth::oauth_state::OAuthState;

use super::{handle_oauth_user, is_allowed_oauth};

pub const GOOGLE_OAUTH2: &str = "Google OAuth2";

#[endpoint {
    method = OPTIONS,
    path =  "/v0/auth/google",
    tags = ["auth"]
}]
pub async fn auth_google_options(
    _rqctx: RequestContext<ApiContext>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into(), Post.into()]))
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AuthGoogleQuery {
    /// Invitation JWT.
    pub invite: Option<Jwt>,
    /// Organization UUID to claim.
    pub claim: Option<OrganizationUuid>,
    /// Plan level.
    pub plan: Option<PlanLevel>,
}

#[endpoint {
    method = GET,
    path =  "/v0/auth/google",
    tags = ["auth"]
}]
pub async fn auth_google_get(
    rqctx: RequestContext<ApiContext>,
    query_params: Query<AuthGoogleQuery>,
) -> Result<ResponseOk<JsonOAuthUrl>, HttpError> {
    let json = get_inner(&rqctx.log, rqctx.context(), query_params.into_inner()).await?;
    Ok(Get::pub_response_ok(json))
}

async fn get_inner(
    log: &Logger,
    context: &ApiContext,
    query_params: AuthGoogleQuery,
) -> Result<JsonOAuthUrl, HttpError> {
    let Some(google_client) = &context.google_client else {
        let err = "Google OAuth2 is not configured";
        slog::warn!(log, "{err}");
        return Err(payment_required_error(err));
    };
    is_allowed_oauth(context).await?;

    // TODO: Currently, we do not protect against CSRF attacks,
    // as we allow any client to use our authentication endpoints.
    // So the `state` parameter is currently just used to pass callback information.
    // In the future, we may want to restrict allowed clients,
    // at which point we should generate and validate a CSRF token here
    // along with the callback information.
    // https://datatracker.ietf.org/doc/html/rfc6749#section-10.12
    let AuthGoogleQuery {
        invite,
        claim,
        plan,
    } = query_params;

    let state_struct = OAuthState::new(invite, claim, plan);
    let state = state_struct.encode(log, &context.token_key)?;
    let url = google_client.auth_url(state);

    Ok(JsonOAuthUrl { url })
}

#[endpoint {
    method = POST,
    path = "/v0/auth/google",
    tags = ["auth"]
}]
pub async fn auth_google_post(
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
    let Some(google_client) = &context.google_client else {
        let err = "Google OAuth2 is not configured";
        slog::warn!(log, "{err}");
        return Err(payment_required_error(err));
    };
    is_allowed_oauth(context).await?;

    let oauth_state = OAuthState::decode(log, &context.token_key, &json_oauth.state)?;

    let (name, email) = google_client
        .oauth_user(json_oauth.code.clone())
        .await
        .map_err(unauthorized_error)?;

    handle_oauth_user(log, context, oauth_state, name, email, GOOGLE_OAUTH2).await
}
