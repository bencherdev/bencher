use bencher_json::JsonApiVersion;
use dropshot::{endpoint, HttpError, RequestContext};

use crate::{
    context::ApiContext,
    endpoints::{
        endpoint::{CorsResponse, Get, ResponseOk},
        Endpoint,
    },
    API_VERSION,
};

#[allow(clippy::no_effect_underscore_binding, clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/server/version",
    tags = ["server"]
}]
pub async fn server_version_options(
    _rqctx: RequestContext<ApiContext>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into()]))
}

/// View server version
///
/// View the API server version.
/// This is used to verify that the CLI and API server are compatible.
/// It can also be used as a simple endpoint to verify that the server is running.
#[allow(clippy::no_effect_underscore_binding, clippy::unused_async)]
#[endpoint {
    method = GET,
    path = "/v0/server/version",
    tags = ["server"]
}]
pub async fn server_version_get(
    _rqctx: RequestContext<ApiContext>,
) -> Result<ResponseOk<JsonApiVersion>, HttpError> {
    Ok(Get::pub_response_ok(JsonApiVersion {
        version: API_VERSION.into(),
    }))
}
