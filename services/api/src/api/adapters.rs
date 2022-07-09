use std::sync::Arc;

use chrono::NaiveDateTime;
use diesel::{
    Insertable,
    QueryDsl,
    Queryable,
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
use report::{
    Adapter as JsonAdapter,
    Report as JsonReport,
};
use schemars::JsonSchema;
use serde::{
    Deserialize,
    Serialize,
};
use tokio::sync::{
    Mutex,
    MutexGuard,
};

use crate::{
    api::headers::CorsHeaders,
    db::{
        model::{
            Adapter as DbAdapter,
            NewReport,
            Report,
        },
        schema,
        schema::{
            adapter as adapter_table,
            report as report_table,
        },
    },
    diesel::ExpressionMethods,
};

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct Adapter {
    pub uuid: String,
    pub name: String,
}

impl From<DbAdapter> for Adapter {
    fn from(adapter: DbAdapter) -> Self {
        let DbAdapter { id: _, uuid, name } = adapter;
        Self { uuid, name }
    }
}

#[endpoint {
    method = GET,
    path = "/v0/adapters",
    tags = ["adapters"]
}]
pub async fn api_get_adapters(
    rqctx: Arc<RequestContext<Mutex<SqliteConnection>>>,
) -> Result<HttpResponseHeaders<HttpResponseOk<Vec<Adapter>>, CorsHeaders>, HttpError> {
    let db_connection = rqctx.context();

    let conn = db_connection.lock().await;
    let adapters: Vec<Adapter> = adapter_table::table
        .load::<DbAdapter>(&*conn)
        .expect("Error loading adapters.")
        .into_iter()
        .map(Into::into)
        .collect();

    Ok(HttpResponseHeaders::new(
        HttpResponseOk(adapters),
        CorsHeaders::new_origin_all("GET".into(), "Content-Type".into()),
    ))
}

#[derive(Deserialize, JsonSchema)]
pub struct PathParams {
    pub adapter_uuid: String,
}

#[endpoint {
    method = GET,
    path = "/v0/adapters/{adapter_uuid}",
    tags = ["adapters"]
}]
pub async fn api_get_adapter(
    rqctx: Arc<RequestContext<Mutex<SqliteConnection>>>,
    path_params: Path<PathParams>,
) -> Result<HttpResponseHeaders<HttpResponseOk<Adapter>, CorsHeaders>, HttpError> {
    let db_connection = rqctx.context();

    let path_params = path_params.into_inner();
    let conn = db_connection.lock().await;
    let adapter = adapter_table::table
        .filter(schema::adapter::uuid.eq(path_params.adapter_uuid))
        .first::<DbAdapter>(&*conn)
        .unwrap()
        .into();

    Ok(HttpResponseHeaders::new(
        HttpResponseOk(adapter),
        CorsHeaders::new_origin_all("GET".into(), "Content-Type".into()),
    ))
}
