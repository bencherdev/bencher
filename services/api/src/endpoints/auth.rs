use std::{
    str::FromStr,
    sync::Arc,
};

use bencher_json::JsonTestbed;
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
        model::testbed::{
            InsertTestbed,
            QueryTestbed,
        },
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
    body: TypedBody<JsonTestbed>,
) -> Result<HttpResponseAccepted<()>, HttpError> {
    let db_connection = rqctx.context();

    let json_testbed = body.into_inner();
    let conn = db_connection.lock().await;
    let insert_testbed = InsertTestbed::new(json_testbed);
    diesel::insert_into(schema::testbed::table)
        .values(&insert_testbed)
        .execute(&*conn)
        .expect("Error saving new testbed to database.");

    Ok(HttpResponseAccepted(()))
}
