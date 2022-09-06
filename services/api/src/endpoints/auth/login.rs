use std::sync::Arc;

use bencher_json::{jwt::JsonWebToken, JsonEmpty, JsonLogin};
use diesel::{QueryDsl, RunQueryDsl};
use dropshot::{
    endpoint, HttpError, HttpResponseAccepted, HttpResponseHeaders, HttpResponseOk, RequestContext,
    TypedBody,
};
use tracing::info;

use crate::{
    db::{model::user::QueryUser, schema},
    diesel::ExpressionMethods,
    util::{cors::get_cors, headers::CorsHeaders, http_error, Context},
};

#[endpoint {
    method = OPTIONS,
    path =  "/v0/auth/login",
    tags = ["auth"]
}]
pub async fn options(
    _rqctx: Arc<RequestContext<Context>>,
) -> Result<HttpResponseHeaders<HttpResponseOk<String>>, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = POST,
    path = "/v0/auth/login",
    tags = ["auth"]
}]
pub async fn post(
    rqctx: Arc<RequestContext<Context>>,
    body: TypedBody<JsonLogin>,
) -> Result<HttpResponseHeaders<HttpResponseAccepted<JsonEmpty>, CorsHeaders>, HttpError> {
    let json_login = body.into_inner();
    let context = &mut *rqctx.context().lock().await;

    let conn = &mut context.db;
    let query_user = schema::user::table
        .filter(schema::user::email.eq(&json_login.email))
        .first::<QueryUser>(conn)
        .map_err(|_| http_error!("Failed to login user."))?;

    // Check to see if the user account has been locked
    if query_user.locked {
        return Err(http_error!("Failed to login user."));
    }

    let token = JsonWebToken::new_auth(&context.key, query_user.email)
        .map_err(|_| http_error!("Failed to login user."))?;

    // TODO log this as trace if SMTP is configured
    info!("Confirm \"{}\" with: {token}", json_login.email);

    Ok(HttpResponseHeaders::new(
        HttpResponseAccepted(JsonEmpty::default()),
        CorsHeaders::new_pub("POST".into()),
    ))
}
