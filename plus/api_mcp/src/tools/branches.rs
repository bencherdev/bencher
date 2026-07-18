use api_projects::branches::{
    ProjBranchParams, ProjBranchQuery, ProjBranchesPagination, ProjBranchesParams,
    ProjBranchesQuery, get_ls_inner, get_one_inner, patch_inner, post_inner,
};
use bencher_json::{JsonNewBranch, project::branch::JsonUpdateBranch};
use bencher_schema::{context::ApiContext, model::user::actor::ApiActor};
use dropshot::HttpError;
use rmcp::model::{CallToolResult, Tool};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;
use slog::Logger;

use super::{input_schema, json_result, list_result, parse_input};

#[derive(Deserialize, JsonSchema)]
struct ListBranchesInput {
    #[serde(flatten)]
    path: ProjBranchesParams,
    #[serde(flatten)]
    pagination: ProjBranchesPagination,
    #[serde(flatten)]
    query: ProjBranchesQuery,
}

#[derive(Deserialize, JsonSchema)]
struct CreateBranchInput {
    #[serde(flatten)]
    path: ProjBranchesParams,
    #[serde(flatten)]
    branch: JsonNewBranch,
}

#[derive(Deserialize, JsonSchema)]
struct ViewBranchInput {
    #[serde(flatten)]
    path: ProjBranchParams,
    #[serde(flatten)]
    query: ProjBranchQuery,
}

#[derive(Deserialize, JsonSchema)]
struct UpdateBranchInput {
    #[serde(flatten)]
    path: ProjBranchParams,
    #[serde(flatten)]
    branch: JsonUpdateBranch,
}

pub fn list_tool(name: &'static str) -> Tool {
    Tool::new(
        name,
        "List branches for a project.",
        input_schema::<ListBranchesInput>(),
    )
}

pub async fn list(
    context: &ApiContext,
    api_actor: &ApiActor,
    arguments: Value,
) -> Result<CallToolResult, HttpError> {
    let ListBranchesInput {
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
        "Create a branch for a project.",
        input_schema::<CreateBranchInput>(),
    )
}

pub async fn create(
    log: &Logger,
    context: &ApiContext,
    api_actor: &ApiActor,
    arguments: Value,
) -> Result<CallToolResult, HttpError> {
    let CreateBranchInput { path, branch } = parse_input(arguments)?;
    let json = post_inner(log, context, path, branch, api_actor).await?;
    json_result(&json)
}

pub fn view_tool(name: &'static str) -> Tool {
    Tool::new(
        name,
        "View a branch. The `branch` argument accepts a branch slug or UUID.",
        input_schema::<ViewBranchInput>(),
    )
}

pub async fn view(
    context: &ApiContext,
    api_actor: &ApiActor,
    arguments: Value,
) -> Result<CallToolResult, HttpError> {
    let ViewBranchInput { path, query } = parse_input(arguments)?;
    let json = get_one_inner(context, path, query, api_actor).await?;
    json_result(&json)
}

pub fn update_tool(name: &'static str) -> Tool {
    Tool::new(
        name,
        "Update a branch. Project API keys (`bencher_run_`) cannot rename branches.",
        input_schema::<UpdateBranchInput>(),
    )
}

pub async fn update(
    log: &Logger,
    context: &ApiContext,
    api_actor: &ApiActor,
    arguments: Value,
) -> Result<CallToolResult, HttpError> {
    let UpdateBranchInput { path, branch } = parse_input(arguments)?;
    let json = patch_inner(log, context, api_actor, path, branch).await?;
    json_result(&json)
}
