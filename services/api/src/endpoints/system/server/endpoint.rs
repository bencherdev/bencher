use bencher_json::JsonEndpoint;
use dropshot::{endpoint, HttpError, RequestContext};

use crate::{
    context::ApiContext,
    endpoints::{
        endpoint::{pub_response_ok, response_ok, CorsResponse, ResponseOk},
        Endpoint,
    },
    model::user::auth::AuthUser,
    ApiError,
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
    let endpoint = Endpoint::GetOne;

    let context = rqctx.context();
    let json = get_one_inner(context).await.map_err(|e| {
        if let ApiError::HttpError(e) = e {
            e
        } else {
            endpoint.err(e).into()
        }
    })?;

    if auth_user.is_some() {
        response_ok!(endpoint, json)
    } else {
        pub_response_ok!(endpoint, json)
    }
}

#[allow(clippy::unused_async)]
async fn get_one_inner(context: &ApiContext) -> Result<JsonEndpoint, ApiError> {
    Ok(JsonEndpoint {
        endpoint: context.endpoint.clone().into(),
    })
}
