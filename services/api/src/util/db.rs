use std::env;

use diesel::{prelude::*, sqlite::SqliteConnection};

use crate::ApiError;

const BENCHER_DB: &str = "BENCHER_DB";
const DEFAULT_DB: &str = "bencher.db";

pub fn get_db_connection() -> Result<SqliteConnection, ApiError> {
    let database_url = env::var(BENCHER_DB).unwrap_or(DEFAULT_DB.into());
    Ok(SqliteConnection::establish(&database_url)?)
}
