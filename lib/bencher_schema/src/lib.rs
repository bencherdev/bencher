#![allow(clippy::result_large_err)]

use diesel::connection::SimpleConnection;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};

pub mod context;
pub mod error;
pub mod macros;
pub mod model;
#[allow(unused_qualifications)]
pub mod schema;
#[allow(unused_qualifications)]
pub mod view;

pub use context::ApiContext;

pub const API_VERSION: &str = env!("CARGO_PKG_VERSION");

const MIGRATIONS: EmbeddedMigrations = embed_migrations!("./migrations");

// TODO Custom max TTL
pub const INVITE_TOKEN_TTL: u32 = u32::MAX;
pub const CLAIM_TOKEN_TTL: u32 = 60;

#[derive(Debug, thiserror::Error)]
pub enum MigrationError {
    #[error("Failed to run database migrations: {0}")]
    Migrations(Box<dyn std::error::Error + Send + Sync>),
    #[error("Failed to run database pragma off: {0}")]
    PragmaOff(diesel::result::Error),
    #[error("Failed to run database pragma on: {0}")]
    PragmaOn(diesel::result::Error),
}

pub fn run_migrations(database: &mut context::DbConnection) -> Result<(), MigrationError> {
    // It is not possible to enable or disable foreign key constraints in the middle of a multi-statement transaction
    // (when SQLite is not in autocommit mode).
    // Attempting to do so does not return an error; it simply has no effect.
    // https://www.sqlite.org/foreignkeys.html#fk_enable
    // Therefore, we must run all migrations with foreign key constraints disabled.
    // Still use `PRAGMA foreign_keys = OFF` in the migration scripts to disable foreign key constraints when using the CLI.
    database
        .batch_execute("PRAGMA foreign_keys = OFF")
        .map_err(MigrationError::PragmaOff)?;
    database
        .run_pending_migrations(MIGRATIONS)
        .map(|_| ())
        .map_err(MigrationError::Migrations)?;
    database
        .batch_execute("PRAGMA foreign_keys = ON")
        .map_err(MigrationError::PragmaOn)?;

    Ok(())
}
