use std::{
    str::FromStr,
    sync::Arc,
};

use bencher_json::JsonUser;
use diesel::{
    QueryDsl,
    RunQueryDsl,
    SqliteConnection,
};
use dropshot::{
    endpoint,
    HttpError,
    HttpResponseAccepted,
    HttpResponseHeaders,
    HttpResponseOk,
    Path,
    RequestContext,
    TypedBody,
};
use schemars::JsonSchema;
use serde::{
    Deserialize,
    Serialize,
};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::{
    db::{
        model::user::InsertUser,
        schema,
    },
    diesel::ExpressionMethods,
    util::headers::CorsHeaders,
};

#[endpoint {
    method = POST,
    path = "/v0/auth/signup",
    tags = ["auth"]
}]
pub async fn api_post_signup(
    rqctx: Arc<RequestContext<Mutex<SqliteConnection>>>,
    body: TypedBody<JsonUser>,
) -> Result<HttpResponseAccepted<()>, HttpError> {
    let db_connection = rqctx.context();

    let json_user = body.into_inner();
    let conn = db_connection.lock().await;
    let insert_user = InsertUser::new(json_user)?;
    diesel::insert_into(schema::user::table)
        .values(&insert_user)
        .execute(&*conn)
        .expect("Error saving new user to database.");

    Ok(HttpResponseAccepted(()))
}
