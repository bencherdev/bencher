use std::sync::Arc;

use bencher_json::{
    JsonBenchmark,
    ResourceId,
};
use diesel::{
    expression_methods::BoolExpressionMethods,
    QueryDsl,
    RunQueryDsl,
};
use dropshot::{
    endpoint,
    HttpError,
    HttpResponseHeaders,
    HttpResponseOk,
    Path,
    RequestContext,
};
use schemars::JsonSchema;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    db::{
        model::{
            benchmark::QueryBenchmark,
            project::QueryProject,
        },
        schema,
    },
    diesel::ExpressionMethods,
    util::{
        cors::get_cors,
        headers::CorsHeaders,
        http_error,
        Context,
    },
};

#[derive(Deserialize, JsonSchema)]
pub struct GetLsParams {
    pub project: ResourceId,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/benchmarks",
    tags = ["projects", "benchmarks"]
}]
pub async fn dir_options(
    _rqctx: Arc<RequestContext<Context>>,
    _path_params: Path<GetLsParams>,
) -> Result<HttpResponseHeaders<HttpResponseOk<String>>, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/benchmarks",
    tags = ["projects", "benchmarks"]
}]
pub async fn get_ls(
    rqctx: Arc<RequestContext<Context>>,
    path_params: Path<GetLsParams>,
) -> Result<HttpResponseHeaders<HttpResponseOk<Vec<JsonBenchmark>>, CorsHeaders>, HttpError> {
    let db_connection = rqctx.context();
    let path_params = path_params.into_inner();

    let conn = &mut *db_connection.lock().await;
    let query_project = QueryProject::from_resource_id(conn, &path_params.project)?;
    let json: Vec<JsonBenchmark> = schema::benchmark::table
        .filter(schema::benchmark::project_id.eq(&query_project.id))
        .order(schema::benchmark::name)
        .load::<QueryBenchmark>(conn)
        .map_err(|_| http_error!("Failed to get benchmarks."))?
        .into_iter()
        .filter_map(|query| query.to_json(conn).ok())
        .collect();

    Ok(HttpResponseHeaders::new(
        HttpResponseOk(json),
        CorsHeaders::new_pub("GET".into()),
    ))
}

#[derive(Deserialize, JsonSchema)]
pub struct GetOneParams {
    pub project:   ResourceId,
    pub benchmark: Uuid,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/benchmarks/{benchmark}",
    tags = ["projects", "benchmarks"]
}]
pub async fn one_options(
    _rqctx: Arc<RequestContext<Context>>,
    _path_params: Path<GetOneParams>,
) -> Result<HttpResponseHeaders<HttpResponseOk<String>>, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/benchmarks/{benchmark}",
    tags = ["projects", "benchmarks"]
}]
pub async fn get_one(
    rqctx: Arc<RequestContext<Context>>,
    path_params: Path<GetOneParams>,
) -> Result<HttpResponseHeaders<HttpResponseOk<JsonBenchmark>, CorsHeaders>, HttpError> {
    let db_connection = rqctx.context();
    let path_params = path_params.into_inner();
    let benchmark = path_params.benchmark.to_string();

    let conn = &mut *db_connection.lock().await;
    let project = QueryProject::from_resource_id(conn, &path_params.project)?;
    let query = if let Ok(query) = schema::benchmark::table
        .filter(
            schema::benchmark::project_id
                .eq(project.id)
                .and(schema::benchmark::uuid.eq(&benchmark)),
        )
        .first::<QueryBenchmark>(conn)
    {
        Ok(query)
    } else {
        Err(http_error!("Failed to get benchmark."))
    }?;
    let json = query.to_json(conn)?;

    Ok(HttpResponseHeaders::new(
        HttpResponseOk(json),
        CorsHeaders::new_pub("GET".into()),
    ))
}
