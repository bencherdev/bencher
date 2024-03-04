use bencher_json::{JsonUpdateUser, JsonUser, ResourceId};
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
use dropshot::{endpoint, HttpError, Path, RequestContext, TypedBody};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::{
    conn_lock,
    context::ApiContext,
    endpoints::{
        endpoint::{CorsResponse, Get, Patch, ResponseOk},
        Endpoint,
    },
    error::{forbidden_error, resource_conflict_err},
    model::user::{
        auth::{AuthUser, BearerToken},
        same_user, QueryUser, UpdateUser,
    },
    schema,
};

#[derive(Deserialize, JsonSchema)]
pub struct UserParams {
    /// The slug or UUID for a user.
    pub user: ResourceId,
}

#[allow(clippy::no_effect_underscore_binding, clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/users/{user}",
    tags = ["users"]
}]
pub async fn user_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<UserParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into(), Patch.into()]))
}

/// View a user
///
/// View a user.
/// Only the authenticated user themselves and server admins have access to this endpoint.
/// To view users within your organization, use the organization members endpoints.
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

/// Update a user
///
/// Update a user.
/// Only the authenticated user themselves and server admins have access to this endpoint.
/// Some fields can only be updated by an admin.
/// To manage users within your organization, use the organization members endpoints.
#[endpoint {
    method = PATCH,
    path =  "/v0/users/{user}",
    tags = ["users"]
}]
pub async fn user_patch(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<UserParams>,
    body: TypedBody<JsonUpdateUser>,
) -> Result<ResponseOk<JsonUser>, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    let json = patch_inner(
        rqctx.context(),
        path_params.into_inner(),
        body.into_inner(),
        &auth_user,
    )
    .await?;
    Ok(Patch::auth_response_ok(json))
}

async fn patch_inner(
    context: &ApiContext,
    path_params: UserParams,
    json_user: JsonUpdateUser,
    auth_user: &AuthUser,
) -> Result<JsonUser, HttpError> {
    let query_user = QueryUser::from_resource_id(conn_lock!(context), &path_params.user)?;
    same_user!(auth_user, context.rbac, query_user.uuid);

    let admin_only_error = |field: &str| {
        forbidden_error(format!(
            "Only admins can update the `{field}` field for a user. User {auth_user:?} is not an admin.",
        ))
    };
    if json_user.admin.is_some() && !auth_user.is_admin(&context.rbac) {
        return Err(admin_only_error("admin"));
    }
    if json_user.locked.is_some() && !auth_user.is_admin(&context.rbac) {
        return Err(admin_only_error("locked"));
    }

    let update_user = UpdateUser::from(json_user.clone());
    diesel::update(schema::user::table.filter(schema::user::id.eq(query_user.id)))
        .set(&update_user)
        .execute(conn_lock!(context))
        .map_err(resource_conflict_err!(User, (&query_user, &json_user)))?;

    Ok(QueryUser::get(conn_lock!(context), query_user.id)?.into_json())
}
