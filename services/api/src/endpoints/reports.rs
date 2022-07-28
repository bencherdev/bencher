use std::{
    str::FromStr,
    sync::Arc,
};

use bencher_json::{
    JsonNewReport,
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

pub const DEFAULT_PROJECT: &str = "default";

#[endpoint {
    method = GET,
    path = "/v0/reports",
    tags = ["reports"]
}]
pub async fn api_get_reports(
    rqctx: Arc<RequestContext<Context>>,
) -> Result<HttpResponseHeaders<HttpResponseOk<Vec<JsonReport>>, CorsHeaders>, HttpError> {
    let db_connection = rqctx.context();

    let conn = db_connection.lock().await;
    let reports: Vec<JsonReport> = schema::report::table
        .load::<QueryReport>(&*conn)
        .expect("Error loading reports.")
        .into_iter()
        .filter_map(|query_report| query_report.to_json(&*conn).ok())
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
) -> Result<HttpResponseHeaders<HttpResponseOk<JsonReport>, CorsHeaders>, HttpError> {
    let db_connection = rqctx.context();

    let path_params = path_params.into_inner();
    let conn = db_connection.lock().await;
    let query_report = schema::report::table
        .filter(schema::report::uuid.eq(&path_params.report_uuid))
        .first::<QueryReport>(&*conn)
        .map_err(|_| http_error!("Failed to get report."))?;
    let report = query_report.to_json(&*conn)?;

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
    body: TypedBody<JsonNewReport>,
) -> Result<HttpResponseAccepted<()>, HttpError> {
    let uuid = get_token(&rqctx).await?;
    let db_connection = rqctx.context();

    let json_report = body.into_inner();
    let conn = db_connection.lock().await;
    let insert_report = InsertReport::new(&*conn, &uuid, json_report);
    diesel::insert_into(schema::report::table)
        .values(&insert_report)
        .execute(&*conn)
        .map_err(|_| http_error!("Failed to create report."))?;

    Ok(HttpResponseAccepted(()))
}
