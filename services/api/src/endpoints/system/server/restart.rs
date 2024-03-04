use bencher_json::JsonRestart;
use dropshot::{endpoint, HttpError, RequestContext, TypedBody};
use slog::{error, warn, Logger};
use tokio::sync::mpsc::Sender;

use crate::{
    context::ApiContext,
    endpoints::{
        endpoint::{CorsResponse, Post, ResponseAccepted},
        Endpoint,
    },
    model::user::{admin::AdminUser, auth::BearerToken, UserId},
};

pub const DEFAULT_DELAY: u64 = 3;

#[allow(clippy::no_effect_underscore_binding, clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/server/restart",
    tags = ["server"]
}]
pub async fn server_restart_options(
    _rqctx: RequestContext<ApiContext>,
    _body: TypedBody<JsonRestart>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Post.into()]))
}

#[endpoint {
    method = POST,
    path =  "/v0/server/restart",
    tags = ["server"]
}]
pub async fn server_restart_post(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    body: TypedBody<JsonRestart>,
) -> Result<ResponseAccepted<()>, HttpError> {
    let admin_user = AdminUser::from_token(rqctx.context(), bearer_token).await?;
    post_inner(&rqctx.log, rqctx.context(), body.into_inner(), &admin_user).await?;
    Ok(Post::auth_response_accepted(()))
}

#[allow(clippy::no_effect_underscore_binding, clippy::unused_async)]
async fn post_inner(
    log: &Logger,
    context: &ApiContext,
    json_restart: JsonRestart,
    admin_user: &AdminUser,
) -> Result<(), HttpError> {
    countdown(
        log,
        context.restart_tx.clone(),
        json_restart.delay.unwrap_or(DEFAULT_DELAY),
        admin_user.user().id,
    );

    Ok(())
}

pub fn countdown(log: &Logger, restart_tx: Sender<()>, delay: u64, user_id: UserId) {
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
