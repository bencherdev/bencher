use std::sync::Arc;

use bencher_json::{
    token::{
        Audience,
        JsonToken,
    },
    JsonUser,
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
    path =  "/v0/auth/token",
    tags = ["auth"]
}]
pub async fn options(
    _rqctx: Arc<RequestContext<Context>>,
) -> Result<HttpResponseHeaders<HttpResponseOk<String>>, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = POST,
    path = "/v0/auth/token",
    tags = ["auth"]
}]
pub async fn post(
    rqctx: Arc<RequestContext<Context>>,
    body: TypedBody<JsonToken>,
) -> Result<HttpResponseHeaders<HttpResponseAccepted<JsonUser>, CorsHeaders>, HttpError> {
    let api_context = rqctx.context();

    let json_token = body.into_inner();
    let token_data = json_token
        .token
        .validate("todo", Audience::Auth)
        .map_err(|_| http_error!("Failed to login user."))?;

    let api_context = &mut *api_context.lock().await;
    let conn = &mut api_context.db;
    let query_user = schema::user::table
        .filter(schema::user::email.eq(&token_data.claims.sub))
        .first::<QueryUser>(conn)
        .map_err(|_| http_error!("Failed to login user."))?;
    let json_user = query_user.to_json()?;

    Ok(HttpResponseHeaders::new(
        HttpResponseAccepted(json_user),
        CorsHeaders::new_pub("POST".into()),
    ))
}
