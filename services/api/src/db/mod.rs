use std::env;

use diesel::{
    prelude::{
        ConnectionResult,
        *,
    },
    sqlite::SqliteConnection,
};

pub mod model;
pub mod schema;

const BENCHER_DB_URL: &str = "BENCHER_DB_URL";
const DEFAULT_DB_URL: &str = "bencher.db";

pub fn get_db_connection() -> ConnectionResult<SqliteConnection> {
    let database_url = env::var(BENCHER_DB_URL).unwrap_or(DEFAULT_DB_URL.into());
    SqliteConnection::establish(&database_url)
}
