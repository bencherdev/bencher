use std::sync::Arc;
use std::sync::Mutex;

use diesel::pg::PgConnection;
use dropshot::endpoint;
use dropshot::HttpError;
use dropshot::HttpResponseHeaders;
use dropshot::HttpResponseOk;
use dropshot::RequestContext;

use reports::MetaMetrics;

use diesel::prelude::*;
use util::db::model::Report as DbReport;
use util::db::schema::report;
use util::server::headers::CorsHeaders;

#[endpoint {
    method = GET,
    path = "/v0/metrics",
    tags = ["metrics"]
}]
pub async fn api_get_metrics(
    rqctx: Arc<RequestContext<Mutex<PgConnection>>>,
) -> Result<HttpResponseHeaders<HttpResponseOk<Vec<MetaMetrics>>, CorsHeaders>, HttpError> {
    let db_connection = rqctx.context();

    if let Ok(db_conn) = db_connection.lock() {
        let db_conn = &*db_conn;
        let reports: Vec<DbReport> = report::table
            .load::<DbReport>(db_conn)
            .expect("Error loading reports");

        let metrics: Vec<MetaMetrics> = reports.into_iter().map(|report| report.into()).collect();

        let resp = HttpResponseHeaders::new(
            HttpResponseOk(metrics),
            CorsHeaders::new_origin_all("PUT".into(), "Content-Type".into()),
        );

        Ok(resp)
    } else {
        Err(HttpError::for_bad_request(
            Some(String::from("BadInput")),
            format!("Failed to run query"),
        ))
    }
}
