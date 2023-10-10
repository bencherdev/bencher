use bencher_json::JsonPing;
use dropshot::{endpoint, HttpError, RequestContext};

use crate::{
    context::ApiContext,
    endpoints::{
        endpoint::{pub_response_ok, CorsResponse, ResponseOk},
        Endpoint,
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
    Ok(Endpoint::cors(&[Endpoint::Post]))
}

#[allow(clippy::unused_async)]
#[endpoint {
    method = GET,
    path = "/v0/server/ping",
    tags = ["server", "ping"]
}]
pub async fn server_ping_get(
    _rqctx: RequestContext<ApiContext>,
) -> Result<ResponseOk<JsonPing>, HttpError> {
    let endpoint = Endpoint::GetOne;
    pub_response_ok!(endpoint, JsonPing::Pong)
}
