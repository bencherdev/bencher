use bencher_json::JsonApiVersion;
use dropshot::{endpoint, HttpError, HttpResponseHeaders, HttpResponseOk, RequestContext};

use crate::{
    context::ApiContext,
    endpoints::{endpoint::pub_response_ok, Endpoint},
    util::{
        cors::{get_cors, CorsResponse},
        headers::CorsHeaders,
    },
};

const API_VERSION: &str = env!("CARGO_PKG_VERSION");

#[allow(clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/server/version",
    tags = ["server", "version"]
}]
pub async fn server_version_options(
    _rqctx: RequestContext<ApiContext>,
) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<ApiContext>())
}

#[allow(clippy::unused_async)]
#[endpoint {
    method = GET,
    path = "/v0/server/version",
    tags = ["server", "version"]
}]
pub async fn server_version_get(
    _rqctx: RequestContext<ApiContext>,
) -> Result<HttpResponseHeaders<HttpResponseOk<JsonApiVersion>, CorsHeaders>, HttpError> {
    let endpoint = Endpoint::GetOne;
    let json = JsonApiVersion {
        version: API_VERSION.into(),
    };
    pub_response_ok!(endpoint, json)
}
