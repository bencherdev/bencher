#![allow(clippy::result_large_err)]

pub mod config;
pub mod context;
pub mod endpoints;
pub mod error;
pub mod model;
#[allow(unused_qualifications)]
pub mod schema;
pub mod util;

pub use error::{ApiError, WordStr};

pub const API_VERSION: &str = env!("CARGO_PKG_VERSION");
// This is run via a `pre-push` git hook
// So if the `SWAGGER_PATH` below is ever updated
// also update `./git/hooks/pre-push` accordingly.
pub const SWAGGER_PATH: &str = "../console/src/content/api/swagger.json";
pub const SWAGGER_SPEC: &str = include_str!("../../console/src/content/api/swagger.json");
