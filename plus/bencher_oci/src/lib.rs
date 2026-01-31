#![cfg(feature = "plus")]

//! Bencher OCI Registry - A Bencher Plus Feature
//!
//! This module implements an OCI Distribution Spec compliant container registry
//! that integrates with the Bencher API server.

// Reference dev-dependency used only in integration tests to silence unused_crate_dependencies warning
#[cfg(test)]
use reqwest as _;

mod context;
mod endpoints;
mod error;
mod storage;
mod types;

pub use context::{get_storage, init_storage};
pub use endpoints::{register, Api};
pub use error::OciError;
pub use storage::{OciStorage, OciStorageConfig, OciStorageError};
pub use types::{Digest, Reference, RepositoryName, Tag, UploadId};
