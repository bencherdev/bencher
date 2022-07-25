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
        headers::CorsHeaders,
        Context,
    },
};

#[endpoint {
    method = POST,
    path = "/v0/auth/signup",
    tags = ["auth"]
}]
pub async fn api_post_signup(
    rqctx: Arc<RequestContext<Context>>,
    body: TypedBody<JsonSignup>,
) -> Result<HttpResponseHeaders<HttpResponseAccepted<JsonUser>, CorsHeaders>, HttpError> {
    let db_connection = rqctx.context();

    let json_signup = body.into_inner();
    let email = json_signup.email.clone();
    let conn = db_connection.lock().await;
    let insert_user = InsertUser::new(json_signup)?;
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
        .filter(schema::user::email.eq(email))
        .first::<QueryUser>(&*conn)
        .unwrap();
    let json_user = query_user.try_into()?;

    Ok(HttpResponseHeaders::new(
        HttpResponseAccepted(json_user),
        CorsHeaders::new_origin_all("POST".into(), "Content-Type".into()),
    ))
}
