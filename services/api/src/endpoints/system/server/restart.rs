use std::sync::Arc;

use bencher_json::{JsonEmpty, JsonRestart};
use dropshot::{endpoint, HttpError, RequestContext, TypedBody};
use tokio::sync::mpsc::Sender;
use tracing::warn;

use crate::{
    context::Context,
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

#[endpoint {
    method = OPTIONS,
    path =  "/v0/server/restart",
    tags = ["server"]
}]
pub async fn options(
    _rqctx: Arc<RequestContext<Context>>,
    _body: TypedBody<JsonRestart>,
) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = POST,
    path =  "/v0/server/restart",
    tags = ["server"]
}]
pub async fn post(
    rqctx: Arc<RequestContext<Context>>,
    body: TypedBody<JsonRestart>,
) -> Result<ResponseAccepted<JsonEmpty>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::new(RESTART_RESOURCE, Method::Post);

    let context = rqctx.context();
    let json_restart = body.into_inner();
    let json = post_inner(context, json_restart, &auth_user)
        .await
        .map_err(|e| endpoint.err(e))?;

    response_accepted!(endpoint, json)
}

async fn post_inner(
    context: &Context,
    json_restart: JsonRestart,
    auth_user: &AuthUser,
) -> Result<JsonEmpty, ApiError> {
    let api_context = &mut *context.lock().await;

    if !auth_user.is_admin(&api_context.rbac) {
        return Err(ApiError::Admin(auth_user.id));
    }

    countdown(
        api_context.restart_tx.clone(),
        json_restart.delay.unwrap_or(DEFAULT_DELAY),
        auth_user.id,
    )
    .await;

    Ok(JsonEmpty {})
}

pub async fn countdown(restart_tx: Sender<()>, delay: u64, user_id: i32) {
    tokio::spawn(async move {
        for tick in (0..=delay).rev() {
            if tick == 0 {
                warn!("Received admin request from {user_id} to restart. Restarting server now.",);
                let _ = restart_tx.send(()).await;
            } else {
                warn!(
                    "Received admin request from {user_id} to restart. Server will restart in {tick} seconds.",
                );
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
        }
    });
}
