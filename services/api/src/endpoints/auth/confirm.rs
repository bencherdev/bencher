use std::sync::Arc;

use bencher_json::{
    auth::JsonConfirmed,
    token::{
        JsonToken,
        JsonWebToken,
    },
};
use diesel::{
    QueryDsl,
    RunQueryDsl,
};
use dropshot::{
    endpoint,
    HttpError,
    HttpResponseAccepted,
    HttpResponseHeaders,
    HttpResponseOk,
    RequestContext,
    TypedBody,
};

use crate::{
    db::{
        model::user::QueryUser,
        schema,
    },
    diesel::ExpressionMethods,
    util::{
        cors::get_cors,
        headers::CorsHeaders,
        http_error,
        Context,
    },
};

#[endpoint {
    method = OPTIONS,
    path =  "/v0/auth/confirm",
    tags = ["auth"]
}]
pub async fn options(
    _rqctx: Arc<RequestContext<Context>>,
) -> Result<HttpResponseHeaders<HttpResponseOk<String>>, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = POST,
    path = "/v0/auth/confirm",
    tags = ["auth"]
}]
pub async fn post(
    rqctx: Arc<RequestContext<Context>>,
    body: TypedBody<JsonToken>,
) -> Result<HttpResponseHeaders<HttpResponseAccepted<JsonConfirmed>, CorsHeaders>, HttpError> {
    let context = &mut *rqctx.context().lock().await;

    let json_token = body.into_inner();
    let token_data = json_token
        .token
        .validate_auth(&context.key)
        .map_err(|_| http_error!("Failed to login user."))?;

    let conn = &mut context.db;
    let query_user = schema::user::table
        .filter(schema::user::email.eq(token_data.claims.email()))
        .first::<QueryUser>(conn)
        .map_err(|_| http_error!("Failed to login user."))?;
    let json_user = query_user.to_json()?;

    let json_confirmed = JsonConfirmed {
        user:  json_user,
        token: JsonWebToken::new_client(&context.key, token_data.claims.email().to_string())
            .map_err(|_| http_error!("Failed to login user."))?,
    };

    Ok(HttpResponseHeaders::new(
        HttpResponseAccepted(json_confirmed),
        CorsHeaders::new_pub("POST".into()),
    ))
}
