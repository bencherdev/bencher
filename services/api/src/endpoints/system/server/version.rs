use bencher_json::JsonApiVersion;
use dropshot::{endpoint, HttpError, RequestContext};

use crate::{
    context::ApiContext,
    endpoints::{
        endpoint::{pub_response_ok, CorsResponse, ResponseOk},
        Endpoint,
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
    Ok(Endpoint::cors(&[Endpoint::GetOne]))
}

#[allow(clippy::unused_async)]
#[endpoint {
    method = GET,
    path = "/v0/server/version",
    tags = ["server", "version"]
}]
pub async fn server_version_get(
    _rqctx: RequestContext<ApiContext>,
) -> Result<ResponseOk<JsonApiVersion>, HttpError> {
    let endpoint = Endpoint::GetOne;
    let json = JsonApiVersion {
        version: API_VERSION.into(),
    };
    pub_response_ok!(endpoint, json)
}
