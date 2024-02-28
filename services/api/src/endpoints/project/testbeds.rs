use bencher_json::{
    project::testbed::JsonUpdateTestbed, JsonDirection, JsonNewTestbed, JsonPagination,
    JsonTestbed, JsonTestbeds, ResourceId, ResourceName,
};
use bencher_rbac::project::Permission;
use diesel::{
    BelongingToDsl, BoolExpressionMethods, ExpressionMethods, QueryDsl, RunQueryDsl,
    TextExpressionMethods,
};
use dropshot::{endpoint, HttpError, Path, Query, RequestContext, TypedBody};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::{
    conn_lock,
    context::ApiContext,
    endpoints::{
        endpoint::{
            CorsResponse, Delete, Get, Patch, Post, ResponseCreated, ResponseDeleted, ResponseOk,
        },
        Endpoint,
    },
    error::{resource_conflict_err, resource_not_found_err},
    model::user::auth::{AuthUser, PubBearerToken},
    model::{
        project::{
            testbed::{InsertTestbed, QueryTestbed, UpdateTestbed},
            QueryProject,
        },
        user::auth::BearerToken,
    },
    schema,
    util::search::Search,
};

#[derive(Deserialize, JsonSchema)]
pub struct ProjTestbedsParams {
    /// The slug or UUID for a project.
    pub project: ResourceId,
}

pub type ProjTestbedsPagination = JsonPagination<ProjTestbedsSort>;

#[derive(Clone, Copy, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProjTestbedsSort {
    #[default]
    Name,
}

#[derive(Deserialize, JsonSchema)]
pub struct ProjTestbedsQuery {
    pub name: Option<ResourceName>,
    pub search: Option<Search>,
}

#[allow(clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/testbeds",
    tags = ["projects", "testbeds"]
}]
pub async fn proj_testbeds_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<ProjTestbedsParams>,
    _pagination_params: Query<ProjTestbedsPagination>,
    _query_params: Query<ProjTestbedsQuery>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into(), Post.into()]))
}

#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/testbeds",
    tags = ["projects", "testbeds"]
}]
pub async fn proj_testbeds_get(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<ProjTestbedsParams>,
    pagination_params: Query<ProjTestbedsPagination>,
    query_params: Query<ProjTestbedsQuery>,
) -> Result<ResponseOk<JsonTestbeds>, HttpError> {
    let auth_user = AuthUser::new_pub(&rqctx).await?;
    let json = get_ls_inner(
        rqctx.context(),
        auth_user.as_ref(),
        path_params.into_inner(),
        pagination_params.into_inner(),
        query_params.into_inner(),
    )
    .await?;
    Ok(Get::response_ok(json, auth_user.is_some()))
}

async fn get_ls_inner(
    context: &ApiContext,
    auth_user: Option<&AuthUser>,
    path_params: ProjTestbedsParams,
    pagination_params: ProjTestbedsPagination,
    query_params: ProjTestbedsQuery,
) -> Result<JsonTestbeds, HttpError> {
    let query_project = QueryProject::is_allowed_public(
        conn_lock!(context),
        &context.rbac,
        &path_params.project,
        auth_user,
    )?;

    let mut query = QueryTestbed::belonging_to(&query_project).into_boxed();

    if let Some(name) = query_params.name.as_ref() {
        query = query.filter(schema::testbed::name.eq(name));
    }
    if let Some(search) = query_params.search.as_ref() {
        query = query.filter(
            schema::testbed::name
                .like(search)
                .or(schema::testbed::slug.like(search))
                .or(schema::testbed::uuid.like(search)),
        );
    }

    query = match pagination_params.order() {
        ProjTestbedsSort::Name => match pagination_params.direction {
            Some(JsonDirection::Asc) | None => query.order(schema::testbed::name.asc()),
            Some(JsonDirection::Desc) => query.order(schema::testbed::name.desc()),
        },
    };

    Ok(query
        .offset(pagination_params.offset())
        .limit(pagination_params.limit())
        .load::<QueryTestbed>(conn_lock!(context))
        .map_err(resource_not_found_err!(Testbed, &query_project))?
        .into_iter()
        .map(|testbed| testbed.into_json_for_project(&query_project))
        .collect())
}

#[endpoint {
    method = POST,
    path =  "/v0/projects/{project}/testbeds",
    tags = ["projects", "testbeds"]
}]
pub async fn proj_testbed_post(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<ProjTestbedsParams>,
    body: TypedBody<JsonNewTestbed>,
) -> Result<ResponseCreated<JsonTestbed>, HttpError> {
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
    path_params: ProjTestbedsParams,
    json_testbed: JsonNewTestbed,
    auth_user: &AuthUser,
) -> Result<JsonTestbed, HttpError> {
    // Verify that the user is allowed
    let query_project = QueryProject::is_allowed(
        conn_lock!(context),
        &context.rbac,
        &path_params.project,
        auth_user,
        Permission::Create,
    )?;

    // Soft creation
    // If the new testbed name already exists then return the existing testbed
    // instead of erroring due to the unique constraint
    // This is useful to help prevent race conditions in CI
    if let Some(true) = json_testbed.soft {
        if let Ok(testbed) = QueryTestbed::belonging_to(&query_project)
            .filter(schema::testbed::name.eq(json_testbed.name.as_ref()))
            .first::<QueryTestbed>(conn_lock!(context))
        {
            return Ok(testbed.into_json_for_project(&query_project));
        }
    }
    let insert_testbed =
        InsertTestbed::from_json(conn_lock!(context), query_project.id, json_testbed)?;

    diesel::insert_into(schema::testbed::table)
        .values(&insert_testbed)
        .execute(conn_lock!(context))
        .map_err(resource_conflict_err!(Testbed, insert_testbed))?;

    schema::testbed::table
        .filter(schema::testbed::uuid.eq(&insert_testbed.uuid))
        .first::<QueryTestbed>(conn_lock!(context))
        .map(|testbed| testbed.into_json_for_project(&query_project))
        .map_err(resource_not_found_err!(Testbed, insert_testbed))
}

#[derive(Deserialize, JsonSchema)]
pub struct ProjTestbedParams {
    /// The slug or UUID for a project.
    pub project: ResourceId,
    /// The slug or UUID for a testbed.
    pub testbed: ResourceId,
}

#[allow(clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/testbeds/{testbed}",
    tags = ["projects", "testbeds"]
}]
pub async fn proj_testbed_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<ProjTestbedParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into(), Patch.into(), Delete.into()]))
}

#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/testbeds/{testbed}",
    tags = ["projects", "testbeds"]
}]
pub async fn proj_testbed_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: PubBearerToken,
    path_params: Path<ProjTestbedParams>,
) -> Result<ResponseOk<JsonTestbed>, HttpError> {
    let auth_user = AuthUser::from_pub_token(rqctx.context(), bearer_token).await?;
    let json = get_one_inner(
        rqctx.context(),
        path_params.into_inner(),
        auth_user.as_ref(),
    )
    .await?;
    Ok(Get::response_ok(json, auth_user.is_some()))
}

async fn get_one_inner(
    context: &ApiContext,
    path_params: ProjTestbedParams,
    auth_user: Option<&AuthUser>,
) -> Result<JsonTestbed, HttpError> {
    let query_project = QueryProject::is_allowed_public(
        conn_lock!(context),
        &context.rbac,
        &path_params.project,
        auth_user,
    )?;

    QueryTestbed::belonging_to(&query_project)
        .filter(QueryTestbed::eq_resource_id(&path_params.testbed)?)
        .first::<QueryTestbed>(conn_lock!(context))
        .map(|testbed| testbed.into_json_for_project(&query_project))
        .map_err(resource_not_found_err!(
            Testbed,
            (&query_project, path_params.testbed)
        ))
}

#[endpoint {
    method = PATCH,
    path =  "/v0/projects/{project}/testbeds/{testbed}",
    tags = ["projects", "testbeds"]
}]
pub async fn proj_testbed_patch(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<ProjTestbedParams>,
    body: TypedBody<JsonUpdateTestbed>,
) -> Result<ResponseOk<JsonTestbed>, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    let context = rqctx.context();
    let json = patch_inner(
        context,
        path_params.into_inner(),
        body.into_inner(),
        &auth_user,
    )
    .await?;
    Ok(Patch::auth_response_ok(json))
}

async fn patch_inner(
    context: &ApiContext,
    path_params: ProjTestbedParams,
    json_testbed: JsonUpdateTestbed,
    auth_user: &AuthUser,
) -> Result<JsonTestbed, HttpError> {
    // Verify that the user is allowed
    let query_project = QueryProject::is_allowed(
        conn_lock!(context),
        &context.rbac,
        &path_params.project,
        auth_user,
        Permission::Edit,
    )?;

    let query_testbed = QueryTestbed::from_resource_id(
        conn_lock!(context),
        query_project.id,
        &path_params.testbed,
    )?;
    let update_testbed = UpdateTestbed::from(json_testbed.clone());
    diesel::update(schema::testbed::table.filter(schema::testbed::id.eq(query_testbed.id)))
        .set(&update_testbed)
        .execute(conn_lock!(context))
        .map_err(resource_conflict_err!(
            Testbed,
            (&query_testbed, &json_testbed)
        ))?;

    QueryTestbed::get(conn_lock!(context), query_testbed.id)
        .map(|testbed| testbed.into_json_for_project(&query_project))
        .map_err(resource_not_found_err!(Testbed, query_testbed))
}

#[endpoint {
    method = DELETE,
    path =  "/v0/projects/{project}/testbeds/{testbed}",
    tags = ["projects", "testbeds"]
}]
pub async fn proj_testbed_delete(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<ProjTestbedParams>,
) -> Result<ResponseDeleted, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    delete_inner(rqctx.context(), path_params.into_inner(), &auth_user).await?;
    Ok(Delete::auth_response_deleted())
}

async fn delete_inner(
    context: &ApiContext,
    path_params: ProjTestbedParams,
    auth_user: &AuthUser,
) -> Result<(), HttpError> {
    // Verify that the user is allowed
    let query_project = QueryProject::is_allowed(
        conn_lock!(context),
        &context.rbac,
        &path_params.project,
        auth_user,
        Permission::Delete,
    )?;

    let query_testbed = QueryTestbed::from_resource_id(
        conn_lock!(context),
        query_project.id,
        &path_params.testbed,
    )?;

    diesel::delete(schema::testbed::table.filter(schema::testbed::id.eq(query_testbed.id)))
        .execute(conn_lock!(context))
        .map_err(resource_conflict_err!(Testbed, query_testbed))?;

    Ok(())
}
