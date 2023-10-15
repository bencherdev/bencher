use bencher_json::{project::threshold::JsonStatistic, ResourceId, StatisticUuid};
use diesel::{ExpressionMethods, JoinOnDsl, QueryDsl, RunQueryDsl, SelectableHelper};
use dropshot::{endpoint, HttpError, Path, RequestContext};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::{
    context::ApiContext,
    endpoints::{
        endpoint::{CorsResponse, Get, ResponseOk},
        Endpoint,
    },
    error::resource_not_found_err,
    model::project::{threshold::statistic::QueryStatistic, QueryProject},
    model::user::auth::AuthUser,
    schema,
};

#[derive(Deserialize, JsonSchema)]
pub struct ProjStatisticParams {
    pub project: ResourceId,
    pub statistic: StatisticUuid,
}

#[allow(clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/statistics/{statistic}",
    tags = ["projects", "statistics"]
}]
pub async fn proj_statistic_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<ProjStatisticParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into()]))
}

#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/statistics/{statistic}",
    tags = ["projects", "statistics"]
}]
pub async fn proj_statistic_get(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<ProjStatisticParams>,
) -> Result<ResponseOk<JsonStatistic>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await.ok();
    let json = get_one_inner(
        rqctx.context(),
        path_params.into_inner(),
        auth_user.as_ref(),
    )
    .await?;
    Ok(Get::response_ok(json, auth_user.is_some()))
}

async fn get_one_inner(
    context: &ApiContext,
    path_params: ProjStatisticParams,
    auth_user: Option<&AuthUser>,
) -> Result<JsonStatistic, HttpError> {
    let conn = &mut *context.conn().await;

    let query_project =
        QueryProject::is_allowed_public(conn, &context.rbac, &path_params.project, auth_user)?;

    schema::statistic::table
        .inner_join(
            schema::threshold::table.on(schema::statistic::threshold_id.eq(schema::threshold::id)),
        )
        .filter(schema::threshold::project_id.eq(query_project.id))
        .filter(schema::statistic::uuid.eq(path_params.statistic))
        .select(QueryStatistic::as_select())
        .first(conn)
        .map_err(resource_not_found_err!(
            Statistic,
            (query_project, path_params.statistic)
        ))?
        .into_json(conn)
}
