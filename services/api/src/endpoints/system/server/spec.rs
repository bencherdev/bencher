use bencher_json::JsonSpec;
use dropshot::{endpoint, HttpError, RequestContext};

use crate::{
    context::ApiContext,
    endpoints::{
        endpoint::{CorsResponse, Get, ResponseOk},
        Endpoint,
    },
    SWAGGER_SPEC,
};

#[allow(clippy::no_effect_underscore_binding, clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/server/spec",
    tags = ["server"]
}]
pub async fn server_spec_options(
    _rqctx: RequestContext<ApiContext>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into()]))
}

/// View server OpenAPI specification
///
/// View the API server OpenAPI specification.
/// The OpenAPI specification can be used to generate API client code.
#[allow(
    clippy::no_effect_underscore_binding,
    clippy::doc_markdown,
    clippy::unused_async
)]
#[endpoint {
    method = GET,
    path = "/v0/server/spec",
    tags = ["server"]
}]
pub async fn server_spec_get(
    _rqctx: RequestContext<ApiContext>,
) -> Result<ResponseOk<JsonSpec>, HttpError> {
    Ok(Get::pub_response_ok(SWAGGER_SPEC.clone()))
}
