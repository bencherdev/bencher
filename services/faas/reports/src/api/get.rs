use std::sync::{
    Arc,
    Mutex,
};

use diesel::{
    pg::PgConnection,
    prelude::*,
};
use dropshot::{
    endpoint,
    HttpError,
    HttpResponseHeaders,
    HttpResponseOk,
    RequestContext,
};
use reports::MetaMetrics;
use util::{
    db::{
        model::Report as DbReport,
        schema::report,
    },
    server::headers::CorsHeaders,
};

#[endpoint {
    method = GET,
    path = "/v0/metrics",
    tags = ["metrics"]
}]
pub async fn api_get_metrics(
    rqctx: Arc<RequestContext<Mutex<PgConnection>>>,
) -> Result<HttpResponseHeaders<HttpResponseOk<Vec<MetaMetrics>>, CorsHeaders>, HttpError> {
    let api_context = rqctx.context();

    if let Ok(db_conn) = db_connection.lock() {
        let db_conn = &*db_conn;
        let reports: Vec<DbReport> = report::table
            .load::<DbReport>(db_conn)
            .expect("Error loading reports");

        let metrics: Vec<MetaMetrics> = reports.into_iter().map(|report| report.into()).collect();

        let resp =
            HttpResponseHeaders::new(HttpResponseOk(metrics), CorsHeaders::new_pub("PUT".into()));

        Ok(resp)
    } else {
        Err(HttpError::for_bad_request(
            Some(String::from("BadInput")),
            format!("Failed to run query"),
        ))
    }
}
