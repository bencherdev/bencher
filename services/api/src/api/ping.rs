use std::sync::Arc;

use diesel::sqlite::SqliteConnection;
use dropshot::{
    endpoint,
    HttpError,
    HttpResponseHeaders,
    HttpResponseOk,
    RequestContext,
};
use tokio::sync::Mutex;

use crate::api::headers::CorsHeaders;

#[endpoint {
    method = GET,
    path = "/",
    tags = ["ping"]
}]
pub async fn api_get_ping(
    rqctx: Arc<RequestContext<Mutex<SqliteConnection>>>,
) -> Result<HttpResponseHeaders<HttpResponseOk<String>, CorsHeaders>, HttpError> {
    let db_connection = rqctx.context();

    let _conn = db_connection.lock().await;
    let resp = HttpResponseHeaders::new(
        HttpResponseOk("PONG".into()),
        CorsHeaders::new_origin_all("GET".into(), "Content-Type".into()),
    );

    Ok(resp)
}
