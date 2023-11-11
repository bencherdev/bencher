#![allow(clippy::result_large_err)]
#![recursion_limit = "512"]

use bencher_json::JsonSpec;
use once_cell::sync::Lazy;

pub mod config;
pub mod context;
pub mod endpoints;
pub mod error;
pub mod model;
#[allow(unused_qualifications)]
pub mod schema;
pub mod util;

pub const API_VERSION: &str = env!("CARGO_PKG_VERSION");
// This is run via a `pre-push` git hook
// So if the `SWAGGER_PATH` below is ever updated
// also update `./git/hooks/pre-push` accordingly.
#[cfg(feature = "swagger")]
pub const SWAGGER_PATH: &str = "../console/src/content/api/swagger.json";
pub const SWAGGER_SPEC_STR: &str = include_str!("../../console/src/content/api/swagger.json");
#[allow(clippy::expect_used)]
pub static SWAGGER_SPEC: Lazy<JsonSpec> = Lazy::new(|| {
    JsonSpec(
        SWAGGER_SPEC_STR
            .parse()
            .expect("Failed to parse OpenAPI spec"),
    )
});
