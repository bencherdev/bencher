#![allow(clippy::result_large_err)]

// Needed for distroless builds
use libsqlite3_sys as _;
// Needed for setting default provider
use tokio_rustls as _;
// Needed for binary
use bencher_logger as _;
use serde_yaml as _;

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
