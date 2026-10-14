use api_projects::benchmarks::{
    ProjBenchmarkParams, ProjBenchmarksPagination, ProjBenchmarksParams, ProjBenchmarksQuery,
    get_ls_inner, get_one_inner, patch_inner, post_inner,
};
use bencher_json::project::benchmark::{
    JsonBenchmark, JsonBenchmarks, JsonNewBenchmark, JsonUpdateBenchmark,
};
use bencher_schema::{context::ApiContext, model::user::actor::ApiActor};
use dropshot::HttpError;
use rmcp::model::{CallToolResult, Tool};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;

use super::{ListOutput, json_result, json_schema, list_result, parse_input};

#[derive(Deserialize, JsonSchema)]
struct ListBenchmarksInput {
    #[serde(flatten)]
    path: ProjBenchmarksParams,
    #[serde(flatten)]
    pagination: ProjBenchmarksPagination,
    #[serde(flatten)]
    query: ProjBenchmarksQuery,
}

#[derive(Deserialize, JsonSchema)]
struct CreateBenchmarkInput {
    #[serde(flatten)]
    path: ProjBenchmarksParams,
    #[serde(flatten)]
    benchmark: JsonNewBenchmark,
}

#[derive(Deserialize, JsonSchema)]
struct ViewBenchmarkInput {
    #[serde(flatten)]
    path: ProjBenchmarkParams,
}

#[derive(Deserialize, JsonSchema)]
struct UpdateBenchmarkInput {
    #[serde(flatten)]
    path: ProjBenchmarkParams,
    #[serde(flatten)]
    benchmark: JsonUpdateBenchmark,
}

pub fn list_tool(name: &'static str) -> Tool {
    Tool::new(
        name,
        "List benchmarks for a project.",
        json_schema::<ListBenchmarksInput>(),
    )
    .with_raw_output_schema(json_schema::<ListOutput<JsonBenchmarks>>())
}

pub async fn list(
    context: &ApiContext,
    api_actor: &ApiActor,
    arguments: Value,
) -> Result<CallToolResult, HttpError> {
    let ListBenchmarksInput {
        path,
        pagination,
        query,
    } = parse_input(arguments)?;
    let (json, total_count) = get_ls_inner(context, api_actor, path, pagination, query).await?;
    list_result(json, total_count)
}

pub fn create_tool(name: &'static str) -> Tool {
    Tool::new(
        name,
        "Create a benchmark for a project.",
        json_schema::<CreateBenchmarkInput>(),
    )
    .with_raw_output_schema(json_schema::<JsonBenchmark>())
}

pub async fn create(
    context: &ApiContext,
    api_actor: &ApiActor,
    arguments: Value,
) -> Result<CallToolResult, HttpError> {
    let CreateBenchmarkInput { path, benchmark } = parse_input(arguments)?;
    let json = post_inner(context, path, benchmark, api_actor).await?;
    json_result(&json)
}

pub fn view_tool(name: &'static str) -> Tool {
    Tool::new(
        name,
        "View a benchmark. The `benchmark` argument accepts a benchmark slug or UUID.",
        json_schema::<ViewBenchmarkInput>(),
    )
    .with_raw_output_schema(json_schema::<JsonBenchmark>())
}

pub async fn view(
    context: &ApiContext,
    api_actor: &ApiActor,
    arguments: Value,
) -> Result<CallToolResult, HttpError> {
    let ViewBenchmarkInput { path } = parse_input(arguments)?;
    let json = get_one_inner(context, path, api_actor).await?;
    json_result(&json)
}

pub fn update_tool(name: &'static str) -> Tool {
    Tool::new(
        name,
        "Update a benchmark. Project API keys (`bencher_run_`) cannot rename benchmarks.",
        json_schema::<UpdateBenchmarkInput>(),
    )
    .with_raw_output_schema(json_schema::<JsonBenchmark>())
}

pub async fn update(
    context: &ApiContext,
    api_actor: &ApiActor,
    arguments: Value,
) -> Result<CallToolResult, HttpError> {
    let UpdateBenchmarkInput { path, benchmark } = parse_input(arguments)?;
    let json = patch_inner(context, api_actor, path, benchmark).await?;
    json_result(&json)
}
