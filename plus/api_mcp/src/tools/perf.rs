use api_projects::console::perf::{self, ProjConsolePerfParams};
use bencher_json::{
    JsonProject, Url,
    project::console::{JsonConsolePerf, JsonConsolePerfQuery, JsonConsolePerfQueryParams},
};
use bencher_schema::{
    actor_conn, context::ApiContext, error::bad_request_error, model::user::actor::ApiActor,
};
use dropshot::HttpError;
use rmcp::model::{CallToolResult, Tool};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{json_result, json_schema, parse_input};

#[derive(Deserialize, JsonSchema)]
struct QueryPerfInput {
    #[serde(flatten)]
    path: ProjConsolePerfParams,
    #[serde(flatten)]
    query: JsonConsolePerfQueryParams,
}

/// The plot the console draws for a query, with its project and a link to it.
#[derive(Serialize, JsonSchema)]
struct QueryPerfOutput {
    project: JsonProject,
    /// Opens the plot in Bencher.
    url: Url,
    #[serde(flatten)]
    perf: JsonConsolePerf,
}

pub fn query_tool(name: &'static str) -> Tool {
    Tool::new(
        name,
        "Query benchmark metrics over time for a project, as the plot that Bencher draws. \
         This is the primary tool for investigating performance history and regressions. \
         `branches`, `testbeds`, `benchmarks`, and `measures` are comma-separated lists of UUIDs; \
         use the corresponding list tools to find them. \
         `metrics` is an optional comma-separated list of the metric names to draw, such as `value` or `p99`; \
         without it, each branch, testbed, variant, and measure draws every metric name it reported in the window. \
         `start_time` and `end_time` bound the window in milliseconds since epoch; \
         without them, it ends now and starts four weeks before its end. \
         The result's `window` is the window read, which is at most three months for an anonymous caller. \
         Each of its `lines` is one branch, testbed, benchmark variant, measure, and metric name, \
         with indices into `branches`, `testbeds`, `benchmarks`, `variants`, `measures`, and `models`. \
         A line's `series.y` holds one value or null for each of the shared `points`. \
         `series.baseline`, `series.lower`, and `series.upper` hold the threshold's baseline and limits at each point, \
         and `series.alerts` names the points that alerted. \
         `points.x` holds each point's report start time in milliseconds, \
         and `points.report` indexes into `reports`, \
         which carry each report's version number and git hash, when it has one. \
         A plot holds at most 64 lines, and `total` counts every line the query names. \
         `url` opens the plot in Bencher.",
        json_schema::<QueryPerfInput>(),
    )
    .with_raw_output_schema(json_schema::<QueryPerfOutput>())
}

pub async fn query(
    context: &ApiContext,
    api_actor: &ApiActor,
    arguments: Value,
) -> Result<CallToolResult, HttpError> {
    let QueryPerfInput { path, query } = parse_input(arguments)?;
    let query: JsonConsolePerfQuery = query.try_into().map_err(bad_request_error)?;
    let (project, perf) = perf::get_inner(context, path, query.clone(), api_actor).await?;
    let url = query.perf.to_console_url(
        &context.console_url,
        &project.slug,
        project.is_public(),
        query.metrics.as_deref().unwrap_or_default(),
        None,
    );
    let project = project.into_json(actor_conn!(context, api_actor))?;
    json_result(&QueryPerfOutput {
        project,
        url: url.into(),
        perf,
    })
}
