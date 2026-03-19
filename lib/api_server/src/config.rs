use bencher_config::Config;
use bencher_endpoint::{CorsResponse, Endpoint, Get, ResponseOk};
use bencher_json::{JsonConfig, system::config::JsonConsole};
use bencher_schema::{
    context::ApiContext,
    error::issue_error,
    model::user::{
        admin::AdminUser,
        auth::BearerToken,
        public::{PubBearerToken, PublicUser},
    },
};
use dropshot::{HttpError, RequestContext, endpoint};
use slog::Logger;

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

/// View server configuration
///
/// View the API server configuration.
/// The user must be an admin on the server to use this route.
#[endpoint {
    method = GET,
    path =  "/v0/server/config",
    tags = ["server"]
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
                "Failed to load config file",
                "Failed to load configuration file",
                e,
            )
        })?
        .unwrap_or_default()
        .into())
}

#[endpoint {
        method = OPTIONS,
        path =  "/v0/server/config/console",
        tags = ["server"]
    }]
pub async fn server_config_console_options(
    _rqctx: RequestContext<ApiContext>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into()]))
}

/// View console configuration
///
/// View the Bencher Console configuration managed by the API server.
/// This is a public route and does not require authentication.
#[endpoint {
        method = GET,
        path =  "/v0/server/config/console",
        tags = ["server"]
    }]
pub async fn server_config_console_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: PubBearerToken,
) -> Result<ResponseOk<JsonConsole>, HttpError> {
    let public_user = PublicUser::from_token(
        &rqctx.log,
        rqctx.context(),
        #[cfg(feature = "plus")]
        rqctx.request.headers(),
        bearer_token,
    )
    .await?;
    Ok(Get::response_ok(
        JsonConsole {
            url: rqctx.context().console_url.clone().into(),
        },
        public_user.is_auth(),
    ))
}
