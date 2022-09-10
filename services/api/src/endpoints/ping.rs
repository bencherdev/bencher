use std::{fmt, sync::Arc};

use derive_more::Display;
use dropshot::{endpoint, HttpError, HttpResponseHeaders, HttpResponseOk, RequestContext};

use crate::{
    util::{headers::CorsHeaders, Context},
    Endpoint, IntoEndpoint, ToMethod,
};

const PONG: &str = "PONG";

#[derive(Debug, Display, Clone, Copy)]
#[display(fmt = "{}", self.to_method())]
pub enum Method {
    Get,
}

impl IntoEndpoint for Method {
    fn into_endpoint(self) -> Endpoint {
        Endpoint::Ping(self)
    }
}

impl ToMethod for Method {
    fn to_method(&self) -> http::Method {
        match self {
            Self::Get => http::Method::GET,
        }
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
    let endpoint = Method::Get;

    let _context = &mut *rqctx.context().lock().await;

    let resp = HttpResponseHeaders::new(
        HttpResponseOk(PONG.into()),
        CorsHeaders::new_pub_endpoint(endpoint),
    );

    Ok(resp)
}
