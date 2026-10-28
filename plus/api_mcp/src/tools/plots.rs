use api_projects::plots::{
    ProjPlotParams, ProjPlotsPagination, ProjPlotsParams, ProjPlotsQuery, get_ls_inner,
    get_one_inner, patch_inner, post_inner,
};
use bencher_json::{JsonNewPlot, JsonPlot, JsonPlots, project::plot::JsonUpdatePlot};
use bencher_schema::{context::ApiContext, model::user::actor::ApiActor};
use dropshot::HttpError;
use rmcp::model::{CallToolResult, Tool};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;

use super::{ListOutput, json_result, json_schema, list_result, parse_input};

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
struct CreatePlotInput {
    #[serde(flatten)]
    path: ProjPlotsParams,
    #[serde(flatten)]
    plot: JsonNewPlot,
}

#[derive(Deserialize, JsonSchema)]
struct ViewPlotInput {
    #[serde(flatten)]
    path: ProjPlotParams,
}

#[derive(Deserialize, JsonSchema)]
struct UpdatePlotInput {
    #[serde(flatten)]
    path: ProjPlotParams,
    #[serde(flatten)]
    plot: JsonUpdatePlot,
}

pub fn list_tool(name: &'static str) -> Tool {
    Tool::new(
        name,
        "List saved dashboard plots for a project.",
        json_schema::<ListPlotsInput>(),
    )
    .with_raw_output_schema(json_schema::<ListOutput<JsonPlots>>())
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
    list_result(json, total_count)
}

pub fn create_tool(name: &'static str) -> Tool {
    Tool::new(
        name,
        "Create a saved dashboard plot for a project. A project can have a maximum of 64 plots at a time.",
        json_schema::<CreatePlotInput>(),
    )
    .with_raw_output_schema(json_schema::<JsonPlot>())
}

pub async fn create(
    context: &ApiContext,
    api_actor: &ApiActor,
    arguments: Value,
) -> Result<CallToolResult, HttpError> {
    let CreatePlotInput { path, plot } = parse_input(arguments)?;
    let json = post_inner(context, path, plot, api_actor).await?;
    json_result(&json)
}

pub fn view_tool(name: &'static str) -> Tool {
    Tool::new(
        name,
        "View a saved dashboard plot. The `plot` argument is a plot UUID.",
        json_schema::<ViewPlotInput>(),
    )
    .with_raw_output_schema(json_schema::<JsonPlot>())
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

pub fn update_tool(name: &'static str) -> Tool {
    Tool::new(
        name,
        "Update a saved dashboard plot. The `plot` argument is a plot UUID.",
        json_schema::<UpdatePlotInput>(),
    )
    .with_raw_output_schema(json_schema::<JsonPlot>())
}

pub async fn update(
    context: &ApiContext,
    api_actor: &ApiActor,
    arguments: Value,
) -> Result<CallToolResult, HttpError> {
    let UpdatePlotInput { path, plot } = parse_input(arguments)?;
    let json = patch_inner(context, api_actor, path, plot).await?;
    json_result(&json)
}
