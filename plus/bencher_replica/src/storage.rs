//! Replica storage backends: enum dispatch over local filesystem and
//! S3-compatible object storage (plus a fault-injection wrapper for tests).
//!
//! Semantics pinned here and enforced by the contract test suite
//! (`tests/storage_contract.rs`, run against every backend):
//!
//! - Keys are `/`-separated relative paths with no leading slash, no `.` or
//!   `..` components, and no empty components. The object methods
//!   (`put`, `get`, `get_stream`, `delete`, `start_multipart`) reject anything
//!   else with [`StorageError::InvalidKey`] before touching the backend, so a
//!   crate-public caller cannot escape the storage root. (`list`, `list_dirs`,
//!   and `delete_prefix` take a prefix, which may legitimately end
//!   mid-component, and so are not validated this way.)
//! - `put` is atomically visible AND durable when it returns `Ok`: a reader
//!   never observes a partial object under its final key, and an acknowledged
//!   write survives a power loss (crate invariant I1 gates checkpoints on
//!   frames being durably uploaded). Local: the temp file is fsynced, renamed
//!   into place, and the parent directory is fsynced so the rename itself is
//!   durable; newly created parent directories are fsynced as they are made.
//!   S3: durability is the 200 response to the single PUT (or, for a streaming
//!   upload, to `CompleteMultipartUpload`).
//! - `get` of a missing key returns [`StorageError::NotFound`], never empty
//!   bytes.
//! - `list` returns full keys, sorted lexicographically, with pagination
//!   handled internally. A backend failure is an `Err`, NEVER an empty list:
//!   conflating the two would make an unreachable replica look empty and
//!   trigger a spurious new generation. (S3: a truncated page that carries no
//!   continuation token is an `Err`, never a silently short listing, since the
//!   engine treats LIST as the source of truth per invariant I6.)
//! - `delete` is idempotent: deleting a missing key succeeds.
//!
//! ## Pinned backend divergences
//!
//! These behaviors differ between backends by design. The engine only ever
//! uses directory-aligned prefixes and never relies on the divergent cases,
//! but they are pinned here (and in the backend-specific sections of the
//! contract suite) so they cannot change silently:
//!
//! - `delete_prefix` interprets its argument differently. S3 deletes every
//!   object whose key has the raw string as a prefix, so `delete_prefix("gen")`
//!   would also remove `generations/...`. The local backend treats the prefix
//!   as a directory path and calls `remove_dir_all`, so a prefix that ends
//!   mid-component (`"gen"` when only `generations/` exists) matches no
//!   directory and no-ops. Always pass a directory-aligned prefix.
//! - Local `list` errors when a prefix names a path whose component is a
//!   regular file (e.g. `list("a/")` when `a` is an object); S3 returns an
//!   empty listing.
//! - Local `get` of a key that is actually a directory errors (`EISDIR`,
//!   surfaced as [`StorageError::Local`]); S3 has no directories and returns
//!   [`StorageError::NotFound`].

use bytes::Bytes;

use crate::local::{LocalError, LocalStorage};
use crate::s3::{S3Error, S3Storage};

/// A replica storage backend.
pub enum ReplicaStorage {
    Local(LocalStorage),
    S3(Box<S3Storage>),
    /// Fault-injection wrapper around another backend; test-only.
    #[cfg(any(test, feature = "testing"))]
    Flaky(Box<crate::testing::FlakyStorage>),
}

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    /// The requested key does not exist. Every backend maps its native
    /// missing-object error to this variant (never to an empty result).
    #[error("Object not found: {key}")]
    NotFound { key: String },
    /// An object key was not a valid `/`-separated relative path (see
    /// [`validate_key`]); rejected before touching the backend so a key can
    /// never escape the storage root.
    #[error("Invalid object key ({key}): {reason}")]
    InvalidKey { key: String, reason: &'static str },
    /// Boxed to keep the common `Ok`/`NotFound` paths small.
    #[error("Local replica storage: {0}")]
    Local(#[source] Box<LocalError>),
    /// Boxed to keep the common `Ok`/`NotFound` paths small.
    #[error("S3 replica storage: {0}")]
    S3(#[source] Box<S3Error>),
    /// Injected by the fault-injection test backend.
    #[cfg(any(test, feature = "testing"))]
    #[error("Injected fault: {op} {key}")]
    Injected { op: &'static str, key: String },
}

impl From<LocalError> for StorageError {
    fn from(error: LocalError) -> Self {
        Self::Local(Box::new(error))
    }
}

impl From<S3Error> for StorageError {
    fn from(error: S3Error) -> Self {
        Self::S3(Box::new(error))
    }
}

/// Reject an object key that is not a `/`-separated relative path: a leading
/// slash (which would replace the local root via `Utf8PathBuf::join`), a `.`
/// or `..` component (which would escape the root), or an empty component
/// (a leading, trailing, or doubled slash). Both backends call this before
/// any object operation so the contract holds regardless of who supplied the
/// key.
pub(crate) fn validate_key(key: &str) -> Result<(), StorageError> {
    let reject = |reason| {
        Err(StorageError::InvalidKey {
            key: key.to_owned(),
            reason,
        })
    };
    if key.starts_with('/') {
        return reject("leading slash");
    }
    for component in key.split('/') {
        match component {
            "" => return reject("empty path component"),
            "." | ".." => return reject("relative path component"),
            _ => {},
        }
    }
    Ok(())
}

/// Validate a listing/deletion prefix. Looser than [`validate_key`]: a prefix
/// may be empty (the whole store), may carry a trailing slash, and may end
/// mid-component. Still rejected are dot components and a leading slash: the
/// local backend maps prefixes onto the filesystem, where `..` in a
/// destructive path like `delete_prefix` would escape the storage root.
pub(crate) fn validate_prefix(prefix: &str) -> Result<(), StorageError> {
    if prefix.is_empty() {
        return Ok(());
    }
    let reject = |reason| {
        Err(StorageError::InvalidKey {
            key: prefix.to_owned(),
            reason,
        })
    };
    if prefix.starts_with('/') {
        return reject("leading slash");
    }
    let mut components = prefix.split('/').peekable();
    while let Some(component) = components.next() {
        match component {
            // A single trailing empty component is the trailing slash.
            "" if components.peek().is_none() => {},
            "" => return reject("empty path component"),
            "." | ".." => return reject("relative path component"),
            _ => {},
        }
    }
    Ok(())
}

impl ReplicaStorage {
    /// Store a whole object atomically under `key`.
    pub async fn put(&self, key: &str, bytes: Bytes) -> Result<(), StorageError> {
        match self {
            Self::Local(local) => local.put(key, bytes).await,
            Self::S3(s3) => s3.put(key, bytes).await,
            #[cfg(any(test, feature = "testing"))]
            Self::Flaky(flaky) => flaky.put(key, bytes).await,
        }
    }

    /// Fetch a whole object. Missing keys are [`StorageError::NotFound`].
    pub async fn get(&self, key: &str) -> Result<Bytes, StorageError> {
        match self {
            Self::Local(local) => local.get(key).await,
            Self::S3(s3) => s3.get(key).await,
            #[cfg(any(test, feature = "testing"))]
            Self::Flaky(flaky) => flaky.get(key).await,
        }
    }

    /// Fetch an object as a byte stream (used for multi-GB snapshot
    /// downloads during restore).
    pub async fn get_stream(
        &self,
        key: &str,
    ) -> Result<Box<dyn tokio::io::AsyncRead + Send + Unpin>, StorageError> {
        match self {
            Self::Local(local) => local.get_stream(key).await,
            Self::S3(s3) => s3.get_stream(key).await,
            #[cfg(any(test, feature = "testing"))]
            Self::Flaky(flaky) => flaky.get_stream(key).await,
        }
    }

    /// List all keys under `prefix` (recursive), sorted lexicographically.
    /// Pagination is handled internally. Errors are errors, never `vec![]`.
    pub async fn list(&self, prefix: &str) -> Result<Vec<String>, StorageError> {
        validate_prefix(prefix)?;
        match self {
            Self::Local(local) => local.list(prefix).await,
            Self::S3(s3) => s3.list(prefix).await,
            #[cfg(any(test, feature = "testing"))]
            Self::Flaky(flaky) => flaky.list(prefix).await,
        }
    }

    /// List the immediate "subdirectory" components under `prefix` (S3:
    /// delimiter `/` common prefixes; local: child directories), sorted
    /// lexicographically. Returned values are the bare path components, not
    /// full keys.
    pub async fn list_dirs(&self, prefix: &str) -> Result<Vec<String>, StorageError> {
        validate_prefix(prefix)?;
        match self {
            Self::Local(local) => local.list_dirs(prefix).await,
            Self::S3(s3) => s3.list_dirs(prefix).await,
            #[cfg(any(test, feature = "testing"))]
            Self::Flaky(flaky) => flaky.list_dirs(prefix).await,
        }
    }

    /// Delete one object. Idempotent: deleting a missing key is `Ok`.
    pub async fn delete(&self, key: &str) -> Result<(), StorageError> {
        match self {
            Self::Local(local) => local.delete(key).await,
            Self::S3(s3) => s3.delete(key).await,
            #[cfg(any(test, feature = "testing"))]
            Self::Flaky(flaky) => flaky.delete(key).await,
        }
    }

    /// Delete every object under `prefix` (generation pruning).
    pub async fn delete_prefix(&self, prefix: &str) -> Result<(), StorageError> {
        validate_prefix(prefix)?;
        match self {
            Self::Local(local) => local.delete_prefix(prefix).await,
            Self::S3(s3) => s3.delete_prefix(prefix).await,
            #[cfg(any(test, feature = "testing"))]
            Self::Flaky(flaky) => flaky.delete_prefix(prefix).await,
        }
    }

    /// Best-effort sweep of crash-orphaned incomplete uploads: S3 aborts
    /// uncompleted multipart uploads (which accrue storage cost until
    /// aborted), and the local backend removes orphaned `.partial-` write
    /// fragments. The fault-injection backend has nothing to reclaim.
    /// Never fails the caller.
    pub async fn abort_incomplete_uploads(&self, log: &slog::Logger) {
        match self {
            Self::S3(s3) => s3.abort_incomplete_uploads(log).await,
            Self::Local(local) => local.abort_incomplete_uploads(log).await,
            #[cfg(any(test, feature = "testing"))]
            Self::Flaky(_) => {},
        }
    }

    /// Begin a streaming upload for a large object (snapshots). Parts are
    /// buffered/uploaded as written; the object becomes visible under `key`
    /// only when [`MultipartUpload::finish`] succeeds.
    pub async fn start_multipart(&self, key: &str) -> Result<MultipartUpload, StorageError> {
        match self {
            Self::Local(local) => Ok(MultipartUpload::Local(local.start_multipart(key).await?)),
            Self::S3(s3) => Ok(MultipartUpload::S3(Box::new(
                s3.start_multipart(key).await?,
            ))),
            #[cfg(any(test, feature = "testing"))]
            Self::Flaky(flaky) => Ok(MultipartUpload::Flaky(Box::new(
                flaky.start_multipart(key).await?,
            ))),
        }
    }
}

/// An in-progress streaming upload. Dropping without [`Self::finish`] must
/// never leave a visible object under the final key (local: temp file; S3:
/// uncompleted multipart upload).
pub enum MultipartUpload {
    Local(crate::local::LocalMultipart),
    S3(Box<crate::s3::S3Multipart>),
    #[cfg(any(test, feature = "testing"))]
    Flaky(Box<crate::testing::FlakyMultipart>),
}

impl MultipartUpload {
    /// Append a part. Parts may be any size; the S3 backend buffers
    /// internally to satisfy the 5 MiB minimum-part rule.
    pub async fn write_part(&mut self, bytes: Bytes) -> Result<(), StorageError> {
        match self {
            Self::Local(local) => local.write_part(bytes).await,
            Self::S3(s3) => s3.write_part(bytes).await,
            #[cfg(any(test, feature = "testing"))]
            Self::Flaky(flaky) => flaky.write_part(bytes).await,
        }
    }

    /// Complete the upload, making the object visible under its final key.
    pub async fn finish(self) -> Result<(), StorageError> {
        match self {
            Self::Local(local) => local.finish().await,
            Self::S3(s3) => s3.finish().await,
            #[cfg(any(test, feature = "testing"))]
            Self::Flaky(flaky) => flaky.finish().await,
        }
    }

    /// Abort the upload, releasing any partial state.
    pub async fn abort(self) -> Result<(), StorageError> {
        match self {
            Self::Local(local) => local.abort().await,
            Self::S3(s3) => s3.abort().await,
            #[cfg(any(test, feature = "testing"))]
            Self::Flaky(flaky) => flaky.abort().await,
        }
    }
}

#[cfg(test)]
mod tests {
    use camino::Utf8Path;
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn not_found_display_names_key() {
        let error = StorageError::NotFound {
            key: "generations/g1/snapshot.json".to_owned(),
        };
        assert_eq!(
            error.to_string(),
            "Object not found: generations/g1/snapshot.json",
            "NotFound display must name the key"
        );
    }

    #[test]
    fn validate_key_accepts_relative_paths() {
        for key in [
            "snapshot.json",
            "generations/g1/snapshot.json",
            "generations/20260710T145900Z-3f8a2c1d/wal/0000000000-9d2f1c4a8b3e6f70/x.wal.zst",
        ] {
            validate_key(key).unwrap_or_else(|error| panic!("{key} must be valid: {error}"));
        }
    }

    #[test]
    fn validate_prefix_accepts_prefix_shapes() {
        for prefix in [
            "",
            "generations/",
            "generations/g1",
            "generations/g1/wal/epoch00",
            "generations/20260710T145900Z-3f8a2c1d/wal/",
        ] {
            validate_prefix(prefix)
                .unwrap_or_else(|error| panic!("{prefix:?} must be valid: {error}"));
        }
    }

    #[test]
    fn validate_prefix_rejects_escaping_shapes() {
        let cases = [
            ("/leading", "leading slash"),
            ("/", "leading slash"),
            ("..", "relative path component"),
            ("../escape", "relative path component"),
            ("gen/../escape", "relative path component"),
            ("gen/./here", "relative path component"),
            ("gen/..", "relative path component"),
            ("gen//doubled", "empty path component"),
            ("gen//", "empty path component"),
        ];
        for (prefix, reason) in cases {
            match validate_prefix(prefix) {
                Err(StorageError::InvalidKey {
                    key: found,
                    reason: found_reason,
                }) => {
                    assert_eq!(found, prefix, "InvalidKey names the wrong prefix");
                    assert_eq!(found_reason, reason, "wrong reason for {prefix}");
                },
                other => panic!("{prefix} must be rejected, got: {other:?}"),
            }
        }
    }

    #[test]
    fn validate_key_rejects_escaping_shapes() {
        let cases = [
            ("/leading", "leading slash"),
            ("/", "leading slash"),
            ("..", "relative path component"),
            ("../escape", "relative path component"),
            ("gen/../escape", "relative path component"),
            ("gen/./here", "relative path component"),
            ("", "empty path component"),
            ("gen//doubled", "empty path component"),
            ("trailing/", "empty path component"),
        ];
        for (key, reason) in cases {
            match validate_key(key) {
                Err(StorageError::InvalidKey {
                    key: found,
                    reason: found_reason,
                }) => {
                    assert_eq!(found, key, "InvalidKey names the wrong key");
                    assert_eq!(found_reason, reason, "wrong reason for {key}");
                },
                other => panic!("{key} must be rejected, got: {other:?}"),
            }
        }
    }

    #[test]
    fn injected_display_names_op_and_key() {
        let error = StorageError::Injected {
            op: "list",
            key: "generations/".to_owned(),
        };
        assert_eq!(
            error.to_string(),
            "Injected fault: list generations/",
            "Injected display must name the op and key"
        );
    }

    /// Smoke test that the enum dispatch delegates to the wrapped backend;
    /// full behavior is covered by `tests/storage_contract.rs`.
    #[tokio::test]
    async fn local_variant_dispatches() {
        let tmp = tempfile::tempdir().expect("tempdir failed");
        let root = Utf8Path::from_path(tmp.path())
            .expect("tempdir path is UTF-8")
            .to_path_buf();
        let storage = ReplicaStorage::Local(LocalStorage::new(root));
        storage
            .put("dispatch/key", Bytes::from_static(b"value"))
            .await
            .expect("put failed");
        let got = storage.get("dispatch/key").await.expect("get failed");
        assert_eq!(got.as_ref(), b"value".as_slice(), "dispatch roundtrip");
        let keys = storage.list("").await.expect("list failed");
        assert_eq!(keys, vec!["dispatch/key".to_owned()], "dispatch list");
        let mut upload = storage
            .start_multipart("dispatch/multi")
            .await
            .expect("start failed");
        upload
            .write_part(Bytes::from_static(b"m"))
            .await
            .expect("write failed");
        upload.finish().await.expect("finish failed");
        let got = storage.get("dispatch/multi").await.expect("get failed");
        assert_eq!(got.as_ref(), b"m".as_slice(), "dispatch multipart");
        storage.delete("dispatch/key").await.expect("delete failed");
        storage
            .delete_prefix("dispatch/")
            .await
            .expect("delete_prefix failed");
    }
}
