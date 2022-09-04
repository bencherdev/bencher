use std::env;

use diesel::{
    prelude::{
        ConnectionResult,
        *,
    },
    sqlite::SqliteConnection,
};

const BENCHER_DB: &str = "BENCHER_DB";
const DEFAULT_DB: &str = "bencher.db";

pub fn get_db_connection() -> ConnectionResult<SqliteConnection> {
    let database_url = env::var(BENCHER_DB).unwrap_or(DEFAULT_DB.into());
    SqliteConnection::establish(&database_url)
}
