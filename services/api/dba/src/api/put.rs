use std::io::BufWriter;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

use diesel::pg::PgConnection;
use diesel_migrations::run_pending_migrations_in_directory;
use dropshot::endpoint;
use dropshot::HttpError;
use dropshot::HttpResponseHeaders;
use dropshot::HttpResponseOk;
use dropshot::RequestContext;
use util::server::headers::CorsHeaders;

diesel_migrations::embed_migrations!("../util/migrations");

#[endpoint {
    method = PUT,
    path = "/v0/dba/migrate",
    tags = ["dba"]
}]
pub async fn api_put_dba_migrate(
    rqctx: Arc<RequestContext<Mutex<PgConnection>>>,
) -> Result<HttpResponseHeaders<HttpResponseOk<String>, CorsHeaders>, HttpError> {
    let db_connection = rqctx.context();

    if let Ok(db_conn) = db_connection.lock() {
        let db_conn = &*db_conn;

        let migrations_dir = PathBuf::from("../util/migrations");
        let mut output = BufWriter::new(Vec::new());
        run_pending_migrations_in_directory(db_conn, &migrations_dir, &mut output).map_err(
            |e| {
                HttpError::for_bad_request(
                    Some(String::from("BadInput")),
                    format!("Failed to run migration: {e}"),
                )
            },
        )?;

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

        let resp = HttpResponseHeaders::new(
            HttpResponseOk(contents),
            CorsHeaders::new_origin_all("PUT".into(), "Content-Type".into()),
        );

        Ok(resp)
    } else {
        Err(HttpError::for_bad_request(
            Some(String::from("BadInput")),
            format!("Failed to run query"),
        ))
    }
}
