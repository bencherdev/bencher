use std::sync::Arc;

use dropshot::{endpoint, HttpError, HttpResponseHeaders, HttpResponseOk, RequestContext};

use crate::{
    util::{headers::CorsHeaders, Context},
    PingMethod,
};

const PONG: &str = "PONG";

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
        CorsHeaders::new_pub(endpoint.to_string()),
    );

    Ok(resp)
}
