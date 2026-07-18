use api_projects::plots::{
    ProjPlotParams, ProjPlotsPagination, ProjPlotsParams, ProjPlotsQuery, get_ls_inner,
    get_one_inner,
};
use bencher_schema::{context::ApiContext, model::user::actor::ApiActor};
use dropshot::HttpError;
use rmcp::model::{CallToolResult, Tool};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;

use super::{input_schema, json_result, list_result, parse_input};

#[derive(Deserialize, JsonSchema)]
struct ListPlotsInput {
    #[serde(flatten)]
    path: ProjPlotsParams,
    #[serde(flatten)]
    pagination: ProjPlotsPagination,
    #[serde(flatten)]
    query: ProjPlotsQuery,
}

#[derive(Deserialize, JsonSchema)]
struct ViewPlotInput {
    #[serde(flatten)]
    path: ProjPlotParams,
}

pub fn list_tool(name: &'static str) -> Tool {
    Tool::new(
        name,
        "List saved dashboard plots for a project.",
        input_schema::<ListPlotsInput>(),
    )
}

pub async fn list(
    context: &ApiContext,
    api_actor: &ApiActor,
    arguments: Value,
) -> Result<CallToolResult, HttpError> {
    let ListPlotsInput {
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
        "View a saved dashboard plot. The `plot` argument is a plot UUID.",
        input_schema::<ViewPlotInput>(),
    )
}

pub async fn view(
    context: &ApiContext,
    api_actor: &ApiActor,
    arguments: Value,
) -> Result<CallToolResult, HttpError> {
    let ViewPlotInput { path } = parse_input(arguments)?;
    let json = get_one_inner(context, path, api_actor).await?;
    json_result(&json)
}
