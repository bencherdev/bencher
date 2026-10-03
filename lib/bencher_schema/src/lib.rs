#[cfg(test)]
use criterion as _;
use diesel::{RunQueryDsl as _, connection::SimpleConnection as _};
use diesel_migrations::{
    EmbeddedMigrations, HarnessWithOutput, MigrationHarness as _, embed_migrations,
};
// Linked for the bundled SQLite amalgamation, which the migrations need for `jsonb()`.
use libsqlite3_sys as _;

pub mod context;
pub mod error;
pub mod macros;
pub mod model;
pub mod schema;
#[cfg(test)]
pub mod test_util;

pub use context::ApiContext;
#[cfg(feature = "plus")]
pub use context::{HeaderMap, RateLimiting};

/// The migrations this binary carries.
///
/// Public so that tests can drive them with `MigrationHarness` directly: putting a
/// database back into the shape a migration found it in, and reading it with the
/// same binary, is how a migration's claim to leave responses unchanged is checked.
pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("./migrations");

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
    use std::collections::{BTreeMap, BTreeSet};

    use diesel::{Connection as _, RunQueryDsl as _, connection::SimpleConnection as _};

    use super::{cache_size, run_migrations};

    #[derive(diesel::QueryableByName)]
    struct TableName {
        #[diesel(sql_type = diesel::sql_types::Text)]
        name: String,
    }

    #[derive(diesel::QueryableByName)]
    struct KeyColumn {
        #[diesel(sql_type = diesel::sql_types::Text)]
        table: String,
        #[diesel(sql_type = diesel::sql_types::Text)]
        key: String,
        #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Text>)]
        column: Option<String>,
    }

    // The planner runs on its structural heuristics: a statistics table left behind
    // by `ANALYZE` would steer it into scanning instead of using the indexes.
    #[test]
    fn migrations_leave_no_planner_statistics_behind() {
        let mut conn = diesel::SqliteConnection::establish(":memory:")
            .expect("Failed to create an in-memory database");

        run_migrations(&mut conn).expect("Failed to run migrations");

        let statistics_tables: Vec<String> =
            diesel::sql_query("SELECT name FROM sqlite_master WHERE name LIKE 'sqlite_stat%'")
                .load::<TableName>(&mut conn)
                .expect("Failed to list the statistics tables")
                .into_iter()
                .map(|table| table.name)
                .collect();
        assert!(
            statistics_tables.is_empty(),
            "the migrations leave no statistics tables behind, found {statistics_tables:?}"
        );
    }

    // Deleting a parent row looks up its children in every table that references it,
    // which walks the whole table unless the child columns lead the rowid alias or an index.
    #[test]
    fn migrations_index_every_foreign_key() {
        let mut conn = diesel::SqliteConnection::establish(":memory:")
            .expect("Failed to create an in-memory database");

        run_migrations(&mut conn).expect("Failed to run migrations");

        let foreign_keys = key_columns(
            &mut conn,
            r#"SELECT m.name AS "table", CAST(f.id AS TEXT) AS "key", f."from" AS "column"
                FROM sqlite_master AS m, pragma_foreign_key_list(m.name) AS f
                WHERE m.type = 'table'
                    AND m.name NOT LIKE 'sqlite_%'
                    AND m.name != '__diesel_schema_migrations'
                ORDER BY m.name, f.id, f.seq"#,
        );
        let indexes = key_columns(
            &mut conn,
            r#"SELECT m.name AS "table", i.name AS "key", c.name AS "column", c.seqno AS position
                FROM sqlite_master AS m,
                    pragma_index_list(m.name) AS i,
                    pragma_index_info(i.name) AS c
                WHERE m.type = 'table' AND i.partial = 0
                UNION ALL
                SELECT m.name, 'rowid', c.name, 0
                FROM sqlite_master AS m, pragma_table_info(m.name) AS c
                WHERE m.type = 'table'
                    AND c.pk = 1
                    AND upper(c.type) = 'INTEGER'
                    AND NOT EXISTS (SELECT 1 FROM pragma_table_info(m.name) WHERE pk > 1)
                ORDER BY "table", "key", position"#,
        );

        let unindexed: BTreeSet<String> = foreign_keys
            .iter()
            .filter(|((table, _), columns)| {
                !indexes.iter().any(|((indexed_table, _), indexed_columns)| {
                    indexed_table == table && leads(indexed_columns, columns)
                })
            })
            .map(|((table, _), columns)| {
                let columns: Vec<&str> = columns.iter().flatten().map(String::as_str).collect();
                format!("{table}.{}", columns.join(", "))
            })
            .collect();
        assert!(
            unindexed.is_empty(),
            "every foreign key's columns lead an index, these do not: {unindexed:?}"
        );
    }

    /// The columns of every key the query reads, in key order, by table and key.
    fn key_columns(
        conn: &mut diesel::SqliteConnection,
        query: &str,
    ) -> BTreeMap<(String, String), Vec<Option<String>>> {
        let mut keys = BTreeMap::<_, Vec<_>>::new();
        for KeyColumn { table, key, column } in diesel::sql_query(query)
            .load::<KeyColumn>(conn)
            .expect("Failed to read the keys")
        {
            keys.entry((table, key)).or_default().push(column);
        }
        keys
    }

    /// Whether the index leads with exactly the key's columns, in any order.
    fn leads(index: &[Option<String>], key: &[Option<String>]) -> bool {
        index.get(..key.len()).is_some_and(|leading| {
            let mut leading = leading.to_vec();
            let mut key = key.to_vec();
            leading.sort_unstable();
            key.sort_unstable();
            leading == key
        })
    }

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
