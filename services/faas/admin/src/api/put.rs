use std::{
    io::BufWriter,
    sync::{
        Arc,
        Mutex,
    },
};

use diesel::pg::PgConnection;
use diesel_migrations::embed_migrations;
use dropshot::{
    endpoint,
    HttpError,
    HttpResponseHeaders,
    HttpResponseOk,
    RequestContext,
};
use util::server::headers::CorsHeaders;

embed_migrations!("../util/migrations");

#[endpoint {
    method = PUT,
    path = "/v0/admin/migrate",
    tags = ["admin"]
}]
pub async fn api_put_admin_migrate(
    rqctx: Arc<RequestContext<Mutex<PgConnection>>>,
) -> Result<HttpResponseHeaders<HttpResponseOk<String>, CorsHeaders>, HttpError> {
    let db_connection = rqctx.context();

    if let Ok(db_conn) = db_connection.lock() {
        let db_conn = &*db_conn;

        let mut output = BufWriter::new(Vec::new());
        embedded_migrations::run_with_output(db_conn, &mut output).map_err(|e| {
            HttpError::for_bad_request(
                Some(String::from("BadInput")),
                format!("Failed to run migration: {e}"),
            )
        })?;

        let bytes = output.into_inner().map_err(|e| {
            HttpError::for_bad_request(
                Some(String::from("BadInput")),
                format!("Failed to run migration: {e}"),
            )
        })?;
        let contents = String::from_utf8(bytes).map_err(|e| {
            HttpError::for_bad_request(
                Some(String::from("BadInput")),
                format!("Failed to run migration: {e}"),
            )
        })?;

        let resp =
            HttpResponseHeaders::new(HttpResponseOk(contents), CorsHeaders::new_pub("PUT".into()));

        Ok(resp)
    } else {
        Err(HttpError::for_bad_request(
            Some(String::from("BadInput")),
            format!("Failed to run query"),
        ))
    }
}
