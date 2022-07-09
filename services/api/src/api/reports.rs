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
use uuid::Uuid;

use crate::{
    api::headers::CorsHeaders,
    db::{
        model::{
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

pub const DEFAULT_PROJECT: &str = "default";

#[endpoint {
    method = GET,
    path = "/v0/reports",
    tags = ["reports"]
}]
pub async fn api_get_reports(
    rqctx: Arc<RequestContext<Mutex<SqliteConnection>>>,
) -> Result<HttpResponseHeaders<HttpResponseOk<Vec<Report>>, CorsHeaders>, HttpError> {
    let db_connection = rqctx.context();

    let conn = db_connection.lock().await;
    let reports: Vec<Report> = report_table::table
        .load::<Report>(&*conn)
        .expect("Error loading reports.");

    Ok(HttpResponseHeaders::new(
        HttpResponseOk(reports),
        CorsHeaders::new_origin_all("GET".into(), "Content-Type".into()),
    ))
}

#[endpoint {
    method = POST,
    path = "/v0/reports",
    tags = ["reports"]
}]
pub async fn api_put_report(
    rqctx: Arc<RequestContext<Mutex<SqliteConnection>>>,
    body: TypedBody<JsonReport>,
) -> Result<HttpResponseAccepted<()>, HttpError> {
    let db_connection = rqctx.context();

    let json_report = body.into_inner();
    let conn = db_connection.lock().await;
    let new_report = map_report(&*conn, json_report);
    diesel::insert_into(report_table::table)
        .values(&new_report)
        .execute(&*conn)
        .expect("Error saving new report to database.");

    Ok(HttpResponseAccepted(()))
}

pub fn map_report(conn: &SqliteConnection, report: JsonReport) -> NewReport {
    let JsonReport {
        project,
        testbed,
        adapter,
        start_time,
        end_time,
        metrics: _,
    } = report;
    NewReport {
        uuid: Uuid::new_v4().to_string(),
        project: unwrap_project(project.as_deref()),
        testbed,
        adapter_id: map_adapter(conn, adapter),
        start_time: start_time.naive_utc(),
        end_time: end_time.naive_utc(),
    }
}

pub fn map_adapter(conn: &SqliteConnection, adapter: JsonAdapter) -> i32 {
    adapter_table::table
        .filter(schema::adapter::name.eq(adapter.to_string()))
        .select(schema::adapter::id)
        .first(conn)
        .unwrap()
}

fn unwrap_project(project: Option<&str>) -> String {
    if let Some(project) = project {
        slug::slugify(project)
    } else {
        DEFAULT_PROJECT.into()
    }
}
