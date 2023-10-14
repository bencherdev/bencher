use bencher_json::{system::auth::JsonAuthUser, JsonAuthToken};
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
use dropshot::{endpoint, HttpError, RequestContext, TypedBody};

use crate::{
    context::ApiContext,
    endpoints::{
        endpoint::{pub_response_accepted, CorsResponse, ResponseAccepted},
        Endpoint,
    },
    model::user::QueryUser,
    schema, ApiError,
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
    Ok(Endpoint::cors(&[Endpoint::Post]))
}

#[endpoint {
    method = POST,
    path = "/v0/auth/confirm",
    tags = ["auth"]
}]
pub async fn auth_confirm_post(
    rqctx: RequestContext<ApiContext>,
    body: TypedBody<JsonAuthToken>,
) -> Result<ResponseAccepted<JsonAuthUser>, HttpError> {
    let endpoint = Endpoint::Post;

    let json = post_inner(rqctx.context(), body.into_inner())
        .await
        .map_err(|e| {
            if let ApiError::HttpError(e) = e {
                e
            } else {
                endpoint.err(e).into()
            }
        })?;

    pub_response_accepted!(endpoint, json)
}

async fn post_inner(
    context: &ApiContext,
    json_token: JsonAuthToken,
) -> Result<JsonAuthUser, ApiError> {
    let conn = &mut *context.conn().await;

    let claims = context
        .secret_key
        .validate_auth(&json_token.token)
        .map_err(ApiError::from)?;

    let email = claims.email();
    let user = schema::user::table
        .filter(schema::user::email.eq(email))
        .first::<QueryUser>(conn)
        .map_err(ApiError::from)?
        .into_json();

    let token = context
        .secret_key
        .new_client(email.parse()?, CLIENT_TOKEN_TTL)
        .map_err(ApiError::from)?;

    Ok(JsonAuthUser { user, token })
}
