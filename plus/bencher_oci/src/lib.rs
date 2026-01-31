#![cfg(feature = "plus")]

//! Bencher OCI Registry - A Bencher Plus Feature
//!
//! This module provides the OCI Distribution Spec compliant container registry
//! storage implementation and types.

// Reference dev-dependency used only in integration tests to silence unused_crate_dependencies warning
#[cfg(test)]
use reqwest as _;

mod error;
mod storage;
mod types;

pub use error::OciError;
pub use storage::{OciStorage, OciStorageConfig, OciStorageError};
pub use types::{Digest, DigestError, Reference, RepositoryName, RepositoryNameError, Tag, UploadId};
