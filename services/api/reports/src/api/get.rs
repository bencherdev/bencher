use std::sync::Arc;

use dropshot::endpoint;
use dropshot::HttpError;
use dropshot::HttpResponseOk;
use dropshot::RequestContext;

#[endpoint {
    method = GET,
    path = "/v0/reports",
}]
pub async fn api_get_reports(
    _rqctx: Arc<RequestContext<()>>,
) -> Result<HttpResponseOk<()>, HttpError> {
    Ok(HttpResponseOk(()))
}
