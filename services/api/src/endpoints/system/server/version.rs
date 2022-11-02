use std::sync::Arc;

use bencher_json::JsonVersion;
use dropshot::{endpoint, HttpError, HttpResponseHeaders, HttpResponseOk, RequestContext};

use crate::{
    context::Context,
    endpoints::{endpoint::pub_response_ok, Endpoint, Method},
    util::{
        cors::{get_cors, CorsResponse},
        headers::CorsHeaders,
    },
};

use super::Resource;

const VERSION_RESOURCE: Resource = Resource::Version;
const API_VERSION: &str = env!("CARGO_PKG_VERSION");

#[endpoint {
    method = OPTIONS,
    path =  "/v0/server/version",
    tags = ["server", "version"]
}]
pub async fn options(_rqctx: Arc<RequestContext<Context>>) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = GET,
    path = "/v0/server/version",
    tags = ["server", "version"]
}]
pub async fn get(
    _rqctx: Arc<RequestContext<Context>>,
) -> Result<HttpResponseHeaders<HttpResponseOk<JsonVersion>, CorsHeaders>, HttpError> {
    let endpoint = Endpoint::new(VERSION_RESOURCE, Method::GetOne);
    let json = JsonVersion {
        version: API_VERSION.into(),
    };
    pub_response_ok!(endpoint, json)
}
