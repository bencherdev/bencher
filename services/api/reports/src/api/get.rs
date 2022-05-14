use std::sync::Arc;
use std::sync::Mutex;

use diesel::pg::PgConnection;
use dropshot::endpoint;
use dropshot::HttpError;
use dropshot::HttpResponseOk;
use dropshot::RequestContext;

use reports::MetaMetrics;

use diesel::prelude::*;
use util::db::model::Report as DbReport;
use util::db::schema::report;

#[endpoint {
    method = GET,
    path = "/v0/metrics",
}]
pub async fn api_get_metrics(
    rqctx: Arc<RequestContext<Mutex<PgConnection>>>,
) -> Result<HttpResponseOk<Vec<MetaMetrics>>, HttpError> {
    let db_connection = rqctx.context();

    if let Ok(db_conn) = db_connection.lock() {
        let db_conn = &*db_conn;
        let reports: Vec<DbReport> = report::table
            .load::<DbReport>(db_conn)
            .expect("Error loading reports");

        let metrics: Vec<MetaMetrics> = reports.into_iter().map(|report| report.into()).collect();

        Ok(HttpResponseOk(metrics))
    } else {
        Err(HttpError::for_bad_request(
            Some(String::from("BadInput")),
            format!("Failed to run query"),
        ))
    }
}
