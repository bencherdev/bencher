use std::sync::LazyLock;

use bencher_endpoint::{CorsResponse, Endpoint, Get, ResponseOk};
use bencher_json::JsonOpenApiSpec;
use bencher_schema::context::ApiContext;
use dropshot::{HttpError, RequestContext, endpoint};

pub const SPEC_STR: &str = include_str!("../../../services/api/openapi.json");
#[expect(clippy::expect_used)]
pub static SPEC: LazyLock<JsonOpenApiSpec> =
    LazyLock::new(|| JsonOpenApiSpec(SPEC_STR.parse().expect("Failed to parse OpenAPI spec")));

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
#[expect(clippy::doc_markdown)]
#[endpoint {
    method = GET,
    path = "/v0/server/spec",
    tags = ["server"]
}]
pub async fn server_spec_get(
    _rqctx: RequestContext<ApiContext>,
) -> Result<ResponseOk<JsonOpenApiSpec>, HttpError> {
    Ok(Get::pub_response_ok(SPEC.clone()))
}
