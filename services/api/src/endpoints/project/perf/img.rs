use bencher_json::{project::perf::JsonPerfQueryParams, JsonEmpty, JsonPerfQuery, ResourceId};

use dropshot::{endpoint, HttpError, Path, Query, RequestContext};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::{
    context::Context,
    endpoints::{
        endpoint::{pub_response_ok, response_ok, ResponseOk},
        Endpoint, Method,
    },
    model::project::QueryProject,
    model::user::auth::AuthUser,
    util::cors::{get_cors, CorsResponse},
    ApiError,
};

use super::Resource;

const PERF_IMG_RESOURCE: Resource = Resource::PerfImg;

#[derive(Deserialize, JsonSchema)]
pub struct DirPath {
    pub project: ResourceId,
}

#[allow(clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/perf/img",
    tags = ["projects", "perf"]
}]
pub async fn options(
    _rqctx: RequestContext<Context>,
    _path_params: Path<DirPath>,
    _query_params: Query<JsonPerfQueryParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/perf/img",
    tags = ["projects", "perf"]
}]
pub async fn get(
    rqctx: RequestContext<Context>,
    path_params: Path<DirPath>,
    query_params: Query<JsonPerfQueryParams>,
) -> Result<ResponseOk<JsonEmpty>, HttpError> {
    // Second round of marshaling
    let json_perf_query = query_params
        .into_inner()
        .try_into()
        .map_err(ApiError::from)?;

    let auth_user = AuthUser::new(&rqctx).await.ok();
    let endpoint = Endpoint::new(PERF_IMG_RESOURCE, Method::GetLs);

    let json = get_inner(
        rqctx.context(),
        path_params.into_inner(),
        json_perf_query,
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

async fn get_inner(
    context: &Context,
    path_params: DirPath,
    json_perf_query: JsonPerfQuery,
    auth_user: Option<&AuthUser>,
) -> Result<JsonEmpty, ApiError> {
    let api_context = &mut *context.lock().await;
    let _project_id =
        QueryProject::is_allowed_public(api_context, &path_params.project, auth_user)?.id;

    let path = format!("/v0/projects/{}/perf", path_params.project);
    let _url = json_perf_query.to_url(api_context.endpoint.as_ref(), &path)?;

    // TODO call Selfie.capture_perf

    Ok(JsonEmpty {})
}
