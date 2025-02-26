#![allow(clippy::result_large_err)]

pub mod context;
pub mod error;
pub mod headers;
pub mod macros;
pub mod model;
#[allow(unused_qualifications)]
pub mod schema;
#[allow(unused_qualifications)]
pub mod view;

pub const API_VERSION: &str = env!("CARGO_PKG_VERSION");
