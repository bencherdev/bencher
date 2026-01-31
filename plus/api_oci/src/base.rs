//! OCI Base Endpoint - GET /v2/

use bencher_endpoint::{CorsResponse, Endpoint, Get, ResponseOk};
use bencher_schema::context::ApiContext;
use dropshot::{HttpError, RequestContext, endpoint};
use schemars::JsonSchema;
use serde::Serialize;

/// Response for the OCI base endpoint
#[derive(Debug, Serialize, JsonSchema)]
pub struct OciBaseResponse {}

/// CORS preflight for OCI base endpoint
#[endpoint {
    method = OPTIONS,
    path = "/v2/",
    tags = ["oci"],
}]
pub async fn oci_base_options(
    _rqctx: RequestContext<ApiContext>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into()]))
}

/// OCI API version check endpoint
///
/// Returns 200 OK if the registry implements the OCI Distribution Spec.
/// This is the first endpoint clients call to verify registry compatibility.
#[endpoint {
    method = GET,
    path = "/v2/",
    tags = ["oci"],
}]
pub async fn oci_base(
    _rqctx: RequestContext<ApiContext>,
) -> Result<ResponseOk<OciBaseResponse>, HttpError> {
    Ok(Get::pub_response_ok(OciBaseResponse {}))
}
