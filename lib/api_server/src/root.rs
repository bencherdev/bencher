#[cfg(feature = "plus")]
use bencher_endpoint::Post;
use bencher_endpoint::{CorsResponse, Endpoint, Get, ResponseOk};
use bencher_schema::context::ApiContext;
use dropshot::{HttpError, RequestContext, endpoint};


#[endpoint {
    method = OPTIONS,
    path =  "/",
    tags = ["server"]
}]
pub async fn server_root_options(
    _rqctx: RequestContext<ApiContext>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[
        Get.into(),
        // TODO remove in due time
        // Due to a bug in the original server stats implementation,
        // the endpoint was set to the API server root path
        // instead of the `/v0/server/stats` path.
        #[cfg(feature = "plus")]
        Post.into(),
    ]))
}


#[endpoint {
    method = GET,
    path = "/",
    tags = ["server"]
}]
pub async fn server_root_get(
    _rqctx: RequestContext<ApiContext>,
) -> Result<ResponseOk<()>, HttpError> {
    Ok(Get::pub_response_ok(()))
}
