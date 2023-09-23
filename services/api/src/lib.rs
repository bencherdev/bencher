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
