use api_projects::jobs::{
    ProjJobParams, ProjJobsPagination, ProjJobsParams, ProjJobsQuery, get_ls_inner, get_one_inner,
};
use bencher_json::{JsonJob, JsonJobs};
use bencher_schema::{context::ApiContext, model::user::actor::ApiActor};
use dropshot::HttpError;
use rmcp::model::{CallToolResult, Tool};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;
use slog::Logger;

use super::{ListOutput, json_result, json_schema, list_result, parse_input};

#[derive(Deserialize, JsonSchema)]
struct ListJobsInput {
    #[serde(flatten)]
    path: ProjJobsParams,
    #[serde(flatten)]
    pagination: ProjJobsPagination,
    #[serde(flatten)]
    query: ProjJobsQuery,
}

#[derive(Deserialize, JsonSchema)]
struct ViewJobInput {
    #[serde(flatten)]
    path: ProjJobParams,
}

pub fn list_tool(name: &'static str) -> Tool {
    Tool::new(
        name,
        "List bare metal runner jobs for a project.",
        json_schema::<ListJobsInput>(),
    )
    .with_raw_output_schema(json_schema::<ListOutput<JsonJobs>>())
}

pub async fn list(
    context: &ApiContext,
    api_actor: &ApiActor,
    arguments: Value,
) -> Result<CallToolResult, HttpError> {
    let ListJobsInput {
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
        "View a bare metal runner job. The `job` argument is a job UUID.",
        json_schema::<ViewJobInput>(),
    )
    .with_raw_output_schema(json_schema::<JsonJob>())
}

pub async fn view(
    log: &Logger,
    context: &ApiContext,
    api_actor: &ApiActor,
    arguments: Value,
) -> Result<CallToolResult, HttpError> {
    let ViewJobInput { path } = parse_input(arguments)?;
    let json = get_one_inner(context, path, api_actor, log).await?;
    json_result(&json)
}
