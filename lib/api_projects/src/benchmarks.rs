use std::cell::RefCell;

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
            benchmark::{
                QueryBenchmark, UpdateBenchmark, aliases_by_benchmark_id,
                list_aliases_for_benchmark, replace_benchmark_aliases,
                validate_benchmark_aliases_uniqueness,
            },
        },
        user::{
            auth::{AuthUser, BearerToken},
            public::{PubBearerToken, PublicUser},
        },
    },
    public_conn, schema, write_conn,
};
use diesel::{
    BelongingToDsl as _, BoolExpressionMethods as _, Connection as _, ExpressionMethods as _,
    QueryDsl as _, RunQueryDsl as _, TextExpressionMethods as _, dsl::exists,
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

    let ids: Vec<_> = benchmarks.iter().map(|b| b.id).collect();
    let alias_map =
        aliases_by_benchmark_id(public_conn!(context, public_user), query_project.id, &ids)
            .map_err(resource_not_found_err!(
                Benchmark,
                (&query_project, &pagination_params, &query_params)
            ))?;

    // Drop connection lock before iterating
    let json_benchmarks = benchmarks
        .into_iter()
        .map(|benchmark| {
            let aliases = alias_map.get(&benchmark.id).cloned().unwrap_or_default();
            benchmark.into_json_for_project_with_aliases(&query_project, aliases)
        })
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
        let alias_match = exists(
            schema::benchmark_alias::table
                .filter(schema::benchmark_alias::benchmark_id.eq(schema::benchmark::id))
                .filter(schema::benchmark_alias::project_id.eq(query_project.id))
                .filter(schema::benchmark_alias::alias.eq(name.as_ref())),
        );
        query = query.filter(schema::benchmark::name.eq(name).or(alias_match));
    }
    if let Some(search) = query_params.search.as_ref() {
        let alias_search = exists(
            schema::benchmark_alias::table
                .filter(schema::benchmark_alias::benchmark_id.eq(schema::benchmark::id))
                .filter(schema::benchmark_alias::project_id.eq(query_project.id))
                .filter(schema::benchmark_alias::alias.like(search)),
        );
        query = query.filter(
            schema::benchmark::name
                .like(search)
                .or(schema::benchmark::slug.like(search))
                .or(schema::benchmark::uuid.like(search))
                .or(alias_search),
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

    let benchmark = QueryBenchmark::create(context, query_project.id, json_benchmark).await?;
    benchmark.into_json_for_project(auth_conn!(context), &query_project)
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

    let benchmark_param = path_params.benchmark.clone();
    let benchmark = QueryBenchmark::belonging_to(&query_project)
        .filter(QueryBenchmark::eq_resource_id(&path_params.benchmark))
        .first::<QueryBenchmark>(public_conn!(context, public_user))
        .map_err(resource_not_found_err!(
            Benchmark,
            (&query_project, benchmark_param.clone())
        ))?;
    benchmark.into_json_for_project(public_conn!(context, public_user), &query_project)
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

    let effective_name = json_benchmark
        .name
        .clone()
        .unwrap_or_else(|| query_benchmark.name.clone());

    let update_benchmark = UpdateBenchmark::from(json_benchmark.clone());
    let conn = write_conn!(context);
    let validation_err: RefCell<Option<HttpError>> = RefCell::new(None);
    let txn_result = conn.transaction(|conn| -> diesel::QueryResult<()> {
        if let Some(list) = json_benchmark.aliases.as_ref() {
            validate_benchmark_aliases_uniqueness(
                conn,
                query_project.id,
                Some(query_benchmark.id),
                &effective_name,
                list,
            )
            .map_err(|e| {
                *validation_err.borrow_mut() = Some(e);
                diesel::result::Error::RollbackTransaction
            })?;
        } else if json_benchmark.name.is_some() {
            let current = list_aliases_for_benchmark(conn, query_benchmark.id).map_err(|e| {
                *validation_err.borrow_mut() =
                    Some(resource_not_found_err!(Benchmark, &query_benchmark)(e));
                diesel::result::Error::RollbackTransaction
            })?;
            validate_benchmark_aliases_uniqueness(
                conn,
                query_project.id,
                Some(query_benchmark.id),
                &effective_name,
                &current,
            )
            .map_err(|e| {
                *validation_err.borrow_mut() = Some(e);
                diesel::result::Error::RollbackTransaction
            })?;
        }

        diesel::update(
            schema::benchmark::table.filter(schema::benchmark::id.eq(query_benchmark.id)),
        )
        .set(&update_benchmark)
        .execute(conn)?;

        if let Some(list) = json_benchmark.aliases.as_ref() {
            replace_benchmark_aliases(conn, query_project.id, query_benchmark.id, list)?;
        }

        Ok(())
    });

    match txn_result {
        Ok(()) => {},
        Err(e) => {
            if let Some(he) = validation_err.into_inner() {
                return Err(he);
            }
            return Err(resource_conflict_err!(
                Benchmark,
                (&query_benchmark, &json_benchmark)
            )(e));
        },
    }

    let benchmark = QueryBenchmark::get(auth_conn!(context), query_benchmark.id)
        .map_err(resource_not_found_err!(Benchmark, query_benchmark))?;
    benchmark.into_json_for_project(auth_conn!(context), &query_project)
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
        .execute(write_conn!(context))
        .map_err(resource_conflict_err!(Benchmark, query_benchmark))?;

    Ok(())
}
