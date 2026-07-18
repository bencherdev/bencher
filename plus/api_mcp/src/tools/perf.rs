use api_projects::perf::{self, ProjPerfParams, img};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use bencher_json::{
    JsonPerfQuery,
    project::perf::{JsonPerfImgQueryParams, JsonPerfQueryParams},
};
use bencher_schema::{context::ApiContext, error::bad_request_error, model::user::actor::ApiActor};
use dropshot::HttpError;
use rmcp::model::{CallToolResult, ContentBlock, Tool};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;
use slog::Logger;

use super::{input_schema, json_result, parse_input};

#[derive(Deserialize, JsonSchema)]
struct QueryPerfInput {
    #[serde(flatten)]
    path: ProjPerfParams,
    #[serde(flatten)]
    query: JsonPerfQueryParams,
}

#[derive(Deserialize, JsonSchema)]
struct PerfImageInput {
    #[serde(flatten)]
    path: ProjPerfParams,
    #[serde(flatten)]
    query: JsonPerfImgQueryParams,
}

pub fn query_tool(name: &'static str) -> Tool {
    Tool::new(
        name,
        "Query benchmark metrics over time for a project. \
         This is the primary tool for investigating performance history and regressions. \
         `branches`, `testbeds`, `benchmarks`, and `measures` are comma-separated lists of UUIDs; \
         use the corresponding list tools to find them. \
         `start_time` and `end_time` bound the results in milliseconds since epoch.",
        input_schema::<QueryPerfInput>(),
    )
}

pub async fn query(
    log: &Logger,
    context: &ApiContext,
    api_actor: &ApiActor,
    arguments: Value,
) -> Result<CallToolResult, HttpError> {
    let QueryPerfInput { path, query } = parse_input(arguments)?;
    let query: JsonPerfQuery = query.try_into().map_err(bad_request_error)?;
    let json = perf::get_inner(log, context, path, query, api_actor).await?;
    json_result(&json)
}

pub fn image_tool(name: &'static str) -> Tool {
    Tool::new(
        name,
        "Render a perf plot image (JPEG) for a project. \
         Takes the same query arguments as `query_perf`, plus an optional `title`.",
        input_schema::<PerfImageInput>(),
    )
}

pub async fn image(
    log: &Logger,
    context: &ApiContext,
    api_actor: &ApiActor,
    arguments: Value,
) -> Result<CallToolResult, HttpError> {
    let PerfImageInput { path, mut query } = parse_input(arguments)?;
    let title = query.title.take();
    let query_params: JsonPerfQueryParams = query.into();
    let query: JsonPerfQuery = query_params.try_into().map_err(bad_request_error)?;
    let jpeg = img::get_inner(log, context, path, title.as_deref(), query, api_actor).await?;
    let data = STANDARD.encode(jpeg);
    Ok(CallToolResult::success(vec![ContentBlock::image(
        data,
        "image/jpeg",
    )]))
}
