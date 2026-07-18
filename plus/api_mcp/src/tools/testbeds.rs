use api_projects::testbeds::{
    ProjTestbedParams, ProjTestbedQuery, ProjTestbedsPagination, ProjTestbedsParams,
    ProjTestbedsQuery, get_ls_inner, get_one_inner, patch_inner, post_inner,
};
use bencher_json::{JsonNewTestbed, project::testbed::JsonUpdateTestbed};
use bencher_schema::{context::ApiContext, model::user::actor::ApiActor};
use dropshot::HttpError;
use rmcp::model::{CallToolResult, Tool};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;

use super::{input_schema, json_result, list_result, parse_input};

#[derive(Deserialize, JsonSchema)]
struct ListTestbedsInput {
    #[serde(flatten)]
    path: ProjTestbedsParams,
    #[serde(flatten)]
    pagination: ProjTestbedsPagination,
    #[serde(flatten)]
    query: ProjTestbedsQuery,
}

#[derive(Deserialize, JsonSchema)]
struct CreateTestbedInput {
    #[serde(flatten)]
    path: ProjTestbedsParams,
    #[serde(flatten)]
    testbed: JsonNewTestbed,
}

#[derive(Deserialize, JsonSchema)]
struct ViewTestbedInput {
    #[serde(flatten)]
    path: ProjTestbedParams,
    #[serde(flatten)]
    query: ProjTestbedQuery,
}

#[derive(Deserialize, JsonSchema)]
struct UpdateTestbedInput {
    #[serde(flatten)]
    path: ProjTestbedParams,
    #[serde(flatten)]
    testbed: JsonUpdateTestbed,
}

pub fn list_tool(name: &'static str) -> Tool {
    Tool::new(
        name,
        "List testbeds for a project.",
        input_schema::<ListTestbedsInput>(),
    )
}

pub async fn list(
    context: &ApiContext,
    api_actor: &ApiActor,
    arguments: Value,
) -> Result<CallToolResult, HttpError> {
    let ListTestbedsInput {
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
        "Create a testbed for a project.",
        input_schema::<CreateTestbedInput>(),
    )
}

pub async fn create(
    context: &ApiContext,
    api_actor: &ApiActor,
    arguments: Value,
) -> Result<CallToolResult, HttpError> {
    let CreateTestbedInput { path, testbed } = parse_input(arguments)?;
    let json = post_inner(context, path, testbed, api_actor).await?;
    json_result(&json)
}

pub fn view_tool(name: &'static str) -> Tool {
    Tool::new(
        name,
        "View a testbed. The `testbed` argument accepts a testbed slug or UUID.",
        input_schema::<ViewTestbedInput>(),
    )
}

pub async fn view(
    context: &ApiContext,
    api_actor: &ApiActor,
    arguments: Value,
) -> Result<CallToolResult, HttpError> {
    let ViewTestbedInput { path, query } = parse_input(arguments)?;
    let json = get_one_inner(context, path, query, api_actor).await?;
    json_result(&json)
}

pub fn update_tool(name: &'static str) -> Tool {
    Tool::new(
        name,
        "Update a testbed. Project API keys (`bencher_run_`) cannot rename testbeds.",
        input_schema::<UpdateTestbedInput>(),
    )
}

pub async fn update(
    context: &ApiContext,
    api_actor: &ApiActor,
    arguments: Value,
) -> Result<CallToolResult, HttpError> {
    let UpdateTestbedInput { path, testbed } = parse_input(arguments)?;
    let json = patch_inner(context, api_actor, path, testbed).await?;
    json_result(&json)
}
