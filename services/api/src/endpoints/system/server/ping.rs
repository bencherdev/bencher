use bencher_json::JsonPing;
use dropshot::{endpoint, HttpError, RequestContext};

use crate::{
    context::ApiContext,
    endpoints::{
        endpoint::{CorsResponse, Get, ResponseOk},
        Endpoint,
    },
};

#[allow(clippy::no_effect_underscore_binding, clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/server/ping",
    tags = ["server", "ping"]
}]
pub async fn server_ping_options(
    _rqctx: RequestContext<ApiContext>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into()]))
}

#[allow(clippy::no_effect_underscore_binding, clippy::unused_async)]
#[endpoint {
    method = GET,
    path = "/v0/server/ping",
    tags = ["server", "ping"]
}]
pub async fn server_ping_get(
    _rqctx: RequestContext<ApiContext>,
) -> Result<ResponseOk<JsonPing>, HttpError> {
    Ok(Get::pub_response_ok(JsonPing::Pong))
}
