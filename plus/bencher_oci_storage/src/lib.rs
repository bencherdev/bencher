#![cfg(feature = "plus")]

//! Bencher OCI Registry - A Bencher Plus Feature
//!
//! This module provides the OCI Distribution Spec compliant container registry
//! storage implementation and types.
//!
//! ## Storage Backends
//!
//! Two storage backends are supported:
//! - **S3**: For production deployments with scalability and cross-instance consistency
//! - **Local**: For development and single-instance deployments (stores files next to database)

// Reference dev-dependency used only in integration tests to silence unused_crate_dependencies warning
#[cfg(test)]
use reqwest as _;

mod error;
mod local;
mod storage;
mod types;

pub use bencher_json::ProjectUuid;
pub use bencher_json::system::config::DEFAULT_MAX_BODY_SIZE;
pub use error::OciError;
pub use storage::{BlobBody, ListTagsResult, OciStorage, OciStorageError};
pub use types::{Digest, DigestError, Reference, Tag, UploadId};
