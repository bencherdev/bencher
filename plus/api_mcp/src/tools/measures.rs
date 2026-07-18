use api_projects::measures::{
    ProjMeasureParams, ProjMeasuresPagination, ProjMeasuresParams, ProjMeasuresQuery, get_ls_inner,
    get_one_inner, patch_inner, post_inner,
};
use bencher_json::{JsonNewMeasure, project::measure::JsonUpdateMeasure};
use bencher_schema::{context::ApiContext, model::user::actor::ApiActor};
use dropshot::HttpError;
use rmcp::model::{CallToolResult, Tool};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;

use super::{input_schema, json_result, list_result, parse_input};

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
        input_schema::<ListMeasuresInput>(),
    )
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
    list_result(&json, total_count)
}

pub fn create_tool(name: &'static str) -> Tool {
    Tool::new(
        name,
        "Create a measure for a project.",
        input_schema::<CreateMeasureInput>(),
    )
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
        input_schema::<ViewMeasureInput>(),
    )
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
        input_schema::<UpdateMeasureInput>(),
    )
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
