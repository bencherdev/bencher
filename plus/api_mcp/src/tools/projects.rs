use api_organizations::projects::{
    OrgProjectsPagination, OrgProjectsParams, OrgProjectsQuery, get_ls_inner,
};
use api_projects::projects::{ProjectParams, get_one_inner};
use bencher_json::{JsonProject, JsonProjects};
use bencher_schema::{context::ApiContext, model::user::actor::ApiActor};
use dropshot::HttpError;
use rmcp::model::{CallToolResult, Tool};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;

use super::{ListOutput, json_result, json_schema, list_result, parse_input};

#[derive(Deserialize, JsonSchema)]
struct ListProjectsInput {
    #[serde(flatten)]
    path: OrgProjectsParams,
    #[serde(flatten)]
    pagination: OrgProjectsPagination,
    #[serde(flatten)]
    query: OrgProjectsQuery,
}

#[derive(Deserialize, JsonSchema)]
struct ViewProjectInput {
    #[serde(flatten)]
    path: ProjectParams,
}

pub fn list_tool(name: &'static str) -> Tool {
    Tool::new(
        name,
        "List an organization's projects. \
         The `organization` argument accepts an organization slug or UUID. \
         With a project API key (`bencher_run_`), only that project is returned; \
         with a user API key, every project in the organization is returned.",
        json_schema::<ListProjectsInput>(),
    )
    .with_raw_output_schema(json_schema::<ListOutput<JsonProjects>>())
}

pub async fn list(
    context: &ApiContext,
    api_actor: &ApiActor,
    arguments: Value,
) -> Result<CallToolResult, HttpError> {
    let ListProjectsInput {
        path,
        pagination,
        query,
    } = parse_input(arguments)?;
    let (json, total_count) = get_ls_inner(context, path, pagination, query, api_actor).await?;
    list_result(json, total_count)
}

pub fn view_tool(name: &'static str) -> Tool {
    Tool::new(
        name,
        "View a project. The `project` argument accepts a project slug or UUID.",
        json_schema::<ViewProjectInput>(),
    )
    .with_raw_output_schema(json_schema::<JsonProject>())
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
