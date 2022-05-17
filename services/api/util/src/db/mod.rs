use diesel::pg::PgConnection;
use diesel::prelude::ConnectionResult;
use diesel::prelude::*;
use std::env;

pub mod model;
pub mod schema;

pub fn get_db_connection() -> ConnectionResult<PgConnection> {
    let database_url =
        env::var("DATABASE_URL").unwrap_or("postgres://postgres:postgres@localhost:5432/bencher".into());
    PgConnection::establish(&database_url)
}
