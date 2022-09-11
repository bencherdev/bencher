use std::sync::Arc;

use dropshot::{endpoint, HttpError, HttpResponseHeaders, HttpResponseOk, RequestContext};

use crate::{
    endpoints::{endpoint::pub_response_ok, Endpoint, Method, Resource},
    util::{headers::CorsHeaders, Context},
};

const PONG: &str = "PONG";

#[endpoint {
    method = GET,
    path = "/v0/ping",
    tags = ["ping"]
}]
pub async fn get(
    rqctx: Arc<RequestContext<Context>>,
) -> Result<HttpResponseHeaders<HttpResponseOk<String>, CorsHeaders>, HttpError> {
    let endpoint = Endpoint::new(Resource::Ping, Method::GetOne);

    let _context = &mut *rqctx.context().lock().await;

    pub_response_ok!(endpoint, PONG.into())
}
