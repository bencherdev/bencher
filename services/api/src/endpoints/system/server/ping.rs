use bencher_json::JsonPing;
use dropshot::{endpoint, HttpError, HttpResponseHeaders, HttpResponseOk, RequestContext};

use crate::{
    context::ApiContext,
    endpoints::{endpoint::pub_response_ok, Endpoint},
    util::{
        cors::{get_cors, CorsResponse},
        headers::CorsHeaders,
    },
};

#[allow(clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/server/ping",
    tags = ["server", "ping"]
}]
pub async fn server_ping_options(
    _rqctx: RequestContext<ApiContext>,
) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<ApiContext>())
}

#[allow(clippy::unused_async)]
#[endpoint {
    method = GET,
    path = "/v0/server/ping",
    tags = ["server", "ping"]
}]
pub async fn server_ping_get(
    _rqctx: RequestContext<ApiContext>,
) -> Result<HttpResponseHeaders<HttpResponseOk<JsonPing>, CorsHeaders>, HttpError> {
    let endpoint = Endpoint::GetOne;
    pub_response_ok!(endpoint, JsonPing::Pong)
}
