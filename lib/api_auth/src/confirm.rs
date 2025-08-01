use bencher_endpoint::{CorsResponse, Endpoint, Post, ResponseOk};
use bencher_json::{JsonConfirm, system::auth::JsonAuthUser};
use bencher_schema::{
    conn_lock,
    context::ApiContext,
    error::{issue_error, unauthorized_error},
    model::user::QueryUser,
};
use dropshot::{HttpError, RequestContext, TypedBody, endpoint};

use super::CLIENT_TOKEN_TTL;

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
    body: TypedBody<JsonConfirm>,
) -> Result<ResponseOk<JsonAuthUser>, HttpError> {
    let json = post_inner(rqctx.context(), body.into_inner()).await?;
    Ok(Post::pub_response_ok(json))
}

async fn post_inner(
    context: &ApiContext,
    json_confirm: JsonConfirm,
) -> Result<JsonAuthUser, HttpError> {
    let claims = context
        .token_key
        .validate_auth(&json_confirm.token)
        .map_err(unauthorized_error)?;
    let email = claims.email();
    let user = QueryUser::get_with_email(conn_lock!(context), email)?.into_json();

    let token = context
        .token_key
        .new_client(email.clone(), CLIENT_TOKEN_TTL)
        .map_err(|e| {
            issue_error(
                "Failed to create client JWT",
                &format!("Failed to create client JWT ({email} | {CLIENT_TOKEN_TTL})"),
                e,
            )
        })?;

    let claims = context.token_key.validate_client(&token).map_err(|e| {
        issue_error(
            "Failed to validate new client JWT",
            &format!("Failed to validate new client JWT: {token}"),
            e,
        )
    })?;

    Ok(JsonAuthUser {
        user,
        token,
        creation: claims.issued_at(),
        expiration: claims.expiration(),
    })
}
