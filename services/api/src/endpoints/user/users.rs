use bencher_json::{JsonUser, ResourceId};
use dropshot::{endpoint, HttpError, Path, RequestContext};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::{
    conn_lock,
    context::ApiContext,
    endpoints::{
        endpoint::{CorsResponse, Get, ResponseOk},
        Endpoint,
    },
    model::{
        user::QueryUser,
        user::{
            auth::{AuthUser, BearerToken},
            same_user,
        },
    },
};

#[derive(Deserialize, JsonSchema)]
pub struct UserParams {
    pub user: ResourceId,
}

#[allow(clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/users/{user}",
    tags = ["users"]
}]
pub async fn user_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<UserParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into()]))
}

#[endpoint {
    method = GET,
    path =  "/v0/users/{user}",
    tags = ["users"]
}]
pub async fn user_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<UserParams>,
) -> Result<ResponseOk<JsonUser>, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    let json = get_one_inner(rqctx.context(), path_params.into_inner(), &auth_user).await?;
    Ok(Get::auth_response_ok(json))
}

async fn get_one_inner(
    context: &ApiContext,
    path_params: UserParams,
    auth_user: &AuthUser,
) -> Result<JsonUser, HttpError> {
    let query_user = QueryUser::from_resource_id(conn_lock!(context), &path_params.user)?;
    same_user!(auth_user, context.rbac, query_user.uuid);

    Ok(query_user.into_json())
}
