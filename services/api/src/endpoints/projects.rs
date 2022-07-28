use std::{
    str::FromStr,
    sync::Arc,
};

use bencher_json::{
    JsonProject,
    JsonReport,
};
use chrono::NaiveDateTime;
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
use uuid::Uuid;

use crate::{
    db::{
        model::{
            adapter::QueryAdapter,
            project::InsertProject,
            report::{
                InsertReport,
                QueryReport,
            },
            testbed::QueryTestbed,
        },
        schema,
    },
    diesel::ExpressionMethods,
    util::{
        auth::get_token,
        headers::CorsHeaders,
        http_error,
        Context,
    },
};

#[endpoint {
    method = POST,
    path = "/v0/projects",
    tags = ["projects"]
}]
pub async fn api_post_project(
    rqctx: Arc<RequestContext<Context>>,
    body: TypedBody<JsonProject>,
) -> Result<HttpResponseAccepted<()>, HttpError> {
    let uuid = get_token(&rqctx).await?;
    let db_connection = rqctx.context();

    let json_project = body.into_inner();
    let conn = db_connection.lock().await;
    let insert_project = InsertProject::new(&*conn, &uuid, json_project)?;
    diesel::insert_into(schema::project::table)
        .values(&insert_project)
        .execute(&*conn)
        .map_err(|_| http_error!("Failed to create project."))?;

    Ok(HttpResponseAccepted(()))
}
