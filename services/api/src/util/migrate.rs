use diesel::sqlite::SqliteConnection;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use dropshot::HttpError;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("./migrations");

pub fn run_migration(conn: &mut SqliteConnection) -> Result<(), HttpError> {
    conn.run_pending_migrations(MIGRATIONS).map_err(|e| {
        HttpError::for_bad_request(
            Some(String::from("BadInput")),
            format!("Failed to run migration: {e}"),
        )
    })?;
    Ok(())
}
