#![cfg(feature = "plus")]

use bencher_json::{JsonEmpty, JsonServerStats};
use dropshot::{endpoint, HttpError, RequestContext, TypedBody};
use slog::{info, Logger};

use crate::{
    context::{ApiContext, Body, Message, ServerStatsBody},
    endpoints::{
        endpoint::{CorsResponse, Get, Post, ResponseAccepted, ResponseOk},
        Endpoint,
    },
    model::{
        server::QueryServer,
        user::{admin::AdminUser, auth::BearerToken, QueryUser},
    },
};

#[allow(clippy::unused_async)]
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
    let json = get_one_inner(rqctx.context()).await?;
    Ok(Get::auth_response_ok(json))
}

async fn get_one_inner(context: &ApiContext) -> Result<JsonServerStats, HttpError> {
    let conn = &mut *context.conn().await;
    let query_server = QueryServer::get_server(conn)?;
    // Don't include organizations for Bencher Cloud
    let include_organizations = !context.biller.is_some();
    query_server.get_stats(conn, include_organizations)
}

#[endpoint {
    method = POST,
    path =  "/v0/server/stats",
    tags = ["server", "stats"]
}]
pub async fn server_stats_post(
    rqctx: RequestContext<ApiContext>,
    body: TypedBody<JsonServerStats>,
) -> Result<ResponseAccepted<JsonEmpty>, HttpError> {
    let json = post_inner(&rqctx.log, rqctx.context(), body.into_inner()).await?;
    Ok(Post::auth_response_accepted(json))
}

async fn post_inner(
    log: &Logger,
    context: &ApiContext,
    json_server_stats: JsonServerStats,
) -> Result<JsonEmpty, HttpError> {
    let _biller = context.biller()?;
    let conn = &mut *context.conn().await;

    // TODO find a better home for these than my inbox
    info!(log, "Self-Hosted Stats: {json_server_stats:?}");
    let admins = QueryUser::get_admins(conn)?;
    for admin in admins {
        let message = Message {
            to_name: Some(admin.name.clone().into()),
            to_email: admin.email.into(),
            subject: Some("🐰 Self-Hosted Server Stats".into()),
            body: Some(Body::ServerStats(ServerStatsBody {
                server_stats: json_server_stats.clone(),
            })),
        };
        context.messenger.send(log, message);
    }

    Ok(JsonEmpty {})
}
