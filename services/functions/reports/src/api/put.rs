use std::sync::Arc;

use dropshot::endpoint;
use dropshot::HttpError;
use dropshot::HttpResponseAccepted;
use dropshot::RequestContext;
use dropshot::TypedBody;

#[endpoint {
    method = PUT,
    path = "/v0/reports",
}]
pub async fn api_put_reports(
    _rqctx: Arc<RequestContext<()>>,
    body: TypedBody<String>,
) -> Result<HttpResponseAccepted<String>, HttpError> {
    let body = body.into_inner();
    Ok(HttpResponseAccepted(body))
}
