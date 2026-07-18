use api_projects::alerts::{
    ProjAlertParams, ProjAlertsPagination, ProjAlertsParams, ProjAlertsQuery, get_ls_inner,
    get_one_inner, patch_inner,
};
use bencher_json::project::alert::JsonUpdateAlert;
use bencher_schema::{context::ApiContext, model::user::actor::ApiActor};
use dropshot::HttpError;
use rmcp::model::{CallToolResult, Tool};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;

use super::{input_schema, json_result, list_result, parse_input};

#[derive(Deserialize, JsonSchema)]
struct ListAlertsInput {
    #[serde(flatten)]
    path: ProjAlertsParams,
    #[serde(flatten)]
    pagination: ProjAlertsPagination,
    #[serde(flatten)]
    query: ProjAlertsQuery,
}

#[derive(Deserialize, JsonSchema)]
struct ViewAlertInput {
    #[serde(flatten)]
    path: ProjAlertParams,
}

#[derive(Deserialize, JsonSchema)]
struct UpdateAlertInput {
    #[serde(flatten)]
    path: ProjAlertParams,
    #[serde(flatten)]
    alert: JsonUpdateAlert,
}

pub fn list_tool(name: &'static str) -> Tool {
    Tool::new(
        name,
        "List performance alerts for a project, generated when a threshold boundary is exceeded.",
        input_schema::<ListAlertsInput>(),
    )
}

pub async fn list(
    context: &ApiContext,
    api_actor: &ApiActor,
    arguments: Value,
) -> Result<CallToolResult, HttpError> {
    let ListAlertsInput {
        path,
        pagination,
        query,
    } = parse_input(arguments)?;
    let (json, total_count) = get_ls_inner(context, api_actor, path, pagination, query).await?;
    list_result(&json, total_count)
}

pub fn view_tool(name: &'static str) -> Tool {
    Tool::new(
        name,
        "View an alert, including the boundary that was exceeded. The `alert` argument is an alert UUID.",
        input_schema::<ViewAlertInput>(),
    )
}

pub async fn view(
    context: &ApiContext,
    api_actor: &ApiActor,
    arguments: Value,
) -> Result<CallToolResult, HttpError> {
    let ViewAlertInput { path } = parse_input(arguments)?;
    let json = get_one_inner(context, path, api_actor).await?;
    json_result(&json)
}

pub fn update_tool(name: &'static str) -> Tool {
    Tool::new(
        name,
        "Update an alert. Project API keys (`bencher_run_`) can only update the alert status \
         (e.g. acknowledge or dismiss).",
        input_schema::<UpdateAlertInput>(),
    )
}

pub async fn update(
    context: &ApiContext,
    api_actor: &ApiActor,
    arguments: Value,
) -> Result<CallToolResult, HttpError> {
    let UpdateAlertInput { path, alert } = parse_input(arguments)?;
    let json = patch_inner(context, api_actor, path, alert).await?;
    json_result(&json)
}
