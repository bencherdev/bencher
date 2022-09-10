use std::sync::Arc;

use bencher_macros::ToMethod;
use dropshot::{endpoint, HttpError, HttpResponseHeaders, HttpResponseOk, RequestContext};

use crate::{
    util::{headers::CorsHeaders, Context},
    Endpoint, IntoEndpoint,
};

const PONG: &str = "PONG";

#[derive(Debug, Clone, Copy, ToMethod)]
pub enum Method {
    GetOne,
}

impl IntoEndpoint for Method {
    fn into_endpoint(self) -> Endpoint {
        Endpoint::Ping(self)
    }
}

#[endpoint {
    method = GET,
    path = "/v0/ping",
    tags = ["ping"]
}]
pub async fn api_get_ping(
    rqctx: Arc<RequestContext<Context>>,
) -> Result<HttpResponseHeaders<HttpResponseOk<String>, CorsHeaders>, HttpError> {
    let endpoint = Method::GetOne;

    let _context = &mut *rqctx.context().lock().await;

    let resp = HttpResponseHeaders::new(
        HttpResponseOk(PONG.into()),
        CorsHeaders::new_pub_endpoint(endpoint),
    );

    Ok(resp)
}
