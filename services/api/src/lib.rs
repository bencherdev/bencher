#![allow(clippy::result_large_err)]

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
#[allow(unused_qualifications)]
pub mod view;

pub const API_VERSION: &str = env!("CARGO_PKG_VERSION");

pub const SPEC_STR: &str = include_str!("../openapi.json");
#[allow(clippy::expect_used)]
pub static SPEC: Lazy<JsonSpec> =
    Lazy::new(|| JsonSpec(SPEC_STR.parse().expect("Failed to parse OpenAPI spec")));
