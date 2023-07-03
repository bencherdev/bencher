use bencher_json::{project::threshold::JsonStatistic, ResourceId};
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
use dropshot::{endpoint, HttpError, Path, RequestContext};
use schemars::JsonSchema;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    context::ApiContext,
    endpoints::{
        endpoint::{pub_response_ok, response_ok, ResponseOk},
        Endpoint, Method,
    },
    error::api_error,
    model::project::{threshold::statistic::QueryStatistic, QueryProject},
    model::user::auth::AuthUser,
    schema,
    util::cors::{get_cors, CorsResponse},
    ApiError,
};

use super::Resource;

const STATISTIC_RESOURCE: Resource = Resource::Statistic;

#[derive(Deserialize, JsonSchema)]
pub struct ProjStatisticParams {
    pub project: ResourceId,
    pub statistic: Uuid,
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
    Ok(get_cors::<ApiContext>())
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
    let endpoint = Endpoint::new(STATISTIC_RESOURCE, Method::GetOne);

    let json = get_one_inner(
        rqctx.context(),
        path_params.into_inner(),
        auth_user.as_ref(),
    )
    .await
    .map_err(|e| endpoint.err(e))?;

    if auth_user.is_some() {
        response_ok!(endpoint, json)
    } else {
        pub_response_ok!(endpoint, json)
    }
}

async fn get_one_inner(
    context: &ApiContext,
    path_params: ProjStatisticParams,
    auth_user: Option<&AuthUser>,
) -> Result<JsonStatistic, ApiError> {
    let conn = &mut *context.conn().await;

    let query_project =
        QueryProject::is_allowed_public(conn, &context.rbac, &path_params.project, auth_user)?;

    schema::statistic::table
        .filter(schema::statistic::project_id.eq(query_project.id))
        .filter(schema::statistic::uuid.eq(path_params.statistic.to_string()))
        .first::<QueryStatistic>(conn)
        .map_err(api_error!())?
        .into_json(conn)
}
