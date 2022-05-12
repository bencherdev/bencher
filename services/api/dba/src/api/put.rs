use std::sync::Arc;
use std::sync::Mutex;

use diesel::pg::PgConnection;
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
    _rqctx: Arc<RequestContext<Mutex<PgConnection>>>,
    _body: TypedBody<()>,
) -> Result<HttpResponseAccepted<()>, HttpError> {
    Ok(HttpResponseAccepted(()))
}

#[endpoint {
    method = PUT,
    path = "/v0/dba/rollback",
}]
pub async fn api_put_rollback(
    _rqctx: Arc<RequestContext<Mutex<PgConnection>>>,
    _body: TypedBody<()>,
) -> Result<HttpResponseAccepted<()>, HttpError> {
    Ok(HttpResponseAccepted(()))
}
