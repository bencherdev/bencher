use bencher_json::{system::auth::JsonConfirm, JsonAuthToken};
use diesel::{QueryDsl, RunQueryDsl};
use dropshot::{endpoint, HttpError, RequestContext, TypedBody};

use crate::{
    context::ApiContext,
    diesel::ExpressionMethods,
    endpoints::{
        endpoint::{pub_response_accepted, ResponseAccepted},
        Endpoint, Method,
    },
    error::api_error,
    model::user::QueryUser,
    schema,
    util::cors::{get_cors, CorsResponse},
    ApiError,
};

use super::{Resource, CLIENT_TOKEN_TTL};

const CONFIRM_RESOURCE: Resource = Resource::Confirm;

#[allow(clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/auth/confirm",
    tags = ["auth"]
}]
pub async fn options(_rqctx: RequestContext<ApiContext>) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<ApiContext>())
}

#[endpoint {
    method = POST,
    path = "/v0/auth/confirm",
    tags = ["auth"]
}]
pub async fn post(
    rqctx: RequestContext<ApiContext>,
    body: TypedBody<JsonAuthToken>,
) -> Result<ResponseAccepted<JsonConfirm>, HttpError> {
    let endpoint = Endpoint::new(CONFIRM_RESOURCE, Method::Post);

    let json = post_inner(rqctx.context(), body.into_inner())
        .await
        .map_err(|e| endpoint.err(e))?;

    pub_response_accepted!(endpoint, json)
}

async fn post_inner(
    context: &ApiContext,
    json_token: JsonAuthToken,
) -> Result<JsonConfirm, ApiError> {
    let conn = &mut *context.conn().await;

    let token_data = context
        .secret_key
        .validate_auth(&json_token.token)
        .map_err(api_error!())?;

    let user = schema::user::table
        .filter(schema::user::email.eq(token_data.claims.email()))
        .first::<QueryUser>(conn)
        .map_err(api_error!())?
        .into_json()?;

    let token = context
        .secret_key
        .new_client(token_data.claims.email().parse()?, CLIENT_TOKEN_TTL)
        .map_err(api_error!())?;

    Ok(JsonConfirm { user, token })
}
