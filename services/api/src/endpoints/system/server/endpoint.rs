use bencher_json::JsonEndpoint;
use dropshot::{endpoint, HttpError, RequestContext};

use crate::{
    context::ApiContext,
    endpoints::{
        endpoint::{CorsResponse, Get, ResponseOk},
        Endpoint,
    },
    model::user::auth::AuthUser,
};

#[allow(clippy::unused_async)]
#[endpoint {
        method = OPTIONS,
        path =  "/v0/server/endpoint",
        tags = ["server", "endpoint"]
    }]
pub async fn server_endpoint_options(
    _rqctx: RequestContext<ApiContext>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Endpoint::GetOne]))
}

#[endpoint {
        method = GET,
        path =  "/v0/server/endpoint",
        tags = ["server", "endpoint"]
    }]
pub async fn server_endpoint_get(
    rqctx: RequestContext<ApiContext>,
) -> Result<ResponseOk<JsonEndpoint>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await.ok();
    Ok(Get::response_ok(
        JsonEndpoint {
            endpoint: rqctx.context().endpoint.clone().into(),
        },
        auth_user.is_some(),
    ))
}
