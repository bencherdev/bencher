use bencher_json::{system::config::JsonUpdateConfig, JsonConfig};
use dropshot::{endpoint, HttpError, RequestContext, TypedBody};

use crate::{
    config::{Config, BENCHER_CONFIG},
    context::ApiContext,
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
    let endpoint = Endpoint::new(CONFIG_RESOURCE, Method::GetOne);

    let context = rqctx.context();
    let json = get_one_inner(context, &auth_user)
        .await
        .map_err(|e| endpoint.err(e))?;

    response_ok!(endpoint, json)
}

async fn get_one_inner(context: &ApiContext, auth_user: &AuthUser) -> Result<JsonConfig, ApiError> {
    if !auth_user.is_admin(&context.rbac) {
        return Err(ApiError::Admin(auth_user.id));
    }

    Ok(Config::load_file().await?.unwrap_or_default().into())
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
    let endpoint = Endpoint::new(CONFIG_RESOURCE, Method::Put);

    let context = rqctx.context();
    let json_update_config = body.into_inner();
    let json = put_inner(context, json_update_config, &auth_user)
        .await
        .map_err(|e| endpoint.err(e))?;

    response_accepted!(endpoint, json)
}

async fn put_inner(
    context: &ApiContext,
    json_update_config: JsonUpdateConfig,
    auth_user: &AuthUser,
) -> Result<JsonConfig, ApiError> {
    if !auth_user.is_admin(&context.rbac) {
        return Err(ApiError::Admin(auth_user.id));
    }

    let JsonUpdateConfig { config, delay } = json_update_config;

    // todo() -> add validation here
    let config_str = serde_json::to_string(&config).map_err(ApiError::Serialize)?;
    std::env::set_var(BENCHER_CONFIG, &config_str);
    Config::write(config_str.as_bytes()).await?;
    let json_config = serde_json::from_str(&config_str).map_err(ApiError::Deserialize)?;

    countdown(
        context.restart_tx.clone(),
        delay.unwrap_or(DEFAULT_DELAY),
        auth_user.id,
    )
    .await;

    Ok(json_config)
}

pub mod endpoint {
    use bencher_json::JsonEndpoint;
    use dropshot::{endpoint, HttpError, RequestContext};

    use crate::{
        context::ApiContext,
        endpoints::{
            endpoint::{pub_response_ok, response_ok, ResponseOk},
            system::server::Resource,
            Endpoint, Method,
        },
        model::user::auth::AuthUser,
        util::cors::{get_cors, CorsResponse},
        ApiError,
    };

    const ENDPOINT_RESOURCE: Resource = Resource::Endpoint;

    #[allow(clippy::unused_async)]
    #[endpoint {
        method = OPTIONS,
        path =  "/v0/server/config/endpoint",
        tags = ["server", "config"]
    }]
    pub async fn server_config_endpoint_options(
        _rqctx: RequestContext<ApiContext>,
    ) -> Result<CorsResponse, HttpError> {
        Ok(get_cors::<ApiContext>())
    }

    #[endpoint {
        method = GET,
        path =  "/v0/server/config/endpoint",
        tags = ["server", "config"]
    }]
    pub async fn server_config_endpoint_get(
        rqctx: RequestContext<ApiContext>,
    ) -> Result<ResponseOk<JsonEndpoint>, HttpError> {
        let auth_user = AuthUser::new(&rqctx).await.ok();
        let endpoint = Endpoint::new(ENDPOINT_RESOURCE, Method::GetOne);

        let context = rqctx.context();
        let json = get_one_inner(context).await.map_err(|e| endpoint.err(e))?;

        if auth_user.is_some() {
            response_ok!(endpoint, json)
        } else {
            pub_response_ok!(endpoint, json)
        }
    }

    async fn get_one_inner(context: &ApiContext) -> Result<JsonEndpoint, ApiError> {
        Ok(JsonEndpoint(context.endpoint.clone().into()))
    }
}
