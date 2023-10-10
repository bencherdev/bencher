use bencher_json::JsonSpec;
use dropshot::{endpoint, HttpError, RequestContext};

use crate::{
    context::ApiContext,
    endpoints::{
        endpoint::{pub_response_ok, CorsResponse, ResponseOk},
        Endpoint,
    },
    SWAGGER_SPEC,
};

#[allow(clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/server/spec",
    tags = ["server", "spec"]
}]
pub async fn server_spec_options(
    _rqctx: RequestContext<ApiContext>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Endpoint::GetOne]))
}

#[allow(clippy::unused_async)]
#[endpoint {
    method = GET,
    path = "/v0/server/spec",
    tags = ["server", "spec"]
}]
pub async fn server_spec_get(
    _rqctx: RequestContext<ApiContext>,
) -> Result<ResponseOk<JsonSpec>, HttpError> {
    let endpoint = Endpoint::GetOne;
    pub_response_ok!(endpoint, SWAGGER_SPEC.clone())
}
