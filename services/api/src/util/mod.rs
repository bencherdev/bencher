pub mod auth;
pub mod error;
pub mod headers;
pub mod migrate;
pub mod registrar;
pub mod server;

pub(crate) use error::http_error;
pub type Context = tokio::sync::Mutex<diesel::SqliteConnection>;
