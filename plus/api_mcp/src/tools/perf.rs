use api_projects::perf::{self, ProjPerfParams};
use bencher_json::{JsonPerf, JsonPerfQuery, project::perf::JsonPerfQueryParams};
use bencher_schema::{context::ApiContext, error::bad_request_error, model::user::actor::ApiActor};
use dropshot::HttpError;
use rmcp::model::{CallToolResult, Tool};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;
use slog::Logger;

use super::{json_result, json_schema, parse_input};

#[derive(Deserialize, JsonSchema)]
struct QueryPerfInput {
    #[serde(flatten)]
    path: ProjPerfParams,
    #[serde(flatten)]
    query: JsonPerfQueryParams,
}

pub fn query_tool(name: &'static str) -> Tool {
    Tool::new(
        name,
        "Query benchmark metrics over time for a project. \
         This is the primary tool for investigating performance history and regressions. \
         `branches`, `testbeds`, `benchmarks`, and `measures` are comma-separated lists of UUIDs; \
         use the corresponding list tools to find them. \
         `start_time` and `end_time` bound the results in milliseconds since epoch.",
        json_schema::<QueryPerfInput>(),
    )
    .with_raw_output_schema(json_schema::<JsonPerf>())
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
