use bencher_json::JsonSpec;
use dropshot::{endpoint, HttpError, HttpResponseHeaders, HttpResponseOk, RequestContext};

use crate::{
    context::ApiContext,
    endpoints::{endpoint::pub_response_ok, Endpoint},
    util::{
        cors::{get_cors, CorsResponse},
        headers::CorsHeaders,
    },
    SWAGGER_SPEC,
};

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
    let endpoint = Endpoint::GetOne;
    pub_response_ok!(endpoint, SWAGGER_SPEC.clone())
}
