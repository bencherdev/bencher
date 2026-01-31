//! OCI Context Helpers
//!
//! This module provides access to OCI storage from request contexts.
//! Currently uses a placeholder; will be properly integrated with `ApiContext`.

use std::sync::OnceLock;

use crate::storage::{OciStorage, OciStorageError};

/// Global OCI storage instance
/// TODO: Properly integrate with `ApiContext` to avoid global state
static OCI_STORAGE: OnceLock<OciStorage> = OnceLock::new();

/// Initialize the global OCI storage
pub fn init_storage(storage: OciStorage) -> Result<(), OciStorage> {
    OCI_STORAGE.set(storage)
}

/// Get a reference to the OCI storage
/// Returns None if OCI storage is not configured
pub fn get_storage() -> Option<&'static OciStorage> {
    OCI_STORAGE.get()
}

/// Get a reference to the OCI storage, returning an error if not configured
pub fn storage() -> Result<&'static OciStorage, OciStorageError> {
    get_storage().ok_or_else(|| OciStorageError::Config("OCI storage not configured".to_owned()))
}
