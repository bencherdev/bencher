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
        endpoint::{pub_response_ok, response_accepted, response_ok, ResponseAccepted, ResponseOk},
        Endpoint, Method,
    },
    model::{
        project::{
            benchmark::{InsertBenchmark, QueryBenchmark, UpdateBenchmark},
            QueryProject,
        },
        user::auth::AuthUser,
    },
    schema,
    util::{
        cors::{get_cors, CorsResponse},
        error::into_json,
        resource_id::fn_resource_id,
    },
    ApiError,
};

use super::Resource;

const BENCHMARK_RESOURCE: Resource = Resource::Benchmark;

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
    Ok(get_cors::<ApiContext>())
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
    let auth_user = AuthUser::new(&rqctx).await.ok();
    let endpoint = Endpoint::new(BENCHMARK_RESOURCE, Method::GetLs);

    let json = get_ls_inner(
        rqctx.context(),
        auth_user.as_ref(),
        path_params.into_inner(),
        pagination_params.into_inner(),
        query_params.into_inner(),
        endpoint,
    )
    .await
    .map_err(|e| {
        if let ApiError::HttpError(e) = e {
            e
        } else {
            endpoint.err(e).into()
        }
    })?;

    if auth_user.is_some() {
        response_ok!(endpoint, json)
    } else {
        pub_response_ok!(endpoint, json)
    }
}

async fn get_ls_inner(
    context: &ApiContext,
    auth_user: Option<&AuthUser>,
    path_params: ProjBenchmarksParams,
    pagination_params: ProjBenchmarksPagination,
    query_params: ProjBenchmarksQuery,
    endpoint: Endpoint,
) -> Result<JsonBenchmarks, ApiError> {
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

    Ok(query
        .offset(pagination_params.offset())
        .limit(pagination_params.limit())
        .load::<QueryBenchmark>(conn)
        .map_err(ApiError::from)?
        .into_iter()
        .filter_map(into_json!(endpoint, conn))
        .collect())
}

#[endpoint {
    method = POST,
    path =  "/v0/projects/{project}/benchmarks",
    tags = ["projects", "benchmarks"]
}]
pub async fn proj_benchmark_post(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<ProjBenchmarksParams>,
    body: TypedBody<JsonNewBenchmark>,
) -> Result<ResponseAccepted<JsonBenchmark>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::new(BENCHMARK_RESOURCE, Method::Post);

    let json = post_inner(
        rqctx.context(),
        path_params.into_inner(),
        body.into_inner(),
        &auth_user,
    )
    .await
    .map_err(|e| {
        if let ApiError::HttpError(e) = e {
            e
        } else {
            endpoint.err(e).into()
        }
    })?;

    response_accepted!(endpoint, json)
}

async fn post_inner(
    context: &ApiContext,
    path_params: ProjBenchmarksParams,
    json_benchmark: JsonNewBenchmark,
    auth_user: &AuthUser,
) -> Result<JsonBenchmark, ApiError> {
    let conn = &mut *context.conn().await;

    // Verify that the user is allowed
    let project_id = QueryProject::is_allowed(
        conn,
        &context.rbac,
        &path_params.project,
        auth_user,
        Permission::Create,
    )?
    .id;

    let insert_benchmark = InsertBenchmark::from_json(conn, project_id, json_benchmark);

    diesel::insert_into(schema::benchmark::table)
        .values(&insert_benchmark)
        .execute(conn)
        .map_err(ApiError::from)?;

    schema::benchmark::table
        .filter(schema::benchmark::uuid.eq(&insert_benchmark.uuid))
        .first::<QueryBenchmark>(conn)
        .map_err(ApiError::from)?
        .into_json(conn)
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
    Ok(get_cors::<ApiContext>())
}

#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/benchmarks/{benchmark}",
    tags = ["projects", "benchmarks"]
}]
pub async fn proj_benchmark_get(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<ProjBenchmarkParams>,
) -> Result<ResponseOk<JsonBenchmark>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await.ok();
    let endpoint = Endpoint::new(BENCHMARK_RESOURCE, Method::GetOne);

    let json = get_one_inner(
        rqctx.context(),
        path_params.into_inner(),
        auth_user.as_ref(),
    )
    .await
    .map_err(|e| {
        if let ApiError::HttpError(e) = e {
            e
        } else {
            endpoint.err(e).into()
        }
    })?;

    if auth_user.is_some() {
        response_ok!(endpoint, json)
    } else {
        pub_response_ok!(endpoint, json)
    }
}

fn_resource_id!(benchmark);

async fn get_one_inner(
    context: &ApiContext,
    path_params: ProjBenchmarkParams,
    auth_user: Option<&AuthUser>,
) -> Result<JsonBenchmark, ApiError> {
    let conn = &mut *context.conn().await;

    let query_project =
        QueryProject::is_allowed_public(conn, &context.rbac, &path_params.project, auth_user)?;

    QueryBenchmark::belonging_to(&query_project)
        .filter(resource_id(&path_params.benchmark)?)
        .first::<QueryBenchmark>(conn)
        .map_err(ApiError::from)?
        .into_json(conn)
}

#[endpoint {
    method = PATCH,
    path =  "/v0/projects/{project}/benchmarks/{benchmark}",
    tags = ["projects", "benchmarks"]
}]
pub async fn proj_benchmark_patch(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<ProjBenchmarkParams>,
    body: TypedBody<JsonUpdateBenchmark>,
) -> Result<ResponseAccepted<JsonBenchmark>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::new(BENCHMARK_RESOURCE, Method::Patch);

    let context = rqctx.context();
    let json = patch_inner(
        context,
        path_params.into_inner(),
        body.into_inner(),
        &auth_user,
    )
    .await
    .map_err(|e| {
        if let ApiError::HttpError(e) = e {
            e
        } else {
            endpoint.err(e).into()
        }
    })?;

    response_accepted!(endpoint, json)
}

async fn patch_inner(
    context: &ApiContext,
    path_params: ProjBenchmarkParams,
    json_benchmark: JsonUpdateBenchmark,
    auth_user: &AuthUser,
) -> Result<JsonBenchmark, ApiError> {
    let conn = &mut *context.conn().await;

    // Verify that the user is allowed
    let project_id = QueryProject::is_allowed(
        conn,
        &context.rbac,
        &path_params.project,
        auth_user,
        Permission::Edit,
    )?
    .id;

    let query_benchmark =
        QueryBenchmark::from_resource_id(conn, project_id, &path_params.benchmark)?;
    diesel::update(schema::benchmark::table.filter(schema::benchmark::id.eq(query_benchmark.id)))
        .set(&UpdateBenchmark::from(json_benchmark))
        .execute(conn)
        .map_err(ApiError::from)?;

    QueryBenchmark::get(conn, query_benchmark.id)?.into_json(conn)
}

#[endpoint {
    method = DELETE,
    path =  "/v0/projects/{project}/benchmarks/{benchmark}",
    tags = ["projects", "benchmarks"]
}]
pub async fn proj_benchmark_delete(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<ProjBenchmarkParams>,
) -> Result<ResponseAccepted<JsonEmpty>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::new(BENCHMARK_RESOURCE, Method::Delete);

    let json = delete_inner(rqctx.context(), path_params.into_inner(), &auth_user)
        .await
        .map_err(|e| {
            if let ApiError::HttpError(e) = e {
                e
            } else {
                endpoint.err(e).into()
            }
        })?;

    response_accepted!(endpoint, json)
}

async fn delete_inner(
    context: &ApiContext,
    path_params: ProjBenchmarkParams,
    auth_user: &AuthUser,
) -> Result<JsonEmpty, ApiError> {
    let conn = &mut *context.conn().await;

    // Verify that the user is allowed
    let project_id = QueryProject::is_allowed(
        conn,
        &context.rbac,
        &path_params.project,
        auth_user,
        Permission::Delete,
    )?
    .id;

    let query_benchmark =
        QueryBenchmark::from_resource_id(conn, project_id, &path_params.benchmark)?;
    diesel::delete(schema::benchmark::table.filter(schema::benchmark::id.eq(query_benchmark.id)))
        .execute(conn)
        .map_err(ApiError::from)?;

    Ok(JsonEmpty {})
}
