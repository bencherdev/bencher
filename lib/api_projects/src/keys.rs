use bencher_endpoint::{
    CorsResponse, Delete, Endpoint, Get, Patch, Post, ResponseCreated, ResponseDeleted, ResponseOk,
    TotalCount,
};
use bencher_json::{
    JsonDirection, JsonNewProjectKey, JsonPagination, JsonProjectKey, JsonProjectKeyCreated,
    JsonProjectKeys, ProjectKeyUuid, ProjectResourceId, ResourceName, Search,
    project::key::JsonUpdateProjectKey,
};
use bencher_rbac::project::Permission;
use bencher_schema::{
    auth_conn,
    context::ApiContext,
    error::{conflict_error, resource_conflict_err, resource_not_found_err},
    model::{
        project::{
            ProjectId, QueryProject,
            key::{InsertProjectKey, QueryProjectKey, UpdateProjectKey},
        },
        user::{
            QueryUser,
            auth::{AuthUser, BearerToken},
        },
    },
    schema, write_conn, write_transaction,
};
use diesel::{
    BoolExpressionMethods as _, ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _,
    TextExpressionMethods as _,
};
use dropshot::{HttpError, Path, Query, RequestContext, TypedBody, endpoint};
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Deserialize, JsonSchema)]
pub struct ProjKeysParams {
    /// The slug or UUID for a project.
    pub project: ProjectResourceId,
}

pub type ProjKeysPagination = JsonPagination<ProjKeysSort>;

#[derive(Debug, Clone, Copy, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProjKeysSort {
    /// Sort by key name.
    #[default]
    Name,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ProjKeysQuery {
    /// Filter by key name, exact match.
    pub name: Option<ResourceName>,
    /// Search by key name or UUID.
    pub search: Option<Search>,
    /// If set to `true`, only returns revoked keys.
    /// If not set or set to `false`, only returns non-revoked keys.
    pub revoked: Option<bool>,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/keys",
    tags = ["projects", "keys"]
}]
pub async fn proj_keys_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<ProjKeysParams>,
    _pagination_params: Query<ProjKeysPagination>,
    _query_params: Query<ProjKeysQuery>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into(), Post.into()]))
}

/// List project keys
///
/// List all API keys for a project.
/// Requires `manage` permission on the project.
/// By default, the keys are sorted in alphabetical order by name.
/// The HTTP response header `X-Total-Count` contains the total number of keys.
#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/keys",
    tags = ["projects", "keys"]
}]
pub async fn proj_keys_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<ProjKeysParams>,
    pagination_params: Query<ProjKeysPagination>,
    query_params: Query<ProjKeysQuery>,
) -> Result<ResponseOk<JsonProjectKeys>, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    let (json, total_count) = get_ls_inner(
        rqctx.context(),
        path_params.into_inner(),
        pagination_params.into_inner(),
        query_params.into_inner(),
        &auth_user,
    )
    .await?;
    Ok(Get::auth_response_ok_with_total_count(json, total_count))
}

async fn get_ls_inner(
    context: &ApiContext,
    path_params: ProjKeysParams,
    pagination_params: ProjKeysPagination,
    query_params: ProjKeysQuery,
    auth_user: &AuthUser,
) -> Result<(JsonProjectKeys, TotalCount), HttpError> {
    let query_project = QueryProject::is_allowed(
        auth_conn!(context),
        &context.rbac,
        &path_params.project,
        auth_user,
        Permission::Manage,
    )?;

    #[cfg(feature = "plus")]
    context.rate_limiting.project_request(query_project.uuid)?;

    let keys = get_ls_query(&pagination_params, &query_params, query_project.id)
        .offset(pagination_params.offset())
        .limit(pagination_params.limit())
        .load::<QueryProjectKey>(auth_conn!(context))
        .map_err(resource_not_found_err!(
            ProjectKey,
            (&pagination_params, &query_params, auth_user)
        ))?;

    let json_keys: JsonProjectKeys = auth_conn!(context, |conn| {
        keys.into_iter()
            .map(|query_key| {
                let creator_uuid = query_key
                    .creator_id
                    .map(|id| QueryUser::get(conn, id).map(|u| u.uuid))
                    .transpose()?;
                Ok(query_key.into_json_for_project(&query_project, creator_uuid))
            })
            .collect::<Result<_, HttpError>>()?
    });

    let total_count = get_ls_query(&pagination_params, &query_params, query_project.id)
        .count()
        .get_result::<i64>(auth_conn!(context))
        .map_err(resource_not_found_err!(
            ProjectKey,
            (&pagination_params, &query_params, auth_user)
        ))?
        .try_into()?;

    Ok((json_keys, total_count))
}

fn get_ls_query<'q>(
    pagination_params: &ProjKeysPagination,
    query_params: &'q ProjKeysQuery,
    project_id: ProjectId,
) -> schema::project_key::BoxedQuery<'q, diesel::sqlite::Sqlite> {
    let mut query = schema::project_key::table
        .filter(schema::project_key::project_id.eq(project_id))
        .into_boxed();

    if let Some(true) = query_params.revoked {
        query = query.filter(schema::project_key::revoked.is_not_null());
    } else {
        query = query.filter(schema::project_key::revoked.is_null());
    }

    if let Some(name) = query_params.name.as_ref() {
        query = query.filter(schema::project_key::name.eq(name));
    }
    if let Some(search) = query_params.search.as_ref() {
        query = query.filter(
            schema::project_key::name
                .like(search)
                .or(schema::project_key::uuid.like(search)),
        );
    }

    match pagination_params.order() {
        ProjKeysSort::Name => match pagination_params.direction {
            Some(JsonDirection::Asc) | None => query.order((
                schema::project_key::name.asc(),
                schema::project_key::creation.asc(),
            )),
            Some(JsonDirection::Desc) => query.order((
                schema::project_key::name.desc(),
                schema::project_key::creation.desc(),
            )),
        },
    }
}

/// Create a project key
///
/// Create a new API key for a project.
/// The plaintext key is only returned once in the response.
/// Requires `manage` permission on the project.
#[endpoint {
    method = POST,
    path =  "/v0/projects/{project}/keys",
    tags = ["projects", "keys"]
}]
pub async fn proj_key_post(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<ProjKeysParams>,
    body: TypedBody<JsonNewProjectKey>,
) -> Result<ResponseCreated<JsonProjectKeyCreated>, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    let json = post_inner(
        rqctx.context(),
        path_params.into_inner(),
        body.into_inner(),
        &auth_user,
    )
    .await?;
    Ok(Post::auth_response_created(json))
}

async fn post_inner(
    context: &ApiContext,
    path_params: ProjKeysParams,
    json_key: JsonNewProjectKey,
    auth_user: &AuthUser,
) -> Result<JsonProjectKeyCreated, HttpError> {
    let query_project = QueryProject::is_allowed(
        auth_conn!(context),
        &context.rbac,
        &path_params.project,
        auth_user,
        Permission::Manage,
    )?;

    #[cfg(feature = "plus")]
    context.rate_limiting.project_request(query_project.uuid)?;
    #[cfg(feature = "plus")]
    context
        .rate_limiting
        .create_credential(auth_user.user.uuid)?;

    let now = context.clock.now();
    let (insert_key, plaintext_key) =
        InsertProjectKey::from_json(query_project.id, auth_user.user.id, json_key, now)?;

    diesel::insert_into(schema::project_key::table)
        .values(&insert_key)
        .execute(write_conn!(context))
        .map_err(resource_conflict_err!(ProjectKey, insert_key))?;

    Ok(insert_key.into_json(query_project.uuid, plaintext_key))
}

#[derive(Deserialize, JsonSchema)]
pub struct ProjKeyParams {
    /// The slug or UUID for a project.
    pub project: ProjectResourceId,
    /// The UUID for a project key.
    pub key: ProjectKeyUuid,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/keys/{key}",
    tags = ["projects", "keys"]
}]
pub async fn proj_key_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<ProjKeyParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into(), Patch.into(), Delete.into()]))
}

/// View a project key
///
/// View an API key for a project.
/// Requires `manage` permission on the project.
#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/keys/{key}",
    tags = ["projects", "keys"]
}]
pub async fn proj_key_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<ProjKeyParams>,
) -> Result<ResponseOk<JsonProjectKey>, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    let json = get_one_inner(rqctx.context(), path_params.into_inner(), &auth_user).await?;
    Ok(Get::auth_response_ok(json))
}

async fn get_one_inner(
    context: &ApiContext,
    path_params: ProjKeyParams,
    auth_user: &AuthUser,
) -> Result<JsonProjectKey, HttpError> {
    let query_project = QueryProject::is_allowed(
        auth_conn!(context),
        &context.rbac,
        &path_params.project,
        auth_user,
        Permission::Manage,
    )?;

    #[cfg(feature = "plus")]
    context.rate_limiting.project_request(query_project.uuid)?;

    auth_conn!(context, |conn| {
        QueryProjectKey::get_project_key(conn, query_project.id, path_params.key)?.into_json(conn)
    })
}

/// Update a project key
///
/// Update an API key for a project (rename only).
/// Requires `manage` permission on the project.
#[endpoint {
    method = PATCH,
    path =  "/v0/projects/{project}/keys/{key}",
    tags = ["projects", "keys"]
}]
pub async fn proj_key_patch(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<ProjKeyParams>,
    body: TypedBody<JsonUpdateProjectKey>,
) -> Result<ResponseOk<JsonProjectKey>, HttpError> {
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
    path_params: ProjKeyParams,
    json_key: JsonUpdateProjectKey,
    auth_user: &AuthUser,
) -> Result<JsonProjectKey, HttpError> {
    let query_project = QueryProject::is_allowed(
        auth_conn!(context),
        &context.rbac,
        &path_params.project,
        auth_user,
        Permission::Manage,
    )?;

    #[cfg(feature = "plus")]
    context.rate_limiting.project_request(query_project.uuid)?;

    let query_key =
        QueryProjectKey::get_project_key(auth_conn!(context), query_project.id, path_params.key)?;

    let update_key = UpdateProjectKey::from(json_key);
    write_transaction!(context, |conn| {
        diesel::update(schema::project_key::table.filter(schema::project_key::id.eq(query_key.id)))
            .set(&update_key)
            .execute(conn)
    })
    .map_err(resource_conflict_err!(
        ProjectKey,
        (&query_project, &query_key)
    ))?;

    auth_conn!(context, |conn| {
        QueryProjectKey::get(conn, query_key.id)?.into_json(conn)
    })
}

/// Revoke a project key
///
/// Revoke an API key for a project.
/// Revocation is terminal: a revoked key can no longer authenticate any request,
/// and the revocation cannot be undone.
/// Requires `manage` permission on the project.
#[endpoint {
    method = DELETE,
    path =  "/v0/projects/{project}/keys/{key}",
    tags = ["projects", "keys"]
}]
pub async fn proj_key_delete(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<ProjKeyParams>,
) -> Result<ResponseDeleted, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    delete_inner(rqctx.context(), path_params.into_inner(), &auth_user).await?;
    Ok(Delete::auth_response_deleted())
}

async fn delete_inner(
    context: &ApiContext,
    path_params: ProjKeyParams,
    auth_user: &AuthUser,
) -> Result<(), HttpError> {
    let query_project = QueryProject::is_allowed(
        auth_conn!(context),
        &context.rbac,
        &path_params.project,
        auth_user,
        Permission::Manage,
    )?;

    #[cfg(feature = "plus")]
    context.rate_limiting.project_request(query_project.uuid)?;

    let query_key =
        QueryProjectKey::get_project_key(auth_conn!(context), query_project.id, path_params.key)?;

    let now = context.clock.now();
    let rows = write_transaction!(context, |conn| QueryProjectKey::revoke(
        conn,
        query_key.id,
        now
    ))
    .map_err(resource_conflict_err!(
        ProjectKey,
        (&query_project, &query_key)
    ))?;
    if rows == 0 {
        return Err(conflict_error("Project key has already been revoked"));
    }

    Ok(())
}
