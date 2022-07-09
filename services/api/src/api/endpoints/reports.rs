use std::sync::Arc;

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
use report::Report as JsonReport;
use schemars::JsonSchema;
use serde::{
    Deserialize,
    Serialize,
};
use tokio::sync::Mutex;

use crate::{
    api::headers::CorsHeaders,
    db::{
        model::{
            adapter::QueryAdapter,
            report::{
                InsertReport,
                QueryReport,
            },
        },
        schema,
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
    fn new(conn: &SqliteConnection, db_report: QueryReport) -> Self {
        let QueryReport {
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
            adapter_uuid: QueryAdapter::get_uuid(conn, adapter_id),
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
    let reports: Vec<Report> = schema::report::table
        .load::<QueryReport>(&*conn)
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
    let db_report = schema::report::table
        .filter(schema::report::uuid.eq(path_params.report_uuid))
        .first::<QueryReport>(&*conn)
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
    let insert_report = InsertReport::new(&*conn, json_report);
    diesel::insert_into(schema::report::table)
        .values(&insert_report)
        .execute(&*conn)
        .expect("Error saving new report to database.");

    Ok(HttpResponseAccepted(()))
}
