//! OCI Storage Layer
//!
//! One implementation over [`object_store::ObjectStore`], configured as either a
//! local filesystem directory or any S3-compatible bucket. Keys are identical
//! across both backends, so the same registry data can be moved between them.

use std::num::NonZeroU64;
use std::path::{Path as FilePath, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::task::{Context, Poll};
use std::time::Duration;

use std::convert::Infallible;

use bencher_json::{
    Clock, ProjectUuid, Secret,
    system::config::{
        DEFAULT_CHUNK_SIZE, DEFAULT_MAX_BODY_SIZE, DEFAULT_MAX_UPLOAD_MEMORY,
        DEFAULT_UPLOAD_TIMEOUT_SECS, MAX_CHUNK_SIZE, MIN_CHUNK_SIZE, RegistryStorage,
    },
};
use bytes::Bytes;
use futures::stream::{self, BoxStream, StreamExt as _, TryStreamExt as _};
use hyper::body::Frame;
use object_store::multipart::MultipartStore;
use object_store::{
    ClientOptions, ObjectMeta, ObjectStore, ObjectStoreExt as _, aws::AmazonS3, path::Path,
};
use serde::Deserialize;
use sha2::{Digest as _, Sha256};
use slog::Logger;
use sync_wrapper::SyncWrapper;
use thiserror::Error;
use tokio::runtime::Handle;
use tokio::task::{JoinError, JoinSet};

use crate::buffers::BufferPool;
use crate::file_multipart::FileMultipart;
use crate::metrics::{self, SweepAction};
use crate::part_copy::PartCopy;
use crate::recording::{RecordingMultipart, RecordingStore};
use crate::s3_error;
use crate::types::{Digest, UploadId};

mod session;
mod upload;
#[cfg(test)]
mod upload_tests;

pub use session::UploadSession;
use session::{GENERATION_DIGITS, HashState, Phase, Progress, SessionState};
use upload::Uploads;
pub use upload::{Appended, Completed, UploadWriteError};

/// Maximum concurrency for parallel object store operations.
/// Clamped to available CPU parallelism, with this upper bound to prevent
/// excessive resource usage.
pub const MAX_CONCURRENCY: usize = 64;

/// Most parts an S3 multipart upload can have.
pub(crate) const MAX_PARTS: u64 = 10_000;

/// The part a mount copies by on S3. A blob of one part or less copies whole.
const MOUNT_PART_SIZE: NonZeroU64 = match NonZeroU64::new(16 * 1024 * 1024) {
    Some(part_size) => part_size,
    None => panic!("the mount part size is zero"),
};

/// Longest an S3-compatible store may go without sending a byte, which reqwest
/// also applies from sending a request to its response headers, so four parts of
/// the default 16 MiB uploading at once need only about 224 KB/s to finish inside it.
const READ_TIMEOUT: Duration = Duration::from_mins(5);

/// Directory holding registry data, created beside the database file.
const REGISTRY_DIR: &str = "registry";

/// Key prefix for the in-progress upload staging area.
const UPLOADS_PREFIX: &str = "_uploads";

/// An upload session's staging object, the multipart upload its parts go to
const DATA: &str = "data";

/// Key prefix for an upload session's state generations
const STATES: &str = "states";

/// Key prefix for an upload session's tails, by offset
const TAIL: &str = "tail";

/// The state object of an upload session in the old layout, which only the sweep reads
const OLD_STATE: &str = "state.json";

/// Region used when none is configured. S3-compatible stores expect this.
const DEFAULT_REGION: &str = "auto";

/// OCI storage backed by a local directory or an S3-compatible bucket
pub struct OciStorage {
    store: Arc<RecordingStore>,
    /// The multipart uploads that upload sessions stream into
    multipart: Arc<RecordingMultipart>,
    /// The S3 backend's server-side part copy, which the object store trait hides
    part_copy: Option<PartCopy>,
    /// Key prefix within the store, if configured
    prefix: Option<Path>,
    /// Upload timeout in seconds for stale upload cleanup
    upload_timeout: u64,
    /// Maximum body size in bytes for uploads
    max_body_size: u64,
    /// The part size in bytes that new upload sessions take.
    /// See [`bencher_json::system::config::DEFAULT_CHUNK_SIZE`].
    chunk_size: usize,
    /// Memory for upload part buffers, shared by every request
    buffers: BufferPool,
    /// Logger for error/warning reporting
    log: Logger,
    /// Concurrency limit for parallel referrer and tag fetches
    concurrency: usize,
    /// Stale upload sweeps, owned so shutdown can stop them and wait for them
    sweep: Mutex<Sweep>,
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

    #[error("ETag mismatch: {0}")]
    EtagMismatch(String),

    #[error("An upload part did not fill within {} seconds", .0.as_secs())]
    PartStalled(Duration),
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
            Self::PartStalled(_) => http::StatusCode::REQUEST_TIMEOUT,
            Self::Storage(_) | Self::Config(_) | Self::Json(_) | Self::EtagMismatch(_) => {
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
        Self::try_from_config_with_upload_memory(
            log,
            storage,
            database_path,
            upload_timeout,
            max_body_size,
            None,
            clock,
        )
    }

    /// Creates a new OCI storage instance from configuration, with
    /// `max_upload_memory` bytes for upload part buffers.
    /// Pass `None` to use the default (512 MiB).
    pub fn try_from_config_with_upload_memory(
        log: Logger,
        storage: Option<RegistryStorage>,
        database_path: &FilePath,
        upload_timeout: Option<u64>,
        max_body_size: Option<u64>,
        max_upload_memory: Option<u64>,
        clock: Option<Clock>,
    ) -> Result<Self, OciStorageError> {
        let (store, multipart, part_copy, prefix, chunk_size) =
            match storage.unwrap_or(RegistryStorage::File { path: None }) {
                RegistryStorage::File { path } => {
                    let dir = path.unwrap_or_else(|| registry_dir(database_path));
                    let store = file_store(&dir)?;
                    let multipart: Arc<dyn MultipartStore> =
                        Arc::new(FileMultipart::new(Arc::clone(&store)));
                    (store, multipart, None, None, None)
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
                    let options = s3_client_options(endpoint.as_deref());
                    let s3 = s3_store(
                        bucket.clone(),
                        endpoint,
                        region,
                        access_key_id,
                        &secret_access_key,
                        options.clone(),
                    )?;
                    let part_copy = PartCopy::new(s3.clone(), bucket, &options, log.clone())?;
                    let multipart: Arc<dyn MultipartStore> = Arc::new(s3.clone());
                    let store: Arc<dyn ObjectStore> = Arc::new(s3);
                    (
                        store,
                        multipart,
                        Some(part_copy),
                        path.map(Path::from),
                        chunk_size,
                    )
                },
            };

        let concurrency = std::thread::available_parallelism()
            .map_or(1, std::num::NonZeroUsize::get)
            .clamp(1, MAX_CONCURRENCY);
        let chunk_size = chunk_size
            .unwrap_or(DEFAULT_CHUNK_SIZE)
            .clamp(MIN_CHUNK_SIZE, MAX_CHUNK_SIZE) as usize;
        let max_body_size = max_body_size.unwrap_or(DEFAULT_MAX_BODY_SIZE);

        let storage = Self {
            store: Arc::new(RecordingStore::new(store)),
            multipart: Arc::new(RecordingMultipart::new(multipart)),
            part_copy,
            prefix,
            upload_timeout: upload_timeout.unwrap_or(DEFAULT_UPLOAD_TIMEOUT_SECS),
            max_body_size,
            chunk_size,
            buffers: BufferPool::new(
                max_upload_memory.unwrap_or(DEFAULT_MAX_UPLOAD_MEMORY),
                part_size(chunk_size, max_body_size) as u64,
            ),
            log,
            concurrency,
            sweep: Mutex::default(),
            clock: clock.unwrap_or(Clock::System),
        };
        // An idle registry still sweeps once at startup. Built outside a runtime,
        // the storage sweeps first at its first upload instead.
        if Handle::try_current().is_ok() {
            storage.spawn_stale_upload_cleanup();
        }
        Ok(storage)
    }

    /// Returns a view type for job output storage operations.
    pub fn job_output(&self) -> crate::job_output::JobOutput<'_> {
        crate::job_output::JobOutput::new(self)
    }

    /// Returns the configured maximum body size in bytes
    pub fn max_body_size(&self) -> u64 {
        self.max_body_size
    }

    /// Returns the configured part size that new upload sessions take.
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

    /// Returns the key of a session's staging object
    fn upload_data_key(&self, upload_id: &UploadId) -> Path {
        self.upload_prefix(upload_id).join(DATA)
    }

    /// Returns the key prefix of a session's state generations
    fn upload_states_prefix(&self, upload_id: &UploadId) -> Path {
        self.upload_prefix(upload_id).join(STATES)
    }

    /// Returns the key of the tail a session commits at `offset`
    fn upload_tail_key(&self, upload_id: &UploadId, offset: u64) -> Path {
        self.upload_prefix(upload_id)
            .join(TAIL)
            .join(offset.to_string())
    }

    /// Returns the key of one generation of a session's state.
    fn upload_state_generation_key(&self, upload_id: &UploadId, generation: u64) -> Path {
        self.upload_states_prefix(upload_id)
            .join(format!("{generation:0GENERATION_DIGITS$}"))
    }

    /// Returns the key for a blob
    fn blob_key(&self, repository: &ProjectUuid, digest: &Digest) -> Path {
        blob_key(&self.root(), repository, digest)
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
        result.bytes().await.map_err(|e| storage_error(&e))
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
            Err(e) => Err(storage_error(&e)),
        }
    }

    /// Lists every object under a prefix.
    async fn list_prefix(&self, prefix: &Path) -> Result<Vec<ObjectMeta>, OciStorageError> {
        self.store
            .list(Some(prefix))
            .try_collect()
            .await
            .map_err(|e| storage_error(&e))
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
        let state = SessionState {
            repository: *repository,
            created_at: self.clock.timestamp(),
            part_size: part_size(self.chunk_size, self.max_body_size) as u64,
            upload: None,
            phase: Phase::Open(Progress {
                offset: 0,
                parts: Vec::new(),
                hash: HashState::of(&Sha256::new()),
                tail: None,
            }),
        };
        self.uploads().create(&upload_id, state).await?;
        Ok(upload_id)
    }

    /// Appends data to an in-progress upload, and commits it.
    ///
    /// Returns the cumulative size of the upload session.
    pub async fn append_upload(
        &self,
        upload_id: &UploadId,
        data: Bytes,
    ) -> Result<u64, OciStorageError> {
        let session = self.uploads().load(upload_id).await?;
        let appended = self
            .append_upload_body(session, stream::iter([Ok::<_, Infallible>(data)]))
            .await
            .map_err(storage_only)?;
        if appended.committed {
            Ok(appended.offset)
        } else {
            Err(OciStorageError::Storage(format!(
                "Another request appended to upload {upload_id} first"
            )))
        }
    }

    /// Gets the current size of an in-progress upload
    pub async fn get_upload_size(&self, upload_id: &UploadId) -> Result<u64, OciStorageError> {
        Ok(self.uploads().load(upload_id).await?.offset())
    }

    /// Completes an upload and stores the blob
    ///
    /// The bytes already stream into a multipart upload as they arrive, so the
    /// finish completes it and moves it to the blob's key, reading no byte back.
    /// A digest mismatch aborts the upload, so no blob is left behind.
    pub async fn complete_upload(
        &self,
        upload_id: &UploadId,
        expected_digest: &Digest,
    ) -> Result<Digest, OciStorageError> {
        let session = self.uploads().load(upload_id).await?;
        self.complete_upload_body(
            session,
            expected_digest,
            stream::empty::<Result<Bytes, Infallible>>(),
        )
        .await
        .map(|completed| completed.digest)
        .map_err(storage_only)
    }

    /// Validates that the upload session belongs to the expected repository
    pub async fn validate_upload_repository(
        &self,
        upload_id: &UploadId,
        expected_repository: &ProjectUuid,
    ) -> Result<(), OciStorageError> {
        self.upload_session(upload_id, expected_repository)
            .await
            .map(|_session| ())
    }

    /// Loads a session for a request that names `repository`.
    pub async fn upload_session(
        &self,
        upload_id: &UploadId,
        repository: &ProjectUuid,
    ) -> Result<UploadSession, OciStorageError> {
        self.uploads().session(upload_id, repository).await
    }

    /// Appends a request body to a session, and commits it once, at its end.
    pub async fn append_upload_body<S, E>(
        &self,
        session: UploadSession,
        body: S,
    ) -> Result<Appended, UploadWriteError<E>>
    where
        S: futures::Stream<Item = Result<Bytes, E>>,
    {
        self.uploads().append(session, body).await
    }

    /// Finishes a session as the blob `digest`, after it appends a final request body.
    ///
    /// A session that an earlier request left past its digest check resumes where
    /// it stopped, and takes no more bytes.
    pub async fn complete_upload_body<S, E>(
        &self,
        session: UploadSession,
        digest: &Digest,
        body: S,
    ) -> Result<Completed, UploadWriteError<E>>
    where
        S: futures::Stream<Item = Result<Bytes, E>>,
    {
        self.uploads().complete(session, digest, body).await
    }

    /// Cancels a session, by its newest state: one that is still open is aborted
    /// and removed, while one past its digest check is left for a retry to
    /// finish, or for the sweep.
    pub async fn cancel_upload(&self, upload_id: &UploadId) -> Result<(), OciStorageError> {
        self.uploads().cancel(upload_id).await
    }

    fn uploads(&self) -> Uploads<'_> {
        Uploads::new(self)
    }

    /// Spawns a background task to clean up all stale uploads that have exceeded the timeout.
    ///
    /// Debounced: skips if a cleanup ran within the last `upload_timeout` seconds,
    /// since stale uploads can't appear faster than the timeout period.
    fn spawn_stale_upload_cleanup(&self) {
        let mut sweep = self.lock_sweep();
        while let Some(joined) = sweep.tasks.try_join_next() {
            report_sweep_join(&self.log, joined);
        }
        if sweep.closed {
            return;
        }
        let now = self.clock.timestamp();
        let timeout_secs = i64::try_from(self.upload_timeout).unwrap_or(i64::MAX);
        if now.saturating_sub(sweep.last) < timeout_secs {
            return;
        }
        sweep.last = now;

        let sweeper = self.sweeper();
        sweep.tasks.spawn(async move {
            sweeper.sweep().await;
        });
    }

    /// Stops new stale upload sweeps and hands over those in flight, for shutdown to stop.
    pub fn close_upload_sweep(&self) -> UploadSweep {
        let mut sweep = self.lock_sweep();
        sweep.closed = true;
        UploadSweep(std::mem::take(&mut sweep.tasks))
    }

    // A spawn and a close share one lock, so no sweep starts once shutdown has taken them.
    fn lock_sweep(&self) -> MutexGuard<'_, Sweep> {
        self.sweep.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// What a sweep, or the removal of one session, needs
    fn sweeper(&self) -> Sweeper {
        Sweeper {
            log: self.log.clone(),
            store: Arc::clone(&self.store),
            multipart: Arc::clone(&self.multipart),
            root: self.root(),
            upload_timeout: self.upload_timeout,
            clock: self.clock.clone(),
        }
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
    /// between checking existence and copying. On S3, a blob larger than one
    /// part copies by parts in parallel, where one `CopyObject` would rewrite
    /// every byte in turn.
    pub async fn mount_blob(
        &self,
        from_repository: &ProjectUuid,
        to_repository: &ProjectUuid,
        digest: &Digest,
    ) -> Result<bool, OciStorageError> {
        let source = self.blob_key(from_repository, digest);
        let destination = self.blob_key(to_repository, digest);
        if let Some(part_copy) = &self.part_copy {
            let size = match self.store.head(&source).await {
                Ok(meta) => meta.size,
                Err(e) if is_not_found(&e) => return Ok(false),
                Err(e) => return Err(storage_error(&e)),
            };
            if size > MOUNT_PART_SIZE.get() {
                self.mount_by_parts(part_copy, &source, &destination, size)
                    .await
                    .inspect_err(|error| {
                        slog::warn!(self.log, "OCI mount by parts failed"; "error" => %error);
                    })?;
                return Ok(true);
            }
        }
        match self.store.copy(&source, &destination).await {
            Ok(()) => Ok(true),
            Err(e) if is_not_found(&e) => Ok(false),
            Err(e) => Err(storage_error(&e)),
        }
    }

    /// Copies a blob by parts, and aborts the copy's upload when any step fails.
    async fn mount_by_parts(
        &self,
        part_copy: &PartCopy,
        source: &Path,
        destination: &Path,
        size: u64,
    ) -> Result<(), OciStorageError> {
        let upload_id = part_copy.open(destination).await?;
        let copied = async {
            let parts = part_copy
                .copy_parts(source, destination, &upload_id, size, MOUNT_PART_SIZE)
                .await?;
            part_copy
                .complete(destination, &upload_id, parts, size)
                .await
        }
        .await;
        if let Err(error) = copied {
            if let Err(abort) = part_copy.abort(destination, &upload_id).await {
                report_cleanup_error(&self.log, "mount_blob: abort part copy", &abort);
            }
            return Err(error);
        }
        Ok(())
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
        let objects: Vec<ObjectMeta> = listed.try_collect().await.map_err(|e| storage_error(&e))?;

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
                let data = result.bytes().await.map_err(|e| storage_error(&e))?;
                serde_json::from_slice(&data)
                    .map(Some)
                    .map_err(|e| OciStorageError::Json(e.to_string()))
            },
            Err(e) if is_not_found(&e) => Ok(None),
            Err(e) => Err(storage_error(&e)),
        }
    }

    // ==================== Shared Helpers ====================

    /// Writes an object.
    async fn put(&self, key: &Path, data: Bytes) -> Result<(), OciStorageError> {
        self.store
            .put(key, data.into())
            .await
            .map(|_result| ())
            .map_err(|e| storage_error(&e))
    }

    /// Reports whether an object exists.
    async fn exists(&self, key: &Path) -> Result<bool, OciStorageError> {
        match self.store.head(key).await {
            Ok(_meta) => Ok(true),
            Err(e) if is_not_found(&e) => Ok(false),
            Err(e) => Err(storage_error(&e)),
        }
    }
}

/// A streaming body for blob content
///
/// The object store hands back a `Send` but not `Sync` stream, while the HTTP
/// body must be both. Polling only ever needs `&mut`, which is exactly what
/// [`SyncWrapper`] makes safe.
///
/// A stream that fails part way asks the store again for the rest, so its error
/// can hold an answer's body: the error the server sees holds only its text, redacted.
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
            Poll::Ready(Some(Err(e))) => Poll::Ready(Some(Err(s3_error::redact(&e).into()))),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }

    fn size_hint(&self) -> hyper::body::SizeHint {
        hyper::body::SizeHint::with_exact(self.size)
    }
}

/// Stale upload sweeps handed over at shutdown
pub struct UploadSweep(JoinSet<()>);

impl UploadSweep {
    /// Cuts the sweeps short and waits for them, since a sweep is best effort.
    pub async fn stop(mut self, log: &Logger) {
        self.0.abort_all();
        while let Some(joined) = self.0.join_next().await {
            report_sweep_join(log, joined);
        }
    }
}

/// Stale upload sweep state, behind one lock
#[derive(Default)]
struct Sweep {
    tasks: JoinSet<()>,
    /// Unix timestamp of the last sweep, for debouncing
    last: i64,
    /// Set at shutdown, after which no sweep starts
    closed: bool,
}

/// Result of listing tags with pagination support
pub struct ListTagsResult {
    /// The tags returned
    pub tags: Vec<String>,
    /// Whether more tags exist beyond the requested limit
    pub has_more: bool,
}

/// Reports a failed cleanup, whose error never holds an answer's raw body.
pub(crate) fn report_cleanup_error(log: &Logger, context: &str, error: &OciStorageError) {
    slog::warn!(log, "OCI cleanup error"; "context" => context, "error" => %error);
    #[cfg(feature = "sentry")]
    sentry::capture_message(
        &format!("OCI cleanup error ({context}): {error}"),
        sentry::Level::Warning,
    );
}

fn report_sweep_join(log: &Logger, joined: Result<(), JoinError>) {
    if let Err(e) = joined
        && !e.is_cancelled()
    {
        slog::error!(log, "Stale upload sweep failed: {e}");
    }
}

/// The multipart part size: at least `chunk_size`, and large enough that the
/// largest allowed body fits in [`MAX_PARTS`] parts.
fn part_size(chunk_size: usize, max_body_size: u64) -> usize {
    let fits = usize::try_from(max_body_size.div_ceil(MAX_PARTS)).unwrap_or(usize::MAX);
    chunk_size.max(fits)
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

/// The HTTP client options that the S3 store and its part copy share.
fn s3_client_options(endpoint: Option<&str>) -> ClientOptions {
    ClientOptions::new()
        .with_timeout_disabled()
        .with_read_timeout(READ_TIMEOUT)
        .with_allow_http(endpoint.is_some_and(|endpoint| endpoint.starts_with("http://")))
}

/// Builds an S3-compatible store.
fn s3_store(
    bucket: String,
    endpoint: Option<String>,
    region: Option<String>,
    access_key_id: String,
    secret_access_key: &Secret,
    client_options: ClientOptions,
) -> Result<AmazonS3, OciStorageError> {
    let mut builder = object_store::aws::AmazonS3Builder::new()
        .with_client_options(client_options)
        .with_bucket_name(bucket)
        .with_access_key_id(access_key_id)
        .with_secret_access_key(secret_access_key.as_ref())
        .with_region(region.unwrap_or_else(|| DEFAULT_REGION.to_owned()));
    if let Some(endpoint) = endpoint {
        builder = builder.with_endpoint(endpoint);
    }
    builder
        .build()
        .map_err(|e| OciStorageError::Config(e.to_string()))
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
        storage_error(error)
    }
}

/// A storage error from an object store error, with any answer's body redacted.
pub(crate) fn storage_error(error: &object_store::Error) -> OciStorageError {
    OciStorageError::Storage(s3_error::redact(error))
}

/// The key of a blob under `root`
fn blob_key(root: &Path, repository: &ProjectUuid, digest: &Digest) -> Path {
    root.clone()
        .join(repository.to_string())
        .join("blobs")
        .join(digest.algorithm())
        .join(digest.hex_hash())
}

/// The storage failure of a call whose body cannot fail.
fn storage_only(error: UploadWriteError<Infallible>) -> OciStorageError {
    match error {
        UploadWriteError::Body(never) => match never {},
        UploadWriteError::Storage(error) => error,
    }
}

/// The stale upload sweep, and the removal of one session, apart from the storage
/// that starts them, so a sweep can run as its own task.
pub(crate) struct Sweeper {
    log: Logger,
    store: Arc<RecordingStore>,
    multipart: Arc<RecordingMultipart>,
    root: Path,
    upload_timeout: u64,
    clock: Clock,
}

impl Sweeper {
    /// Removes every upload session that has outlived the timeout, in any phase.
    ///
    /// The sweep never finishes an upload: it aborts the uploads that a session's
    /// newest state saved, which never touches a completed object, and deletes the
    /// rest. A session in the old layout has no uploads to abort.
    pub(crate) async fn sweep(&self) {
        let uploads_prefix = self.root.clone().join(UPLOADS_PREFIX);
        let listed = match self.store.list_with_delimiter(Some(&uploads_prefix)).await {
            Ok(listed) => listed,
            Err(e) => {
                report_cleanup_error(&self.log, "stale_upload: list prefixes", &storage_error(&e));
                return;
            },
        };

        // `now` pairs with `created_at` in the application time domain, `os_now`
        // with object `last_modified` in the OS wall-clock domain.
        let (now, os_now) = self.clock.timestamps();
        let timeout_secs = i64::try_from(self.upload_timeout).unwrap_or(i64::MAX);

        for upload_prefix in listed.common_prefixes {
            let Some(upload_id) = upload_prefix.filename() else {
                continue;
            };
            if upload_id.parse::<UploadId>().is_err() {
                report_cleanup_error(
                    &self.log,
                    "stale_upload: parse upload ID",
                    &OciStorageError::InvalidContent(format!("Invalid upload ID: {upload_id}")),
                );
                continue;
            }

            let objects = match self
                .store
                .list(Some(&upload_prefix))
                .try_collect::<Vec<_>>()
                .await
            {
                Ok(objects) => objects,
                Err(e) => {
                    report_cleanup_error(
                        &self.log,
                        "stale_upload: list objects",
                        &storage_error(&e),
                    );
                    continue;
                },
            };

            let state = match newest_state_key(&upload_prefix, &objects) {
                Some(key) => self.read(&key).await,
                None => None,
            };
            let created_at = state
                .as_deref()
                .and_then(|data| serde_json::from_slice::<Created>(data).ok())
                .map(|created| created.created_at);
            if !is_stale(created_at, &objects, now, os_now, timeout_secs) {
                continue;
            }
            let session = state
                .as_deref()
                .and_then(|data| serde_json::from_slice::<SessionState>(data).ok());
            let aborted = self.abort(&upload_prefix, session.as_ref()).await;
            for _ in 0..aborted {
                metrics::upload_sweep(SweepAction::Aborted);
            }
            if self.delete(&upload_prefix, objects).await {
                metrics::upload_sweep(SweepAction::Deleted);
            }
        }
    }

    /// Aborts a session's uploads by the IDs its state saved, then deletes its
    /// objects, its states last.
    async fn remove(&self, upload_prefix: &Path, state: Option<&SessionState>) {
        self.abort(upload_prefix, state).await;
        match self
            .store
            .list(Some(upload_prefix))
            .try_collect::<Vec<_>>()
            .await
        {
            Ok(objects) => {
                self.delete(upload_prefix, objects).await;
            },
            Err(e) => {
                report_cleanup_error(&self.log, "cleanup_upload: list", &storage_error(&e));
            },
        }
    }

    /// Aborts the staging upload, and on S3 the destination upload, that a state
    /// saved, and returns how many aborted.
    async fn abort(&self, upload_prefix: &Path, state: Option<&SessionState>) -> u32 {
        let Some(state) = state else {
            return 0;
        };
        let mut uploads = Vec::new();
        if let Some(upload) = &state.upload {
            uploads.push((upload_prefix.clone().join(DATA), upload));
        }
        if let Phase::Promoting(promotion) = &state.phase
            && let Some(destination) = &promotion.destination
            && let Ok(digest) = promotion.finished.digest.parse::<Digest>()
        {
            uploads.push((
                blob_key(&self.root, &state.repository, &digest),
                destination,
            ));
        }
        let mut aborted = 0;
        for (key, upload) in uploads {
            match self.multipart.abort_multipart(&key, upload).await {
                Ok(()) => aborted += 1,
                Err(e) => {
                    report_cleanup_error(&self.log, "cleanup_upload: abort", &storage_error(&e));
                },
            }
        }
        aborted
    }

    /// Deletes a session's objects in 2 batches: every object but its states, then
    /// its states. The states stay while any other object does, so a sweep that
    /// finds what a crash left still reads a state that names the session's
    /// uploads. A batch gives its deletes no order, so a crash inside the states
    /// batch can leave any of them, the newest included.
    ///
    /// Returns whether it removed the session: it listed objects, and each is now gone.
    async fn delete(&self, upload_prefix: &Path, objects: Vec<ObjectMeta>) -> bool {
        let listed = !objects.is_empty();
        let states_prefix = upload_prefix.clone().join(STATES);
        let old_state = upload_prefix.clone().join(OLD_STATE);
        let (mut states, rest): (Vec<_>, Vec<_>) = objects
            .into_iter()
            .map(|meta| meta.location)
            .partition(|key| key.prefix_matches(&states_prefix) || *key == old_state);
        states.sort();

        listed && self.delete_batch(rest).await && self.delete_batch(states).await
    }

    /// Deletes keys together, as one request where the store allows, and returns
    /// whether each is now gone.
    async fn delete_batch(&self, keys: Vec<Path>) -> bool {
        let failures: Vec<object_store::Error> = self
            .store
            .delete_stream(stream::iter(keys.into_iter().map(Ok)).boxed())
            .filter_map(|deleted| async move { deleted.err().filter(|e| !is_not_found(e)) })
            .collect()
            .await;
        for e in &failures {
            report_cleanup_error(&self.log, "cleanup_upload: delete", &storage_error(e));
        }
        failures.is_empty()
    }

    async fn read(&self, key: &Path) -> Option<Bytes> {
        self.store.get(key).await.ok()?.bytes().await.ok()
    }
}

/// The creation time of any session's state, in the old layout or the new.
#[derive(Deserialize)]
struct Created {
    created_at: i64,
}

/// The key of a session's newest state, in the new layout or the old.
fn newest_state_key(upload_prefix: &Path, objects: &[ObjectMeta]) -> Option<Path> {
    let states_prefix = upload_prefix.clone().join(STATES);
    let old_state = upload_prefix.clone().join(OLD_STATE);
    objects
        .iter()
        .map(|meta| &meta.location)
        .filter(|key| key.prefix_matches(&states_prefix))
        .filter_map(|key| Some((session::generation(key)?, key)))
        .max_by_key(|(generation, _key)| *generation)
        .map(|(_generation, key)| key.clone())
        .or_else(|| {
            objects
                .iter()
                .any(|meta| meta.location == old_state)
                .then_some(old_state)
        })
}

/// Whether an upload session has outlived the timeout.
///
/// Prefers the session's recorded creation time. When no state is readable,
/// falls back to the newest object under the prefix, so a session whose state
/// has not been written yet is not deleted out from under an in-flight upload.
fn is_stale(
    created_at: Option<i64>,
    objects: &[ObjectMeta],
    now: i64,
    os_now: i64,
    timeout_secs: i64,
) -> bool {
    if let Some(created_at) = created_at {
        return now.saturating_sub(created_at) > timeout_secs;
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
    use std::num::NonZeroU64;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use bencher_json::DateTime;
    use bytes::Bytes;
    use futures::TryStreamExt as _;
    use object_store::multipart::{MultipartStore, PartId};
    use object_store::{ObjectStore, ObjectStoreExt as _, memory::InMemory, path::Path};
    use slog::Logger;

    use super::{
        BufferPool, Clock, DEFAULT_CHUNK_SIZE, DEFAULT_MAX_BODY_SIZE, DEFAULT_MAX_UPLOAD_MEMORY,
        Digest, ListTagsResult, MAX_CHUNK_SIZE, MAX_PARTS, MIN_CHUNK_SIZE, OciStorage,
        OciStorageError, ProjectUuid, READ_TIMEOUT, RecordingMultipart, RecordingStore,
        RegistryStorage, Sweep, UploadId, part_size,
    };
    use crate::mock_s3::{self, Request, Response};
    use crate::part_copy::{Completion, PartCopy};

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
    fn storage(
        store: Arc<dyn ObjectStore>,
        multipart: Arc<dyn MultipartStore>,
        prefix: Option<&str>,
    ) -> OciStorage {
        OciStorage {
            store: Arc::new(RecordingStore::new(store)),
            multipart: Arc::new(RecordingMultipart::new(multipart)),
            part_copy: None,
            prefix: prefix.map(Path::from),
            upload_timeout: 3600,
            max_body_size: DEFAULT_MAX_BODY_SIZE,
            chunk_size: default_chunk_size(),
            buffers: BufferPool::new(DEFAULT_MAX_UPLOAD_MEMORY, DEFAULT_CHUNK_SIZE),
            log: log(),
            concurrency: 4,
            sweep: Mutex::new(Sweep {
                last: DateTime::TEST.timestamp(),
                ..Sweep::default()
            }),
            clock: clock(),
        }
    }

    fn in_memory(prefix: Option<&str>) -> OciStorage {
        let store = Arc::new(InMemory::new());
        storage(store.clone(), store, prefix)
    }

    /// Makes an upload session look older than any upload timeout.
    async fn backdate(storage: &OciStorage, upload_id: &UploadId) {
        let session = storage.uploads().load(upload_id).await.unwrap();
        let mut state = session.state().clone();
        state.created_at = 0;
        storage
            .store
            .put(
                &storage.upload_state_generation_key(upload_id, session.generation()),
                serde_json::to_vec(&state).unwrap().into(),
            )
            .await
            .unwrap();
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
            storage.upload_data_key(&upload_id).as_ref(),
            "_uploads/3f8d9c1e-0000-4000-8000-000000000abc/data"
        );
        assert_eq!(
            storage.upload_state_generation_key(&upload_id, 7).as_ref(),
            "_uploads/3f8d9c1e-0000-4000-8000-000000000abc/states/00000000000000000007"
        );
        assert_eq!(
            storage.upload_tail_key(&upload_id, 1234).as_ref(),
            "_uploads/3f8d9c1e-0000-4000-8000-000000000abc/tail/1234"
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
            storage.upload_data_key(&upload_id).as_ref(),
            "registry/_uploads/3f8d9c1e-0000-4000-8000-000000000abc/data"
        );
        assert_eq!(
            storage.upload_state_generation_key(&upload_id, 7).as_ref(),
            "registry/_uploads/3f8d9c1e-0000-4000-8000-000000000abc/states/00000000000000000007"
        );
        assert_eq!(
            storage.upload_tail_key(&upload_id, 1234).as_ref(),
            "registry/_uploads/3f8d9c1e-0000-4000-8000-000000000abc/tail/1234"
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
        const UPLOAD: &str = "3f8d9c1e-0000-4000-8000-000000000abc";
        for (raw, encoded) in [
            ("r\u{e9}gistry", "r%C3%A9gistry"),
            (
                "\u{30ec}\u{30b8}\u{30b9}\u{30c8}\u{30ea}",
                "%E3%83%AC%E3%82%B8%E3%82%B9%E3%83%88%E3%83%AA",
            ),
            ("team#1", "team%231"),
        ] {
            let storage = in_memory(Some(raw));
            let repository = project();
            let upload_id: UploadId = UPLOAD.parse().unwrap();

            assert_eq!(
                storage
                    .blob_key(&repository, &digest(EMPTY_SHA256))
                    .as_ref(),
                format!("{encoded}/{PROJECT}/blobs/sha256/{EMPTY_SHA256}"),
                "blob key for {raw}"
            );
            assert_eq!(
                storage
                    .manifest_key(&repository, &digest(EMPTY_SHA256))
                    .as_ref(),
                format!("{encoded}/{PROJECT}/manifests/sha256/{EMPTY_SHA256}"),
                "manifest key for {raw}"
            );
            assert_eq!(
                storage.tag_key(&repository, "latest").as_ref(),
                format!("{encoded}/{PROJECT}/tags/latest"),
                "tag key for {raw}"
            );
            assert_eq!(
                storage
                    .referrer_key(&repository, &digest(SUBJECT_SHA256), &digest(EMPTY_SHA256))
                    .as_ref(),
                format!(
                    "{encoded}/{PROJECT}/referrers/sha256/{SUBJECT_SHA256}/sha256-{EMPTY_SHA256}"
                ),
                "referrer key for {raw}"
            );
            assert_eq!(
                storage
                    .job_output_key(repository, JOB.parse().unwrap())
                    .as_ref(),
                format!("{encoded}/{PROJECT}/output/v0/jobs/{JOB}"),
                "job output key for {raw}"
            );
            assert_eq!(
                storage.upload_data_key(&upload_id).as_ref(),
                format!("{encoded}/_uploads/{UPLOAD}/data"),
                "upload data key for {raw}"
            );
            assert_eq!(
                storage.upload_state_generation_key(&upload_id, 7).as_ref(),
                format!("{encoded}/_uploads/{UPLOAD}/states/00000000000000000007"),
                "upload state key for {raw}"
            );
            assert_eq!(
                storage.upload_tail_key(&upload_id, 1234).as_ref(),
                format!("{encoded}/_uploads/{UPLOAD}/tail/1234"),
                "upload tail key for {raw}"
            );
        }
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

        // The session's objects are gone, tails and states included, leaving only the blob
        storage.get_upload_size(&upload_id).await.unwrap_err();
        let remaining: Vec<Path> = storage
            .store
            .list(None)
            .map_ok(|meta| meta.location)
            .try_collect()
            .await
            .unwrap();
        assert_eq!(remaining, [storage.blob_key(&repository, &stored)]);
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
    /// agree on the key, or completing an upload finds no session.
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

    /// Appends stream into a multipart upload, so the reassembled
    /// blob must be byte exact across several parts with a non-aligned tail.
    #[tokio::test]
    async fn complete_upload_spans_multipart_boundaries() {
        let mut storage = in_memory(None);
        storage.chunk_size = 16;
        storage.max_body_size = 1024;
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

        backdate(&storage, &upload_id).await;

        storage.sweeper().sweep().await;

        storage.get_upload_size(&upload_id).await.unwrap_err();
        assert_eq!(
            storage
                .list_prefix(&storage.upload_prefix(&upload_id))
                .await
                .unwrap(),
            Vec::new()
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

        storage.sweeper().sweep().await;

        assert_eq!(storage.get_upload_size(&upload_id).await.unwrap(), 10);
    }

    #[tokio::test]
    async fn a_stale_session_is_swept_when_the_storage_is_built() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("bencher.db");
        let earlier =
            OciStorage::try_from_config(log(), None, &db_path, None, None, Some(clock())).unwrap();
        earlier.close_upload_sweep().stop(&log()).await;
        let upload_id = earlier.start_upload(&project()).await.unwrap();
        earlier
            .append_upload(&upload_id, Bytes::from_static(b"left by a crash"))
            .await
            .unwrap();
        backdate(&earlier, &upload_id).await;

        let storage =
            OciStorage::try_from_config(log(), None, &db_path, None, None, Some(clock())).unwrap();
        let mut sweep = storage.close_upload_sweep();
        while let Some(joined) = sweep.0.join_next().await {
            joined.unwrap();
        }

        assert_eq!(
            storage
                .list_prefix(&storage.upload_prefix(&upload_id))
                .await
                .unwrap(),
            Vec::new(),
            "the stale session is gone"
        );
    }

    #[tokio::test]
    async fn closing_the_upload_sweep_hands_it_over_and_starts_no_more() {
        let mut storage = in_memory(None);
        storage.upload_timeout = 0;

        storage.start_upload(&project()).await.unwrap();
        let sweep = storage.close_upload_sweep();
        assert_eq!(sweep.0.len(), 1, "the sweep started by the upload");
        sweep.stop(&log()).await;

        storage.start_upload(&project()).await.unwrap();
        assert!(
            storage.close_upload_sweep().0.is_empty(),
            "no sweep starts once closed"
        );
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
        assert_eq!(result.tags, Vec::<String>::new());
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
    fn part_size_fits_the_largest_body_in_the_part_limit() {
        let chunk_size = default_chunk_size();
        assert_eq!(
            part_size(chunk_size, DEFAULT_MAX_BODY_SIZE),
            chunk_size,
            "the default body limit fits at the chunk size"
        );

        let max_body_size = 1024 * 1024 * 1024 * 1024;
        let size = u64::try_from(part_size(chunk_size, max_body_size)).unwrap();
        assert!(
            max_body_size.div_ceil(size) <= MAX_PARTS,
            "a larger body limit fits in the part limit"
        );
        assert!(
            max_body_size.div_ceil(size - 1) > MAX_PARTS,
            "at the smallest part size that does"
        );
    }

    const SLOW_FRAME: &[u8] = b"0123456789abcdef";
    const SLOW_FRAMES: u32 = 20;
    /// Inside the read timeout, while all the gaps together run well past it
    const SLOW_FRAME_GAP: Duration = Duration::from_secs(30);

    /// S3 storage against a mock store on `listener`, beside a ticker task.
    ///
    /// Paused time jumps to the next timer whenever the runtime idles, so a
    /// short tick keeps one jump from ever spanning a read timeout.
    fn mock_s3(
        listener: &tokio::net::TcpListener,
        tasks: &mut tokio::task::JoinSet<()>,
    ) -> (tempfile::TempDir, OciStorage) {
        tasks.spawn(async {
            for _ in 0..100_000 {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        });
        let tmp = tempfile::tempdir().unwrap();
        let storage = OciStorage::try_from_config(
            log(),
            Some(RegistryStorage::S3 {
                bucket: mock_s3::BUCKET.to_owned(),
                path: None,
                endpoint: Some(format!("http://{}", listener.local_addr().unwrap())),
                region: None,
                access_key_id: mock_s3::ACCESS_KEY_ID.to_owned(),
                secret_access_key: mock_s3::SECRET_ACCESS_KEY.parse().unwrap(),
                chunk_size: None,
            }),
            &tmp.path().join("bencher.db"),
            None,
            None,
            Some(clock()),
        )
        .unwrap();
        (tmp, storage)
    }

    /// Accepts one request, reads it through its headers, and answers with the
    /// headers for a blob of `length` bytes.
    async fn answer_blob_request(
        listener: tokio::net::TcpListener,
        length: usize,
    ) -> tokio::net::TcpStream {
        use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

        let (mut socket, _peer) = listener.accept().await.unwrap();
        let mut request = Vec::new();
        let mut buf = [0; 1024];
        while !request.windows(4).any(|window| window == b"\r\n\r\n") {
            let read = socket.read(&mut buf).await.unwrap();
            assert_ne!(read, 0, "the request ended before its headers");
            request.extend_from_slice(&buf[..read]);
        }
        socket
            .write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {length}\r\n\r\n").as_bytes())
            .await
            .unwrap();
        socket
    }

    /// Answers one request with a blob body that trickles out a frame at a time.
    async fn serve_slow_blob(listener: tokio::net::TcpListener) {
        use tokio::io::AsyncWriteExt as _;

        let length = SLOW_FRAME.len() * usize::try_from(SLOW_FRAMES).unwrap();
        let mut socket = answer_blob_request(listener, length).await;
        for _ in 0..SLOW_FRAMES {
            tokio::time::sleep(SLOW_FRAME_GAP).await;
            socket.write_all(SLOW_FRAME).await.unwrap();
        }
    }

    /// Answers one request with headers alone, then hangs up after `stall`.
    async fn serve_stalled_blob(listener: tokio::net::TcpListener, stall: Duration) {
        let socket = answer_blob_request(listener, SLOW_FRAME.len()).await;
        tokio::time::sleep(stall).await;
        drop(socket);
    }

    /// A blob that streams steadily for longer than any whole request deadline
    /// still arrives, since the S3 client only gives up on a read that stalls.
    #[tokio::test(start_paused = true)]
    async fn s3_blob_streams_past_a_whole_request_deadline() {
        use http_body_util::BodyExt as _;

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let mut tasks = tokio::task::JoinSet::new();
        let (_tmp, storage) = mock_s3(&listener, &mut tasks);
        tasks.spawn(serve_slow_blob(listener));

        let start = tokio::time::Instant::now();
        let (body, size) = storage
            .get_blob_stream(&project(), &digest(EMPTY_SHA256))
            .await
            .unwrap();
        let data = body.collect().await.unwrap().to_bytes();

        assert_eq!(
            data,
            SLOW_FRAME.repeat(usize::try_from(SLOW_FRAMES).unwrap()),
            "the whole body arrives"
        );
        assert_eq!(size, u64::try_from(data.len()).unwrap());
        assert!(
            start.elapsed() >= SLOW_FRAME_GAP * SLOW_FRAMES,
            "the body took {:?}",
            start.elapsed()
        );
    }

    /// A blob whose store goes silent fails once the read timeout passes,
    /// instead of waiting for the store to hang up.
    #[tokio::test(start_paused = true)]
    async fn s3_blob_read_times_out_when_the_store_stalls() {
        use http_body_util::BodyExt as _;

        let stall = READ_TIMEOUT * 2;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let mut tasks = tokio::task::JoinSet::new();
        let (_tmp, storage) = mock_s3(&listener, &mut tasks);
        tasks.spawn(serve_stalled_blob(listener, stall));

        let start = tokio::time::Instant::now();
        let (body, _size) = storage
            .get_blob_stream(&project(), &digest(EMPTY_SHA256))
            .await
            .unwrap();
        body.collect().await.unwrap_err();

        let elapsed = start.elapsed();
        assert!(
            elapsed >= READ_TIMEOUT && elapsed < stall,
            "the read gave up after {elapsed:?}"
        );
    }

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
        assert_eq!(
            build(Some(1)),
            usize::try_from(MIN_CHUNK_SIZE).unwrap(),
            "below the floor"
        );
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

    // ==================== S3 part copy ====================

    const MIB: u64 = 1024 * 1024;
    /// The part a mount copies by
    const PART: u64 = 16 * MIB;

    /// S3 storage against the mock store, which answers each request with `answer`.
    async fn s3_answering<A, F>(
        answer: A,
    ) -> (
        tokio::task::JoinSet<()>,
        tempfile::TempDir,
        OciStorage,
        mock_s3::Log,
    )
    where
        A: Fn(Request) -> F + Clone + Send + Sync + 'static,
        F: Future<Output = Response> + Send + 'static,
    {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let mut tasks = tokio::task::JoinSet::new();
        let (tmp, storage) = mock_s3(&listener, &mut tasks);
        let log = mock_s3::Log::default();
        tasks.spawn(mock_s3::serve(listener, log.clone(), answer));
        (tasks, tmp, storage, log)
    }

    /// S3 storage against a mock bucket that holds `objects`, by key and size.
    async fn s3_bucket(
        objects: Vec<(String, u64)>,
    ) -> (
        tokio::task::JoinSet<()>,
        tempfile::TempDir,
        OciStorage,
        mock_s3::Log,
    ) {
        s3_answering(move |request| {
            let response = mock_s3::bucket(&request, &objects);
            async move { response }
        })
        .await
    }

    /// S3 storage against a mock bucket that answers each part copy with
    /// `refuse` until it returns `None`, and counts the part copies.
    async fn s3_refusing<R>(
        refuse: R,
    ) -> (
        tokio::task::JoinSet<()>,
        tempfile::TempDir,
        OciStorage,
        mock_s3::Log,
    )
    where
        R: Fn(u32, &Request) -> Option<Response> + Send + Sync + 'static,
    {
        let attempts = Arc::new(AtomicU32::new(0));
        let refuse = Arc::new(refuse);
        s3_answering(move |request| {
            let response = if request.is_part_copy() {
                let attempt = attempts.fetch_add(1, Ordering::SeqCst) + 1;
                refuse(attempt, &request)
            } else {
                None
            }
            .unwrap_or_else(|| mock_s3::bucket(&request, &[]));
            async move { response }
        })
        .await
    }

    fn part_copy(storage: &OciStorage) -> &PartCopy {
        storage.part_copy.as_ref().unwrap()
    }

    fn mib_parts() -> NonZeroU64 {
        NonZeroU64::new(MIB).unwrap()
    }

    fn upload_id() -> String {
        mock_s3::UPLOAD_ID.to_owned()
    }

    fn copy_source() -> Path {
        Path::from("source/blob")
    }

    fn copy_destination() -> Path {
        Path::from("destination/blob")
    }

    /// Copies one part of `MIB` bytes from the copy source to the copy destination.
    async fn copy_one_part(storage: &OciStorage) -> Result<Vec<PartId>, OciStorageError> {
        part_copy(storage)
            .copy_parts(
                &copy_source(),
                &copy_destination(),
                &upload_id(),
                MIB,
                mib_parts(),
            )
            .await
            .map_err(OciStorageError::from)
    }

    fn part_copies(log: &mock_s3::Log) -> Vec<Request> {
        log.requests()
            .into_iter()
            .filter(Request::is_part_copy)
            .collect()
    }

    /// Like R2, the mock refuses a part copy whose signature leaves out a copy
    /// header, or covers a value other than the one sent. The first part
    /// finishes last, so the parts come back out of order.
    #[tokio::test]
    async fn a_part_copy_signs_both_copy_headers_with_the_values_it_sends() {
        let (_tasks, _tmp, storage, log) = s3_answering(|request| {
            let response = mock_s3::bucket(&request, &[]);
            let first = request.part_number() == Some(1);
            async move {
                if first {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                response
            }
        })
        .await;

        let parts = part_copy(&storage)
            .copy_parts(
                &copy_source(),
                &copy_destination(),
                &upload_id(),
                2 * MIB + 1,
                mib_parts(),
            )
            .await
            .unwrap();

        assert_eq!(
            parts
                .iter()
                .map(|part| part.content_id.clone())
                .collect::<Vec<_>>(),
            [
                mock_s3::part_e_tag(1),
                mock_s3::part_e_tag(2),
                mock_s3::part_e_tag(3)
            ],
            "the parts come back in order"
        );
        let copies = part_copies(&log);
        let mut sent: Vec<(u64, String, String)> = copies
            .iter()
            .map(|request| {
                (
                    request.part_number().unwrap(),
                    request.header("x-amz-copy-source").unwrap().to_owned(),
                    request
                        .header("x-amz-copy-source-range")
                        .unwrap()
                        .to_owned(),
                )
            })
            .collect();
        sent.sort();
        let source = format!("{}/source/blob", mock_s3::BUCKET);
        assert_eq!(
            sent,
            [
                (1, source.clone(), "bytes=0-1048575".to_owned()),
                (2, source.clone(), "bytes=1048576-2097151".to_owned()),
                (3, source, "bytes=2097152-2097152".to_owned()),
            ]
        );
        for request in &copies {
            assert_eq!(request.method, "PUT");
            assert_eq!(request.key(), "destination/blob");
            assert_eq!(
                request.query("uploadId").as_deref(),
                Some(mock_s3::UPLOAD_ID)
            );
            assert_eq!(
                request.query("X-Amz-SignedHeaders").as_deref(),
                Some("host;x-amz-copy-source;x-amz-copy-source-range"),
                "both copy headers are signed"
            );
        }
        assert!(
            log.answers()
                .iter()
                .all(|(_request, status)| *status == 200),
            "the store took every signature"
        );
    }

    /// A copy source names its key as `object_store` writes it in a request path,
    /// so a part copy and a `CopyObject` name the same source.
    #[tokio::test]
    async fn a_copy_source_under_an_encoded_prefix_is_encoded_as_a_request_path() {
        const PREFIX: &str = "r\u{e9}gistry/team#1";
        let repository = project();
        let other: ProjectUuid = OTHER_PROJECT.parse().unwrap();
        let large = digest(EMPTY_SHA256);
        let small = digest(SUBJECT_SHA256);
        let keys = in_memory(Some(PREFIX));
        let (_tasks, _tmp, mut storage, log) = s3_bucket(vec![
            (keys.blob_key(&repository, &large).to_string(), PART + 1),
            (keys.blob_key(&repository, &small).to_string(), PART),
        ])
        .await;
        storage.prefix = Some(Path::from(PREFIX));

        assert!(
            storage
                .mount_blob(&repository, &other, &large)
                .await
                .unwrap()
        );
        assert!(
            storage
                .mount_blob(&repository, &other, &small)
                .await
                .unwrap()
        );

        let sources: Vec<String> = log
            .requests()
            .iter()
            .filter_map(|request| request.header("x-amz-copy-source").map(ToOwned::to_owned))
            .collect();
        let source = |hex: &str| {
            format!(
                "{}/r%25C3%25A9gistry/team%25231/{PROJECT}/blobs/sha256/{hex}",
                mock_s3::BUCKET
            )
        };
        assert_eq!(
            sources,
            [
                source(EMPTY_SHA256),
                source(EMPTY_SHA256),
                source(SUBJECT_SHA256)
            ],
            "two part copies, then one CopyObject"
        );
    }

    /// S3 can answer a copy with a 200 whose body is an `<Error>`.
    #[tokio::test]
    async fn a_part_copy_answered_with_an_error_body_retries() {
        let (_tasks, _tmp, storage, log) = s3_refusing(|attempt, _request| {
            (attempt == 1).then(|| Response::error(200, "InternalError"))
        })
        .await;

        let parts = copy_one_part(&storage).await.unwrap();

        assert_eq!(parts.len(), 1);
        assert_eq!(part_copies(&log).len(), 2, "one retry");
    }

    #[tokio::test]
    async fn a_part_copy_fails_after_its_last_attempt() {
        let (_tasks, _tmp, storage, log) =
            s3_refusing(|_attempt, _request| Some(Response::error(200, "InternalError"))).await;

        let error = copy_one_part(&storage).await.unwrap_err();

        assert!(error.to_string().contains("InternalError"), "{error}");
        assert_eq!(part_copies(&log).len(), 4, "four attempts");
    }

    #[tokio::test]
    async fn a_part_copy_retries_a_busy_or_failing_store() {
        let (_tasks, _tmp, storage, log) = s3_refusing(|attempt, _request| match attempt {
            1 => Some(Response::error(503, "SlowDown")),
            2 => Some(Response::error(429, "TooManyRequests")),
            3 => Some(Response::error(500, "InternalError")),
            _ => None,
        })
        .await;

        copy_one_part(&storage).await.unwrap();

        assert_eq!(part_copies(&log).len(), 4, "the last attempt succeeds");
    }

    #[tokio::test]
    async fn a_part_copy_retries_a_dropped_connection() {
        let (_tasks, _tmp, storage, log) =
            s3_refusing(|attempt, _request| (attempt == 1).then(Response::hang_up)).await;

        copy_one_part(&storage).await.unwrap();

        assert_eq!(part_copies(&log).len(), 2, "one retry");
    }

    /// A part copy's URL holds the access key ID and a signature, so a failure to
    /// reach the store names no URL.
    #[tokio::test]
    async fn a_part_copy_that_cannot_reach_the_store_names_no_signed_url() {
        let (_tasks, _tmp, storage, _log) =
            s3_refusing(|_attempt, _request| Some(Response::hang_up())).await;

        let error = copy_one_part(&storage).await.unwrap_err();

        let OciStorageError::Storage(text) = &error else {
            panic!("{error:?}");
        };
        assert!(text.contains("connection"), "{text}");
        for text in [text.clone(), error.to_string()] {
            for secret in [
                "X-Amz-Credential",
                "X-Amz-Signature",
                mock_s3::ACCESS_KEY_ID,
            ] {
                assert!(!text.contains(secret), "{text} holds {secret}");
            }
        }
    }

    /// Every attempt signs its own URL, so an expired one is signed again.
    #[tokio::test]
    async fn a_part_copy_answered_with_expired_request_retries() {
        let (_tasks, _tmp, storage, log) = s3_refusing(|attempt, _request| {
            (attempt == 1).then(|| Response::error(403, "ExpiredRequest"))
        })
        .await;

        copy_one_part(&storage).await.unwrap();

        let statuses: Vec<u16> = log
            .answers()
            .iter()
            .filter(|(request, _status)| request.is_part_copy())
            .map(|(_request, status)| *status)
            .collect();
        assert_eq!(statuses, [403, 200], "the signed retry is taken");
    }

    #[tokio::test]
    async fn a_refused_part_copy_signature_fails_at_once() {
        let (_tasks, _tmp, storage, log) =
            s3_refusing(|_attempt, request| Some(mock_s3::signature_does_not_match(request))).await;

        let error = copy_one_part(&storage).await.unwrap_err();

        assert_eq!(part_copies(&log).len(), 1, "no retry");
        let text = error.to_string();
        assert!(text.contains("SignatureDoesNotMatch"), "{text}");
        assert!(text.contains(mock_s3::SIGNATURE_MESSAGE), "{text}");
        let host = part_copies(&log)[0].header("host").unwrap().to_owned();
        for echoed in [
            mock_s3::ACCESS_KEY_ID.to_owned(),
            host.clone(),
            mock_s3::hex_bytes(mock_s3::ACCESS_KEY_ID),
            mock_s3::hex_bytes(&host),
        ] {
            assert!(!text.contains(&echoed), "{text} holds {echoed}");
        }
    }

    #[tokio::test]
    async fn a_failed_part_copy_aborts_nothing() {
        let (_tasks, _tmp, storage, log) =
            s3_refusing(|_attempt, _request| Some(Response::error(403, "AccessDenied"))).await;

        copy_one_part(&storage).await.unwrap_err();

        assert!(
            !log.requests().iter().any(Request::is_abort),
            "aborting is the caller's call"
        );
    }

    /// A mount stops starting parts once one fails, lets the copies in flight
    /// finish, and only then aborts its upload.
    #[tokio::test]
    async fn a_failed_mount_aborts_after_the_copies_in_flight_finish() {
        let repository = project();
        let other: ProjectUuid = OTHER_PROJECT.parse().unwrap();
        let blob = digest(EMPTY_SHA256);
        let objects = vec![(
            in_memory(None).blob_key(&repository, &blob).to_string(),
            17 * PART,
        )];
        let (_tasks, _tmp, storage, log) = s3_answering(move |request| {
            let first = request.part_number() == Some(1);
            let response = if request.is_part_copy() && first {
                Response::error(403, "AccessDenied")
            } else {
                mock_s3::bucket(&request, &objects)
            };
            let slow = request.is_part_copy() && !first;
            async move {
                if slow {
                    tokio::time::sleep(Duration::from_millis(200)).await;
                }
                response
            }
        })
        .await;

        storage
            .mount_blob(&repository, &other, &blob)
            .await
            .unwrap_err();

        let answers = log.answers();
        let mut copied: Vec<u64> = answers
            .iter()
            .filter(|(request, _status)| request.is_part_copy())
            .map(|(request, _status)| request.part_number().unwrap())
            .collect();
        copied.sort_unstable();
        assert_eq!(
            copied,
            (1..=16).collect::<Vec<_>>(),
            "no part starts once one fails"
        );
        let last_copy = answers
            .iter()
            .rposition(|(request, _status)| request.is_part_copy())
            .unwrap();
        let abort = answers
            .iter()
            .position(|(request, _status)| request.is_abort())
            .expect("the mount aborts its upload");
        assert!(
            last_copy < abort,
            "the abort waits for the copies in flight"
        );
        assert!(
            !answers
                .iter()
                .any(|(request, _status)| request.is_complete()),
            "nothing completes"
        );
    }

    fn two_parts() -> Vec<PartId> {
        [1, 2]
            .map(|number| PartId {
                content_id: mock_s3::part_e_tag(number),
            })
            .to_vec()
    }

    /// The multipart `ETag` of parts `"part 1"` and `"part 2"`, worked out apart from this code.
    const TWO_PARTS_E_TAG: &str = "\"cd8e045aced0cb4620c8dfe11dffb901-2\"";

    #[tokio::test]
    async fn a_completion_matches_the_multipart_etag_of_its_parts() {
        let (_tasks, _tmp, storage, _log) = s3_bucket(Vec::new()).await;

        let completion = part_copy(&storage)
            .complete(&copy_destination(), &upload_id(), two_parts(), 2 * MIB)
            .await
            .unwrap();

        assert!(
            matches!(&completion, Completion::Completed { e_tag } if e_tag == TWO_PARTS_E_TAG),
            "{completion:?}"
        );
    }

    #[tokio::test]
    async fn a_completion_whose_etag_does_not_match_its_parts_fails() {
        let (_tasks, _tmp, storage, _log) = s3_answering(|request| {
            let response = if request.is_complete() {
                Response::xml(
                    200,
                    "<CompleteMultipartUploadResult><ETag>\"cd8e045aced0cb4620c8dfe11dffb902-2\"</ETag></CompleteMultipartUploadResult>",
                )
            } else {
                mock_s3::bucket(&request, &[])
            };
            async move { response }
        })
        .await;

        part_copy(&storage)
            .complete(&copy_destination(), &upload_id(), two_parts(), 2 * MIB)
            .await
            .unwrap_err();
    }

    /// S3 storage whose completion answers `NoSuchUpload`, over a destination of `size` bytes.
    async fn s3_completed_before(
        code: &'static str,
        size: u64,
    ) -> (
        tokio::task::JoinSet<()>,
        tempfile::TempDir,
        OciStorage,
        mock_s3::Log,
    ) {
        let objects = vec![(copy_destination().to_string(), size)];
        s3_answering(move |request| {
            let response = if request.is_complete() {
                Response::error(404, code)
            } else {
                mock_s3::bucket(&request, &objects)
            };
            async move { response }
        })
        .await
    }

    #[tokio::test]
    async fn a_completion_lost_to_a_retry_finds_the_object_by_its_size() {
        let (_tasks, _tmp, storage, _log) = s3_completed_before("NoSuchUpload", 2 * MIB).await;

        let completion = part_copy(&storage)
            .complete(&copy_destination(), &upload_id(), two_parts(), 2 * MIB)
            .await
            .unwrap();

        assert!(matches!(completion, Completion::Found), "{completion:?}");
    }

    #[tokio::test]
    async fn a_completion_lost_to_a_retry_fails_on_another_size() {
        let (_tasks, _tmp, storage, _log) = s3_completed_before("NoSuchUpload", 2 * MIB - 1).await;

        part_copy(&storage)
            .complete(&copy_destination(), &upload_id(), two_parts(), 2 * MIB)
            .await
            .unwrap_err();
    }

    #[tokio::test]
    async fn a_completion_missing_for_another_reason_fails() {
        let (_tasks, _tmp, storage, _log) = s3_completed_before("NoSuchKey", 2 * MIB).await;

        part_copy(&storage)
            .complete(&copy_destination(), &upload_id(), two_parts(), 2 * MIB)
            .await
            .unwrap_err();
    }

    /// The text of a storage error reaches the server's log, so it holds no more
    /// of a failing answer's body than its `Code` and `Message`.
    #[tokio::test]
    async fn a_storage_error_holds_only_the_code_and_message_of_the_answer() {
        let (_tasks, _tmp, storage, log) = s3_answering(|request| {
            let response = if request.is_list() {
                mock_s3::bucket(&request, &[])
            } else {
                mock_s3::signature_does_not_match(&request)
            };
            async move { response }
        })
        .await;

        let error = storage
            .get_blob(&project(), &digest(EMPTY_SHA256))
            .await
            .unwrap_err();

        let text = error.to_string();
        assert!(text.contains("SignatureDoesNotMatch"), "{text}");
        assert!(text.contains(mock_s3::SIGNATURE_MESSAGE), "{text}");
        let host = log.requests()[0].header("host").unwrap().to_owned();
        for echoed in [
            mock_s3::ACCESS_KEY_ID.to_owned(),
            mock_s3::hex_bytes(mock_s3::ACCESS_KEY_ID),
            mock_s3::hex_bytes(&host),
            format!("host:{host}"),
        ] {
            assert!(!text.contains(&echoed), "{text} holds {echoed}");
        }
    }

    /// A blob stream that fails part way asks again for the rest, and when that
    /// answer fails, its error reaches the server with only its `Code` and `Message`.
    #[tokio::test]
    async fn a_failed_blob_stream_holds_only_the_code_and_message_of_the_answer() {
        use http_body_util::BodyExt as _;

        let gets = Arc::new(AtomicU32::new(0));
        let (_tasks, _tmp, storage, log) = s3_answering(move |request| {
            let response = if request.method == "GET" && !request.is_list() {
                if gets.fetch_add(1, Ordering::SeqCst) == 0 {
                    Response::cut_short(SLOW_FRAME.len() * 2, SLOW_FRAME)
                } else {
                    mock_s3::signature_does_not_match(&request)
                }
            } else {
                mock_s3::bucket(&request, &[])
            };
            async move { response }
        })
        .await;

        let (body, _size) = storage
            .get_blob_stream(&project(), &digest(EMPTY_SHA256))
            .await
            .unwrap();
        let error = body.collect().await.unwrap_err();

        assert_eq!(log.requests().len(), 2, "the stream asked again");
        let text = error.to_string();
        assert!(text.contains("SignatureDoesNotMatch"), "{text}");
        let host = log.requests()[1].header("host").unwrap().to_owned();
        for echoed in [
            mock_s3::ACCESS_KEY_ID.to_owned(),
            mock_s3::hex_bytes(mock_s3::ACCESS_KEY_ID),
            mock_s3::hex_bytes(&host),
            format!("host:{host}"),
        ] {
            assert!(!text.contains(&echoed), "{text} holds {echoed}");
        }
    }

    /// The kinds of the requests a mount sent, after the startup sweep.
    fn mount_requests(log: &mock_s3::Log) -> Vec<String> {
        log.requests()
            .iter()
            .map(|request| {
                if request.is_part_copy() {
                    format!(
                        "part copy {}",
                        request.header("x-amz-copy-source-range").unwrap()
                    )
                } else if request.is_copy_object() {
                    "copy object".to_owned()
                } else if request.is_create_multipart() {
                    "create multipart".to_owned()
                } else if request.is_complete() {
                    "complete".to_owned()
                } else {
                    request.method.clone()
                }
            })
            .collect()
    }

    #[tokio::test]
    async fn an_s3_mount_above_one_part_copies_by_parts() {
        let repository = project();
        let other: ProjectUuid = OTHER_PROJECT.parse().unwrap();
        let blob = digest(EMPTY_SHA256);
        let source = in_memory(None).blob_key(&repository, &blob);
        let (_tasks, _tmp, storage, log) = s3_bucket(vec![(source.to_string(), PART + 1)]).await;

        assert!(
            storage
                .mount_blob(&repository, &other, &blob)
                .await
                .unwrap()
        );

        let mut requests = mount_requests(&log);
        requests[2..4].sort();
        assert_eq!(
            requests,
            [
                "HEAD",
                "create multipart",
                "part copy bytes=0-16777215",
                "part copy bytes=16777216-16777216",
                "complete",
            ]
        );
        let destination = in_memory(None).blob_key(&other, &blob).to_string();
        assert!(
            log.requests()
                .iter()
                .skip(1)
                .all(|request| request.key() == destination),
            "the upload is at the mounted key"
        );
    }

    #[tokio::test]
    async fn an_s3_mount_of_one_part_copies_the_object() {
        let repository = project();
        let other: ProjectUuid = OTHER_PROJECT.parse().unwrap();
        let blob = digest(EMPTY_SHA256);
        let source = in_memory(None).blob_key(&repository, &blob);
        let (_tasks, _tmp, storage, log) = s3_bucket(vec![(source.to_string(), PART)]).await;

        assert!(
            storage
                .mount_blob(&repository, &other, &blob)
                .await
                .unwrap()
        );

        assert_eq!(mount_requests(&log), ["HEAD", "copy object"]);
    }

    #[tokio::test]
    async fn an_s3_mount_of_a_missing_blob_reports_no_mount() {
        let (_tasks, _tmp, storage, log) = s3_bucket(Vec::new()).await;
        let other: ProjectUuid = OTHER_PROJECT.parse().unwrap();

        assert!(
            !storage
                .mount_blob(&project(), &other, &digest(EMPTY_SHA256))
                .await
                .unwrap()
        );

        assert_eq!(mount_requests(&log), ["HEAD"]);
    }
}
