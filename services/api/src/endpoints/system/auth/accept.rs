use bencher_json::{system::auth::JsonAccept, JsonAuthAck};
use dropshot::{endpoint, HttpError, RequestContext, TypedBody};

use crate::{
    conn_lock,
    context::ApiContext,
    endpoints::{
        endpoint::{CorsResponse, Get, Post, ResponseAccepted},
        Endpoint,
    },
    model::user::auth::{AuthUser, BearerToken},
};

#[allow(clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/auth/accept",
    tags = ["auth", "organizations"]
}]
pub async fn auth_accept_options(
    _rqctx: RequestContext<ApiContext>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into(), Post.into()]))
}

#[endpoint {
    method = POST,
    path = "/v0/auth/accept",
    tags = ["auth", "organizations"]
}]
pub async fn auth_accept_post(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    body: TypedBody<JsonAccept>,
) -> Result<ResponseAccepted<JsonAuthAck>, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    let json = post_inner(rqctx.context(), body.into_inner(), auth_user).await?;
    Ok(Post::auth_response_accepted(json))
}

async fn post_inner(
    context: &ApiContext,
    json_accept: JsonAccept,
    auth_user: AuthUser,
) -> Result<JsonAuthAck, HttpError> {
    auth_user.user.check_is_locked()?;
    auth_user
        .user
        .accept_invite(conn_lock!(context), &context.token_key, &json_accept.invite)?;

    Ok(JsonAuthAck {
        email: auth_user.user.email,
    })
}
