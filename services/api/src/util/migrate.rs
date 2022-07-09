use std::io::BufWriter;

use diesel::sqlite::SqliteConnection;
use diesel_migrations::embed_migrations;
use dropshot::HttpError;

embed_migrations!("./migrations");

pub fn run_migration(conn: &SqliteConnection) -> Result<(), HttpError> {
    let mut output = BufWriter::new(Vec::new());
    embedded_migrations::run_with_output(conn, &mut output).map_err(|e| {
        HttpError::for_bad_request(
            Some(String::from("BadInput")),
            format!("Failed to run migration: {e}"),
        )
    })
}
