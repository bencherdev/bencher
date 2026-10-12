use api_projects::reports::{
    ProjReportParams, ProjReportsPagination, ProjReportsParams, get_ls_inner, get_one_inner,
    post_inner,
};
use bencher_json::{
    JsonNewReport, JsonReport, JsonReports,
    project::report::{JsonReportQuery, JsonReportQueryParams},
};
use bencher_schema::{context::ApiContext, error::bad_request_error, model::user::actor::ApiActor};
use dropshot::HttpError;
use rmcp::model::{CallToolResult, Tool};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;
use slog::Logger;

use super::{ListOutput, json_result, json_schema, list_result, parse_input};

#[derive(Deserialize, JsonSchema)]
struct ListReportsInput {
    #[serde(flatten)]
    path: ProjReportsParams,
    #[serde(flatten)]
    pagination: ProjReportsPagination,
    #[serde(flatten)]
    query: JsonReportQueryParams,
}

#[derive(Deserialize, JsonSchema)]
struct CreateReportInput {
    #[serde(flatten)]
    path: ProjReportsParams,
    #[serde(flatten)]
    report: JsonNewReport,
}

#[derive(Deserialize, JsonSchema)]
struct ViewReportInput {
    #[serde(flatten)]
    path: ProjReportParams,
}

pub fn list_tool(name: &'static str) -> Tool {
    Tool::new(
        name,
        "List reports for a project, sorted by date time in reverse chronological order by default. \
         Results and alerts are collapsed to counts unless `expand` is set to true.",
        json_schema::<ListReportsInput>(),
    )
    .with_raw_output_schema(json_schema::<ListOutput<JsonReports>>())
}

pub async fn list(
    log: &Logger,
    context: &ApiContext,
    api_actor: &ApiActor,
    arguments: Value,
) -> Result<CallToolResult, HttpError> {
    let ListReportsInput {
        path,
        pagination,
        query,
    } = parse_input(arguments)?;
    let query: JsonReportQuery = query.try_into().map_err(bad_request_error)?;
    let (json, total_count) =
        get_ls_inner(log, context, path, pagination, query, api_actor).await?;
    list_result(json, total_count)
}

pub fn create_tool(name: &'static str) -> Tool {
    Tool::new(
        name,
        "Create a report from raw benchmark results for an existing branch and testbed. \
         Prefer `submit_run` unless the branch and testbed are already known to exist.",
        json_schema::<CreateReportInput>(),
    )
    .with_raw_output_schema(json_schema::<JsonReport>())
}

pub async fn create(
    log: &Logger,
    context: &ApiContext,
    api_actor: ApiActor,
    arguments: Value,
) -> Result<CallToolResult, HttpError> {
    let CreateReportInput { path, report } = parse_input(arguments)?;
    let json = post_inner(log, context, path, report, api_actor).await?;
    json_result(&json)
}

pub fn view_tool(name: &'static str) -> Tool {
    Tool::new(
        name,
        "View a report, including its full results and any alerts.",
        json_schema::<ViewReportInput>(),
    )
    .with_raw_output_schema(json_schema::<JsonReport>())
}

pub async fn view(
    log: &Logger,
    context: &ApiContext,
    api_actor: &ApiActor,
    arguments: Value,
) -> Result<CallToolResult, HttpError> {
    let ViewReportInput { path } = parse_input(arguments)?;
    let json = get_one_inner(log, context, path, api_actor).await?;
    json_result(&json)
}
