use std::sync::Arc;

use dropshot::{endpoint, HttpError, HttpResponseHeaders, HttpResponseOk, RequestContext};

use crate::{
    endpoints::{endpoint::pub_response_ok, Endpoint, Method, Resource},
    util::{
        cors::{get_cors, CorsResponse},
        headers::CorsHeaders,
        Context,
    },
};

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
    let endpoint = Endpoint::new(Resource::Ping, Method::GetOne);
    pub_response_ok!(endpoint, PONG.into())
}
