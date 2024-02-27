use bencher_json::{
    project::perf::{JsonPerfImgQueryParams, JsonPerfQueryParams},
    JsonPerfQuery,
};
use bencher_plot::LinePlot;
use dropshot::{endpoint, HttpError, Path, Query, RequestContext};
use http::{Response, StatusCode};
use hyper::Body;

use crate::{
    context::ApiContext,
    endpoints::{
        endpoint::{CorsResponse, Get},
        Endpoint,
    },
    error::{bad_request_error, issue_error},
    model::user::auth::{AuthUser, PubBearerToken},
};

use super::ProjPerfParams;

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
    Ok(Endpoint::cors(&[Get.into()]))
}

/// Generate a dynamic image of performance metrics for a project
///
/// Generate a dynamic image of performance metrics for a project.
/// The query results are every permutation of each branch, testbed, benchmark, and measure.
/// There is a limit of 8 permutations for a single image.
/// Therefore, only the first 8 permutations are plotted.
#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/perf/img",
    tags = ["projects", "perf"]
}]
pub async fn proj_perf_img_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: PubBearerToken,
    path_params: Path<ProjPerfParams>,
    query_params: Query<JsonPerfImgQueryParams>,
) -> Result<Response<Body>, HttpError> {
    let mut json_perf_img_query_params = query_params.into_inner();
    let title = json_perf_img_query_params.title.take();
    let json_perf_query_params: JsonPerfQueryParams = json_perf_img_query_params.into();
    // Second round of marshaling
    let json_perf_query = json_perf_query_params
        .try_into()
        .map_err(bad_request_error)?;

    let auth_user = AuthUser::from_pub_token(rqctx.context(), bearer_token).await?;
    let jpeg = get_inner(
        rqctx.context(),
        path_params.into_inner(),
        title.as_deref(),
        json_perf_query,
        auth_user.as_ref(),
    )
    .await?;

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
) -> Result<Vec<u8>, HttpError> {
    let json_perf = super::get_inner(context, path_params, json_perf_query, auth_user).await?;
    LinePlot::new().draw(title, &json_perf).map_err(|e| {
        issue_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to draw perf plot",
            &format!("Failed draw perf plot: {json_perf:?}"),
            e,
        )
    })
}
