use api_projects::metrics::{ProjMetricParams, get_one_inner};
use bencher_json::JsonOneMetric;
use bencher_schema::{context::ApiContext, model::user::actor::ApiActor};
use dropshot::HttpError;
use rmcp::model::{CallToolResult, Tool};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;

use super::{json_result, json_schema, parse_input};

#[derive(Deserialize, JsonSchema)]
struct ViewMetricInput {
    #[serde(flatten)]
    path: ProjMetricParams,
}

pub fn view_tool(name: &'static str) -> Tool {
    Tool::new(
        name,
        "View a single metric with its full context: report, branch, testbed, benchmark, \
         measure, and any threshold boundary. The `metric` argument is a metric UUID.",
        json_schema::<ViewMetricInput>(),
    )
    .with_raw_output_schema(json_schema::<JsonOneMetric>())
}

pub async fn view(
    context: &ApiContext,
    api_actor: &ApiActor,
    arguments: Value,
) -> Result<CallToolResult, HttpError> {
    let ViewMetricInput { path } = parse_input(arguments)?;
    let json = get_one_inner(context, path, api_actor).await?;
    json_result(&json)
}
