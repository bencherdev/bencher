#![allow(clippy::result_large_err)]

use bencher_json::JsonSpec;
use std::sync::LazyLock;

pub mod config;
pub mod endpoints;
mod macros;

pub const API_VERSION: &str = env!("CARGO_PKG_VERSION");

pub const SPEC_STR: &str = include_str!("../openapi.json");
#[allow(clippy::expect_used)]
pub static SPEC: LazyLock<JsonSpec> =
    LazyLock::new(|| JsonSpec(SPEC_STR.parse().expect("Failed to parse OpenAPI spec")));
