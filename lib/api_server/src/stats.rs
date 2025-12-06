#![cfg(feature = "plus")]

use bencher_endpoint::{CorsResponse, Endpoint, Get, Post, ResponseAccepted, ResponseOk};
use bencher_json::{BooleanParam, JsonServerStats, JsonUuid, SelfHostedStartup, ServerUuid};
use bencher_schema::{
    conn_lock,
    context::ApiContext,
    error::issue_error,
    model::{
        server::QueryServer,
        user::{admin::AdminUser, auth::BearerToken},
    },
};
use dropshot::{HttpError, Path, Query, RequestContext, TypedBody, endpoint};
use schemars::JsonSchema;
use serde::Deserialize;
use slog::Logger;

#[endpoint {
    method = OPTIONS,
    path =  "/v0/server/stats",
    tags = ["server", "stats"]
}]
pub async fn server_stats_options(
    _rqctx: RequestContext<ApiContext>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into(), Post.into()]))
}

/// View server stats
///
/// ➕ Bencher Plus: View the API server stats.
/// The user must be an admin on the server to use this route.
#[endpoint {
    method = GET,
    path =  "/v0/server/stats",
    tags = ["server", "stats"]
}]
pub async fn server_stats_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
) -> Result<ResponseOk<JsonServerStats>, HttpError> {
    let _admin_user = AdminUser::from_token(rqctx.context(), bearer_token).await?;
    let json = get_one_inner(&rqctx.log, rqctx.context()).await?;
    Ok(Get::auth_response_ok(json))
}

async fn get_one_inner(log: &Logger, context: &ApiContext) -> Result<JsonServerStats, HttpError> {
    let query_server = QueryServer::get_server(conn_lock!(context))?;
    let db_path = context.database.path.clone();
    query_server.get_stats(log.clone(), db_path).await
}

// TODO remove in due time
// Due to a bug in the original server stats implementation,
// the endpoint was set to the API server root path
// instead of the `/v0/server/stats` path.
#[endpoint {
    method = POST,
    path =  "/",
    tags = ["server", "stats"]
}]
pub async fn root_server_stats_post(
    rqctx: RequestContext<ApiContext>,
    body: TypedBody<JsonServerStats>,
) -> Result<ResponseAccepted<()>, HttpError> {
    post_inner(&rqctx.log, rqctx.context(), body.into_inner()).await?;
    Ok(Post::auth_response_accepted(()))
}

#[endpoint {
    method = POST,
    path =  "/v0/server/stats",
    tags = ["server", "stats"]
}]
pub async fn server_stats_post(
    rqctx: RequestContext<ApiContext>,
    body: TypedBody<JsonServerStats>,
) -> Result<ResponseAccepted<()>, HttpError> {
    post_inner(&rqctx.log, rqctx.context(), body.into_inner()).await?;
    Ok(Post::auth_response_accepted(()))
}

async fn post_inner(
    log: &Logger,
    context: &ApiContext,
    json_server_stats: JsonServerStats,
) -> Result<(), HttpError> {
    let _biller = context.biller()?;

    let server_stats = serde_json::to_string_pretty(&json_server_stats).map_err(|e| {
        slog::error!(log, "Failed to serialize stats: {e}");
        issue_error(
            "Failed to serialize stats",
            &format!("Failed to serialize stats: {json_server_stats:?}"),
            e,
        )
    })?;
    slog::info!(log, "Self-Hosted Stats: {server_stats:?}");
    QueryServer::send_stats_to_backend(
        log,
        &context.database.connection,
        &context.messenger,
        &server_stats,
        Some(json_server_stats.server.uuid),
    )
    .await?;

    Ok(())
}

#[derive(Deserialize, JsonSchema)]
pub struct StatsParams {
    /// The UUID for the self-hosted server.
    pub server: ServerUuid,
}

#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
pub struct StatsQuery {
    /// Server startup.
    pub startup: BooleanParam<SelfHostedStartup>,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/server/stats/{server}",
    tags = ["server", "stats"]
}]
pub async fn server_startup_stats_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<StatsParams>,
    _query_params: Query<StatsQuery>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into()]))
}

#[endpoint {
    method = GET,
    path =  "/v0/server/stats/{server}",
    tags = ["server", "stats"]
}]
pub async fn server_startup_stats_get(
    _rqctx: RequestContext<ApiContext>,
    path_params: Path<StatsParams>,
    query_params: Query<StatsQuery>,
) -> Result<ResponseOk<JsonUuid>, HttpError> {
    let json = get_startup_inner(&path_params.into_inner(), query_params.into_inner());
    Ok(Get::pub_response_ok(json))
}

fn get_startup_inner(path_params: &StatsParams, query_params: StatsQuery) -> JsonUuid {
    let uuid = path_params.server.into();

    #[cfg(feature = "otel")]
    {
        let StatsQuery { startup } = query_params;

        if startup.into() {
            bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::SelfHostedServerStartup(
                uuid,
            ));
        }
    }

    JsonUuid { uuid }
}
