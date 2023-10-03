use bencher_json::JsonSpec;
use dropshot::{endpoint, HttpError, HttpResponseHeaders, HttpResponseOk, RequestContext};

use crate::{
    context::ApiContext,
    endpoints::{endpoint::pub_response_ok, Endpoint, Method},
    util::{
        cors::{get_cors, CorsResponse},
        headers::CorsHeaders,
    },
    SWAGGER_SPEC,
};

use super::Resource;

const SPEC_RESOURCE: Resource = Resource::Spec;

#[allow(clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/server/spec",
    tags = ["server", "spec"]
}]
pub async fn server_spec_options(
    _rqctx: RequestContext<ApiContext>,
) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<ApiContext>())
}

#[allow(clippy::unused_async)]
#[endpoint {
    method = GET,
    path = "/v0/server/spec",
    tags = ["server", "spec"]
}]
pub async fn server_spec_get(
    _rqctx: RequestContext<ApiContext>,
) -> Result<HttpResponseHeaders<HttpResponseOk<JsonSpec>, CorsHeaders>, HttpError> {
    let endpoint = Endpoint::new(SPEC_RESOURCE, Method::GetOne);
    pub_response_ok!(endpoint, SWAGGER_SPEC.clone())
}
