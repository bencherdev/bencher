use bencher_endpoint::{CorsResponse, Endpoint, Get, Patch, ResponseOk, TotalCount};
use bencher_json::{
    JsonDirection, JsonPagination, JsonUpdateUser, JsonUser, Sanitize as _, Search, UserName,
    UserResourceId, user::JsonUsers,
};
use bencher_schema::{
    conn_lock,
    context::ApiContext,
    error::{forbidden_error, resource_conflict_err, resource_not_found_err},
    model::user::{
        QueryUser, UpdateUser,
        admin::AdminUser,
        auth::{AuthUser, BearerToken},
        same_user,
    },
    schema,
};
use diesel::{
    BoolExpressionMethods as _, ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _,
    TextExpressionMethods as _,
};
use dropshot::{HttpError, Path, Query, RequestContext, TypedBody, endpoint};
use schemars::JsonSchema;
use serde::Deserialize;

pub type UsersPagination = JsonPagination<UsersSort>;

#[derive(Clone, Copy, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum UsersSort {
    /// Sort by user name.
    #[default]
    Name,
}

#[derive(Deserialize, JsonSchema)]
pub struct UsersQuery {
    /// Filter by user name, exact match.
    pub name: Option<UserName>,
    /// Search by user name, slug, or UUID.
    pub search: Option<Search>,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/users",
    tags = ["users"]
}]
pub async fn users_options(
    _rqctx: RequestContext<ApiContext>,
    _pagination_params: Query<UsersPagination>,
    _query_params: Query<UsersQuery>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into()]))
}

/// List users
///
/// List all users.
/// The user must be an admin on the server to use this route.
/// The HTTP response header `X-Total-Count` contains the total number of users.
#[endpoint {
    method = GET,
    path =  "/v0/users",
    tags = ["users"]
}]
pub async fn users_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    pagination_params: Query<UsersPagination>,
    query_params: Query<UsersQuery>,
) -> Result<ResponseOk<JsonUsers>, HttpError> {
    let _admin_user = AdminUser::from_token(rqctx.context(), bearer_token).await?;
    let (json, total_count) = get_ls_inner(
        rqctx.context(),
        pagination_params.into_inner(),
        query_params.into_inner(),
    )
    .await?;
    Ok(Get::auth_response_ok_with_total_count(json, total_count))
}

async fn get_ls_inner(
    context: &ApiContext,
    pagination_params: UsersPagination,
    query_params: UsersQuery,
) -> Result<(JsonUsers, TotalCount), HttpError> {
    let users = get_ls_query(&pagination_params, &query_params)
        .offset(pagination_params.offset())
        .limit(pagination_params.limit())
        .load::<QueryUser>(conn_lock!(context))
        .map_err(resource_not_found_err!(User))?;

    // Drop connection lock before iterating
    let json_users = users.into_iter().map(QueryUser::into_json).collect();

    let total_count = get_ls_query(&pagination_params, &query_params)
        .count()
        .get_result::<i64>(conn_lock!(context))
        .map_err(resource_not_found_err!(User))?
        .try_into()?;

    Ok((json_users, total_count))
}

fn get_ls_query<'q>(
    pagination_params: &UsersPagination,
    query_params: &'q UsersQuery,
) -> schema::user::BoxedQuery<'q, diesel::sqlite::Sqlite> {
    let mut query = schema::user::table.into_boxed();

    if let Some(name) = query_params.name.as_ref() {
        query = query.filter(schema::user::name.eq(name));
    }
    if let Some(search) = query_params.search.as_ref() {
        query = query.filter(
            schema::user::name
                .like(search)
                .or(schema::user::slug.like(search))
                .or(schema::user::uuid.like(search)),
        );
    }

    match pagination_params.order() {
        UsersSort::Name => match pagination_params.direction {
            Some(JsonDirection::Asc) | None => {
                query.order((schema::user::name.asc(), schema::user::slug.asc()))
            },
            Some(JsonDirection::Desc) => {
                query.order((schema::user::name.desc(), schema::user::slug.desc()))
            },
        },
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct UserParams {
    /// The slug or UUID for a user.
    pub user: UserResourceId,
}

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
        let mut auth_user = auth_user.clone();
        auth_user.sanitize();
        forbidden_error(format!(
            "Only admins can update the `{field}` field for a user. User is not an admin: {auth_user:?}",
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
