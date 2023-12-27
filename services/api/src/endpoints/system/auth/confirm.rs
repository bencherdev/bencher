use bencher_json::{system::auth::JsonAuthUser, JsonAuthConfirm};
use dropshot::{endpoint, HttpError, RequestContext, TypedBody};
use http::StatusCode;

use crate::{
    context::ApiContext,
    endpoints::{
        endpoint::{CorsResponse, Post, ResponseOk},
        Endpoint,
    },
    error::{issue_error, unauthorized_error},
    model::user::QueryUser,
};

use super::CLIENT_TOKEN_TTL;

#[allow(clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/auth/confirm",
    tags = ["auth"]
}]
pub async fn auth_confirm_options(
    _rqctx: RequestContext<ApiContext>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Post.into()]))
}

#[endpoint {
    method = POST,
    path = "/v0/auth/confirm",
    tags = ["auth"]
}]
pub async fn auth_confirm_post(
    rqctx: RequestContext<ApiContext>,
    body: TypedBody<JsonAuthConfirm>,
) -> Result<ResponseOk<JsonAuthUser>, HttpError> {
    let json = post_inner(rqctx.context(), body.into_inner()).await?;
    Ok(Post::pub_response_ok(json))
}

async fn post_inner(
    context: &ApiContext,
    json_auth_confirm: JsonAuthConfirm,
) -> Result<JsonAuthUser, HttpError> {
    let conn = &mut *context.conn().await;

    let claims = context
        .token_key
        .validate_auth(&json_auth_confirm.token)
        .map_err(unauthorized_error)?;
    let email = claims.email();
    let user = QueryUser::get_with_email(conn, email)?.into_json();

    let token = context
        .token_key
        .new_client(email.clone(), CLIENT_TOKEN_TTL)
        .map_err(|e| {
            issue_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to create client JWT",
                &format!("Failed to create client JWT ({email} | {CLIENT_TOKEN_TTL})"),
                e,
            )
        })?;

    Ok(JsonAuthUser { user, token })
}
