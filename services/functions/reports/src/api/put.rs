use std::sync::Arc;

use dropshot::endpoint;
use dropshot::HttpError;
use dropshot::HttpResponseAccepted;
use dropshot::RequestContext;
use dropshot::TypedBody;

use reports::Report;

#[endpoint {
    method = PUT,
    path = "/v0/reports",
}]
pub async fn api_put_reports(
    _rqctx: Arc<RequestContext<()>>,
    body: TypedBody<Report>,
) -> Result<HttpResponseAccepted<Report>, HttpError> {
    let body = body.into_inner();
    Ok(HttpResponseAccepted(body))
}
