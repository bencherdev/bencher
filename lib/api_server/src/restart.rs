use bencher_endpoint::{CorsResponse, Endpoint, Post, ResponseAccepted};
use bencher_json::JsonRestart;
use bencher_schema::{
    context::ApiContext,
    model::user::{UserId, admin::AdminUser, auth::BearerToken},
};
use dropshot::{HttpError, RequestContext, TypedBody, endpoint};
use slog::{Logger, error, warn};
use tokio::sync::mpsc::Sender;

const DEFAULT_DELAY: u64 = 3;

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

/// Restart server
///
/// Restart the API server.
/// The user must be an admin on the server to use this route.
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

#[expect(clippy::unused_async, reason = "For consistency with other endpoints")]
async fn post_inner(
    log: &Logger,
    context: &ApiContext,
    json_restart: JsonRestart,
    admin_user: &AdminUser,
) -> Result<(), HttpError> {
    countdown(
        log,
        context.restart_tx.clone(),
        json_restart.delay,
        admin_user.user().id,
    );

    Ok(())
}

pub fn countdown(log: &Logger, restart_tx: Sender<()>, delay: Option<u64>, user_id: UserId) {
    let delay = delay.unwrap_or(DEFAULT_DELAY);
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
                warn!(
                    countdown_log,
                    "Received admin request from {user_id} to restart. Server will restart in {tick} seconds.",
                );
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
        }
    });
}
