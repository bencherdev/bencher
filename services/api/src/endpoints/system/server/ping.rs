use std::sync::Arc;

use dropshot::{endpoint, HttpError, HttpResponseHeaders, HttpResponseOk, Path, RequestContext};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::{
    context::Context,
    endpoints::{endpoint::pub_response_ok, Endpoint, Method},
    util::{
        cors::{get_cors, CorsResponse},
        headers::CorsHeaders,
    },
};

use super::Resource;

const PING_RESOURCE: Resource = Resource::Ping;
const PONG: &str = "PONG";

#[endpoint {
    method = OPTIONS,
    path =  "/v0/server/ping",
    tags = ["server", "ping"]
}]
pub async fn options(_rqctx: Arc<RequestContext<Context>>) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = GET,
    path = "/v0/server/ping",
    tags = ["server", "ping"]
}]
pub async fn get(
    _rqctx: Arc<RequestContext<Context>>,
) -> Result<HttpResponseHeaders<HttpResponseOk<String>, CorsHeaders>, HttpError> {
    let endpoint = Endpoint::new(PING_RESOURCE, Method::GetOne);
    pub_response_ok!(endpoint, PONG.into())
}

#[derive(Deserialize, JsonSchema)]
pub struct GlobPath {
    pub glob: Vec<String>,
}

#[endpoint {
    method = GET,
    path = "/v0/server/ping/{glob:.*}",
    tags = ["server", "ping"],
    unpublished = true,
}]
pub async fn glob(
    _rqctx: Arc<RequestContext<Context>>,
    _path_params: Path<GlobPath>,
) -> Result<HttpResponseHeaders<HttpResponseOk<String>, CorsHeaders>, HttpError> {
    let endpoint = Endpoint::new(PING_RESOURCE, Method::GetOne);
    pub_response_ok!(endpoint, PONG.into())
}
