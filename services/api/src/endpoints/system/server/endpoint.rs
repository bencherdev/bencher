use bencher_json::JsonEndpoint;
use dropshot::{endpoint, HttpError, RequestContext};

use crate::{
    context::ApiContext,
    endpoints::{
        endpoint::{CorsResponse, Get, ResponseOk},
        Endpoint,
    },
    model::user::auth::{AuthUser, PubBearerToken},
};

// TODO remove in due time
#[allow(clippy::no_effect_underscore_binding, clippy::unused_async)]
#[endpoint {
        method = OPTIONS,
        path =  "/v0/server/endpoint",
        tags = ["server"]
    }]
pub async fn server_endpoint_options(
    _rqctx: RequestContext<ApiContext>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into()]))
}

// TODO remove in due time
/// DEPRECATED: View server endpoint
#[endpoint {
        method = GET,
        path =  "/v0/server/endpoint",
        tags = ["server"]
    }]
pub async fn server_endpoint_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: PubBearerToken,
) -> Result<ResponseOk<JsonEndpoint>, HttpError> {
    let auth_user = AuthUser::from_pub_token(rqctx.context(), bearer_token).await?;
    Ok(Get::response_ok(
        JsonEndpoint {
            endpoint: rqctx.context().console_url.clone().into(),
        },
        auth_user.is_some(),
    ))
}
