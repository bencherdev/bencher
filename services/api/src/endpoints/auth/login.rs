use std::sync::Arc;

use bencher_json::{
    JsonLogin,
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
) -> Result<HttpResponseHeaders<HttpResponseAccepted<JsonUser>, CorsHeaders>, HttpError> {
    let db_connection = rqctx.context();

    let json_login = body.into_inner();
    let conn = &mut *db_connection.lock().await;
    let query_user = schema::user::table
        .filter(schema::user::email.eq(&json_login.email))
        .first::<QueryUser>(conn)
        .map_err(|_| http_error!("Failed to login user."))?;
    let json_user = query_user.try_into()?;

    Ok(HttpResponseHeaders::new(
        HttpResponseAccepted(json_user),
        CorsHeaders::new_pub("POST".into()),
    ))
}
