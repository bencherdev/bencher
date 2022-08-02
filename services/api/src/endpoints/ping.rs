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
    let db_connection = rqctx.context();

    let _conn = db_connection.lock().await;
    let resp = HttpResponseHeaders::new(
        HttpResponseOk("PONG".into()),
        CorsHeaders::new_pub("GET".into()),
    );

    Ok(resp)
}
