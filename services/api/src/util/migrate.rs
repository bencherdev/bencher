use diesel::sqlite::SqliteConnection;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};

use crate::ApiError;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("./migrations");

pub fn run_migrations(conn: &mut SqliteConnection) -> Result<(), ApiError> {
    conn.run_pending_migrations(MIGRATIONS)
        .map_err(ApiError::Migrations)?;
    Ok(())
}
