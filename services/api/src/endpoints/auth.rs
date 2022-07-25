use std::sync::Arc;

use bencher_json::JsonSignup;
use diesel::RunQueryDsl;
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
        model::user::InsertUser,
        schema,
    },
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
) -> Result<HttpResponseHeaders<HttpResponseAccepted<()>, CorsHeaders>, HttpError> {
    let db_connection = rqctx.context();

    let json_user = body.into_inner();
    let conn = db_connection.lock().await;
    let insert_user = InsertUser::new(json_user)?;
    diesel::insert_into(schema::user::table)
        .values(&insert_user)
        .execute(&*conn)
        .expect("Error saving new user to database.");

    Ok(HttpResponseHeaders::new(
        HttpResponseAccepted(()),
        CorsHeaders::new_origin_all("POST".into(), "Content-Type".into()),
    ))
}
