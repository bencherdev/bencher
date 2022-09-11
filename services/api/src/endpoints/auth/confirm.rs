use std::sync::Arc;

use bencher_json::{
    auth::{JsonAuthToken, JsonConfirm},
    jwt::JsonWebToken,
};
use diesel::{QueryDsl, RunQueryDsl};
use dropshot::{
    endpoint, HttpError, HttpResponseAccepted, HttpResponseHeaders, RequestContext, TypedBody,
};

use crate::{
    diesel::ExpressionMethods,
    model::user::QueryUser,
    schema,
    util::{
        cors::{get_cors, CorsResponse},
        headers::CorsHeaders,
        map_http_error, Context,
    },
};

#[derive(Debug, Clone, Copy)]
pub enum Method {
    Post,
}

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
) -> Result<HttpResponseHeaders<HttpResponseAccepted<JsonConfirm>, CorsHeaders>, HttpError> {
    let context = &mut *rqctx.context().lock().await;

    let json_token = body.into_inner();
    let token_data = json_token
        .token
        .validate_auth(&context.key)
        .map_err(map_http_error!("Failed to login user."))?;

    let conn = &mut context.db;
    let query_user = schema::user::table
        .filter(schema::user::email.eq(token_data.claims.email()))
        .first::<QueryUser>(conn)
        .map_err(map_http_error!("Failed to login user."))?;
    let json_user = query_user.into_json()?;

    let token = JsonWebToken::new_client(&context.key, token_data.claims.email().to_string())
        .map_err(map_http_error!("Failed to login user."))?;

    let json_confirmed = JsonConfirm {
        user: json_user,
        token,
    };

    Ok(HttpResponseHeaders::new(
        HttpResponseAccepted(json_confirmed),
        CorsHeaders::new_pub("POST".into()),
    ))
}
