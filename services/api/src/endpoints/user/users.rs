use bencher_json::{JsonUser, ResourceId};
use dropshot::{endpoint, HttpError, Path, RequestContext};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::{
    context::ApiContext,
    endpoints::{
        endpoint::{response_ok, ResponseOk},
        Endpoint, Method,
    },
    error::api_error,
    model::{
        user::QueryUser,
        user::{auth::AuthUser, token::same_user},
    },
    util::cors::{get_cors, CorsResponse},
    ApiError,
};

use super::Resource;

const USER_RESOURCE: Resource = Resource::User;

#[derive(Deserialize, JsonSchema)]
pub struct OnePath {
    pub user: ResourceId,
}

#[allow(clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/users/{user}",
    tags = ["users"]
}]
pub async fn one_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<OnePath>,
) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<ApiContext>())
}

#[endpoint {
    method = GET,
    path =  "/v0/users/{user}",
    tags = ["users"]
}]
pub async fn get_one(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<OnePath>,
) -> Result<ResponseOk<JsonUser>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::new(USER_RESOURCE, Method::GetOne);

    let context = rqctx.context();
    let path_params = path_params.into_inner();
    let json = get_one_inner(context, path_params, &auth_user)
        .await
        .map_err(|e| endpoint.err(e))?;

    response_ok!(endpoint, json)
}

async fn get_one_inner(
    context: &ApiContext,
    path_params: OnePath,
    auth_user: &AuthUser,
) -> Result<JsonUser, ApiError> {
    let conn = &mut *context.conn().await;

    let query_user = QueryUser::from_resource_id(conn, &path_params.user)?;
    same_user!(auth_user, context.rbac, query_user.id);

    query_user.into_json().map_err(api_error!())
}
