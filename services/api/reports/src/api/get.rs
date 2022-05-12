use std::sync::Arc;
use std::sync::Mutex;

use diesel::pg::PgConnection;
use dropshot::endpoint;
use dropshot::HttpError;
use dropshot::HttpResponseOk;
use dropshot::RequestContext;

#[endpoint {
    method = GET,
    path = "/v0/reports",
}]
pub async fn api_get_reports(
    _rqctx: Arc<RequestContext<Mutex<PgConnection>>>,
) -> Result<HttpResponseOk<()>, HttpError> {
    Ok(HttpResponseOk(()))
}
