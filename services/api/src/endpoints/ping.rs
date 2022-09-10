use std::{fmt, sync::Arc};

use dropshot::{endpoint, HttpError, HttpResponseHeaders, HttpResponseOk, RequestContext};

use crate::{
    util::{headers::CorsHeaders, Context},
    Endpoint, IntoEndpoint, ToMethod,
};

const PONG: &str = "PONG";

#[derive(Debug, Clone, Copy)]
pub enum PingMethod {
    Get,
}

impl IntoEndpoint for PingMethod {
    fn into_endpoint(self) -> Endpoint {
        Endpoint::Ping(self)
    }
}

impl ToMethod for PingMethod {
    fn to_method(&self) -> http::Method {
        match self {
            Self::Get => http::Method::GET,
        }
    }
}

impl fmt::Display for PingMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_method())
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
    let endpoint = PingMethod::Get;

    let _context = &mut *rqctx.context().lock().await;

    let resp = HttpResponseHeaders::new(
        HttpResponseOk(PONG.into()),
        CorsHeaders::new_pub_endpoint(endpoint),
    );

    Ok(resp)
}
