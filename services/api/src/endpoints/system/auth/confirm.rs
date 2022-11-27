use std::sync::Arc;

use bencher_json::system::{
    auth::{JsonAuthToken, JsonConfirm},
    jwt::JsonWebToken,
};
use diesel::{QueryDsl, RunQueryDsl};
use dropshot::{endpoint, HttpError, RequestContext, TypedBody};

use crate::{
    context::Context,
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

#[endpoint {
    method = OPTIONS,
    path =  "/v0/auth/confirm",
    tags = ["auth"]
}]
pub async fn options(_rqctx: Arc<RequestContext<Context>>) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = POST,
    path = "/v0/auth/confirm",
    tags = ["auth"]
}]
pub async fn post(
    rqctx: Arc<RequestContext<Context>>,
    body: TypedBody<JsonAuthToken>,
) -> Result<ResponseAccepted<JsonConfirm>, HttpError> {
    let endpoint = Endpoint::new(CONFIRM_RESOURCE, Method::Post);

    let json = post_inner(rqctx.context(), body.into_inner())
        .await
        .map_err(|e| endpoint.err(e))?;

    pub_response_accepted!(endpoint, json)
}

async fn post_inner(context: &Context, json_token: JsonAuthToken) -> Result<JsonConfirm, ApiError> {
    let api_context = &mut *context.lock().await;
    let conn = &mut api_context.database;

    let token_data = json_token
        .token
        .validate_auth(&api_context.secret_key.decoding)
        .map_err(api_error!())?;

    let user = schema::user::table
        .filter(schema::user::email.eq(token_data.claims.email()))
        .first::<QueryUser>(conn)
        .map_err(api_error!())?
        .into_json()?;

    let token = JsonWebToken::new_client(
        &api_context.secret_key.encoding,
        token_data.claims.email().parse()?,
        CLIENT_TOKEN_TTL,
    )
    .map_err(api_error!())?;

    Ok(JsonConfirm { user, token })
}
