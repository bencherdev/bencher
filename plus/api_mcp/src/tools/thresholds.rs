use api_projects::thresholds::{
    ProjThresholdParams, ProjThresholdQuery, ProjThresholdsPagination, ProjThresholdsParams,
    get_ls_inner, get_one_inner, post_inner, put_inner,
};
use bencher_json::{
    JsonNewThreshold,
    project::threshold::{JsonThresholdQuery, JsonThresholdQueryParams, JsonUpdateThreshold},
};
use bencher_schema::{context::ApiContext, error::bad_request_error, model::user::actor::ApiActor};
use dropshot::HttpError;
use rmcp::model::{CallToolResult, Tool};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;

use super::{input_schema, json_result, list_result, parse_input};

#[derive(Deserialize, JsonSchema)]
struct ListThresholdsInput {
    #[serde(flatten)]
    path: ProjThresholdsParams,
    #[serde(flatten)]
    pagination: ProjThresholdsPagination,
    #[serde(flatten)]
    query: JsonThresholdQueryParams,
}

#[derive(Deserialize, JsonSchema)]
struct CreateThresholdInput {
    #[serde(flatten)]
    path: ProjThresholdsParams,
    #[serde(flatten)]
    threshold: JsonNewThreshold,
}

#[derive(Deserialize, JsonSchema)]
struct ViewThresholdInput {
    #[serde(flatten)]
    path: ProjThresholdParams,
    #[serde(flatten)]
    query: ProjThresholdQuery,
}

#[derive(Deserialize, JsonSchema)]
struct UpdateThresholdInput {
    #[serde(flatten)]
    path: ProjThresholdParams,
    #[serde(flatten)]
    threshold: JsonUpdateThreshold,
}

pub fn list_tool(name: &'static str) -> Tool {
    Tool::new(
        name,
        "List thresholds for a project. \
         A threshold is the combination of a branch, testbed, and measure with a statistical model.",
        input_schema::<ListThresholdsInput>(),
    )
}

pub async fn list(
    context: &ApiContext,
    api_actor: &ApiActor,
    arguments: Value,
) -> Result<CallToolResult, HttpError> {
    let ListThresholdsInput {
        path,
        pagination,
        query,
    } = parse_input(arguments)?;
    let query: JsonThresholdQuery = query.try_into().map_err(bad_request_error)?;
    let (json, total_count) = get_ls_inner(context, path, pagination, query, api_actor).await?;
    list_result(&json, total_count)
}

pub fn create_tool(name: &'static str) -> Tool {
    Tool::new(
        name,
        "Create a threshold for a project: a branch, testbed, and measure with a statistical model \
         used to detect performance regressions.",
        input_schema::<CreateThresholdInput>(),
    )
}

pub async fn create(
    context: &ApiContext,
    api_actor: &ApiActor,
    arguments: Value,
) -> Result<CallToolResult, HttpError> {
    let CreateThresholdInput { path, threshold } = parse_input(arguments)?;
    let json = post_inner(context, path, &threshold, api_actor).await?;
    json_result(&json)
}

pub fn view_tool(name: &'static str) -> Tool {
    Tool::new(
        name,
        "View a threshold. The `threshold` argument is a threshold UUID.",
        input_schema::<ViewThresholdInput>(),
    )
}

pub async fn view(
    context: &ApiContext,
    api_actor: &ApiActor,
    arguments: Value,
) -> Result<CallToolResult, HttpError> {
    let ViewThresholdInput { path, query } = parse_input(arguments)?;
    let json = get_one_inner(context, path, query, api_actor).await?;
    json_result(&json)
}

pub fn update_tool(name: &'static str) -> Tool {
    Tool::new(
        name,
        "Update a threshold's statistical model, or remove the model by setting it to null.",
        input_schema::<UpdateThresholdInput>(),
    )
}

pub async fn update(
    context: &ApiContext,
    api_actor: &ApiActor,
    arguments: Value,
) -> Result<CallToolResult, HttpError> {
    let UpdateThresholdInput { path, threshold } = parse_input(arguments)?;
    let json = put_inner(context, path, threshold, api_actor).await?;
    json_result(&json)
}
