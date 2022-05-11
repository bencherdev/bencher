use std::sync::Arc;

use dropshot::endpoint;
use dropshot::HttpError;
use dropshot::HttpResponseAccepted;
use dropshot::RequestContext;
use dropshot::TypedBody;

#[endpoint {
    method = PUT,
    path = "/v0/dba/migrate",
}]
pub async fn api_put_migrate(
    _rqctx: Arc<RequestContext<()>>,
    _body: TypedBody<()>,
) -> Result<HttpResponseAccepted<()>, HttpError> {
    Ok(HttpResponseAccepted(()))
}
