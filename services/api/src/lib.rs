#![feature(let_chains)]
#[macro_use]
extern crate diesel;

pub mod db;
pub mod endpoints;
pub mod util;

const BENCHER_SECRET_KEY: &str = "BENCHER_SECRET_KEY";

lazy_static::lazy_static! {
    pub static ref SECRET_KEY: String = std::env::var(BENCHER_SECRET_KEY).unwrap_or_else(|e| {
        tracing::info!("Failed to find \"{BENCHER_SECRET_KEY}\": {e}");
        let secret_key = uuid::Uuid::new_v4().to_string();
        tracing::info!("Generated temporary secret key: {secret_key}");
        secret_key
    });
}
