use std::sync::Arc;

use bencher_json::{
    JsonSignup,
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
        model::user::{
            InsertUser,
            QueryUser,
        },
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
) -> Result<HttpResponseHeaders<HttpResponseAccepted<JsonUser>, CorsHeaders>, HttpError> {
    let db_connection = rqctx.context();

    let json_signup = body.into_inner();
    let conn = db_connection.lock().await;
    let insert_user = InsertUser::new(&conn, json_signup)?;
    let email = insert_user.email.clone();
    diesel::insert_into(schema::user::table)
        .values(&insert_user)
        .execute(&*conn)
        .map_err(|e| {
            HttpError::for_bad_request(
                Some(String::from("BadInput")),
                format!("Error saving new user to database: {e}"),
            )
        })?;

    let query_user = schema::user::table
        .filter(schema::user::email.eq(&email))
        .first::<QueryUser>(&*conn)
        .map_err(|_| http_error!("Failed to signup user."))?;
    let json_user = query_user.try_into()?;

    Ok(HttpResponseHeaders::new(
        HttpResponseAccepted(json_user),
        CorsHeaders::new_pub("POST".into()),
    ))
}
