use bencher_json::{project::threshold::JsonStatistic, ResourceId, StatisticUuid};
use diesel::{ExpressionMethods, JoinOnDsl, QueryDsl, RunQueryDsl, SelectableHelper};
use dropshot::{endpoint, HttpError, Path, RequestContext};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::{
    conn_lock,
    context::ApiContext,
    endpoints::{
        endpoint::{CorsResponse, Get, ResponseOk},
        Endpoint,
    },
    error::resource_not_found_err,
    model::user::auth::AuthUser,
    model::{
        project::{threshold::statistic::QueryStatistic, QueryProject},
        user::auth::PubBearerToken,
    },
    schema,
};

#[derive(Deserialize, JsonSchema)]
pub struct ProjStatisticParams {
    /// The slug or UUID for a project.
    pub project: ResourceId,
    /// The UUID for a statistic.
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
    bearer_token: PubBearerToken,
    path_params: Path<ProjStatisticParams>,
) -> Result<ResponseOk<JsonStatistic>, HttpError> {
    let auth_user = AuthUser::from_pub_token(rqctx.context(), bearer_token).await?;
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
    let query_project = QueryProject::is_allowed_public(
        conn_lock!(context),
        &context.rbac,
        &path_params.project,
        auth_user,
    )?;

    conn_lock!(context, |conn| schema::statistic::table
        .inner_join(
            schema::threshold::table.on(schema::statistic::threshold_id.eq(schema::threshold::id)),
        )
        .filter(schema::threshold::project_id.eq(query_project.id))
        .filter(schema::statistic::uuid.eq(path_params.statistic))
        .select(QueryStatistic::as_select())
        .first(conn)
        .map_err(resource_not_found_err!(
            Statistic,
            (&query_project, path_params.statistic)
        ))?
        .into_json(conn))
}
