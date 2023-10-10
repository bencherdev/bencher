use bencher_json::{system::config::JsonUpdateConfig, JsonConfig};
use dropshot::{endpoint, HttpError, RequestContext, TypedBody};
use slog::Logger;

use crate::{
    config::{Config, BENCHER_CONFIG},
    context::ApiContext,
    endpoints::{
        endpoint::{response_accepted, response_ok, ResponseAccepted, ResponseOk},
        Endpoint,
    },
    model::user::auth::AuthUser,
    util::cors::{get_cors, CorsResponse},
    ApiError,
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
    Ok(get_cors::<ApiContext>())
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
    let endpoint = Endpoint::GetOne;

    let context = rqctx.context();
    let json = get_one_inner(&rqctx.log, context, &auth_user)
        .await
        .map_err(|e| {
            if let ApiError::HttpError(e) = e {
                e
            } else {
                endpoint.err(e).into()
            }
        })?;

    response_ok!(endpoint, json)
}

async fn get_one_inner(
    log: &Logger,
    context: &ApiContext,
    auth_user: &AuthUser,
) -> Result<JsonConfig, ApiError> {
    if !auth_user.is_admin(&context.rbac) {
        return Err(ApiError::Admin(auth_user.id));
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
    let endpoint = Endpoint::Put;

    let context = rqctx.context();
    let json_config = body.into_inner();
    let json = put_inner(&rqctx.log, context, json_config, &auth_user)
        .await
        .map_err(|e| {
            if let ApiError::HttpError(e) = e {
                e
            } else {
                endpoint.err(e).into()
            }
        })?;

    response_accepted!(endpoint, json)
}

async fn put_inner(
    log: &Logger,
    context: &ApiContext,
    json_config: JsonUpdateConfig,
    auth_user: &AuthUser,
) -> Result<JsonConfig, ApiError> {
    if !auth_user.is_admin(&context.rbac) {
        return Err(ApiError::Admin(auth_user.id));
    }

    let JsonUpdateConfig { config, delay } = json_config;

    // todo() -> add validation here
    let config_str = serde_json::to_string(&config).map_err(ApiError::Serialize)?;
    std::env::set_var(BENCHER_CONFIG, &config_str);
    Config::write(log, config_str.as_bytes()).await?;
    let json_config = serde_json::from_str(&config_str).map_err(ApiError::Deserialize)?;

    countdown(
        log,
        context.restart_tx.clone(),
        delay.unwrap_or(DEFAULT_DELAY),
        auth_user.id,
    );

    Ok(json_config)
}
