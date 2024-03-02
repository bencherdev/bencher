use bencher_json::{project::threshold::JsonStatistic, ModelUuid, ResourceId};
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
        project::{threshold::model::QueryModel, QueryProject},
        user::auth::PubBearerToken,
    },
    schema,
};

#[derive(Deserialize, JsonSchema)]
pub struct ProjStatisticParams {
    /// The slug or UUID for a project.
    pub project: ResourceId,
    /// The UUID for a statistic.
    pub statistic: ModelUuid,
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

/// View a statistic
///
/// View a statistic for a project.
/// If the project is public, then the user does not need to be authenticated.
/// If the project is private, then the user must be authenticated and have `view` permissions for the project.
/// The statistics returned by this endpoint may no longer be active for their threshold,
/// if their threshold has been updated with a new statistic.
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

    conn_lock!(context, |conn| schema::model::table
        .inner_join(
            schema::threshold::table.on(schema::model::threshold_id.eq(schema::threshold::id)),
        )
        .filter(schema::threshold::project_id.eq(query_project.id))
        .filter(schema::model::uuid.eq(path_params.statistic))
        .select(QueryModel::as_select())
        .first(conn)
        .map_err(resource_not_found_err!(
            Statistic,
            (&query_project, path_params.statistic)
        ))?
        .into_json(conn))
}
