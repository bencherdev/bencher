use api_projects::measures::{
    ProjMeasureParams, ProjMeasuresPagination, ProjMeasuresParams, ProjMeasuresQuery, get_ls_inner,
    get_one_inner, patch_inner, post_inner,
};
use bencher_json::{
    JsonMeasure, JsonMeasures, JsonNewMeasure, project::measure::JsonUpdateMeasure,
};
use bencher_schema::{context::ApiContext, model::user::actor::ApiActor};
use dropshot::HttpError;
use rmcp::model::{CallToolResult, Tool};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;

use super::{ListOutput, json_result, json_schema, list_result, parse_input};

#[derive(Deserialize, JsonSchema)]
struct ListMeasuresInput {
    #[serde(flatten)]
    path: ProjMeasuresParams,
    #[serde(flatten)]
    pagination: ProjMeasuresPagination,
    #[serde(flatten)]
    query: ProjMeasuresQuery,
}

#[derive(Deserialize, JsonSchema)]
struct CreateMeasureInput {
    #[serde(flatten)]
    path: ProjMeasuresParams,
    #[serde(flatten)]
    measure: JsonNewMeasure,
}

#[derive(Deserialize, JsonSchema)]
struct ViewMeasureInput {
    #[serde(flatten)]
    path: ProjMeasureParams,
}

#[derive(Deserialize, JsonSchema)]
struct UpdateMeasureInput {
    #[serde(flatten)]
    path: ProjMeasureParams,
    #[serde(flatten)]
    measure: JsonUpdateMeasure,
}

pub fn list_tool(name: &'static str) -> Tool {
    Tool::new(
        name,
        "List measures for a project (e.g. latency, throughput).",
        json_schema::<ListMeasuresInput>(),
    )
    .with_raw_output_schema(json_schema::<ListOutput<JsonMeasures>>())
}

pub async fn list(
    context: &ApiContext,
    api_actor: &ApiActor,
    arguments: Value,
) -> Result<CallToolResult, HttpError> {
    let ListMeasuresInput {
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
        "Create a measure for a project.",
        json_schema::<CreateMeasureInput>(),
    )
    .with_raw_output_schema(json_schema::<JsonMeasure>())
}

pub async fn create(
    context: &ApiContext,
    api_actor: &ApiActor,
    arguments: Value,
) -> Result<CallToolResult, HttpError> {
    let CreateMeasureInput { path, measure } = parse_input(arguments)?;
    let json = post_inner(context, path, measure, api_actor).await?;
    json_result(&json)
}

pub fn view_tool(name: &'static str) -> Tool {
    Tool::new(
        name,
        "View a measure. The `measure` argument accepts a measure slug or UUID.",
        json_schema::<ViewMeasureInput>(),
    )
    .with_raw_output_schema(json_schema::<JsonMeasure>())
}

pub async fn view(
    context: &ApiContext,
    api_actor: &ApiActor,
    arguments: Value,
) -> Result<CallToolResult, HttpError> {
    let ViewMeasureInput { path } = parse_input(arguments)?;
    let json = get_one_inner(context, path, api_actor).await?;
    json_result(&json)
}

pub fn update_tool(name: &'static str) -> Tool {
    Tool::new(
        name,
        "Update a measure. Project API keys (`bencher_run_`) cannot rename measures.",
        json_schema::<UpdateMeasureInput>(),
    )
    .with_raw_output_schema(json_schema::<JsonMeasure>())
}

pub async fn update(
    context: &ApiContext,
    api_actor: &ApiActor,
    arguments: Value,
) -> Result<CallToolResult, HttpError> {
    let UpdateMeasureInput { path, measure } = parse_input(arguments)?;
    let json = patch_inner(context, api_actor, path, measure).await?;
    json_result(&json)
}
