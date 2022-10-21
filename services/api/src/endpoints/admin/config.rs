use std::sync::Arc;

use bencher_json::{JsonConfig, JsonNewToken, JsonToken, ResourceId};
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

const CONFIG_RESOURCE: Resource = Resource::Config;

#[endpoint {
    method = OPTIONS,
    path =  "/v0/admin/config",
    tags = ["admin", "config"]
}]
pub async fn post_options(_rqctx: Arc<RequestContext<Context>>) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = POST,
    path =  "/v0/admin/config",
    tags = ["admin", "config"]
}]
pub async fn post(
    rqctx: Arc<RequestContext<Context>>,
    body: TypedBody<JsonConfig>,
) -> Result<ResponseAccepted<JsonConfig>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::new(CONFIG_RESOURCE, Method::Post);

    let context = rqctx.context();
    let json_config = body.into_inner();
    let json = post_inner(context, json_config, &auth_user)
        .await
        .map_err(|e| endpoint.err(e))?;

    response_accepted!(endpoint, json)
}

async fn post_inner(
    context: &Context,
    json_config: JsonConfig,
    auth_user: &AuthUser,
) -> Result<JsonConfig, ApiError> {
    let api_context = &mut *context.lock().await;

    if !auth_user.is_admin(&api_context.rbac) {
        return Err(ApiError::Admin(auth_user.id));
    }

    todo!()
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/admin/config",
    tags = ["admin", "config"]
}]
pub async fn one_options(_rqctx: Arc<RequestContext<Context>>) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = GET,
    path =  "/v0/admin/config",
    tags = ["admin", "config"]
}]
pub async fn get_one(
    rqctx: Arc<RequestContext<Context>>,
) -> Result<ResponseOk<JsonConfig>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::new(CONFIG_RESOURCE, Method::GetOne);

    let context = rqctx.context();
    let json = get_one_inner(context, &auth_user)
        .await
        .map_err(|e| endpoint.err(e))?;

    response_ok!(endpoint, json)
}

async fn get_one_inner(context: &Context, auth_user: &AuthUser) -> Result<JsonConfig, ApiError> {
    let api_context = &mut *context.lock().await;

    if !auth_user.is_admin(&api_context.rbac) {
        return Err(ApiError::Admin(auth_user.id));
    }

    todo!()
}
