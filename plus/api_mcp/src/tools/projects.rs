use api_projects::projects::{
    ProjectParams, ProjectsPagination, ProjectsQuery, get_ls_inner, get_one_inner,
};
use bencher_schema::{context::ApiContext, model::user::actor::ApiActor};
use dropshot::HttpError;
use rmcp::model::{CallToolResult, Tool};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;

use super::{input_schema, json_result, list_result, parse_input};

#[derive(Deserialize, JsonSchema)]
struct ListProjectsInput {
    #[serde(flatten)]
    pagination: ProjectsPagination,
    #[serde(flatten)]
    query: ProjectsQuery,
}

#[derive(Deserialize, JsonSchema)]
struct ViewProjectInput {
    #[serde(flatten)]
    path: ProjectParams,
}

pub fn list_tool(name: &'static str) -> Tool {
    Tool::new(
        name,
        "List projects. \
         With a project API key (`bencher_run_`), only that project is returned; \
         with a user API key, all projects visible to the user are returned.",
        input_schema::<ListProjectsInput>(),
    )
}

pub async fn list(
    context: &ApiContext,
    api_actor: &ApiActor,
    arguments: Value,
) -> Result<CallToolResult, HttpError> {
    let ListProjectsInput { pagination, query } = parse_input(arguments)?;
    let (json, total_count) = get_ls_inner(context, pagination, query, api_actor).await?;
    list_result(&json, total_count)
}

pub fn view_tool(name: &'static str) -> Tool {
    Tool::new(
        name,
        "View a project. The `project` argument accepts a project slug or UUID.",
        input_schema::<ViewProjectInput>(),
    )
}

pub async fn view(
    context: &ApiContext,
    api_actor: &ApiActor,
    arguments: Value,
) -> Result<CallToolResult, HttpError> {
    let ViewProjectInput { path } = parse_input(arguments)?;
    let json = get_one_inner(context, path, api_actor).await?;
    json_result(&json)
}
