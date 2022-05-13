use std::sync::Arc;
use std::sync::Mutex;

use diesel::pg::PgConnection;
use dropshot::endpoint;
use dropshot::HttpError;
use dropshot::HttpResponseOk;
use dropshot::RequestContext;

// use reports::MetaMetrics;
use util::db::model::{NewReport as NewDbReport, Report as DbReport};
use util::db::schema::report;

#[endpoint {
    method = GET,
    path = "/v0/metrics",
}]
pub async fn api_get_metrics(
    rqctx: Arc<RequestContext<Mutex<PgConnection>>>,
) -> Result<HttpResponseOk<()>, HttpError> {
    let db_connection = rqctx.context();

    // let results = reports
    //     .load::<DbReport>(&connection)
    //     .expect("Error loading reports");

    Ok(HttpResponseOk(()))
}
