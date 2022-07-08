use std::sync::Arc;

use diesel::{
    sqlite::SqliteConnection,
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
use report::Report as JsonReport;
use tokio::sync::Mutex;

use crate::{
    api::headers::CorsHeaders,
    db::{
        model::{
            NewReport,
            Report,
        },
        schema::report as report_table,
    },
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
        .expect("Error loading reports");

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
    let new_report = NewReport::from(json_report);

    let conn = db_connection.lock().await;
    diesel::insert_into(report_table::table)
        .values(&new_report)
        .execute(&*conn)
        .expect("Error saving new report to database.");

    Ok(HttpResponseAccepted(()))
}
