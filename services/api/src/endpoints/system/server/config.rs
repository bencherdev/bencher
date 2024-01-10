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
    error::{bad_request_error, issue_error},
    model::user::{admin::AdminUser, auth::BearerToken},
};

use super::restart::{countdown, DEFAULT_DELAY};

#[allow(clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/server/config",
    tags = ["server", "config"]
}]
pub async fn server_config_options(
    _rqctx: RequestContext<ApiContext>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into()]))
}

#[endpoint {
    method = GET,
    path =  "/v0/server/config",
    tags = ["server", "config"]
}]
pub async fn server_config_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
) -> Result<ResponseOk<JsonConfig>, HttpError> {
    let _admin_user = AdminUser::from_token(rqctx.context(), bearer_token).await?;
    let json = get_one_inner(&rqctx.log).await?;
    Ok(Get::auth_response_ok(json))
}

async fn get_one_inner(log: &Logger) -> Result<JsonConfig, HttpError> {
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

#[endpoint {
    method = PUT,
    path =  "/v0/server/config",
    tags = ["server", "config"]
}]
pub async fn server_config_put(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    body: TypedBody<JsonUpdateConfig>,
) -> Result<ResponseAccepted<JsonConfig>, HttpError> {
    let admin_user = AdminUser::from_token(rqctx.context(), bearer_token).await?;
    let json = put_inner(&rqctx.log, rqctx.context(), body.into_inner(), &admin_user).await?;
    Ok(Put::auth_response_accepted(json))
}

async fn put_inner(
    log: &Logger,
    context: &ApiContext,
    json_config: JsonUpdateConfig,
    admin_user: &AdminUser,
) -> Result<JsonConfig, HttpError> {
    let JsonUpdateConfig { config, delay } = json_config;

    // TODO add validation here
    let config_str = serde_json::to_string(&config).map_err(bad_request_error)?;
    std::env::set_var(BENCHER_CONFIG, &config_str);
    Config::write(log, config_str.as_bytes())
        .await
        .map_err(|e| {
            issue_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to write config file",
                "Failed to write configuration file",
                e,
            )
        })?;
    let json_config = serde_json::from_str(&config_str).map_err(bad_request_error)?;

    countdown(
        log,
        context.restart_tx.clone(),
        delay.unwrap_or(DEFAULT_DELAY),
        admin_user.user().id,
    );

    Ok(json_config)
}
