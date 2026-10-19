use bencher_endpoint::{CorsResponse, Endpoint, Get, ResponseOk};
use bencher_schema::{context::ApiContext, error::service_unavailable_error, public_conn};
use diesel::RunQueryDsl as _;
use dropshot::{HttpError, RequestContext, endpoint};

#[endpoint {
    method = OPTIONS,
    path =  "/v0/server/health",
    tags = ["server"]
}]
pub async fn server_health_options(
    _rqctx: RequestContext<ApiContext>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into()]))
}

/// Check server health
///
/// Check that the API server is running and reaches its database.
#[endpoint {
    method = GET,
    path = "/v0/server/health",
    tags = ["server"]
}]
pub async fn server_health_get(
    rqctx: RequestContext<ApiContext>,
) -> Result<ResponseOk<()>, HttpError> {
    let context = rqctx.context();
    diesel::sql_query("SELECT 1 FROM sqlite_schema LIMIT 1")
        .execute(public_conn!(context))
        .map_err(service_unavailable_error)?;
    Ok(Get::pub_response_ok(()))
}
