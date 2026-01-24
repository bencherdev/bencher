use bencher_endpoint::{
    CorsResponse, Delete, Endpoint, Get, Patch, Post, ResponseCreated, ResponseDeleted, ResponseOk,
    TotalCount,
};
use bencher_json::{
    BenchmarkName, BenchmarkResourceId, JsonBenchmark, JsonBenchmarks, JsonDirection,
    JsonPagination, ProjectResourceId, Search,
    project::benchmark::{JsonNewBenchmark, JsonUpdateBenchmark},
};
use bencher_rbac::project::Permission;
use bencher_schema::{
    auth_conn,
    context::ApiContext,
    error::{resource_conflict_err, resource_not_found_err},
    model::{
        project::{
            QueryProject,
            benchmark::{QueryBenchmark, UpdateBenchmark},
        },
        user::{
            auth::{AuthUser, BearerToken},
            public::{PubBearerToken, PublicUser},
        },
    },
    public_conn, schema,
};
use diesel::{
    BelongingToDsl as _, BoolExpressionMethods as _, ExpressionMethods as _, QueryDsl as _,
    RunQueryDsl as _, TextExpressionMethods as _,
};
use dropshot::{HttpError, Path, Query, RequestContext, TypedBody, endpoint};
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Deserialize, JsonSchema)]
pub struct ProjBenchmarksParams {
    /// The slug or UUID for a project.
    pub project: ProjectResourceId,
}

pub type ProjBenchmarksPagination = JsonPagination<ProjBenchmarksSort>;

#[derive(Debug, Clone, Copy, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProjBenchmarksSort {
    /// Sort by benchmark name.
    #[default]
    Name,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ProjBenchmarksQuery {
    /// Filter by benchmark name, exact match.
    pub name: Option<BenchmarkName>,
    /// Search by benchmark name, slug, or UUID.
    pub search: Option<Search>,
    /// If set to `true`, only returns archived benchmarks.
    /// If not set or set to `false`, only returns non-archived benchmarks.
    pub archived: Option<bool>,
}

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

/// List benchmarks for a project
///
/// List all benchmarks for a project.
/// If the project is public, then the user does not need to be authenticated.
/// If the project is private, then the user must be authenticated and have `view` permissions for the project.
/// By default, the benchmarks are sorted in alphabetical order by name.
/// The HTTP response header `X-Total-Count` contains the total number of benchmarks.
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
    let public_user = PublicUser::new(&rqctx).await?;
    let (json, total_count) = get_ls_inner(
        rqctx.context(),
        &public_user,
        path_params.into_inner(),
        pagination_params.into_inner(),
        query_params.into_inner(),
    )
    .await?;
    Ok(Get::response_ok_with_total_count(
        json,
        public_user.is_auth(),
        total_count,
    ))
}

async fn get_ls_inner(
    context: &ApiContext,
    public_user: &PublicUser,
    path_params: ProjBenchmarksParams,
    pagination_params: ProjBenchmarksPagination,
    query_params: ProjBenchmarksQuery,
) -> Result<(JsonBenchmarks, TotalCount), HttpError> {
    let query_project = QueryProject::is_allowed_public(
        public_conn!(context, public_user),
        &context.rbac,
        &path_params.project,
        public_user,
    )?;

    let benchmarks = get_ls_query(&query_project, &pagination_params, &query_params)
        .offset(pagination_params.offset())
        .limit(pagination_params.limit())
        .load::<QueryBenchmark>(public_conn!(context, public_user))
        .map_err(resource_not_found_err!(
            Benchmark,
            (&query_project, &pagination_params, &query_params)
        ))?;

    // Drop connection lock before iterating
    let json_benchmarks = benchmarks
        .into_iter()
        .map(|benchmark| benchmark.into_json_for_project(&query_project))
        .collect();

    let total_count = get_ls_query(&query_project, &pagination_params, &query_params)
        .count()
        .get_result::<i64>(public_conn!(context, public_user))
        .map_err(resource_not_found_err!(
            Plot,
            (&query_project, &pagination_params, &query_params)
        ))?
        .try_into()?;

    Ok((json_benchmarks, total_count))
}

fn get_ls_query<'q>(
    query_project: &'q QueryProject,
    pagination_params: &ProjBenchmarksPagination,
    query_params: &'q ProjBenchmarksQuery,
) -> schema::benchmark::BoxedQuery<'q, diesel::sqlite::Sqlite> {
    let mut query = QueryBenchmark::belonging_to(&query_project).into_boxed();

    if let Some(name) = query_params.name.as_ref() {
        query = query.filter(schema::benchmark::name.eq(name));
    }
    if let Some(search) = query_params.search.as_ref() {
        query = query.filter(
            schema::benchmark::name
                .like(search)
                .or(schema::benchmark::slug.like(search))
                .or(schema::benchmark::uuid.like(search)),
        );
    }

    if let Some(true) = query_params.archived {
        query = query.filter(schema::benchmark::archived.is_not_null());
    } else {
        query = query.filter(schema::benchmark::archived.is_null());
    }

    match pagination_params.order() {
        ProjBenchmarksSort::Name => match pagination_params.direction {
            Some(JsonDirection::Asc) | None => query.order(schema::benchmark::name.asc()),
            Some(JsonDirection::Desc) => query.order(schema::benchmark::name.desc()),
        },
    }
}

/// Create a benchmark
///
/// Create a benchmark for a project.
/// The user must have `create` permissions for the project.
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
) -> Result<ResponseCreated<JsonBenchmark>, HttpError> {
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
    path_params: ProjBenchmarksParams,
    json_benchmark: JsonNewBenchmark,
    auth_user: &AuthUser,
) -> Result<JsonBenchmark, HttpError> {
    // Verify that the user is allowed
    let query_project = QueryProject::is_allowed(
        auth_conn!(context),
        &context.rbac,
        &path_params.project,
        auth_user,
        Permission::Create,
    )?;

    QueryBenchmark::create(context, query_project.id, json_benchmark)
        .await
        .map(|benchmark| benchmark.into_json_for_project(&query_project))
}

#[derive(Deserialize, JsonSchema)]
pub struct ProjBenchmarkParams {
    /// The slug or UUID for a project.
    pub project: ProjectResourceId,
    /// The slug or UUID for a benchmark.
    pub benchmark: BenchmarkResourceId,
}

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

/// View a benchmark
///
/// View a benchmark for a project.
/// If the project is public, then the user does not need to be authenticated.
/// If the project is private, then the user must be authenticated and have `view` permissions for the project.
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
    let public_user = PublicUser::from_token(
        &rqctx.log,
        rqctx.context(),
        #[cfg(feature = "plus")]
        rqctx.request.headers(),
        bearer_token,
    )
    .await?;
    let json = get_one_inner(rqctx.context(), path_params.into_inner(), &public_user).await?;
    Ok(Get::response_ok(json, public_user.is_auth()))
}

async fn get_one_inner(
    context: &ApiContext,
    path_params: ProjBenchmarkParams,
    public_user: &PublicUser,
) -> Result<JsonBenchmark, HttpError> {
    let query_project = QueryProject::is_allowed_public(
        public_conn!(context, public_user),
        &context.rbac,
        &path_params.project,
        public_user,
    )?;

    QueryBenchmark::belonging_to(&query_project)
        .filter(QueryBenchmark::eq_resource_id(&path_params.benchmark))
        .first::<QueryBenchmark>(public_conn!(context, public_user))
        .map(|benchmark| benchmark.into_json_for_project(&query_project))
        .map_err(resource_not_found_err!(
            Benchmark,
            (&query_project, path_params.benchmark)
        ))
}

/// Update a benchmark
///
/// Update a benchmark for a project.
/// The user must have `edit` permissions for the project.
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
) -> Result<ResponseOk<JsonBenchmark>, HttpError> {
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
    path_params: ProjBenchmarkParams,
    json_benchmark: JsonUpdateBenchmark,
    auth_user: &AuthUser,
) -> Result<JsonBenchmark, HttpError> {
    // Verify that the user is allowed
    let query_project = QueryProject::is_allowed(
        auth_conn!(context),
        &context.rbac,
        &path_params.project,
        auth_user,
        Permission::Edit,
    )?;

    let query_benchmark = QueryBenchmark::from_resource_id(
        auth_conn!(context),
        query_project.id,
        &path_params.benchmark,
    )?;
    let update_benchmark = UpdateBenchmark::from(json_benchmark.clone());
    diesel::update(schema::benchmark::table.filter(schema::benchmark::id.eq(query_benchmark.id)))
        .set(&update_benchmark)
        .execute(auth_conn!(context))
        .map_err(resource_conflict_err!(
            Benchmark,
            (&query_benchmark, &json_benchmark)
        ))?;

    QueryBenchmark::get(auth_conn!(context), query_benchmark.id)
        .map(|benchmark| benchmark.into_json_for_project(&query_project))
        .map_err(resource_not_found_err!(Benchmark, query_benchmark))
}

/// Delete a benchmark
///
/// Delete a benchmark for a project.
/// The user must have `delete` permissions for the project.
/// All reports that use this benchmark must be deleted first!
#[endpoint {
    method = DELETE,
    path =  "/v0/projects/{project}/benchmarks/{benchmark}",
    tags = ["projects", "benchmarks"]
}]
pub async fn proj_benchmark_delete(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<ProjBenchmarkParams>,
) -> Result<ResponseDeleted, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    delete_inner(rqctx.context(), path_params.into_inner(), &auth_user).await?;
    Ok(Delete::auth_response_deleted())
}

async fn delete_inner(
    context: &ApiContext,
    path_params: ProjBenchmarkParams,
    auth_user: &AuthUser,
) -> Result<(), HttpError> {
    // Verify that the user is allowed
    let query_project = QueryProject::is_allowed(
        auth_conn!(context),
        &context.rbac,
        &path_params.project,
        auth_user,
        Permission::Delete,
    )?;

    let query_benchmark = QueryBenchmark::from_resource_id(
        auth_conn!(context),
        query_project.id,
        &path_params.benchmark,
    )?;
    diesel::delete(schema::benchmark::table.filter(schema::benchmark::id.eq(query_benchmark.id)))
        .execute(auth_conn!(context))
        .map_err(resource_conflict_err!(Benchmark, query_benchmark))?;

    Ok(())
}
