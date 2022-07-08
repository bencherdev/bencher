use std::env;

use diesel::pg::PgConnection;
use diesel::prelude::ConnectionResult;
use diesel::prelude::*;

pub mod model;
pub mod schema;

const BENCHER_DB_URL: &str = "BENCHER_DB_URL";
const DEFAULT_DB_URL: &str = "postgres://postgres:postgres@localhost:5432/bencher";

pub fn get_db_connection() -> ConnectionResult<PgConnection> {
    let database_url = env::var(BENCHER_DB_URL).unwrap_or(DEFAULT_DB_URL.into());
    PgConnection::establish(&database_url)
}
