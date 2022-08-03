use std::sync::Arc;

use bencher_json::{
    JsonNewReport,
    JsonReport,
};
use diesel::{
    QueryDsl,
    RunQueryDsl,
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
use serde::Deserialize;

use crate::{
    db::{
        model::report::{
            InsertReport,
            QueryReport,
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
    let json: Vec<JsonReport> = schema::report::table
        .load::<QueryReport>(&*conn)
        .map_err(|_| http_error!("Failed to get reports."))?
        .into_iter()
        .filter_map(|query| query.to_json(&*conn).ok())
        .collect();

    Ok(HttpResponseHeaders::new(
        HttpResponseOk(json),
        CorsHeaders::new_pub("GET".into()),
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
    let query = schema::report::table
        .filter(schema::report::uuid.eq(&path_params.report_uuid))
        .first::<QueryReport>(&*conn)
        .map_err(|_| http_error!("Failed to get report."))?;
    let json = query.to_json(&*conn)?;

    Ok(HttpResponseHeaders::new(
        HttpResponseOk(json),
        CorsHeaders::new_pub("GET".into()),
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
    let insert_report = InsertReport::from_json(&*conn, &uuid, json_report)?;
    diesel::insert_into(schema::report::table)
        .values(&insert_report)
        .execute(&*conn)
        .map_err(|_| http_error!("Failed to create report."))?;

    Ok(HttpResponseAccepted(()))
}
