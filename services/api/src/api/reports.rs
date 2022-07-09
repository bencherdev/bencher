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
use uuid::Uuid;

use crate::{
    api::headers::CorsHeaders,
    db::{
        model::{
            NewReport,
            Report as DbReport,
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

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct Report {
    pub uuid:         String,
    pub project:      Option<String>,
    pub testbed:      Option<String>,
    pub adapter_uuid: String,
    pub start_time:   NaiveDateTime,
    pub end_time:     NaiveDateTime,
}

impl Report {
    fn new(conn: &SqliteConnection, db_report: DbReport) -> Self {
        let DbReport {
            id: _,
            uuid,
            project,
            testbed,
            adapter_id,
            start_time,
            end_time,
        } = db_report;
        Report {
            uuid,
            project,
            testbed,
            adapter_uuid: map_adapter_id(conn, adapter_id),
            start_time,
            end_time,
        }
    }
}

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
        .load::<DbReport>(&*conn)
        .expect("Error loading reports.")
        .into_iter()
        .map(|db_report| Report::new(&*conn, db_report))
        .collect();

    Ok(HttpResponseHeaders::new(
        HttpResponseOk(reports),
        CorsHeaders::new_origin_all("GET".into(), "Content-Type".into()),
    ))
}

#[derive(Deserialize, JsonSchema)]
pub struct PathParams {
    pub report_uuid: String,
}

#[endpoint {
    method = GET,
    path = "/v0/reports/{report_uuid}",
    tags = ["reports"]
}]
pub async fn api_get_report(
    rqctx: Arc<RequestContext<Mutex<SqliteConnection>>>,
    path_params: Path<PathParams>,
) -> Result<HttpResponseHeaders<HttpResponseOk<Report>, CorsHeaders>, HttpError> {
    let db_connection = rqctx.context();

    let path_params = path_params.into_inner();
    let conn = db_connection.lock().await;
    let db_report = report_table::table
        .filter(schema::report::uuid.eq(path_params.report_uuid))
        .first::<DbReport>(&*conn)
        .unwrap();
    let report = Report::new(&*conn, db_report);

    Ok(HttpResponseHeaders::new(
        HttpResponseOk(report),
        CorsHeaders::new_origin_all("GET".into(), "Content-Type".into()),
    ))
}

#[endpoint {
    method = POST,
    path = "/v0/reports",
    tags = ["reports"]
}]
pub async fn api_post_report(
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
        adapter_id: map_adapter_name(conn, adapter),
        start_time: start_time.naive_utc(),
        end_time: end_time.naive_utc(),
    }
}

pub fn map_adapter_name(conn: &SqliteConnection, adapter: JsonAdapter) -> i32 {
    adapter_table::table
        .filter(schema::adapter::name.eq(adapter.to_string()))
        .select(schema::adapter::id)
        .first(conn)
        .unwrap()
}

pub fn map_adapter_id(conn: &SqliteConnection, adapter_id: i32) -> String {
    adapter_table::table
        .filter(schema::adapter::id.eq(adapter_id))
        .select(schema::adapter::uuid)
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
