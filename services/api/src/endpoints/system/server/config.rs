use std::sync::Arc;

use bencher_json::{system::config::JsonUpdateConfig, JsonConfig};
use dropshot::{endpoint, HttpError, RequestContext, TypedBody};

use crate::{
    config::{Config, BENCHER_CONFIG},
    context::Context,
    endpoints::{
        endpoint::{response_accepted, response_ok, ResponseAccepted, ResponseOk},
        Endpoint, Method,
    },
    model::user::auth::AuthUser,
    util::cors::{get_cors, CorsResponse},
    ApiError,
};

use super::{
    restart::{countdown, DEFAULT_DELAY},
    Resource,
};

const CONFIG_RESOURCE: Resource = Resource::Config;

#[endpoint {
    method = OPTIONS,
    path =  "/v0/server/config",
    tags = ["server", "config"]
}]
pub async fn options(_rqctx: Arc<RequestContext<Context>>) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = GET,
    path =  "/v0/server/config",
    tags = ["server", "config"]
}]
pub async fn get_one(
    rqctx: Arc<RequestContext<Context>>,
) -> Result<ResponseOk<JsonConfig>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::new(CONFIG_RESOURCE, Method::GetOne);

    let context = rqctx.context();
    let json = get_one_inner(context, &auth_user)
        .await
        .map_err(|e| endpoint.err(e))?;

    response_ok!(endpoint, json)
}

async fn get_one_inner(context: &Context, auth_user: &AuthUser) -> Result<JsonConfig, ApiError> {
    let api_context = &mut *context.lock().await;

    if !auth_user.is_admin(&api_context.rbac) {
        return Err(ApiError::Admin(auth_user.id));
    }

    Ok(Config::load_file().await.unwrap_or_default().into())
}

#[endpoint {
    method = PUT,
    path =  "/v0/server/config",
    tags = ["server", "config"]
}]
pub async fn put(
    rqctx: Arc<RequestContext<Context>>,
    body: TypedBody<JsonUpdateConfig>,
) -> Result<ResponseAccepted<JsonConfig>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::new(CONFIG_RESOURCE, Method::Post);

    let context = rqctx.context();
    let json_update_config = body.into_inner();
    let json = put_inner(context, json_update_config, &auth_user)
        .await
        .map_err(|e| endpoint.err(e))?;

    response_accepted!(endpoint, json)
}

async fn put_inner(
    context: &Context,
    json_update_config: JsonUpdateConfig,
    auth_user: &AuthUser,
) -> Result<JsonConfig, ApiError> {
    let api_context = &mut *context.lock().await;

    if !auth_user.is_admin(&api_context.rbac) {
        return Err(ApiError::Admin(auth_user.id));
    }

    let JsonUpdateConfig { config, delay } = json_update_config;

    // todo() -> add validation here
    let config_str = serde_json::to_string(&config).map_err(ApiError::Serialize)?;
    std::env::set_var(BENCHER_CONFIG, &config_str);
    Config::write(config_str.as_bytes()).await?;
    let json_config = serde_json::from_str(&config_str).map_err(ApiError::Deserialize)?;

    countdown(
        api_context.restart_tx.clone(),
        delay.unwrap_or(DEFAULT_DELAY),
        auth_user.id,
    )
    .await;

    Ok(json_config)
}
