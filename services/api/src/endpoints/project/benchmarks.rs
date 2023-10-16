use bencher_json::{
    project::benchmark::{JsonNewBenchmark, JsonUpdateBenchmark},
    BenchmarkName, JsonBenchmark, JsonBenchmarks, JsonDirection, JsonEmpty, JsonPagination,
    ResourceId,
};
use bencher_rbac::project::Permission;
use diesel::{BelongingToDsl, ExpressionMethods, QueryDsl, RunQueryDsl};
use dropshot::{endpoint, HttpError, Path, Query, RequestContext, TypedBody};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::{
    context::ApiContext,
    endpoints::{
        endpoint::{CorsResponse, Delete, Get, Patch, Post, ResponseAccepted, ResponseOk},
        Endpoint,
    },
    error::{resource_conflict_err, resource_not_found_err},
    model::{
        project::{
            benchmark::{InsertBenchmark, QueryBenchmark, UpdateBenchmark},
            QueryProject,
        },
        user::auth::{AuthUser, BearerToken, PubBearerToken},
    },
    schema,
};

#[derive(Deserialize, JsonSchema)]
pub struct ProjBenchmarksParams {
    pub project: ResourceId,
}

pub type ProjBenchmarksPagination = JsonPagination<ProjBenchmarksSort>;

#[derive(Clone, Copy, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProjBenchmarksSort {
    #[default]
    Name,
}

#[derive(Deserialize, JsonSchema)]
pub struct ProjBenchmarksQuery {
    pub name: Option<BenchmarkName>,
}

#[allow(clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/benchmarks",
    tags = ["projects", "benchmarks"]
}]
pub async fn proj_benchmarks_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<ProjBenchmarksParams>,
    _pagination_params: Query<ProjBenchmarksPagination>,
    _query_params: Query<ProjBenchmarksQuery>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into(), Post.into()]))
}

#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/benchmarks",
    tags = ["projects", "benchmarks"]
}]
pub async fn proj_benchmarks_get(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<ProjBenchmarksParams>,
    pagination_params: Query<ProjBenchmarksPagination>,
    query_params: Query<ProjBenchmarksQuery>,
) -> Result<ResponseOk<JsonBenchmarks>, HttpError> {
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
    path_params: ProjBenchmarksParams,
    pagination_params: ProjBenchmarksPagination,
    query_params: ProjBenchmarksQuery,
) -> Result<JsonBenchmarks, HttpError> {
    let conn = &mut *context.conn().await;

    let query_project =
        QueryProject::is_allowed_public(conn, &context.rbac, &path_params.project, auth_user)?;

    let mut query = QueryBenchmark::belonging_to(&query_project).into_boxed();

    if let Some(name) = query_params.name.as_ref() {
        query = query.filter(schema::benchmark::name.eq(name.as_ref()));
    }

    query = match pagination_params.order() {
        ProjBenchmarksSort::Name => match pagination_params.direction {
            Some(JsonDirection::Asc) | None => query.order(schema::benchmark::name.asc()),
            Some(JsonDirection::Desc) => query.order(schema::benchmark::name.desc()),
        },
    };

    let project = &query_project;
    Ok(query
        .offset(pagination_params.offset())
        .limit(pagination_params.limit())
        .load::<QueryBenchmark>(conn)
        .map_err(resource_not_found_err!(Benchmark, project))?
        .into_iter()
        .map(|benchmark| benchmark.into_json_for_project(project))
        .collect())
}

#[endpoint {
    method = POST,
    path =  "/v0/projects/{project}/benchmarks",
    tags = ["projects", "benchmarks"]
}]
pub async fn proj_benchmark_post(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<ProjBenchmarksParams>,
    body: TypedBody<JsonNewBenchmark>,
) -> Result<ResponseAccepted<JsonBenchmark>, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    let json = post_inner(
        rqctx.context(),
        path_params.into_inner(),
        body.into_inner(),
        &auth_user,
    )
    .await?;
    Ok(Post::auth_response_accepted(json))
}

async fn post_inner(
    context: &ApiContext,
    path_params: ProjBenchmarksParams,
    json_benchmark: JsonNewBenchmark,
    auth_user: &AuthUser,
) -> Result<JsonBenchmark, HttpError> {
    let conn = &mut *context.conn().await;

    // Verify that the user is allowed
    let query_project = QueryProject::is_allowed(
        conn,
        &context.rbac,
        &path_params.project,
        auth_user,
        Permission::Create,
    )?;

    let insert_benchmark = InsertBenchmark::from_json(conn, query_project.id, json_benchmark)?;

    diesel::insert_into(schema::benchmark::table)
        .values(&insert_benchmark)
        .execute(conn)
        .map_err(resource_conflict_err!(Benchmark, insert_benchmark))?;

    schema::benchmark::table
        .filter(schema::benchmark::uuid.eq(&insert_benchmark.uuid))
        .first::<QueryBenchmark>(conn)
        .map(|benchmark| benchmark.into_json_for_project(&query_project))
        .map_err(resource_not_found_err!(Benchmark, insert_benchmark))
}

#[derive(Deserialize, JsonSchema)]
pub struct ProjBenchmarkParams {
    pub project: ResourceId,
    pub benchmark: ResourceId,
}

#[allow(clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/benchmarks/{benchmark}",
    tags = ["projects", "benchmarks"]
}]
pub async fn proj_benchmark_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<ProjBenchmarkParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into(), Patch.into(), Delete.into()]))
}

#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/benchmarks/{benchmark}",
    tags = ["projects", "benchmarks"]
}]
pub async fn proj_benchmark_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: PubBearerToken,
    path_params: Path<ProjBenchmarkParams>,
) -> Result<ResponseOk<JsonBenchmark>, HttpError> {
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
    path_params: ProjBenchmarkParams,
    auth_user: Option<&AuthUser>,
) -> Result<JsonBenchmark, HttpError> {
    let conn = &mut *context.conn().await;

    let query_project =
        QueryProject::is_allowed_public(conn, &context.rbac, &path_params.project, auth_user)?;

    QueryBenchmark::belonging_to(&query_project)
        .filter(crate::model::project::benchmark::resource_id(
            &path_params.benchmark,
        )?)
        .first::<QueryBenchmark>(conn)
        .map(|benchmark| benchmark.into_json_for_project(&query_project))
        .map_err(resource_not_found_err!(
            Benchmark,
            (query_project, path_params.benchmark)
        ))
}

#[endpoint {
    method = PATCH,
    path =  "/v0/projects/{project}/benchmarks/{benchmark}",
    tags = ["projects", "benchmarks"]
}]
pub async fn proj_benchmark_patch(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<ProjBenchmarkParams>,
    body: TypedBody<JsonUpdateBenchmark>,
) -> Result<ResponseAccepted<JsonBenchmark>, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    let json = patch_inner(
        rqctx.context(),
        path_params.into_inner(),
        body.into_inner(),
        &auth_user,
    )
    .await?;
    Ok(Patch::auth_response_accepted(json))
}

async fn patch_inner(
    context: &ApiContext,
    path_params: ProjBenchmarkParams,
    json_benchmark: JsonUpdateBenchmark,
    auth_user: &AuthUser,
) -> Result<JsonBenchmark, HttpError> {
    let conn = &mut *context.conn().await;

    // Verify that the user is allowed
    let query_project = QueryProject::is_allowed(
        conn,
        &context.rbac,
        &path_params.project,
        auth_user,
        Permission::Edit,
    )?;

    let query_benchmark =
        QueryBenchmark::from_resource_id(conn, query_project.id, &path_params.benchmark)?;
    diesel::update(schema::benchmark::table.filter(schema::benchmark::id.eq(query_benchmark.id)))
        .set(&UpdateBenchmark::from(json_benchmark.clone()))
        .execute(conn)
        .map_err(resource_conflict_err!(
            Benchmark,
            (query_benchmark.clone(), json_benchmark)
        ))?;

    QueryBenchmark::get(conn, query_benchmark.id)
        .map(|benchmark| benchmark.into_json_for_project(&query_project))
        .map_err(resource_not_found_err!(Benchmark, query_benchmark))
}

#[endpoint {
    method = DELETE,
    path =  "/v0/projects/{project}/benchmarks/{benchmark}",
    tags = ["projects", "benchmarks"]
}]
pub async fn proj_benchmark_delete(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<ProjBenchmarkParams>,
) -> Result<ResponseAccepted<JsonEmpty>, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    let json = delete_inner(rqctx.context(), path_params.into_inner(), &auth_user).await?;
    Ok(Delete::auth_response_accepted(json))
}

async fn delete_inner(
    context: &ApiContext,
    path_params: ProjBenchmarkParams,
    auth_user: &AuthUser,
) -> Result<JsonEmpty, HttpError> {
    let conn = &mut *context.conn().await;

    // Verify that the user is allowed
    let query_project = QueryProject::is_allowed(
        conn,
        &context.rbac,
        &path_params.project,
        auth_user,
        Permission::Delete,
    )?;

    let query_benchmark =
        QueryBenchmark::from_resource_id(conn, query_project.id, &path_params.benchmark)?;
    diesel::delete(schema::benchmark::table.filter(schema::benchmark::id.eq(query_benchmark.id)))
        .execute(conn)
        .map_err(resource_conflict_err!(Benchmark, query_benchmark))?;

    Ok(JsonEmpty {})
}
