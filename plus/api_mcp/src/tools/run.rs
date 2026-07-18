use api_run::run::post_run;
use bencher_json::JsonNewRun;
use bencher_schema::{context::ApiContext, model::user::actor::ApiActor};
use dropshot::HttpError;
use rmcp::model::{CallToolResult, Tool};
use serde_json::Value;
use slog::Logger;

use super::{input_schema, json_result, parse_input};

pub fn submit_tool(name: &'static str) -> Tool {
    Tool::new(
        name,
        "Submit benchmark results to create a run. \
         The `results` field takes the raw output of a benchmark harness as strings; \
         the server parses them using the adapter in `settings.adapter` (defaults to `magic` auto-detection). \
         Branch, testbed, and thresholds are created on the fly if they do not exist. \
         Prefer the `bencher run` CLI when benchmarks are executed from a local checkout; \
         use this tool when the results are already in hand.",
        input_schema::<JsonNewRun>(),
    )
}

pub async fn submit(
    log: &Logger,
    context: &ApiContext,
    api_actor: ApiActor,
    headers: &http::HeaderMap,
    arguments: Value,
) -> Result<CallToolResult, HttpError> {
    let json_run: JsonNewRun = parse_input(arguments)?;
    let json = post_run(log, context, api_actor, headers, json_run).await?;
    json_result(&json)
}
