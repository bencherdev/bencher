#[macro_use]
extern crate diesel;

pub mod config;
pub mod endpoints;
pub mod error;
pub mod model;
pub mod schema;
pub mod util;

pub use error::{ApiError, WordStr};
