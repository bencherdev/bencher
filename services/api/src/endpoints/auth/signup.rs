use std::sync::Arc;

use bencher_json::{jwt::JsonWebToken, JsonEmpty, JsonSignup};
use diesel::RunQueryDsl;
use dropshot::{
    endpoint, HttpError, HttpResponseAccepted, HttpResponseHeaders, HttpResponseOk, RequestContext,
    TypedBody,
};
use tracing::info;

use crate::{
    db::{model::user::InsertUser, schema},
    util::{cors::get_cors, headers::CorsHeaders, http_error, Context},
};

#[endpoint {
    method = OPTIONS,
    path =  "/v0/auth/signup",
    tags = ["auth"]
}]
pub async fn options(
    _rqctx: Arc<RequestContext<Context>>,
) -> Result<HttpResponseHeaders<HttpResponseOk<String>>, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = POST,
    path =  "/v0/auth/signup",
    tags = ["auth"]
}]
pub async fn post(
    rqctx: Arc<RequestContext<Context>>,
    body: TypedBody<JsonSignup>,
) -> Result<HttpResponseHeaders<HttpResponseAccepted<JsonEmpty>, CorsHeaders>, HttpError> {
    let json_signup = body.into_inner();
    let context = &mut *rqctx.context().lock().await;

    let conn = &mut context.db;
    let insert_user = InsertUser::from_json(conn, json_signup)?;
    diesel::insert_into(schema::user::table)
        .values(&insert_user)
        .execute(conn)
        .map_err(|_| http_error!("Failed to signup user."))?;

    let token = JsonWebToken::new_auth(&context.key, insert_user.email.clone())
        .map_err(|_| http_error!("Failed to login user."))?;

    // TODO log this as trace if SMTP is configured
    info!("Confirm \"{}\" with: {token}", insert_user.email);

    Ok(HttpResponseHeaders::new(
        HttpResponseAccepted(JsonEmpty::default()),
        CorsHeaders::new_pub("POST".into()),
    ))
}
