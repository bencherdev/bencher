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

const PING_RESOURCE: Resource = Resource::Ping;
const PONG: &str = "PONG";

#[allow(clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/server/ping",
    tags = ["server", "ping"]
}]
pub async fn options(_rqctx: RequestContext<Context>) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<Context>())
}

#[allow(clippy::unused_async)]
#[endpoint {
    method = GET,
    path = "/v0/server/ping",
    tags = ["server", "ping"]
}]
pub async fn get(
    _rqctx: RequestContext<Context>,
) -> Result<HttpResponseHeaders<HttpResponseOk<String>, CorsHeaders>, HttpError> {
    let endpoint = Endpoint::new(PING_RESOURCE, Method::GetOne);
    pub_response_ok!(endpoint, PONG.into())
}
