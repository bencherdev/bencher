#![cfg(feature = "plus")]

use bencher_json::{system::config::JsonUpdateConfig, JsonConfig};
use dropshot::{endpoint, HttpError, RequestContext, TypedBody};
use http::StatusCode;
use slog::Logger;

use crate::{
    config::{Config, BENCHER_CONFIG},
    context::ApiContext,
    endpoints::{
        endpoint::{CorsResponse, Get, Put, ResponseAccepted, ResponseOk},
        Endpoint,
    },
    error::{bad_request_error, forbidden_error, issue_error},
    model::user::{
        admin::AdminUser,
        auth::{AuthUser, BearerToken},
    },
};

use super::restart::{countdown, DEFAULT_DELAY};

#[allow(clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/server/stats",
    tags = ["server", "stats"]
}]
pub async fn server_stats_options(
    _rqctx: RequestContext<ApiContext>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into()]))
}

#[endpoint {
    method = GET,
    path =  "/v0/server/stats",
    tags = ["server", "stats"]
}]
pub async fn server_stats_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
) -> Result<ResponseOk<JsonConfig>, HttpError> {
    let _admin_user = AdminUser::from_token(rqctx.context(), bearer_token).await?;
    let json = get_one_inner(&rqctx.log, rqctx.context()).await?;
    Ok(Get::auth_response_ok(json))
}

async fn get_one_inner(log: &Logger, context: &ApiContext) -> Result<JsonConfig, HttpError> {
    Ok(Config::load_file(log)
        .await
        .map_err(|e| {
            issue_error(
                StatusCode::NOT_FOUND,
                "Failed to load config file",
                "Failed to load configuration file",
                e,
            )
        })?
        .unwrap_or_default()
        .into())
}
