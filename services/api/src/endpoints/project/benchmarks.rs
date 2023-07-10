use bencher_json::{JsonBenchmark, JsonDirection, JsonPagination, ResourceId};
use diesel::{expression_methods::BoolExpressionMethods, ExpressionMethods, QueryDsl, RunQueryDsl};
use dropshot::{endpoint, HttpError, Path, Query, RequestContext};
use schemars::JsonSchema;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    context::ApiContext,
    endpoints::{
        endpoint::{pub_response_ok, response_ok, ResponseOk},
        Endpoint, Method,
    },
    error::api_error,
    model::{
        project::{benchmark::QueryBenchmark, QueryProject},
        user::auth::AuthUser,
    },
    schema,
    util::{
        cors::{get_cors, CorsResponse},
        error::into_json,
    },
    ApiError,
};

use super::Resource;

const BENCHMARK_RESOURCE: Resource = Resource::Benchmark;

#[derive(Deserialize, JsonSchema)]
pub struct ProjBenchmarksParams {
    pub project: ResourceId,
}

pub type ProjBenchmarksQuery = JsonPagination<ProjBenchmarksSort, ProjBenchmarksQueryParams>;

#[derive(Clone, Copy, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProjBenchmarksSort {
    #[default]
    Name,
}

#[derive(Deserialize, JsonSchema)]
pub struct ProjBenchmarksQueryParams {
    pub name: Option<String>,
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
    query_params: Query<ProjBenchmarksQuery>,
) -> Result<ResponseOk<Vec<JsonBenchmark>>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await.ok();
    let endpoint = Endpoint::new(BENCHMARK_RESOURCE, Method::GetLs);

    let json = get_ls_inner(
        rqctx.context(),
        auth_user.as_ref(),
        path_params.into_inner(),
        query_params.into_inner(),
        endpoint,
    )
    .await
    .map_err(|e| endpoint.err(e))?;

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
    query_params: ProjBenchmarksQuery,
    endpoint: Endpoint,
) -> Result<Vec<JsonBenchmark>, ApiError> {
    let conn = &mut *context.conn().await;

    let query_project =
        QueryProject::is_allowed_public(conn, &context.rbac, &path_params.project, auth_user)?;

    let mut query = schema::benchmark::table
        .filter(schema::benchmark::project_id.eq(&query_project.id))
        .into_boxed();

    if let Some(name) = &query_params.query.name {
        query = query.filter(schema::benchmark::name.eq(name));
    }

    query = match query_params.order() {
        ProjBenchmarksSort::Name => match query_params.direction {
            Some(JsonDirection::Asc) | None => query.order(schema::benchmark::name.asc()),
            Some(JsonDirection::Desc) => query.order(schema::benchmark::name.desc()),
        },
    };

    Ok(query
        .offset(query_params.offset())
        .limit(query_params.limit())
        .load::<QueryBenchmark>(conn)
        .map_err(api_error!())?
        .into_iter()
        .filter_map(into_json!(endpoint, conn))
        .collect())
}

#[derive(Deserialize, JsonSchema)]
pub struct ProjBenchmarkParams {
    pub project: ResourceId,
    pub benchmark: Uuid,
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
    .map_err(|e| endpoint.err(e))?;

    if auth_user.is_some() {
        response_ok!(endpoint, json)
    } else {
        pub_response_ok!(endpoint, json)
    }
}

async fn get_one_inner(
    context: &ApiContext,
    path_params: ProjBenchmarkParams,
    auth_user: Option<&AuthUser>,
) -> Result<JsonBenchmark, ApiError> {
    let conn = &mut *context.conn().await;

    let query_project =
        QueryProject::is_allowed_public(conn, &context.rbac, &path_params.project, auth_user)?;

    schema::benchmark::table
        .filter(
            schema::benchmark::project_id
                .eq(query_project.id)
                .and(schema::benchmark::uuid.eq(path_params.benchmark.to_string())),
        )
        .first::<QueryBenchmark>(conn)
        .map_err(api_error!())?
        .into_json(conn)
}
