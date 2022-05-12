use diesel::pg::PgConnection;
use diesel::prelude::ConnectionResult;
use diesel::prelude::*;
use std::env;

pub fn get_db_connection() -> ConnectionResult<PgConnection> {
    let username = env::var("DB_USER").unwrap_or("postgres".into());
    let password = env::var("DB_PASSWORD").unwrap_or("postgres".into());
    let host = env::var("DB_HOST").unwrap_or("localhost".into());
    let db_name = env::var("DB_NAME").unwrap_or("bencher".into());
    let url = postgres_url(&username, &password, &host, &db_name);
    PgConnection::establish(&url)
}

fn postgres_url(username: &str, password: &str, host: &str, db_name: &str) -> String {
    format!("postgres://{username}:{password}@{host}/{db_name}")
}
