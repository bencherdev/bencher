use bencher_endpoint::{CorsResponse, Endpoint, Get};
use bencher_schema::{context::ApiContext, error::not_found_error};
use dropshot::{Body, HttpError, RequestContext, endpoint};
use http::{Response, StatusCode, header};

const TEXT_PLAIN_UTF_8: &str = "text/plain; charset=utf-8";
const NOT_CONFIGURED: &str = "No OpenAI apps challenge is configured";

#[endpoint {
    method = OPTIONS,
    path = "/.well-known/openai-apps-challenge",
    unpublished = true,
}]
pub async fn openai_apps_challenge_options(
    rqctx: RequestContext<ApiContext>,
) -> Result<CorsResponse, HttpError> {
    if rqctx.context().openai_apps_challenge.is_none() {
        return Err(not_found_error(NOT_CONFIGURED));
    }
    Ok(Endpoint::cors(&[Get.into()]))
}

#[endpoint {
    method = GET,
    path = "/.well-known/openai-apps-challenge",
    unpublished = true,
}]
pub async fn openai_apps_challenge_get(
    rqctx: RequestContext<ApiContext>,
) -> Result<Response<Body>, HttpError> {
    let Some(challenge) = &rqctx.context().openai_apps_challenge else {
        return Err(not_found_error(NOT_CONFIGURED));
    };
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, TEXT_PLAIN_UTF_8)
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .body(challenge.as_ref().to_owned().into())
        .map_err(Into::into)
}
