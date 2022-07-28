use std::{
    str::FromStr,
    sync::Arc,
};

use bencher_json::JsonReport;
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
        Context,
    },
};

pub const DEFAULT_PROJECT: &str = "default";

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct Report {
    pub uuid:         Uuid,
    pub project:      Option<String>,
    pub testbed_uuid: Option<Uuid>,
    pub adapter_uuid: Uuid,
    pub start_time:   NaiveDateTime,
    pub end_time:     NaiveDateTime,
}

impl Report {
    fn new(conn: &SqliteConnection, db_report: QueryReport) -> Self {
        let QueryReport {
            id: _,
            uuid,
            user_id,
            project,
            testbed_id,
            adapter_id,
            start_time,
            end_time,
        } = db_report;
        Report {
            uuid: Uuid::from_str(&uuid).unwrap(),
            project,
            testbed_uuid: QueryTestbed::get_uuid(conn, testbed_id),
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
    rqctx: Arc<RequestContext<Context>>,
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
        CorsHeaders::new_origin_all("GET".into(), "Content-Type".into(), None),
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
    rqctx: Arc<RequestContext<Context>>,
    path_params: Path<PathParams>,
) -> Result<HttpResponseHeaders<HttpResponseOk<Report>, CorsHeaders>, HttpError> {
    let db_connection = rqctx.context();

    let path_params = path_params.into_inner();
    let conn = db_connection.lock().await;
    let db_report = schema::report::table
        .filter(schema::report::uuid.eq(&path_params.report_uuid))
        .first::<QueryReport>(&*conn)
        .unwrap();
    let report = Report::new(&*conn, db_report);

    Ok(HttpResponseHeaders::new(
        HttpResponseOk(report),
        CorsHeaders::new_origin_all("GET".into(), "Content-Type".into(), None),
    ))
}

#[endpoint {
    method = POST,
    path = "/v0/reports",
    tags = ["reports"]
}]
pub async fn api_post_report(
    rqctx: Arc<RequestContext<Context>>,
    body: TypedBody<JsonReport>,
) -> Result<HttpResponseAccepted<()>, HttpError> {
    let uuid = get_token(&rqctx).await?;
    let db_connection = rqctx.context();

    let json_report = body.into_inner();
    let conn = db_connection.lock().await;
    let insert_report = InsertReport::new(&*conn, &uuid, json_report);
    diesel::insert_into(schema::report::table)
        .values(&insert_report)
        .execute(&*conn)
        .expect("Error saving new report to database.");

    Ok(HttpResponseAccepted(()))
}
