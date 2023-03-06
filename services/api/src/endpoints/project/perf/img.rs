use bencher_json::{project::perf::JsonPerfQueryParams, JsonPerfQuery, ResourceId};

use bencher_selfie::Selfie;
use dropshot::{endpoint, HttpError, Path, Query, RequestContext};
use http::{Response, StatusCode};
use hyper::Body;
use schemars::JsonSchema;
use serde::Deserialize;
use tracing::info;

use crate::{
    context::Context,
    endpoints::{Endpoint, Method},
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
) -> Result<Response<Body>, HttpError> {
    // Second round of marshaling
    let json_perf_query = query_params
        .into_inner()
        .try_into()
        .map_err(ApiError::from)?;

    let endpoint = Endpoint::new(PERF_IMG_RESOURCE, Method::GetLs);

    let jpeg = get_inner(rqctx.context(), path_params.into_inner(), json_perf_query)
        .await
        .map_err(|e| endpoint.err(e))?;

    Response::builder()
        .status(StatusCode::OK)
        .header(http::header::CONTENT_TYPE, "image/jpeg")
        .header(http::header::CACHE_CONTROL, "private, max-age=0, no-cache")
        .body(jpeg.into())
        .map_err(Into::into)
}

async fn get_inner(
    context: &Context,
    path_params: DirPath,
    json_perf_query: JsonPerfQuery,
) -> Result<Vec<u8>, ApiError> {
    let endpoint = context.lock().await.endpoint.clone();
    let path = format!("/perf/{}", path_params.project);
    let url = json_perf_query.to_url(endpoint.as_ref(), &path, &[("img", Some("true".into()))])?;
    info!("{url}");

    // I have no idea why this helps, but it does...
    // Without it the headless chrome request just hangs.
    let _lens_cap = reqwest::get("http://example.com").await;

    Selfie::new_embedded()?
        .capture_perf(url.as_ref())
        .map_err(Into::into)
}
