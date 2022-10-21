use std::sync::Arc;

use bencher_json::{JsonConfig, JsonEmpty, JsonNewToken, JsonToken, ResourceId};
use diesel::{expression_methods::BoolExpressionMethods, ExpressionMethods, QueryDsl, RunQueryDsl};
use dropshot::{endpoint, HttpError, Path, RequestContext, TypedBody};
use schemars::JsonSchema;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    endpoints::{
        endpoint::{response_accepted, response_ok, ResponseAccepted, ResponseOk},
        Endpoint, Method,
    },
    error::api_error,
    model::{
        user::QueryUser,
        user::{
            auth::AuthUser,
            token::{same_user, InsertToken, QueryToken},
        },
    },
    schema,
    util::{
        cors::{get_cors, CorsResponse},
        Context,
    },
    ApiError,
};

use super::Resource;

const RESTART_RESOURCE: Resource = Resource::Restart;

#[endpoint {
    method = OPTIONS,
    path =  "/v0/admin/restart",
    tags = ["admin"]
}]
pub async fn post_options(
    _rqctx: Arc<RequestContext<Context>>,
    _body: TypedBody<JsonEmpty>,
) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = POST,
    path =  "/v0/admin/restart",
    tags = ["admin"]
}]
pub async fn post(
    rqctx: Arc<RequestContext<Context>>,
    _body: TypedBody<JsonEmpty>,
) -> Result<ResponseAccepted<JsonEmpty>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::new(RESTART_RESOURCE, Method::Post);

    let context = rqctx.context();
    let json = post_inner(context, &auth_user)
        .await
        .map_err(|e| endpoint.err(e))?;

    response_accepted!(endpoint, json)
}

async fn post_inner(context: &Context, auth_user: &AuthUser) -> Result<JsonEmpty, ApiError> {
    let api_context = &mut *context.lock().await;

    if !auth_user.is_admin(&api_context.rbac) {
        return Err(ApiError::Admin(auth_user.id));
    }

    let restart_txt = api_context.restart_tx.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        let _ = restart_txt.send(()).await;
    });

    Ok(JsonEmpty {})
}
