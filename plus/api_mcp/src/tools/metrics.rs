use api_projects::metrics::{ProjMetricParams, get_one_inner};
use bencher_schema::{context::ApiContext, model::user::actor::ApiActor};
use dropshot::HttpError;
use rmcp::model::{CallToolResult, Tool};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;

use super::{input_schema, json_result, parse_input};

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
        input_schema::<ViewMetricInput>(),
    )
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
