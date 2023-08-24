use bencher_json::{JsonEmpty, JsonRestart};
use dropshot::{endpoint, HttpError, RequestContext, TypedBody};
use slog::{error, warn, Logger};
use tokio::sync::mpsc::Sender;

use crate::{
    context::ApiContext,
    endpoints::{
        endpoint::{response_accepted, ResponseAccepted},
        Endpoint, Method,
    },
    model::user::auth::AuthUser,
    util::cors::{get_cors, CorsResponse},
    ApiError,
};

use super::Resource;

const RESTART_RESOURCE: Resource = Resource::Restart;

pub const DEFAULT_DELAY: u64 = 3;

#[allow(clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/server/restart",
    tags = ["server"]
}]
pub async fn server_restart_options(
    _rqctx: RequestContext<ApiContext>,
    _body: TypedBody<JsonRestart>,
) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<ApiContext>())
}

#[endpoint {
    method = POST,
    path =  "/v0/server/restart",
    tags = ["server"]
}]
pub async fn server_restart_post(
    rqctx: RequestContext<ApiContext>,
    body: TypedBody<JsonRestart>,
) -> Result<ResponseAccepted<JsonEmpty>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::new(RESTART_RESOURCE, Method::Post);

    let context = rqctx.context();
    let json_restart = body.into_inner();
    let json = post_inner(&rqctx.log, context, json_restart, &auth_user)
        .await
        .map_err(|e| endpoint.err(e))?;

    response_accepted!(endpoint, json)
}

async fn post_inner(
    log: &Logger,
    context: &ApiContext,
    json_restart: JsonRestart,
    auth_user: &AuthUser,
) -> Result<JsonEmpty, ApiError> {
    if !auth_user.is_admin(&context.rbac) {
        return Err(ApiError::Admin(auth_user.id));
    }

    countdown(
        log,
        context.restart_tx.clone(),
        json_restart.delay.unwrap_or(DEFAULT_DELAY),
        auth_user.id,
    )
    .await;

    Ok(JsonEmpty {})
}

pub async fn countdown(log: &Logger, restart_tx: Sender<()>, delay: u64, user_id: i32) {
    let countdown_log = log.clone();
    tokio::spawn(async move {
        for tick in (0..=delay).rev() {
            if tick == 0 {
                warn!(
                    countdown_log,
                    "Received admin request from {user_id} to restart. Restarting server now.",
                );
                if let Err(e) = restart_tx.send(()).await {
                    error!(countdown_log, "Failed to send restart for {user_id}: {e}");
                }
            } else {
                warn!(countdown_log,
                    "Received admin request from {user_id} to restart. Server will restart in {tick} seconds.",
                );
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
        }
    });
}
