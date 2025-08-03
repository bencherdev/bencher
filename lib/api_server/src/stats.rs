#![cfg(feature = "plus")]

use bencher_endpoint::{CorsResponse, Endpoint, Get, Post, ResponseAccepted, ResponseOk};
use bencher_json::JsonServerStats;
use bencher_schema::{
    conn_lock,
    context::ApiContext,
    error::issue_error,
    model::{
        server::QueryServer,
        user::{admin::AdminUser, auth::BearerToken},
    },
};
use dropshot::{HttpError, RequestContext, TypedBody, endpoint};
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
    let is_bencher_cloud = context.is_bencher_cloud;
    query_server
        .get_stats(log.clone(), db_path, is_bencher_cloud)
        .await
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
