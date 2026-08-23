#[cfg(test)]
use criterion as _;
use diesel::{RunQueryDsl as _, connection::SimpleConnection as _};
use diesel_migrations::{
    EmbeddedMigrations, HarnessWithOutput, MigrationHarness as _, embed_migrations,
};

pub mod context;
pub mod error;
pub mod macros;
pub mod model;
pub mod schema;
#[cfg(test)]
pub mod test_util;
pub mod view;

pub use context::ApiContext;
#[cfg(feature = "plus")]
pub use context::{HeaderMap, RateLimiting};

const MIGRATIONS: EmbeddedMigrations = embed_migrations!("./migrations");

// TODO Custom max TTL
pub const INVITE_TOKEN_TTL: u32 = u32::MAX;
pub const CLAIM_TOKEN_TTL: u32 = 60;

/// The page cache the migrations run with, in `SQLite`'s negative kibibyte form:
/// 256 MiB.
///
/// A migration that rebuilds a table walks it through this cache, and the cache
/// the connection serves from is sized for serving. Measured on the
/// `report_benchmark` rebuild over 4 million synthetic rows, 64 MiB costs
/// 3,022,663 page reads and writes and 41.7s where 256 MiB costs 385,721 and
/// 18.2s. The ratio of cache to table only worsens as a table grows, so the
/// measured gap is a floor. The window is attended and nothing else is running,
/// so the memory is there to spend and it is given back below.
const MIGRATION_CACHE_SIZE: i64 = -256 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum MigrationError {
    #[error("Failed to run database migrations: {0}")]
    Migrations(Box<dyn std::error::Error + Send + Sync>),
    #[error("Failed to run database pragma off: {0}")]
    PragmaOff(diesel::result::Error),
    #[error("Failed to run database pragma on: {0}")]
    PragmaOn(diesel::result::Error),
    #[error("Failed to read the database cache size: {0}")]
    CacheSize(diesel::result::Error),
    #[error("Failed to set the database cache size: {0}")]
    SetCacheSize(diesel::result::Error),
}

#[derive(diesel::QueryableByName)]
struct CacheSize {
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    cache_size: i64,
}

/// The connection's current `cache_size`, in whatever form it was set in.
fn cache_size(database: &mut context::DbConnection) -> Result<i64, MigrationError> {
    diesel::sql_query("PRAGMA cache_size")
        .get_result::<CacheSize>(database)
        .map(|pragma| pragma.cache_size)
        .map_err(MigrationError::CacheSize)
}

fn set_cache_size(
    database: &mut context::DbConnection,
    cache_size: i64,
) -> Result<(), MigrationError> {
    database
        .batch_execute(&format!("PRAGMA cache_size = {cache_size}"))
        .map_err(MigrationError::SetCacheSize)
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

    // The serving cache size is read before it is replaced, so that whatever the
    // connection was configured with is what it goes back to, on the way out and on
    // the way out of a failure alike.
    let serving_cache_size = cache_size(database)?;
    set_cache_size(database, MIGRATION_CACHE_SIZE)?;

    // `HarnessWithOutput` writes "Running migration <name>" as each one starts. A
    // migration that rebuilds a table can hold the window for a long time, and a
    // line per migration is the difference between watching it and guessing at it.
    let migrated = HarnessWithOutput::write_to_stdout(database)
        .run_pending_migrations(MIGRATIONS)
        .map(|_| ())
        .map_err(MigrationError::Migrations);
    // Restored before the migration result is unwrapped, so a failed migration does
    // not leave the connection serving from the migration window's cache.
    let restored = set_cache_size(database, serving_cache_size);
    migrated?;
    restored?;

    database
        .batch_execute("PRAGMA foreign_keys = ON")
        .map_err(MigrationError::PragmaOn)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use diesel::{Connection as _, connection::SimpleConnection as _};

    use super::{cache_size, run_migrations};

    // The connection that runs the migrations is the connection the API then serves
    // from, so the window's cache size has to be handed back when the window closes.
    #[test]
    fn migrations_leave_the_cache_size_where_they_found_it() {
        let mut conn = diesel::SqliteConnection::establish(":memory:")
            .expect("Failed to create an in-memory database");
        conn.batch_execute("PRAGMA cache_size = -8192")
            .expect("Failed to set the serving cache size");

        run_migrations(&mut conn).expect("Failed to run migrations");

        assert_eq!(
            cache_size(&mut conn).expect("Failed to read the cache size"),
            -8192,
            "the migrations give back the cache size they were handed"
        );
    }
}
