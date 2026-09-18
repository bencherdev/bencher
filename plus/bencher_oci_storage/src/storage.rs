//! OCI Storage Layer
//!
//! One implementation over [`object_store::ObjectStore`], configured as either a
//! local filesystem directory or any S3-compatible bucket. Keys are identical
//! across both backends, so the same registry data can be moved between them.

use std::path::{Path as FilePath, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};
use std::task::{Context, Poll};

use bencher_json::{
    Clock, ProjectUuid, Secret,
    system::config::{
        DEFAULT_CHUNK_SIZE, DEFAULT_MAX_BODY_SIZE, DEFAULT_UPLOAD_TIMEOUT_SECS, MAX_CHUNK_SIZE,
        RegistryStorage,
    },
};
use bytes::Bytes;
use chrono::Utc;
use futures::stream::{self, BoxStream, StreamExt as _, TryStreamExt as _};
use hyper::body::Frame;
use object_store::{ObjectMeta, ObjectStore, ObjectStoreExt as _, WriteMultipart, path::Path};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use slog::Logger;
use sync_wrapper::SyncWrapper;
use thiserror::Error;

use crate::types::{Digest, UploadId};

/// Maximum concurrency for parallel object store operations.
/// Clamped to available CPU parallelism, with this upper bound to prevent
/// excessive resource usage.
pub const MAX_CONCURRENCY: usize = 64;

/// Maximum number of multipart parts uploaded in parallel when completing an upload.
/// Each in-flight part holds `chunk_size` bytes in memory.
const MAX_UPLOAD_CONCURRENCY: usize = 4;

/// Directory holding registry data, created beside the database file.
const REGISTRY_DIR: &str = "registry";

/// Key prefix for the in-progress upload staging area.
const UPLOADS_PREFIX: &str = "_uploads";

/// Key prefix for an upload session's chunk objects.
const CHUNKS_PREFIX: &str = "chunks";

/// Region used when none is configured. S3-compatible stores expect this.
const DEFAULT_REGION: &str = "auto";

/// OCI storage backed by a local directory or an S3-compatible bucket
pub struct OciStorage {
    store: Arc<dyn ObjectStore>,
    /// Key prefix within the store, if configured
    prefix: Option<Path>,
    /// Upload timeout in seconds for stale upload cleanup
    upload_timeout: u64,
    /// Maximum body size in bytes for uploads
    max_body_size: u64,
    /// Minimum chunk size in bytes for buffering upload data before storing.
    /// See [`bencher_json::system::config::DEFAULT_CHUNK_SIZE`].
    chunk_size: usize,
    /// Logger for error/warning reporting
    log: Logger,
    /// Concurrency limit for parallel referrer and tag fetches
    concurrency: usize,
    /// Unix timestamp of the last stale upload cleanup (for debouncing)
    last_cleanup: AtomicI64,
    /// Clock for getting the current time (injectable for testing)
    clock: Clock,
}

/// Storage errors
#[derive(Debug, Error)]
pub enum OciStorageError {
    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Upload not found: {0}")]
    UploadNotFound(String),

    #[error("Blob not found: {0}")]
    BlobNotFound(String),

    #[error("Manifest not found: {0}")]
    ManifestNotFound(String),

    #[error("Digest mismatch: expected {expected}, got {actual}")]
    DigestMismatch { expected: String, actual: String },

    #[error("Invalid content: {0}")]
    InvalidContent(String),

    #[error("Blob upload invalid content: {0}")]
    BlobUploadInvalidContent(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("JSON serialization error: {0}")]
    Json(String),

    #[error("Size exceeded: {size} bytes exceeds maximum {max} bytes")]
    SizeExceeded { size: u64, max: u64 },
}

impl OciStorageError {
    /// Returns the appropriate HTTP status code for this storage error
    pub fn status_code(&self) -> http::StatusCode {
        match self {
            Self::UploadNotFound(_) | Self::BlobNotFound(_) | Self::ManifestNotFound(_) => {
                http::StatusCode::NOT_FOUND
            },
            Self::DigestMismatch { .. }
            | Self::InvalidContent(_)
            | Self::BlobUploadInvalidContent(_) => http::StatusCode::BAD_REQUEST,
            Self::SizeExceeded { .. } => http::StatusCode::PAYLOAD_TOO_LARGE,
            Self::Storage(_) | Self::Config(_) | Self::Json(_) => {
                http::StatusCode::INTERNAL_SERVER_ERROR
            },
        }
    }
}

impl OciStorage {
    /// Creates a new OCI storage instance from configuration
    ///
    /// The `upload_timeout` specifies how long (in seconds) before stale uploads
    /// are cleaned up. Pass `None` to use the default (1 hour).
    /// The `max_body_size` specifies the maximum body size in bytes.
    /// Pass `None` to use the default (1 GiB).
    pub fn try_from_config(
        log: Logger,
        storage: Option<RegistryStorage>,
        database_path: &FilePath,
        upload_timeout: Option<u64>,
        max_body_size: Option<u64>,
        clock: Option<Clock>,
    ) -> Result<Self, OciStorageError> {
        let (store, prefix, chunk_size) = match storage
            .unwrap_or(RegistryStorage::File { path: None })
        {
            RegistryStorage::File { path } => {
                let dir = path.unwrap_or_else(|| registry_dir(database_path));
                (file_store(&dir)?, None, None)
            },
            RegistryStorage::S3 {
                bucket,
                path,
                endpoint,
                region,
                access_key_id,
                secret_access_key,
                chunk_size,
            } => {
                let store = s3_store(bucket, endpoint, region, access_key_id, &secret_access_key)?;
                (store, path.map(Path::from), chunk_size)
            },
        };

        let concurrency = std::thread::available_parallelism()
            .map_or(1, std::num::NonZeroUsize::get)
            .clamp(1, MAX_CONCURRENCY);
        let chunk_size = chunk_size
            .unwrap_or(DEFAULT_CHUNK_SIZE)
            .clamp(DEFAULT_CHUNK_SIZE, MAX_CHUNK_SIZE) as usize;

        Ok(Self {
            store,
            prefix,
            upload_timeout: upload_timeout.unwrap_or(DEFAULT_UPLOAD_TIMEOUT_SECS),
            max_body_size: max_body_size.unwrap_or(DEFAULT_MAX_BODY_SIZE),
            chunk_size,
            log,
            concurrency,
            last_cleanup: AtomicI64::new(0),
            clock: clock.unwrap_or(Clock::System),
        })
    }

    /// Returns a view type for job output storage operations.
    pub fn job_output(&self) -> crate::job_output::JobOutput<'_> {
        crate::job_output::JobOutput::new(self)
    }

    /// Returns the configured maximum body size in bytes
    pub fn max_body_size(&self) -> u64 {
        self.max_body_size
    }

    /// Returns the configured chunk size for buffering upload data before storing.
    /// See [`bencher_json::system::config::DEFAULT_CHUNK_SIZE`].
    pub fn chunk_size(&self) -> usize {
        self.chunk_size
    }

    // ==================== Key Generation ====================

    /// Returns the configured key prefix, or the root of the store
    ///
    /// Every key below is grown from here with `join`, which encodes only the
    /// part being appended. Formatting one `Path` into another would instead
    /// re-encode the whole prefix at each nesting level, so a prefix needing
    /// percent encoding would land reads and writes on different keys.
    fn root(&self) -> Path {
        self.prefix.clone().unwrap_or(Path::ROOT)
    }

    /// Returns the key prefix for the given repository
    fn repository_prefix(&self, repository: &ProjectUuid) -> Path {
        self.root().join(repository.to_string())
    }

    /// Returns the key prefix for the upload staging area
    fn uploads_prefix(&self) -> Path {
        self.root().join(UPLOADS_PREFIX)
    }

    /// Returns the key prefix for a single upload session
    fn upload_prefix(&self, upload_id: &UploadId) -> Path {
        self.uploads_prefix().join(upload_id.to_string())
    }

    /// Returns the key for upload state metadata
    fn upload_state_key(&self, upload_id: &UploadId) -> Path {
        upload_state_key(&self.upload_prefix(upload_id))
    }

    /// Returns the key prefix for upload chunks
    ///
    /// Each append stores its own chunk object so appends stay O(1) and
    /// concurrent appends never contend over shared state.
    fn upload_chunks_prefix(&self, upload_id: &UploadId) -> Path {
        self.upload_prefix(upload_id).join(CHUNKS_PREFIX)
    }

    /// Returns a unique key for a new upload chunk
    ///
    /// The nanosecond timestamp sorts chunks into creation order when listed,
    /// and the UUID keeps concurrent appends from colliding.
    fn new_chunk_key(&self, upload_id: &UploadId) -> Path {
        let timestamp = Utc::now().timestamp_nanos_opt().unwrap_or(0);
        let uuid = uuid::Uuid::new_v4();
        self.upload_chunks_prefix(upload_id)
            .join(format!("{timestamp:020}_{uuid}"))
    }

    /// Returns the key for a blob
    fn blob_key(&self, repository: &ProjectUuid, digest: &Digest) -> Path {
        self.repository_prefix(repository)
            .join("blobs")
            .join(digest.algorithm())
            .join(digest.hex_hash())
    }

    /// Returns the key for a manifest by digest
    fn manifest_key(&self, repository: &ProjectUuid, digest: &Digest) -> Path {
        self.repository_prefix(repository)
            .join("manifests")
            .join(digest.algorithm())
            .join(digest.hex_hash())
    }

    /// Returns the key prefix for all tag links in a repository
    fn tags_prefix(&self, repository: &ProjectUuid) -> Path {
        self.repository_prefix(repository).join("tags")
    }

    /// Returns the key for a manifest tag link
    fn tag_key(&self, repository: &ProjectUuid, tag: &str) -> Path {
        self.tags_prefix(repository).join(tag)
    }

    /// Returns the key prefix for referrers to a given digest
    fn referrers_prefix(&self, repository: &ProjectUuid, subject_digest: &Digest) -> Path {
        self.repository_prefix(repository)
            .join("referrers")
            .join(subject_digest.algorithm())
            .join(subject_digest.hex_hash())
    }

    /// Returns the key for a referrer link
    fn referrer_key(
        &self,
        repository: &ProjectUuid,
        subject_digest: &Digest,
        referrer_digest: &Digest,
    ) -> Path {
        self.referrers_prefix(repository, subject_digest)
            .join(format!(
                "{}-{}",
                referrer_digest.algorithm(),
                referrer_digest.hex_hash()
            ))
    }

    /// Returns the key for a job output blob
    fn job_output_key(&self, project: ProjectUuid, job: bencher_json::JobUuid) -> Path {
        self.repository_prefix(&project)
            .join("output")
            .join("v0")
            .join("jobs")
            .join(job.to_string())
    }

    // ==================== Object Store Helpers ====================

    /// Reads an object in full, mapping a missing object to `not_found`.
    async fn get_bytes(
        &self,
        key: &Path,
        not_found: OciStorageError,
    ) -> Result<Bytes, OciStorageError> {
        let result = self
            .store
            .get(key)
            .await
            .map_err(|e| map_error(&e, not_found))?;
        result
            .bytes()
            .await
            .map_err(|e| OciStorageError::Storage(e.to_string()))
    }

    /// Reads an object in full, without classifying a missing object.
    async fn read(&self, key: &Path) -> Result<Bytes, OciStorageError> {
        self.get_bytes(
            key,
            OciStorageError::Storage(format!("Missing object: {key}")),
        )
        .await
    }

    /// Deletes an object, treating an already-missing object as success.
    async fn delete_key(&self, key: &Path) -> Result<(), OciStorageError> {
        match self.store.delete(key).await {
            Ok(()) => Ok(()),
            Err(e) if is_not_found(&e) => Ok(()),
            Err(e) => Err(OciStorageError::Storage(e.to_string())),
        }
    }

    /// Lists every object under a prefix.
    async fn list_prefix(&self, prefix: &Path) -> Result<Vec<ObjectMeta>, OciStorageError> {
        self.store
            .list(Some(prefix))
            .try_collect()
            .await
            .map_err(|e| OciStorageError::Storage(e.to_string()))
    }

    // ==================== Upload Operations ====================

    /// Starts a new upload session
    ///
    /// Also spawns a background task to clean up any stale uploads older than `upload_timeout`.
    pub async fn start_upload(
        &self,
        repository: &ProjectUuid,
    ) -> Result<UploadId, OciStorageError> {
        self.spawn_stale_upload_cleanup();

        let upload_id = UploadId::new();
        let state = UploadState {
            repository: repository.to_string(),
            created_at: self.clock.timestamp(),
        };
        let data = serde_json::to_vec(&state).map_err(|e| OciStorageError::Json(e.to_string()))?;
        self.store
            .put(&self.upload_state_key(&upload_id), data.into())
            .await
            .map_err(|e| OciStorageError::Storage(e.to_string()))?;

        Ok(upload_id)
    }

    /// Appends data to an in-progress upload
    ///
    /// Each append is stored as its own chunk object, so concurrent appends
    /// create independent objects rather than modifying shared state.
    ///
    /// Returns the cumulative size of the upload session.
    pub async fn append_upload(
        &self,
        upload_id: &UploadId,
        data: Bytes,
    ) -> Result<u64, OciStorageError> {
        // Verify the upload exists
        let _state = self.load_upload_state(upload_id).await?;

        // Calculate the projected total size before storing the chunk
        let buffered = self.chunks_size(upload_id).await?;
        let projected_total = buffered + data.len() as u64;
        if projected_total > self.max_body_size {
            return Err(OciStorageError::SizeExceeded {
                size: projected_total,
                max: self.max_body_size,
            });
        }

        self.store
            .put(&self.new_chunk_key(upload_id), data.into())
            .await
            .map_err(|e| OciStorageError::Storage(e.to_string()))?;

        Ok(projected_total)
    }

    /// Gets the current size of an in-progress upload
    pub async fn get_upload_size(&self, upload_id: &UploadId) -> Result<u64, OciStorageError> {
        let _state = self.load_upload_state(upload_id).await?;
        self.chunks_size(upload_id).await
    }

    /// Completes an upload and stores the blob
    ///
    /// Streams the stored chunks in key order straight into a multipart upload on
    /// the final blob key, hashing as it goes. A digest mismatch or any streaming
    /// error aborts the multipart upload, so no blob is left behind.
    pub async fn complete_upload(
        &self,
        upload_id: &UploadId,
        expected_digest: &Digest,
    ) -> Result<Digest, OciStorageError> {
        let state = self.load_upload_state(upload_id).await?;
        let repository: ProjectUuid = state.repository.parse().map_err(|_e| {
            OciStorageError::InvalidContent(format!(
                "Invalid project UUID in upload state: {}",
                state.repository
            ))
        })?;

        let chunks = self.list_chunks(upload_id).await?;
        if chunks.is_empty() {
            self.cleanup_upload(upload_id).await;
            return Err(OciStorageError::BlobUploadInvalidContent(
                "Cannot complete upload with no data".to_owned(),
            ));
        }

        let blob_key = self.blob_key(&repository, expected_digest);
        let upload = self
            .store
            .put_multipart(&blob_key)
            .await
            .map_err(|e| OciStorageError::Storage(e.to_string()))?;
        let mut write = WriteMultipart::new_with_chunk_size(upload, self.chunk_size);

        let actual_digest = match self.write_chunks(&mut write, &chunks).await {
            Ok(digest) => digest,
            Err(error) => {
                self.abort_multipart(write).await;
                return Err(error);
            },
        };

        if actual_digest.as_str() != expected_digest.as_str() {
            self.abort_multipart(write).await;
            self.cleanup_upload(upload_id).await;
            return Err(OciStorageError::DigestMismatch {
                expected: expected_digest.to_string(),
                actual: actual_digest.to_string(),
            });
        }

        write
            .finish()
            .await
            .map_err(|e| OciStorageError::Storage(e.to_string()))?;
        self.cleanup_upload(upload_id).await;

        Ok(actual_digest)
    }

    /// Streams every chunk into the multipart upload, returning the content digest.
    async fn write_chunks(
        &self,
        write: &mut WriteMultipart,
        chunks: &[Path],
    ) -> Result<Digest, OciStorageError> {
        let mut hasher = Sha256::new();
        for chunk_key in chunks {
            let chunk = self.read(chunk_key).await?;
            hasher.update(&chunk);
            write
                .wait_for_capacity(MAX_UPLOAD_CONCURRENCY)
                .await
                .map_err(|e| OciStorageError::Storage(e.to_string()))?;
            write.put(chunk);
        }
        Digest::sha256(&hex::encode(hasher.finalize()))
            .map_err(|e| OciStorageError::InvalidContent(e.to_string()))
    }

    /// Aborts a multipart upload, reporting but not surfacing any failure.
    async fn abort_multipart(&self, write: WriteMultipart) {
        if let Err(e) = write.abort().await {
            report_cleanup_error(&self.log, "complete_upload: abort multipart", &e);
        }
    }

    /// Validates that the upload session belongs to the expected repository
    pub async fn validate_upload_repository(
        &self,
        upload_id: &UploadId,
        expected_repository: &ProjectUuid,
    ) -> Result<(), OciStorageError> {
        let state = self.load_upload_state(upload_id).await?;
        if state.repository == expected_repository.to_string() {
            Ok(())
        } else {
            Err(OciStorageError::UploadNotFound(upload_id.to_string()))
        }
    }

    /// Cancels an in-progress upload
    pub async fn cancel_upload(&self, upload_id: &UploadId) -> Result<(), OciStorageError> {
        let _state = self.load_upload_state(upload_id).await?;
        self.cleanup_upload(upload_id).await;
        Ok(())
    }

    /// Loads upload state
    async fn load_upload_state(
        &self,
        upload_id: &UploadId,
    ) -> Result<UploadState, OciStorageError> {
        let data = self
            .get_bytes(
                &self.upload_state_key(upload_id),
                OciStorageError::UploadNotFound(upload_id.to_string()),
            )
            .await?;
        serde_json::from_slice(&data).map_err(|e| OciStorageError::Json(e.to_string()))
    }

    /// Lists the chunks of an upload in key order, which is creation order.
    async fn list_chunks(&self, upload_id: &UploadId) -> Result<Vec<Path>, OciStorageError> {
        let mut chunks = self
            .list_prefix(&self.upload_chunks_prefix(upload_id))
            .await?;
        chunks.sort_by(|a, b| a.location.cmp(&b.location));
        Ok(chunks.into_iter().map(|meta| meta.location).collect())
    }

    /// Total size of the chunks stored for an upload.
    async fn chunks_size(&self, upload_id: &UploadId) -> Result<u64, OciStorageError> {
        let chunks = self
            .list_prefix(&self.upload_chunks_prefix(upload_id))
            .await?;
        Ok(chunks.iter().map(|meta| meta.size).sum())
    }

    /// Deletes every object belonging to an upload
    ///
    /// Chunks go first so that the state object, which drives stale upload
    /// discovery, survives a crash part way through.
    async fn cleanup_upload(&self, upload_id: &UploadId) {
        let prefix = self.upload_prefix(upload_id);
        match self.list_prefix(&prefix).await {
            Ok(objects) => delete_upload_objects(&self.log, &self.store, &prefix, objects).await,
            Err(e) => report_cleanup_error(&self.log, "cleanup_upload: list", &e),
        }
    }

    /// Spawns a background task to clean up all stale uploads that have exceeded the timeout.
    ///
    /// Debounced: skips if a cleanup ran within the last `upload_timeout` seconds,
    /// since stale uploads can't appear faster than the timeout period.
    fn spawn_stale_upload_cleanup(&self) {
        let now = self.clock.timestamp();
        let last = self.last_cleanup.load(Ordering::Acquire);
        let timeout_secs = i64::try_from(self.upload_timeout).unwrap_or(i64::MAX);
        if now.saturating_sub(last) < timeout_secs {
            return;
        }
        // Atomically claim the cleanup slot; if another thread raced us, skip.
        if self
            .last_cleanup
            .compare_exchange(last, now, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return;
        }

        let store = Arc::clone(&self.store);
        let uploads_prefix = self.uploads_prefix();
        let upload_timeout = self.upload_timeout;
        let log = self.log.clone();
        let clock = self.clock.clone();

        tokio::spawn(async move {
            cleanup_stale_uploads(&log, &store, &uploads_prefix, upload_timeout, &clock).await;
        });
    }

    // ==================== Blob Operations ====================

    /// Checks if a blob exists
    pub async fn blob_exists(
        &self,
        repository: &ProjectUuid,
        digest: &Digest,
    ) -> Result<bool, OciStorageError> {
        self.exists(&self.blob_key(repository, digest)).await
    }

    /// Gets a blob's content and size (loads entire blob into memory)
    ///
    /// For large blobs, prefer `get_blob_stream` which streams the content.
    pub async fn get_blob(
        &self,
        repository: &ProjectUuid,
        digest: &Digest,
    ) -> Result<(Bytes, u64), OciStorageError> {
        let data = self
            .get_bytes(
                &self.blob_key(repository, digest),
                OciStorageError::BlobNotFound(digest.to_string()),
            )
            .await?;
        let size = data.len() as u64;
        Ok((data, size))
    }

    /// Gets a blob as a streaming body
    ///
    /// Returns a streaming body and the blob size. The content is streamed
    /// rather than loaded entirely into memory, making this suitable for large blobs.
    pub async fn get_blob_stream(
        &self,
        repository: &ProjectUuid,
        digest: &Digest,
    ) -> Result<(BlobBody, u64), OciStorageError> {
        let result = self
            .store
            .get(&self.blob_key(repository, digest))
            .await
            .map_err(|e| map_error(&e, OciStorageError::BlobNotFound(digest.to_string())))?;
        let size = result.meta.size;
        Ok((
            BlobBody {
                stream: SyncWrapper::new(result.into_stream()),
                size,
            },
            size,
        ))
    }

    /// Gets blob metadata (size) without downloading content
    pub async fn get_blob_size(
        &self,
        repository: &ProjectUuid,
        digest: &Digest,
    ) -> Result<u64, OciStorageError> {
        self.store
            .head(&self.blob_key(repository, digest))
            .await
            .map(|meta| meta.size)
            .map_err(|e| map_error(&e, OciStorageError::BlobNotFound(digest.to_string())))
    }

    /// Deletes a blob
    pub async fn delete_blob(
        &self,
        repository: &ProjectUuid,
        digest: &Digest,
    ) -> Result<(), OciStorageError> {
        self.delete_key(&self.blob_key(repository, digest)).await
    }

    /// Mounts a blob from another repository (cross-repo blob mount)
    ///
    /// Copies directly and handles not-found, avoiding a TOCTOU race
    /// between checking existence and copying.
    pub async fn mount_blob(
        &self,
        from_repository: &ProjectUuid,
        to_repository: &ProjectUuid,
        digest: &Digest,
    ) -> Result<bool, OciStorageError> {
        let source = self.blob_key(from_repository, digest);
        let destination = self.blob_key(to_repository, digest);
        match self.store.copy(&source, &destination).await {
            Ok(()) => Ok(true),
            Err(e) if is_not_found(&e) => Ok(false),
            Err(e) => Err(OciStorageError::Storage(e.to_string())),
        }
    }

    // ==================== Manifest Operations ====================

    /// Stores a manifest
    pub async fn put_manifest(
        &self,
        repository: &ProjectUuid,
        content: Bytes,
        tag: Option<&crate::types::Tag>,
        manifest: &bencher_json::oci::Manifest,
    ) -> Result<Digest, OciStorageError> {
        let digest = Digest::from_sha256_bytes(&content);

        let content_len = content.len();
        self.put(&self.manifest_key(repository, &digest), content)
            .await?;

        if let Some(tag) = tag {
            let key = self.tag_key(repository, tag.as_str());
            self.put(&key, Bytes::from(digest.to_string())).await?;
        }

        // A manifest with a subject is discoverable through the referrers API
        if let Some((subject_digest, descriptor)) =
            crate::types::build_referrer_descriptor(manifest, &digest, content_len)
        {
            let key = self.referrer_key(repository, &subject_digest, &digest);
            let data = serde_json::to_vec(&descriptor)
                .map_err(|e| OciStorageError::Json(e.to_string()))?;
            self.put(&key, Bytes::from(data)).await?;
        }

        Ok(digest)
    }

    /// Gets a manifest by digest
    pub async fn get_manifest_by_digest(
        &self,
        repository: &ProjectUuid,
        digest: &Digest,
    ) -> Result<Bytes, OciStorageError> {
        self.get_bytes(
            &self.manifest_key(repository, digest),
            OciStorageError::ManifestNotFound(digest.to_string()),
        )
        .await
    }

    /// Checks if a manifest exists
    pub async fn manifest_exists(
        &self,
        repository: &ProjectUuid,
        digest: &Digest,
    ) -> Result<bool, OciStorageError> {
        self.exists(&self.manifest_key(repository, digest)).await
    }

    /// Resolves a tag to a digest
    pub async fn resolve_tag(
        &self,
        repository: &ProjectUuid,
        tag: &crate::types::Tag,
    ) -> Result<Digest, OciStorageError> {
        let data = self
            .get_bytes(
                &self.tag_key(repository, tag.as_str()),
                OciStorageError::ManifestNotFound(tag.to_string()),
            )
            .await?;
        let digest = std::str::from_utf8(&data)
            .map_err(|e| OciStorageError::InvalidContent(e.to_string()))?;
        digest
            .trim()
            .parse()
            .map_err(|e: crate::types::DigestError| OciStorageError::InvalidContent(e.to_string()))
    }

    /// Lists tags for a repository with optional pagination
    ///
    /// - `limit`: Maximum number of tags to return
    /// - `start_after`: Tag to start listing after (for cursor-based pagination)
    ///
    /// Tags are returned in lexicographic order.
    pub async fn list_tags(
        &self,
        repository: &ProjectUuid,
        limit: Option<usize>,
        start_after: Option<&str>,
    ) -> Result<ListTagsResult, OciStorageError> {
        let prefix = self.tags_prefix(repository);
        let listed = match start_after {
            Some(start) => {
                let offset = prefix.clone().join(start);
                self.store.list_with_offset(Some(&prefix), &offset)
            },
            None => self.store.list(Some(&prefix)),
        };
        let objects: Vec<ObjectMeta> = listed
            .try_collect()
            .await
            .map_err(|e| OciStorageError::Storage(e.to_string()))?;

        let mut tags: Vec<String> = objects
            .iter()
            .filter_map(|meta| meta.location.filename().map(ToOwned::to_owned))
            .collect();
        tags.sort();

        let has_more = limit.is_some_and(|limit| tags.len() > limit);
        if let Some(limit) = limit {
            tags.truncate(limit);
        }
        Ok(ListTagsResult { tags, has_more })
    }

    /// Deletes a manifest by digest
    ///
    /// Also cleans up any referrer link if this manifest references another manifest
    /// via the `subject` field, and any tag pointing at this digest.
    pub async fn delete_manifest(
        &self,
        repository: &ProjectUuid,
        digest: &Digest,
    ) -> Result<(), OciStorageError> {
        // Read the manifest first so a referrer link can be cleaned up with it
        if let Ok(data) = self.get_manifest_by_digest(repository, digest).await
            && let Some(subject_digest) = crate::types::extract_subject_digest(&data)
        {
            let key = self.referrer_key(repository, &subject_digest, digest);
            if let Err(e) = self.delete_key(&key).await {
                report_cleanup_error(&self.log, "delete_manifest: referrer link delete", &e);
            }
        }

        // Clean up any tags that point to this digest (best-effort)
        if let Ok(result) = self.list_tags(repository, None, None).await {
            let tags = stream::iter(
                result
                    .tags
                    .into_iter()
                    .filter_map(|name| name.parse::<crate::types::Tag>().ok())
                    .map(|tag| async move {
                        match self.resolve_tag(repository, &tag).await {
                            Ok(tag_digest) if tag_digest.as_str() == digest.as_str() => Some(tag),
                            _ => None,
                        }
                    }),
            )
            .buffer_unordered(self.concurrency)
            .filter_map(|tag| async { tag })
            .collect::<Vec<_>>()
            .await;

            for tag in tags {
                if let Err(e) = self.delete_tag(repository, &tag).await {
                    report_cleanup_error(&self.log, "delete_manifest: tag link delete", &e);
                }
            }
        }

        self.delete_key(&self.manifest_key(repository, digest))
            .await
    }

    /// Deletes a tag (removes the tag link, not the manifest itself)
    pub async fn delete_tag(
        &self,
        repository: &ProjectUuid,
        tag: &crate::types::Tag,
    ) -> Result<(), OciStorageError> {
        self.delete_key(&self.tag_key(repository, tag.as_str()))
            .await
    }

    /// Lists all manifests that reference a given digest via their subject field
    pub async fn list_referrers(
        &self,
        repository: &ProjectUuid,
        subject_digest: &Digest,
        artifact_type_filter: Option<&str>,
    ) -> Result<Vec<bencher_json::oci::OciDescriptor>, OciStorageError> {
        let keys = self
            .list_prefix(&self.referrers_prefix(repository, subject_digest))
            .await?;

        let log = &self.log;
        let referrers = stream::iter(keys)
            .map(|meta| async move {
                let key = meta.location;
                let Ok(data) = self.read(&key).await else {
                    slog::warn!(log, "Failed to fetch referrer"; "key" => %key);
                    return None;
                };
                let Ok(descriptor) =
                    serde_json::from_slice::<bencher_json::oci::OciDescriptor>(&data)
                else {
                    slog::warn!(log, "Failed to parse referrer JSON"; "key" => %key);
                    return None;
                };
                if let Some(filter) = artifact_type_filter
                    && descriptor.artifact_type.as_deref() != Some(filter)
                {
                    return None;
                }
                Some(descriptor)
            })
            .buffer_unordered(self.concurrency)
            .filter_map(|descriptor| async { descriptor })
            .collect()
            .await;

        Ok(referrers)
    }

    // ==================== Job Output ====================

    pub(crate) async fn put_job_output(
        &self,
        project: ProjectUuid,
        job: bencher_json::JobUuid,
        output: &bencher_json::runner::JsonJobOutput,
    ) -> Result<(), OciStorageError> {
        let data = serde_json::to_vec(output).map_err(|e| OciStorageError::Json(e.to_string()))?;
        self.put(&self.job_output_key(project, job), Bytes::from(data))
            .await
    }

    pub(crate) async fn get_job_output(
        &self,
        project: ProjectUuid,
        job: bencher_json::JobUuid,
    ) -> Result<Option<bencher_json::runner::JsonJobOutput>, OciStorageError> {
        let key = self.job_output_key(project, job);
        match self.store.get(&key).await {
            Ok(result) => {
                let data = result
                    .bytes()
                    .await
                    .map_err(|e| OciStorageError::Storage(e.to_string()))?;
                serde_json::from_slice(&data)
                    .map(Some)
                    .map_err(|e| OciStorageError::Json(e.to_string()))
            },
            Err(e) if is_not_found(&e) => Ok(None),
            Err(e) => Err(OciStorageError::Storage(e.to_string())),
        }
    }

    // ==================== Shared Helpers ====================

    /// Writes an object.
    async fn put(&self, key: &Path, data: Bytes) -> Result<(), OciStorageError> {
        self.store
            .put(key, data.into())
            .await
            .map(|_result| ())
            .map_err(|e| OciStorageError::Storage(e.to_string()))
    }

    /// Reports whether an object exists.
    async fn exists(&self, key: &Path) -> Result<bool, OciStorageError> {
        match self.store.head(key).await {
            Ok(_meta) => Ok(true),
            Err(e) if is_not_found(&e) => Ok(false),
            Err(e) => Err(OciStorageError::Storage(e.to_string())),
        }
    }
}

/// A streaming body for blob content
///
/// The object store hands back a `Send` but not `Sync` stream, while the HTTP
/// body must be both. Polling only ever needs `&mut`, which is exactly what
/// [`SyncWrapper`] makes safe.
pub struct BlobBody {
    stream: SyncWrapper<BoxStream<'static, object_store::Result<Bytes>>>,
    size: u64,
}

impl hyper::body::Body for BlobBody {
    type Data = Bytes;
    type Error = Box<dyn std::error::Error + Send + Sync>;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        match self.stream.get_mut().as_mut().poll_next(cx) {
            Poll::Ready(Some(Ok(bytes))) => Poll::Ready(Some(Ok(Frame::data(bytes)))),
            Poll::Ready(Some(Err(e))) => {
                let boxed: Box<dyn std::error::Error + Send + Sync> = Box::new(e);
                Poll::Ready(Some(Err(boxed)))
            },
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }

    fn size_hint(&self) -> hyper::body::SizeHint {
        hyper::body::SizeHint::with_exact(self.size)
    }
}

/// Result of listing tags with pagination support
pub struct ListTagsResult {
    /// The tags returned
    pub tags: Vec<String>,
    /// Whether more tags exist beyond the requested limit
    pub has_more: bool,
}

/// Upload session state, stored alongside the session's chunks
#[derive(Debug, Serialize, Deserialize)]
struct UploadState {
    /// Repository name
    repository: String,
    /// Unix timestamp when the upload was created
    created_at: i64,
}

pub(crate) fn report_cleanup_error(log: &Logger, context: &str, error: &impl std::fmt::Display) {
    slog::warn!(log, "OCI cleanup error"; "context" => context, "error" => %error);
    #[cfg(feature = "sentry")]
    sentry::capture_message(
        &format!("OCI cleanup error ({context}): {error}"),
        sentry::Level::Warning,
    );
}

/// The registry directory beside the database file
fn registry_dir(database_path: &FilePath) -> PathBuf {
    database_path
        .parent()
        .map_or_else(|| PathBuf::from(REGISTRY_DIR), |dir| dir.join(REGISTRY_DIR))
}

/// Builds a local filesystem store rooted at `dir`, creating it if needed.
fn file_store(dir: &FilePath) -> Result<Arc<dyn ObjectStore>, OciStorageError> {
    std::fs::create_dir_all(dir).map_err(|e| {
        OciStorageError::Config(format!(
            "Failed to create registry directory {}: {e}",
            dir.display()
        ))
    })?;
    let store = object_store::local::LocalFileSystem::new_with_prefix(dir)
        .map_err(|e| {
            OciStorageError::Config(format!(
                "Failed to open registry directory {}: {e}",
                dir.display()
            ))
        })?
        .with_automatic_cleanup(true);
    Ok(Arc::new(store))
}

/// Builds an S3-compatible store.
fn s3_store(
    bucket: String,
    endpoint: Option<String>,
    region: Option<String>,
    access_key_id: String,
    secret_access_key: &Secret,
) -> Result<Arc<dyn ObjectStore>, OciStorageError> {
    let mut builder = object_store::aws::AmazonS3Builder::new()
        .with_bucket_name(bucket)
        .with_access_key_id(access_key_id)
        .with_secret_access_key(secret_access_key.as_ref())
        .with_region(region.unwrap_or_else(|| DEFAULT_REGION.to_owned()));
    if let Some(endpoint) = endpoint {
        if endpoint.starts_with("http://") {
            builder = builder.with_allow_http(true);
        }
        builder = builder.with_endpoint(endpoint);
    }
    let store = builder
        .build()
        .map_err(|e| OciStorageError::Config(e.to_string()))?;
    Ok(Arc::new(store))
}

/// Whether an object store error reports a missing object.
fn is_not_found(error: &object_store::Error) -> bool {
    matches!(error, object_store::Error::NotFound { .. })
}

/// Maps an object store error, converting a missing object to `not_found`.
fn map_error(error: &object_store::Error, not_found: OciStorageError) -> OciStorageError {
    if is_not_found(error) {
        not_found
    } else {
        OciStorageError::Storage(error.to_string())
    }
}

/// The state key for an upload session prefix
fn upload_state_key(upload_prefix: &Path) -> Path {
    upload_prefix.clone().join("state.json")
}

/// Deletes an upload session's objects, leaving the state object for last so a
/// crash part way through still leaves the session discoverable.
async fn delete_upload_objects(
    log: &Logger,
    store: &Arc<dyn ObjectStore>,
    upload_prefix: &Path,
    objects: Vec<ObjectMeta>,
) {
    let state_key = upload_state_key(upload_prefix);
    let (state, rest): (Vec<_>, Vec<_>) = objects
        .into_iter()
        .map(|meta| meta.location)
        .partition(|key| *key == state_key);

    for key in rest.into_iter().chain(state) {
        if let Err(e) = store.delete(&key).await
            && !is_not_found(&e)
        {
            report_cleanup_error(log, "cleanup_upload: delete", &e);
        }
    }
}

/// Cleans up every upload session that has outlived the timeout.
pub(crate) async fn cleanup_stale_uploads(
    log: &Logger,
    store: &Arc<dyn ObjectStore>,
    uploads_prefix: &Path,
    upload_timeout: u64,
    clock: &Clock,
) {
    let Ok(listed) = store.list_with_delimiter(Some(uploads_prefix)).await else {
        report_cleanup_error(log, "stale_upload: list prefixes", &"list request failed");
        return;
    };

    // `now` pairs with `created_at` in the application time domain, `os_now`
    // with object `last_modified` in the OS wall-clock domain.
    let (now, os_now) = clock.timestamps();
    let timeout_secs = i64::try_from(upload_timeout).unwrap_or(i64::MAX);

    for upload_prefix in listed.common_prefixes {
        let Some(upload_id) = upload_prefix.filename() else {
            continue;
        };
        if upload_id.parse::<UploadId>().is_err() {
            report_cleanup_error(
                log,
                "stale_upload: parse upload ID",
                &format!("Invalid upload ID: {upload_id}"),
            );
            continue;
        }

        let objects = match store
            .list(Some(&upload_prefix))
            .try_collect::<Vec<_>>()
            .await
        {
            Ok(objects) => objects,
            Err(e) => {
                report_cleanup_error(log, "stale_upload: list objects", &e);
                continue;
            },
        };

        if is_stale(store, &upload_prefix, &objects, now, os_now, timeout_secs).await {
            delete_upload_objects(log, store, &upload_prefix, objects).await;
        }
    }
}

/// Whether an upload session has outlived the timeout.
///
/// Prefers the session's recorded creation time. When the state object is
/// missing or unreadable, falls back to the newest object under the prefix, so
/// a session whose state has not been written yet is not deleted out from under
/// an in-flight upload.
async fn is_stale(
    store: &Arc<dyn ObjectStore>,
    upload_prefix: &Path,
    objects: &[ObjectMeta],
    now: i64,
    os_now: i64,
    timeout_secs: i64,
) -> bool {
    let state_key = upload_state_key(upload_prefix);
    let state = match store.get(&state_key).await {
        Ok(result) => result
            .bytes()
            .await
            .ok()
            .and_then(|data| serde_json::from_slice::<UploadState>(&data).ok()),
        Err(_e) => None,
    };

    if let Some(state) = state {
        return now.saturating_sub(state.created_at) > timeout_secs;
    }

    // No objects at all under this prefix, so nothing is in flight
    let Some(newest) = objects
        .iter()
        .map(|meta| meta.last_modified.timestamp())
        .max()
    else {
        return true;
    };
    os_now.saturating_sub(newest) > timeout_secs
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use bencher_json::DateTime;
    use bytes::Bytes;
    use object_store::{ObjectStore, ObjectStoreExt as _, memory::InMemory, path::Path};
    use slog::Logger;

    use super::{
        Clock, DEFAULT_CHUNK_SIZE, DEFAULT_MAX_BODY_SIZE, Digest, ListTagsResult, MAX_CHUNK_SIZE,
        OciStorage, OciStorageError, ProjectUuid, RegistryStorage, UploadId, cleanup_stale_uploads,
    };

    /// The clamped default chunk size, as `OciStorage` holds it.
    fn default_chunk_size() -> usize {
        usize::try_from(DEFAULT_CHUNK_SIZE).unwrap()
    }

    const PROJECT: &str = "00000000-0000-0000-0000-000000000001";
    const OTHER_PROJECT: &str = "00000000-0000-0000-0000-000000000002";
    const JOB: &str = "00000000-0000-0000-0000-000000000099";
    /// SHA-256 of the empty input
    const EMPTY_SHA256: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    const SUBJECT_SHA256: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    fn log() -> Logger {
        Logger::root(slog::Discard, slog::o!())
    }

    fn clock() -> Clock {
        Clock::Custom(Arc::new(|| DateTime::TEST))
    }

    fn project() -> ProjectUuid {
        PROJECT.parse().unwrap()
    }

    fn digest(hex: &str) -> Digest {
        Digest::sha256(hex).unwrap()
    }

    /// Storage over an arbitrary object store, for key layout and behavior tests.
    fn storage(store: Arc<dyn ObjectStore>, prefix: Option<&str>) -> OciStorage {
        OciStorage {
            store,
            prefix: prefix.map(Path::from),
            upload_timeout: 3600,
            max_body_size: DEFAULT_MAX_BODY_SIZE,
            chunk_size: default_chunk_size(),
            log: log(),
            concurrency: 4,
            last_cleanup: super::AtomicI64::new(DateTime::TEST.timestamp()),
            clock: clock(),
        }
    }

    fn in_memory(prefix: Option<&str>) -> OciStorage {
        storage(Arc::new(InMemory::new()), prefix)
    }

    /// Storage over a local directory, returning the temporary root alongside it.
    fn on_disk() -> (tempfile::TempDir, OciStorage) {
        let tmp = tempfile::tempdir().unwrap();
        let storage = OciStorage::try_from_config(
            log(),
            None,
            &tmp.path().join("bencher.db"),
            None,
            None,
            Some(clock()),
        )
        .unwrap();
        (tmp, storage)
    }

    fn manifest_json(config_digest: &str) -> String {
        serde_json::json!({
            "schemaVersion": 2,
            "mediaType": "application/vnd.oci.image.manifest.v1+json",
            "config": {
                "mediaType": "application/vnd.oci.image.config.v1+json",
                "digest": config_digest,
                "size": 100
            },
            "layers": []
        })
        .to_string()
    }

    // ==================== Key Layout ====================
    //
    // A bucket copy moves objects by key and existing `file` deployments keep
    // their directories, so these strings are a compatibility contract.

    #[test]
    fn keys_without_a_prefix() {
        let storage = in_memory(None);
        let repository = project();
        let upload_id: UploadId = "3f8d9c1e-0000-4000-8000-000000000abc".parse().unwrap();

        assert_eq!(
            storage
                .blob_key(&repository, &digest(EMPTY_SHA256))
                .as_ref(),
            format!("{PROJECT}/blobs/sha256/{EMPTY_SHA256}")
        );
        assert_eq!(
            storage
                .manifest_key(&repository, &digest(EMPTY_SHA256))
                .as_ref(),
            format!("{PROJECT}/manifests/sha256/{EMPTY_SHA256}")
        );
        assert_eq!(
            storage.tag_key(&repository, "latest").as_ref(),
            format!("{PROJECT}/tags/latest")
        );
        assert_eq!(
            storage
                .referrers_prefix(&repository, &digest(SUBJECT_SHA256))
                .as_ref(),
            format!("{PROJECT}/referrers/sha256/{SUBJECT_SHA256}")
        );
        assert_eq!(
            storage
                .referrer_key(&repository, &digest(SUBJECT_SHA256), &digest(EMPTY_SHA256))
                .as_ref(),
            format!("{PROJECT}/referrers/sha256/{SUBJECT_SHA256}/sha256-{EMPTY_SHA256}")
        );
        assert_eq!(
            storage.upload_state_key(&upload_id).as_ref(),
            "_uploads/3f8d9c1e-0000-4000-8000-000000000abc/state.json"
        );
        assert_eq!(
            storage.upload_chunks_prefix(&upload_id).as_ref(),
            "_uploads/3f8d9c1e-0000-4000-8000-000000000abc/chunks"
        );
        assert_eq!(
            storage
                .job_output_key(repository, JOB.parse().unwrap())
                .as_ref(),
            format!("{PROJECT}/output/v0/jobs/{JOB}")
        );
    }

    #[test]
    fn keys_with_a_prefix() {
        let storage = in_memory(Some("registry"));
        let repository = project();
        let upload_id: UploadId = "3f8d9c1e-0000-4000-8000-000000000abc".parse().unwrap();

        assert_eq!(
            storage
                .blob_key(&repository, &digest(EMPTY_SHA256))
                .as_ref(),
            format!("registry/{PROJECT}/blobs/sha256/{EMPTY_SHA256}")
        );
        assert_eq!(
            storage
                .manifest_key(&repository, &digest(EMPTY_SHA256))
                .as_ref(),
            format!("registry/{PROJECT}/manifests/sha256/{EMPTY_SHA256}")
        );
        assert_eq!(
            storage.tag_key(&repository, "latest").as_ref(),
            format!("registry/{PROJECT}/tags/latest")
        );
        assert_eq!(
            storage
                .referrers_prefix(&repository, &digest(SUBJECT_SHA256))
                .as_ref(),
            format!("registry/{PROJECT}/referrers/sha256/{SUBJECT_SHA256}")
        );
        assert_eq!(
            storage
                .referrer_key(&repository, &digest(SUBJECT_SHA256), &digest(EMPTY_SHA256))
                .as_ref(),
            format!("registry/{PROJECT}/referrers/sha256/{SUBJECT_SHA256}/sha256-{EMPTY_SHA256}")
        );
        assert_eq!(
            storage.upload_state_key(&upload_id).as_ref(),
            "registry/_uploads/3f8d9c1e-0000-4000-8000-000000000abc/state.json"
        );
        assert_eq!(
            storage.upload_chunks_prefix(&upload_id).as_ref(),
            "registry/_uploads/3f8d9c1e-0000-4000-8000-000000000abc/chunks"
        );
        assert_eq!(
            storage
                .job_output_key(repository, JOB.parse().unwrap())
                .as_ref(),
            format!("registry/{PROJECT}/output/v0/jobs/{JOB}")
        );
    }

    /// A prefix needing percent encoding must be encoded exactly once, at every
    /// nesting level, or reads and writes land on different keys.
    #[test]
    fn keys_with_an_encoded_prefix() {
        for raw in [
            "r\u{e9}gistry",
            "\u{30ec}\u{30b8}\u{30b9}\u{30c8}\u{30ea}",
            "team#1",
        ] {
            let storage = in_memory(Some(raw));
            let repository = project();
            let upload_id: UploadId = "3f8d9c1e-0000-4000-8000-000000000abc".parse().unwrap();

            assert_eq!(
                storage.blob_key(&repository, &digest(EMPTY_SHA256)),
                Path::from_iter([raw, PROJECT, "blobs", "sha256", EMPTY_SHA256]),
                "blob key for {raw}"
            );
            assert_eq!(
                storage.tag_key(&repository, "latest"),
                Path::from_iter([raw, PROJECT, "tags", "latest"]),
                "tag key for {raw}"
            );
            assert!(
                storage
                    .tag_key(&repository, "latest")
                    .as_ref()
                    .starts_with(&format!("{}/", storage.tags_prefix(&repository))),
                "tag key sits under the tags prefix for {raw}"
            );
            assert!(
                storage
                    .new_chunk_key(&upload_id)
                    .as_ref()
                    .starts_with(&format!("{}/", storage.upload_chunks_prefix(&upload_id))),
                "chunk key sits under the chunks prefix for {raw}"
            );
            assert!(
                storage
                    .referrer_key(&repository, &digest(SUBJECT_SHA256), &digest(EMPTY_SHA256))
                    .as_ref()
                    .starts_with(&format!(
                        "{}/",
                        storage.referrers_prefix(&repository, &digest(SUBJECT_SHA256))
                    )),
                "referrer key sits under the referrers prefix for {raw}"
            );
            assert_eq!(
                storage.upload_state_key(&upload_id),
                Path::from_iter([
                    raw,
                    super::UPLOADS_PREFIX,
                    "3f8d9c1e-0000-4000-8000-000000000abc",
                    "state.json"
                ]),
                "upload state key for {raw}"
            );
        }
    }

    #[test]
    fn chunk_keys_sit_under_the_chunk_prefix_and_sort_by_creation() {
        let storage = in_memory(None);
        let upload_id = UploadId::new();
        let prefix = format!("{}/", storage.upload_chunks_prefix(&upload_id));

        let first = storage.new_chunk_key(&upload_id);
        let second = storage.new_chunk_key(&upload_id);
        assert!(first.as_ref().starts_with(&prefix), "{first}");
        assert!(second.as_ref().starts_with(&prefix), "{second}");
        // The zero-padded nanosecond timestamp orders chunks lexicographically
        assert!(first < second);
    }

    #[test]
    fn file_storage_defaults_beside_the_database() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("bencher.db");
        OciStorage::try_from_config(log(), None, &db_path, None, None, Some(clock())).unwrap();
        assert!(tmp.path().join("registry").is_dir());
    }

    #[tokio::test]
    async fn file_storage_writes_a_blob_to_the_expected_path() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("bencher.db");
        let storage =
            OciStorage::try_from_config(log(), None, &db_path, None, None, Some(clock())).unwrap();
        let repository = project();

        let data = b"hello world blob data";
        let upload_id = storage.start_upload(&repository).await.unwrap();
        storage
            .append_upload(&upload_id, Bytes::from_static(data))
            .await
            .unwrap();
        let stored = storage
            .complete_upload(&upload_id, &Digest::from_sha256_bytes(data))
            .await
            .unwrap();

        let blob_path = tmp
            .path()
            .join("registry")
            .join(PROJECT)
            .join("blobs")
            .join("sha256")
            .join(stored.hex_hash());
        assert_eq!(std::fs::read(&blob_path).unwrap(), data);
    }

    #[tokio::test]
    async fn file_storage_honors_an_explicit_path() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("nested").join("oci");
        let storage = OciStorage::try_from_config(
            log(),
            Some(RegistryStorage::File {
                path: Some(dir.clone()),
            }),
            &tmp.path().join("bencher.db"),
            None,
            None,
            Some(clock()),
        )
        .unwrap();
        let repository = project();

        let content = manifest_json(&format!("sha256:{SUBJECT_SHA256}"));
        let manifest = bencher_json::oci::Manifest::from_bytes(content.as_bytes()).unwrap();
        let stored = storage
            .put_manifest(&repository, Bytes::from(content), None, &manifest)
            .await
            .unwrap();

        assert!(
            dir.join(PROJECT)
                .join("manifests")
                .join("sha256")
                .join(stored.hex_hash())
                .is_file()
        );
        assert!(!tmp.path().join("registry").exists());
    }

    // ==================== Uploads ====================

    async fn upload_round_trip(storage: &OciStorage) {
        let repository = project();
        let upload_id = storage.start_upload(&repository).await.unwrap();
        storage
            .validate_upload_repository(&upload_id, &repository)
            .await
            .unwrap();

        let first = b"hello ";
        let second = b"world";
        assert_eq!(
            storage
                .append_upload(&upload_id, Bytes::from_static(first))
                .await
                .unwrap(),
            first.len() as u64
        );
        assert_eq!(
            storage
                .append_upload(&upload_id, Bytes::from_static(second))
                .await
                .unwrap(),
            (first.len() + second.len()) as u64
        );
        assert_eq!(
            storage.get_upload_size(&upload_id).await.unwrap(),
            (first.len() + second.len()) as u64
        );

        let expected = Digest::from_sha256_bytes(b"hello world");
        let stored = storage
            .complete_upload(&upload_id, &expected)
            .await
            .unwrap();
        assert_eq!(stored.as_str(), expected.as_str());

        assert!(storage.blob_exists(&repository, &stored).await.unwrap());
        let (data, size) = storage.get_blob(&repository, &stored).await.unwrap();
        assert_eq!(data.as_ref(), b"hello world");
        assert_eq!(size, 11);
        assert_eq!(
            storage.get_blob_size(&repository, &stored).await.unwrap(),
            11
        );

        // The session's objects are gone
        storage.get_upload_size(&upload_id).await.unwrap_err();
    }

    #[tokio::test]
    async fn upload_round_trip_in_memory() {
        upload_round_trip(&in_memory(None)).await;
    }

    #[tokio::test]
    async fn upload_round_trip_in_memory_with_prefix() {
        upload_round_trip(&in_memory(Some("registry"))).await;
    }

    /// The encoded-prefix bug is only visible end to end: writes and reads must
    /// agree on the key, or completing an upload finds no chunks.
    #[tokio::test]
    async fn upload_round_trip_with_an_encoded_prefix() {
        upload_round_trip(&in_memory(Some("r\u{e9}gistry/team#1"))).await;
    }

    #[tokio::test]
    async fn upload_round_trip_on_disk() {
        let (_tmp, storage) = on_disk();
        upload_round_trip(&storage).await;
    }

    async fn digest_mismatch_leaves_no_blob(storage: &OciStorage) {
        let repository = project();
        let upload_id = storage.start_upload(&repository).await.unwrap();
        storage
            .append_upload(&upload_id, Bytes::from_static(b"hello world"))
            .await
            .unwrap();

        let wrong = Digest::from_sha256_bytes(b"something else");
        let error = storage
            .complete_upload(&upload_id, &wrong)
            .await
            .unwrap_err();
        assert!(matches!(error, OciStorageError::DigestMismatch { .. }));

        assert!(!storage.blob_exists(&repository, &wrong).await.unwrap());
        let actual = Digest::from_sha256_bytes(b"hello world");
        assert!(!storage.blob_exists(&repository, &actual).await.unwrap());
        // The failed session is cleaned up
        storage.get_upload_size(&upload_id).await.unwrap_err();
    }

    #[tokio::test]
    async fn digest_mismatch_leaves_no_blob_in_memory() {
        digest_mismatch_leaves_no_blob(&in_memory(None)).await;
    }

    #[tokio::test]
    async fn digest_mismatch_leaves_no_blob_on_disk() {
        let (_tmp, storage) = on_disk();
        digest_mismatch_leaves_no_blob(&storage).await;
    }

    /// Completion streams chunks into a multipart upload, so the reassembled
    /// blob must be byte exact across several parts with a non-aligned tail.
    #[tokio::test]
    async fn complete_upload_spans_multipart_boundaries() {
        let mut storage = in_memory(None);
        storage.chunk_size = 16;
        let repository = project();
        let upload_id = storage.start_upload(&repository).await.unwrap();

        // Append sizes deliberately misalign with the 16 byte part size
        let appends: [Vec<u8>; 4] = [
            vec![b'a'; 10],
            vec![b'b'; 25],
            vec![b'c'; 7],
            vec![b'd'; 30],
        ];
        let mut expected = Vec::new();
        for append in &appends {
            expected.extend_from_slice(append);
            storage
                .append_upload(&upload_id, Bytes::from(append.clone()))
                .await
                .unwrap();
        }
        // 72 bytes is four full parts plus an 8 byte tail
        assert_eq!(expected.len(), 72);
        assert!(expected.len() > storage.chunk_size * 3);
        assert_ne!(
            expected.len() % storage.chunk_size,
            0,
            "tail is not aligned"
        );

        let digest = Digest::from_sha256_bytes(&expected);
        let stored = storage.complete_upload(&upload_id, &digest).await.unwrap();
        assert_eq!(stored.as_str(), digest.as_str());

        let (data, size) = storage.get_blob(&repository, &stored).await.unwrap();
        assert_eq!(data.as_ref(), expected.as_slice());
        assert_eq!(size, expected.len() as u64);
    }

    #[tokio::test]
    async fn complete_upload_with_no_data_fails() {
        let storage = in_memory(None);
        let upload_id = storage.start_upload(&project()).await.unwrap();
        let error = storage
            .complete_upload(&upload_id, &digest(EMPTY_SHA256))
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            OciStorageError::BlobUploadInvalidContent(_)
        ));
    }

    #[tokio::test]
    async fn append_upload_rejects_an_oversized_body() {
        let mut storage = in_memory(None);
        storage.max_body_size = 4;
        let upload_id = storage.start_upload(&project()).await.unwrap();

        let error = storage
            .append_upload(&upload_id, Bytes::from_static(b"too much"))
            .await
            .unwrap_err();
        assert!(matches!(error, OciStorageError::SizeExceeded { .. }));
        // Nothing was stored
        assert_eq!(storage.get_upload_size(&upload_id).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn cancel_upload_removes_the_session() {
        let (_tmp, storage) = on_disk();
        let upload_id = storage.start_upload(&project()).await.unwrap();
        storage
            .append_upload(&upload_id, Bytes::from_static(b"some data"))
            .await
            .unwrap();

        storage.cancel_upload(&upload_id).await.unwrap();
        storage.cancel_upload(&upload_id).await.unwrap_err();
    }

    #[tokio::test]
    async fn validate_upload_repository_rejects_another_project() {
        let storage = in_memory(None);
        let upload_id = storage.start_upload(&project()).await.unwrap();
        let other: ProjectUuid = OTHER_PROJECT.parse().unwrap();

        let error = storage
            .validate_upload_repository(&upload_id, &other)
            .await
            .unwrap_err();
        assert!(matches!(error, OciStorageError::UploadNotFound(_)));
    }

    #[tokio::test]
    async fn stale_uploads_are_cleaned_up() {
        let storage = in_memory(None);
        let upload_id = storage.start_upload(&project()).await.unwrap();
        storage
            .append_upload(&upload_id, Bytes::from_static(b"stale data"))
            .await
            .unwrap();

        // Backdate the session past the timeout
        let state = super::UploadState {
            repository: PROJECT.to_owned(),
            created_at: 0,
        };
        storage
            .store
            .put(
                &storage.upload_state_key(&upload_id),
                serde_json::to_vec(&state).unwrap().into(),
            )
            .await
            .unwrap();

        cleanup_stale_uploads(
            &log(),
            &storage.store,
            &storage.uploads_prefix(),
            3600,
            &clock(),
        )
        .await;

        storage.get_upload_size(&upload_id).await.unwrap_err();
        assert!(
            storage
                .list_prefix(&storage.upload_prefix(&upload_id))
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn fresh_uploads_survive_cleanup() {
        let storage = in_memory(None);
        let upload_id = storage.start_upload(&project()).await.unwrap();
        storage
            .append_upload(&upload_id, Bytes::from_static(b"fresh data"))
            .await
            .unwrap();

        cleanup_stale_uploads(
            &log(),
            &storage.store,
            &storage.uploads_prefix(),
            3600,
            &clock(),
        )
        .await;

        assert_eq!(storage.get_upload_size(&upload_id).await.unwrap(), 10);
    }

    // ==================== Blobs ====================

    async fn store_blob(
        storage: &OciStorage,
        repository: &ProjectUuid,
        data: &'static [u8],
    ) -> Digest {
        let upload_id = storage.start_upload(repository).await.unwrap();
        storage
            .append_upload(&upload_id, Bytes::from_static(data))
            .await
            .unwrap();
        storage
            .complete_upload(&upload_id, &Digest::from_sha256_bytes(data))
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn get_blob_stream_returns_the_content() {
        use http_body_util::BodyExt as _;

        let storage = in_memory(None);
        let repository = project();
        let stored = store_blob(&storage, &repository, b"streamed blob").await;

        let (body, size) = storage.get_blob_stream(&repository, &stored).await.unwrap();
        assert_eq!(size, 13);
        let collected = body.collect().await.unwrap().to_bytes();
        assert_eq!(collected.as_ref(), b"streamed blob");
    }

    #[tokio::test]
    async fn missing_blobs_report_not_found() {
        let storage = in_memory(None);
        let repository = project();
        let missing = Digest::from_sha256_bytes(b"no such blob");

        assert!(!storage.blob_exists(&repository, &missing).await.unwrap());
        assert!(matches!(
            storage.get_blob(&repository, &missing).await.unwrap_err(),
            OciStorageError::BlobNotFound(_)
        ));
        assert!(matches!(
            storage
                .get_blob_size(&repository, &missing)
                .await
                .unwrap_err(),
            OciStorageError::BlobNotFound(_)
        ));
        // Deleting a blob that is not there is not an error
        storage.delete_blob(&repository, &missing).await.unwrap();
    }

    async fn mount_and_delete_blobs(storage: &OciStorage) {
        let repository = project();
        let other: ProjectUuid = OTHER_PROJECT.parse().unwrap();
        let stored = store_blob(storage, &repository, b"mountable blob").await;

        assert!(
            storage
                .mount_blob(&repository, &other, &stored)
                .await
                .unwrap()
        );
        assert!(storage.blob_exists(&other, &stored).await.unwrap());

        // Mounting a blob the source project does not have reports no mount
        let missing = Digest::from_sha256_bytes(b"not mountable");
        assert!(
            !storage
                .mount_blob(&repository, &other, &missing)
                .await
                .unwrap()
        );

        storage.delete_blob(&other, &stored).await.unwrap();
        assert!(!storage.blob_exists(&other, &stored).await.unwrap());
        assert!(storage.blob_exists(&repository, &stored).await.unwrap());
    }

    #[tokio::test]
    async fn mount_and_delete_blobs_in_memory() {
        mount_and_delete_blobs(&in_memory(None)).await;
    }

    #[tokio::test]
    async fn mount_and_delete_blobs_on_disk() {
        let (_tmp, storage) = on_disk();
        mount_and_delete_blobs(&storage).await;
    }

    // ==================== Manifests and tags ====================

    #[tokio::test]
    async fn put_and_get_manifest_by_digest() {
        let storage = in_memory(None);
        let repository = project();
        let content = manifest_json(&format!("sha256:{SUBJECT_SHA256}"));
        let manifest = bencher_json::oci::Manifest::from_bytes(content.as_bytes()).unwrap();

        let stored = storage
            .put_manifest(&repository, Bytes::from(content.clone()), None, &manifest)
            .await
            .unwrap();

        assert!(storage.manifest_exists(&repository, &stored).await.unwrap());
        assert_eq!(
            storage
                .get_manifest_by_digest(&repository, &stored)
                .await
                .unwrap()
                .as_ref(),
            content.as_bytes()
        );

        let missing = Digest::from_sha256_bytes(b"no such manifest");
        assert!(
            !storage
                .manifest_exists(&repository, &missing)
                .await
                .unwrap()
        );
        assert!(matches!(
            storage
                .get_manifest_by_digest(&repository, &missing)
                .await
                .unwrap_err(),
            OciStorageError::ManifestNotFound(_)
        ));
    }

    #[tokio::test]
    async fn resolve_and_delete_tags() {
        let storage = in_memory(None);
        let repository = project();
        let content = manifest_json(&format!("sha256:{SUBJECT_SHA256}"));
        let manifest = bencher_json::oci::Manifest::from_bytes(content.as_bytes()).unwrap();
        let tag: crate::types::Tag = "latest".parse().unwrap();

        let stored = storage
            .put_manifest(&repository, Bytes::from(content), Some(&tag), &manifest)
            .await
            .unwrap();
        assert_eq!(
            storage
                .resolve_tag(&repository, &tag)
                .await
                .unwrap()
                .as_str(),
            stored.as_str()
        );

        storage.delete_tag(&repository, &tag).await.unwrap();
        assert!(matches!(
            storage.resolve_tag(&repository, &tag).await.unwrap_err(),
            OciStorageError::ManifestNotFound(_)
        ));
        // Deleting a tag that is not there is not an error
        storage.delete_tag(&repository, &tag).await.unwrap();
    }

    #[tokio::test]
    async fn delete_manifest_removes_its_tags() {
        let storage = in_memory(None);
        let repository = project();
        let content = manifest_json(&format!("sha256:{SUBJECT_SHA256}"));
        let manifest = bencher_json::oci::Manifest::from_bytes(content.as_bytes()).unwrap();
        let first: crate::types::Tag = "v1".parse().unwrap();
        let second: crate::types::Tag = "v2".parse().unwrap();

        let stored = storage
            .put_manifest(
                &repository,
                Bytes::from(content.clone()),
                Some(&first),
                &manifest,
            )
            .await
            .unwrap();
        storage
            .put_manifest(&repository, Bytes::from(content), Some(&second), &manifest)
            .await
            .unwrap();

        storage.delete_manifest(&repository, &stored).await.unwrap();

        storage.resolve_tag(&repository, &first).await.unwrap_err();
        storage.resolve_tag(&repository, &second).await.unwrap_err();
        assert!(!storage.manifest_exists(&repository, &stored).await.unwrap());
    }

    async fn tag_listing(storage: &OciStorage) {
        let repository = project();
        for name in ["alpha", "beta", "gamma", "delta", "epsilon"] {
            let content = manifest_json(&format!("sha256:{SUBJECT_SHA256}"));
            let manifest = bencher_json::oci::Manifest::from_bytes(content.as_bytes()).unwrap();
            let tag: crate::types::Tag = name.parse().unwrap();
            // A distinct manifest per tag keeps the tags independent
            let content = format!("{content}\n{name}");
            storage
                .put_manifest(&repository, Bytes::from(content), Some(&tag), &manifest)
                .await
                .unwrap();
        }

        let all = storage.list_tags(&repository, None, None).await.unwrap();
        assert_eq!(
            all.tags,
            ["alpha", "beta", "delta", "epsilon", "gamma"],
            "tags are lexicographic"
        );
        assert!(!all.has_more);

        // Walk the pages
        let mut seen = Vec::new();
        let mut cursor: Option<String> = None;
        loop {
            let ListTagsResult { tags, has_more } = storage
                .list_tags(&repository, Some(2), cursor.as_deref())
                .await
                .unwrap();
            assert!(tags.len() <= 2);
            seen.extend(tags.iter().cloned());
            if !has_more {
                break;
            }
            cursor = tags.last().cloned();
        }
        assert_eq!(seen, ["alpha", "beta", "delta", "epsilon", "gamma"]);
    }

    #[tokio::test]
    async fn tag_listing_in_memory() {
        tag_listing(&in_memory(None)).await;
    }

    #[tokio::test]
    async fn tag_listing_on_disk() {
        let (_tmp, storage) = on_disk();
        tag_listing(&storage).await;
    }

    #[tokio::test]
    async fn list_tags_on_an_empty_repository() {
        let storage = in_memory(None);
        let result = storage.list_tags(&project(), Some(10), None).await.unwrap();
        assert!(result.tags.is_empty());
        assert!(!result.has_more);
    }

    // ==================== Referrers ====================

    #[tokio::test]
    async fn referrers_are_listed_and_filtered() {
        let storage = in_memory(None);
        let repository = project();
        let subject = digest(SUBJECT_SHA256);

        let content = serde_json::json!({
            "schemaVersion": 2,
            "mediaType": "application/vnd.oci.image.manifest.v1+json",
            "artifactType": "application/vnd.bencher.report",
            "config": {
                "mediaType": "application/vnd.oci.image.config.v1+json",
                "digest": format!("sha256:{EMPTY_SHA256}"),
                "size": 0
            },
            "layers": [],
            "subject": {
                "mediaType": "application/vnd.oci.image.manifest.v1+json",
                "digest": format!("sha256:{SUBJECT_SHA256}"),
                "size": 100
            }
        })
        .to_string();
        let manifest = bencher_json::oci::Manifest::from_bytes(content.as_bytes()).unwrap();
        let stored = storage
            .put_manifest(&repository, Bytes::from(content), None, &manifest)
            .await
            .unwrap();

        let referrers = storage
            .list_referrers(&repository, &subject, None)
            .await
            .unwrap();
        assert_eq!(referrers.len(), 1);
        assert_eq!(referrers[0].digest, stored.to_string());

        assert_eq!(
            storage
                .list_referrers(
                    &repository,
                    &subject,
                    Some("application/vnd.bencher.report")
                )
                .await
                .unwrap()
                .len(),
            1
        );
        assert!(
            storage
                .list_referrers(&repository, &subject, Some("application/vnd.other"))
                .await
                .unwrap()
                .is_empty()
        );

        // Deleting the manifest takes its referrer link with it
        storage.delete_manifest(&repository, &stored).await.unwrap();
        assert!(
            storage
                .list_referrers(&repository, &subject, None)
                .await
                .unwrap()
                .is_empty()
        );
    }

    // ==================== Job output ====================

    fn job_output(stdout: &str) -> bencher_json::runner::JsonJobOutput {
        bencher_json::runner::JsonJobOutput {
            results: vec![bencher_json::runner::JsonIterationOutput {
                exit_code: 0,
                stdout: Some(stdout.into()),
                stderr: None,
                output: None,
            }],
            error: None,
        }
    }

    async fn job_output_round_trip(storage: &OciStorage) {
        let repository = project();
        let other: ProjectUuid = OTHER_PROJECT.parse().unwrap();
        let job: bencher_json::JobUuid = JOB.parse().unwrap();

        assert!(
            storage
                .job_output()
                .get(repository, job)
                .await
                .unwrap()
                .is_none()
        );

        storage
            .job_output()
            .put(repository, job, &job_output("first"))
            .await
            .unwrap();
        let stored = storage
            .job_output()
            .get(repository, job)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stored.results[0].stdout.as_deref(), Some("first"));

        // Output is scoped to the project
        assert!(
            storage
                .job_output()
                .get(other, job)
                .await
                .unwrap()
                .is_none()
        );

        // A second write replaces the first
        storage
            .job_output()
            .put(repository, job, &job_output("second"))
            .await
            .unwrap();
        let stored = storage
            .job_output()
            .get(repository, job)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stored.results[0].stdout.as_deref(), Some("second"));
    }

    #[tokio::test]
    async fn job_output_round_trip_in_memory() {
        job_output_round_trip(&in_memory(None)).await;
    }

    #[tokio::test]
    async fn job_output_round_trip_on_disk() {
        let (_tmp, storage) = on_disk();
        job_output_round_trip(&storage).await;
    }

    // ==================== Configuration ====================

    #[test]
    fn chunk_size_is_clamped() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("bencher.db");
        let build = |chunk_size| {
            OciStorage::try_from_config(
                log(),
                Some(RegistryStorage::S3 {
                    bucket: "my-bucket".to_owned(),
                    path: None,
                    endpoint: Some("https://s3.example.com".to_owned()),
                    region: None,
                    access_key_id: "key".to_owned(),
                    secret_access_key: "secret".parse().unwrap(),
                    chunk_size,
                }),
                &db_path,
                None,
                None,
                Some(clock()),
            )
            .unwrap()
            .chunk_size()
        };

        let default = default_chunk_size();
        assert_eq!(build(None), default);
        assert_eq!(build(Some(1)), default, "below the floor");
        assert_eq!(build(Some(10 * 1024 * 1024)), 10 * 1024 * 1024);
        assert_eq!(
            build(Some(u64::MAX)),
            usize::try_from(MAX_CHUNK_SIZE).unwrap(),
            "above the ceiling"
        );
    }

    #[test]
    fn s3_storage_takes_a_key_prefix() {
        let tmp = tempfile::tempdir().unwrap();
        let storage = OciStorage::try_from_config(
            log(),
            Some(RegistryStorage::S3 {
                bucket: "my-bucket".to_owned(),
                path: Some("registry".to_owned()),
                endpoint: Some("http://localhost:9000".to_owned()),
                region: Some("auto".to_owned()),
                access_key_id: "key".to_owned(),
                secret_access_key: "secret".parse().unwrap(),
                chunk_size: None,
            }),
            &tmp.path().join("bencher.db"),
            None,
            None,
            Some(clock()),
        )
        .unwrap();

        assert_eq!(
            storage.blob_key(&project(), &digest(EMPTY_SHA256)).as_ref(),
            format!("registry/{PROJECT}/blobs/sha256/{EMPTY_SHA256}")
        );
        // The S3 backend never touches the local filesystem
        assert!(!tmp.path().join("registry").exists());
    }
}
