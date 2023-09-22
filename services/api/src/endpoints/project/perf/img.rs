use bencher_json::{project::perf::JsonPerfQueryParams, JsonPerfQuery};
use bencher_plot::LinePlot;
use dropshot::{endpoint, HttpError, Path, Query, RequestContext};
use http::{Response, StatusCode};
use hyper::Body;

use crate::{
    context::ApiContext,
    endpoints::{Endpoint, Method},
    model::user::auth::AuthUser,
    util::cors::{get_cors, CorsResponse},
    ApiError,
};

use super::{ProjPerfParams, Resource};

const PERF_IMG_RESOURCE: Resource = Resource::PerfImg;

#[allow(clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/perf/img",
    tags = ["projects", "perf"]
}]
pub async fn proj_perf_img_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<ProjPerfParams>,
    _query_params: Query<JsonPerfQueryParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<ApiContext>())
}

#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/perf/img",
    tags = ["projects", "perf"]
}]
pub async fn proj_perf_img_get(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<ProjPerfParams>,
    query_params: Query<JsonPerfQueryParams>,
) -> Result<Response<Body>, HttpError> {
    let mut json_perf_query_params = query_params.into_inner();
    let title = json_perf_query_params.title.take();
    // Second round of marshaling
    let json_perf_query = json_perf_query_params.try_into().map_err(ApiError::from)?;

    let auth_user = AuthUser::new(&rqctx).await.ok();
    let endpoint = Endpoint::new(PERF_IMG_RESOURCE, Method::GetLs);

    let jpeg = get_inner(
        rqctx.context(),
        path_params.into_inner(),
        title.as_deref(),
        json_perf_query,
        auth_user.as_ref(),
    )
    .await
    .map_err(|e| {
        if let ApiError::HttpError(e) = e {
            e
        } else {
            endpoint.err(e).into()
        }
    })?;

    Response::builder()
        .status(StatusCode::OK)
        .header(http::header::CONTENT_TYPE, "image/jpeg")
        .header(http::header::CACHE_CONTROL, "private, max-age=0, no-cache")
        .body(jpeg.into())
        .map_err(Into::into)
}

async fn get_inner(
    context: &ApiContext,
    path_params: ProjPerfParams,
    title: Option<&str>,
    json_perf_query: JsonPerfQuery,
    auth_user: Option<&AuthUser>,
) -> Result<Vec<u8>, ApiError> {
    let json_perf = super::get_inner(context, path_params, json_perf_query, auth_user).await?;
    LinePlot::new().draw(title, &json_perf).map_err(Into::into)
}
