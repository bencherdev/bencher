use std::sync::Arc;

use dropshot::{
    endpoint,
    HttpError,
    HttpResponseHeaders,
    HttpResponseOk,
    RequestContext,
};

use crate::util::{
    headers::CorsHeaders,
    Context,
};

#[endpoint {
    method = GET,
    path = "/v0/ping",
    tags = ["ping"]
}]
pub async fn api_get_ping(
    rqctx: Arc<RequestContext<Context>>,
) -> Result<HttpResponseHeaders<HttpResponseOk<String>, CorsHeaders>, HttpError> {
    let api_context = rqctx.context();

    let api_context = &mut *api_context.lock().await;
    let _conn = &mut api_context.db;

    let resp = HttpResponseHeaders::new(
        HttpResponseOk("PONG".into()),
        CorsHeaders::new_pub("GET".into()),
    );

    Ok(resp)
}
