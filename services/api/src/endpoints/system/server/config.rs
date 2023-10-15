use bencher_json::{system::config::JsonUpdateConfig, JsonConfig};
use dropshot::{endpoint, HttpError, RequestContext, TypedBody};
use slog::Logger;

use crate::{
    config::{Config, BENCHER_CONFIG},
    context::ApiContext,
    endpoints::{
        endpoint::{CorsResponse, Get, Put, ResponseAccepted, ResponseOk},
        Endpoint,
    },
    error::{bad_request_error, forbidden_error},
    model::user::auth::AuthUser,
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
    Ok(Endpoint::cors(&[Endpoint::GetOne]))
}

#[endpoint {
    method = GET,
    path =  "/v0/server/config",
    tags = ["server", "config"]
}]
pub async fn server_config_get(
    rqctx: RequestContext<ApiContext>,
) -> Result<ResponseOk<JsonConfig>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let json = get_one_inner(&rqctx.log, rqctx.context(), &auth_user).await?;
    Ok(Get::auth_response_ok(json))
}

async fn get_one_inner(
    log: &Logger,
    context: &ApiContext,
    auth_user: &AuthUser,
) -> Result<JsonConfig, HttpError> {
    if !auth_user.is_admin(&context.rbac) {
        return Err(forbidden_error(format!(
            "User is not an admin ({auth_user:?}). Only admins can get the server configuration."
        )));
    }

    Ok(Config::load_file(log).await?.unwrap_or_default().into())
}

#[endpoint {
    method = PUT,
    path =  "/v0/server/config",
    tags = ["server", "config"]
}]
pub async fn server_config_put(
    rqctx: RequestContext<ApiContext>,
    body: TypedBody<JsonUpdateConfig>,
) -> Result<ResponseAccepted<JsonConfig>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let json = put_inner(&rqctx.log, rqctx.context(), body.into_inner(), &auth_user).await?;
    Ok(Put::auth_response_accepted(json))
}

async fn put_inner(
    log: &Logger,
    context: &ApiContext,
    json_config: JsonUpdateConfig,
    auth_user: &AuthUser,
) -> Result<JsonConfig, HttpError> {
    if !auth_user.is_admin(&context.rbac) {
        return Err(forbidden_error(format!(
            "User is not an admin ({auth_user:?}). Only admins can update the server configuration."
        )));
    }

    let JsonUpdateConfig { config, delay } = json_config;

    // todo() -> add validation here
    let config_str = serde_json::to_string(&config).map_err(bad_request_error)?;
    std::env::set_var(BENCHER_CONFIG, &config_str);
    Config::write(log, config_str.as_bytes()).await?;
    let json_config = serde_json::from_str(&config_str).map_err(bad_request_error)?;

    countdown(
        log,
        context.restart_tx.clone(),
        delay.unwrap_or(DEFAULT_DELAY),
        auth_user.id,
    );

    Ok(json_config)
}
