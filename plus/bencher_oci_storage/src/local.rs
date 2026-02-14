//! OCI Storage Layer - Local Filesystem Backend
//!
//! This implementation stores OCI artifacts on the local filesystem,
//! sibling to the database file. If the database is at `data/bencher.db`,
//! OCI data will be stored under `data/oci/`.

use std::io;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::atomic::{AtomicI64, Ordering};
use std::task::{Context, Poll};

use bytes::Bytes;
use http_body_util::StreamBody;
use hyper::body::Frame;
use sha2::{Digest as _, Sha256};
use slog::{Logger, error, warn};
use tokio::fs;
use tokio::io::AsyncWriteExt as _;
use tokio_util::io::ReaderStream;

use bencher_json::ProjectUuid;

use crate::storage::OciStorageError;
use crate::types::{Digest, UploadId};

/// A streaming body for blob content from local filesystem
pub struct LocalBlobBody {
    inner: StreamBody<ReaderStreamAdapter>,
    size: u64,
}

/// Adapter to convert `ReaderStream` errors to `BoxError`
struct ReaderStreamAdapter {
    inner: ReaderStream<fs::File>,
}

impl futures::Stream for ReaderStreamAdapter {
    type Item = Result<Frame<Bytes>, Box<dyn std::error::Error + Send + Sync>>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match Pin::new(&mut self.inner).poll_next(cx) {
            Poll::Ready(Some(Ok(bytes))) => Poll::Ready(Some(Ok(Frame::data(bytes)))),
            Poll::Ready(Some(Err(e))) => {
                let boxed: Box<dyn std::error::Error + Send + Sync> = Box::new(e);
                Poll::Ready(Some(Err(boxed)))
            },
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

impl hyper::body::Body for LocalBlobBody {
    type Data = Bytes;
    type Error = Box<dyn std::error::Error + Send + Sync>;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        Pin::new(&mut self.inner).poll_frame(cx)
    }

    fn size_hint(&self) -> hyper::body::SizeHint {
        hyper::body::SizeHint::with_exact(self.size)
    }
}

impl LocalBlobBody {
    pub(crate) fn new(file: fs::File, size: u64) -> Self {
        let reader_stream = ReaderStream::new(file);
        let adapter = ReaderStreamAdapter {
            inner: reader_stream,
        };
        Self {
            inner: StreamBody::new(adapter),
            size,
        }
    }
}

/// Maps an IO result to an `OciStorageError`, converting `NotFound` errors
/// to the provided error variant and other errors to `LocalStorage` errors.
fn map_io_error<T>(
    result: io::Result<T>,
    not_found_error: OciStorageError,
    other_error_msg: &str,
) -> Result<T, OciStorageError> {
    result.map_err(|e| {
        if e.kind() == io::ErrorKind::NotFound {
            not_found_error
        } else {
            OciStorageError::LocalStorage(format!("{other_error_msg}: {e}"))
        }
    })
}

/// Upload state stored on disk
#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct UploadState {
    /// Repository name
    repository: String,
    /// Total bytes uploaded so far
    size: u64,
    /// Unix timestamp when the upload was created
    created_at: i64,
}

/// OCI Storage implementation using local filesystem
pub struct OciLocalStorage {
    /// Base directory for OCI storage (e.g., `data/oci`)
    base_dir: PathBuf,
    /// Upload timeout in seconds for stale upload cleanup
    upload_timeout: u64,
    /// Maximum body size in bytes for uploads
    max_body_size: u64,
    /// Logger for error reporting
    log: Logger,
    /// Unix timestamp of the last stale upload cleanup (for debouncing)
    last_cleanup: AtomicI64,
    /// Clock for getting the current time (injectable for testing)
    clock: crate::Clock,
}

impl OciLocalStorage {
    /// Creates a new local OCI storage instance
    ///
    /// The `database_path` is the path to the `SQLite` database file.
    /// OCI data will be stored in an `oci` subdirectory next to it.
    pub fn new(
        log: Logger,
        database_path: &Path,
        upload_timeout: u64,
        max_body_size: u64,
        clock: crate::Clock,
    ) -> Self {
        let base_dir = database_path
            .parent()
            .map_or_else(|| PathBuf::from("oci"), |p| p.join("oci"));

        Self {
            base_dir,
            upload_timeout,
            max_body_size,
            log,
            last_cleanup: AtomicI64::new(0),
            clock,
        }
    }

    /// Returns the configured maximum body size in bytes
    pub(crate) fn max_body_size(&self) -> u64 {
        self.max_body_size
    }

    // ==================== Path Generation ====================

    /// Returns the directory for uploads
    fn uploads_dir(&self) -> PathBuf {
        self.base_dir.join("_uploads")
    }

    /// Returns the directory for a specific upload
    fn upload_dir(&self, upload_id: &UploadId) -> PathBuf {
        self.uploads_dir().join(upload_id.to_string())
    }

    /// Returns the path for upload state metadata
    fn upload_state_path(&self, upload_id: &UploadId) -> PathBuf {
        self.upload_dir(upload_id).join("state.json")
    }

    /// Returns the path for upload data
    fn upload_data_path(&self, upload_id: &UploadId) -> PathBuf {
        self.upload_dir(upload_id).join("data")
    }

    /// Returns the directory for a repository
    fn repository_dir(&self, repository: &ProjectUuid) -> PathBuf {
        self.base_dir.join(repository.to_string())
    }

    /// Returns the path for a blob
    fn blob_path(&self, repository: &ProjectUuid, digest: &Digest) -> PathBuf {
        self.repository_dir(repository)
            .join("blobs")
            .join(digest.algorithm())
            .join(digest.hex_hash())
    }

    /// Returns the path for a manifest by digest
    fn manifest_path(&self, repository: &ProjectUuid, digest: &Digest) -> PathBuf {
        self.repository_dir(repository)
            .join("manifests")
            .join(digest.algorithm())
            .join(digest.hex_hash())
    }

    /// Returns the path for a tag link
    fn tag_path(&self, repository: &ProjectUuid, tag: &crate::types::Tag) -> PathBuf {
        self.repository_dir(repository)
            .join("tags")
            .join(tag.as_str())
    }

    /// Returns the directory for referrers to a given digest
    fn referrers_dir(&self, repository: &ProjectUuid, subject_digest: &Digest) -> PathBuf {
        self.repository_dir(repository)
            .join("referrers")
            .join(subject_digest.algorithm())
            .join(subject_digest.hex_hash())
    }

    /// Returns the path for a referrer link
    fn referrer_path(
        &self,
        repository: &ProjectUuid,
        subject_digest: &Digest,
        referrer_digest: &Digest,
    ) -> PathBuf {
        self.referrers_dir(repository, subject_digest).join(format!(
            "{}-{}",
            referrer_digest.algorithm(),
            referrer_digest.hex_hash()
        ))
    }

    // ==================== Job Output ====================

    /// Returns the path for a job output blob.
    fn job_output_path(&self, project: ProjectUuid, job: bencher_json::JobUuid) -> PathBuf {
        self.repository_dir(&project)
            .join("output")
            .join("v0")
            .join("jobs")
            .join(job.to_string())
    }

    pub(crate) async fn put_job_output(
        &self,
        project: ProjectUuid,
        job: bencher_json::JobUuid,
        output: &bencher_json::runner::JsonJobOutput,
    ) -> Result<(), OciStorageError> {
        let path = self.job_output_path(project, job);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await.map_err(|e| {
                OciStorageError::LocalStorage(format!("Failed to create job output directory: {e}"))
            })?;
        }
        let data = serde_json::to_vec(output).map_err(|e| OciStorageError::Json(e.to_string()))?;
        fs::write(&path, &data).await.map_err(|e| {
            OciStorageError::LocalStorage(format!("Failed to write job output: {e}"))
        })?;
        Ok(())
    }

    pub(crate) async fn get_job_output(
        &self,
        project: ProjectUuid,
        job: bencher_json::JobUuid,
    ) -> Result<Option<bencher_json::runner::JsonJobOutput>, OciStorageError> {
        let path = self.job_output_path(project, job);
        match fs::read(&path).await {
            Ok(data) => {
                let output = serde_json::from_slice(&data)
                    .map_err(|e| OciStorageError::Json(e.to_string()))?;
                Ok(Some(output))
            },
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(OciStorageError::LocalStorage(format!(
                "Failed to read job output: {e}"
            ))),
        }
    }

    // ==================== Upload State Management ====================

    /// Validates that the upload session belongs to the expected repository
    pub async fn validate_upload_repository(
        &self,
        upload_id: &UploadId,
        expected_repository: &ProjectUuid,
    ) -> Result<(), OciStorageError> {
        let state = self.load_upload_state(upload_id).await?;
        if state.repository != expected_repository.to_string() {
            return Err(OciStorageError::UploadNotFound(upload_id.to_string()));
        }
        Ok(())
    }

    /// Loads upload state from disk
    async fn load_upload_state(
        &self,
        upload_id: &UploadId,
    ) -> Result<UploadState, OciStorageError> {
        let path = self.upload_state_path(upload_id);
        let data = map_io_error(
            fs::read(&path).await,
            OciStorageError::UploadNotFound(upload_id.to_string()),
            "Failed to read upload state",
        )?;

        serde_json::from_slice(&data).map_err(|e| OciStorageError::Json(e.to_string()))
    }

    /// Saves upload state to disk
    async fn save_upload_state(
        &self,
        upload_id: &UploadId,
        state: &UploadState,
    ) -> Result<(), OciStorageError> {
        let path = self.upload_state_path(upload_id);
        let data = serde_json::to_vec(state).map_err(|e| OciStorageError::Json(e.to_string()))?;
        fs::write(&path, &data).await.map_err(|e| {
            OciStorageError::LocalStorage(format!("Failed to write upload state: {e}"))
        })?;
        Ok(())
    }

    // ==================== Upload Operations ====================

    /// Starts a new upload session
    ///
    /// Also spawns a background task to clean up any stale uploads older than `upload_timeout`.
    pub async fn start_upload(
        &self,
        repository: &ProjectUuid,
    ) -> Result<UploadId, OciStorageError> {
        // Spawn background cleanup task (non-blocking)
        self.spawn_stale_upload_cleanup();

        let upload_id = UploadId::new();
        let upload_dir = self.upload_dir(&upload_id);

        // Create upload directory
        fs::create_dir_all(&upload_dir).await.map_err(|e| {
            OciStorageError::LocalStorage(format!("Failed to create upload directory: {e}"))
        })?;

        // Create empty data file
        let data_path = self.upload_data_path(&upload_id);
        fs::File::create(&data_path).await.map_err(|e| {
            OciStorageError::LocalStorage(format!("Failed to create upload data file: {e}"))
        })?;

        // Save initial state with creation timestamp
        let state = UploadState {
            repository: repository.to_string(),
            size: 0,
            created_at: self.clock.timestamp(),
        };
        self.save_upload_state(&upload_id, &state).await?;

        Ok(upload_id)
    }

    /// Appends data to an in-progress upload
    pub async fn append_upload(
        &self,
        upload_id: &UploadId,
        data: Bytes,
    ) -> Result<u64, OciStorageError> {
        // Verify upload exists by loading state
        let _state = self.load_upload_state(upload_id).await?;

        // Check projected size BEFORE writing to avoid persisting oversized data
        let data_path = self.upload_data_path(upload_id);
        let metadata = fs::metadata(&data_path).await.map_err(|e| {
            OciStorageError::LocalStorage(format!("Failed to get upload file metadata: {e}"))
        })?;
        let current_size = metadata.len();
        let projected_total = current_size + data.len() as u64;

        if projected_total > self.max_body_size {
            return Err(OciStorageError::SizeExceeded {
                size: projected_total,
                max: self.max_body_size,
            });
        }

        // Append data to file (size check passed)
        let mut file = fs::OpenOptions::new()
            .append(true)
            .open(&data_path)
            .await
            .map_err(|e| {
                OciStorageError::LocalStorage(format!("Failed to open upload data file: {e}"))
            })?;

        file.write_all(&data).await.map_err(|e| {
            OciStorageError::LocalStorage(format!("Failed to write upload data: {e}"))
        })?;

        file.sync_all().await.map_err(|e| {
            OciStorageError::LocalStorage(format!("Failed to sync upload data: {e}"))
        })?;

        let actual_metadata = fs::metadata(&data_path).await.map_err(|e| {
            OciStorageError::LocalStorage(format!(
                "Failed to get upload file metadata after write: {e}"
            ))
        })?;
        Ok(actual_metadata.len())
    }

    /// Gets the current size of an in-progress upload
    pub async fn get_upload_size(&self, upload_id: &UploadId) -> Result<u64, OciStorageError> {
        // Verify upload exists
        let _state = self.load_upload_state(upload_id).await?;

        // Get actual file size to avoid race conditions with concurrent appends
        let data_path = self.upload_data_path(upload_id);
        let metadata = fs::metadata(&data_path).await.map_err(|e| {
            OciStorageError::LocalStorage(format!("Failed to get upload file metadata: {e}"))
        })?;

        Ok(metadata.len())
    }

    /// Completes an upload and stores the blob
    pub async fn complete_upload(
        &self,
        upload_id: &UploadId,
        expected_digest: &Digest,
    ) -> Result<Digest, OciStorageError> {
        // Load state
        let state = self.load_upload_state(upload_id).await?;

        // Stream the uploaded data for hashing to avoid loading entire file into memory
        let data_path = self.upload_data_path(upload_id);
        let file = fs::File::open(&data_path).await.map_err(|e| {
            OciStorageError::LocalStorage(format!("Failed to open upload data: {e}"))
        })?;
        let metadata = file.metadata().await.map_err(|e| {
            OciStorageError::LocalStorage(format!("Failed to read upload metadata: {e}"))
        })?;
        if metadata.len() == 0 {
            self.cleanup_upload(upload_id).await;
            return Err(OciStorageError::BlobUploadInvalidContent(
                "Cannot complete upload with no data".to_owned(),
            ));
        }

        // Compute actual digest by streaming through the file
        let mut reader = tokio::io::BufReader::new(file);
        let mut hasher = Sha256::new();
        let mut buf = [0u8; 8192];
        loop {
            let n = tokio::io::AsyncReadExt::read(&mut reader, &mut buf)
                .await
                .map_err(|e| {
                    OciStorageError::LocalStorage(format!("Failed to read upload data: {e}"))
                })?;
            if n == 0 {
                break;
            }
            hasher.update(buf.get(..n).unwrap_or_default());
        }
        let hash = hasher.finalize();
        // hex::encode always produces valid hex, so this is infallible in practice
        let actual_digest = Digest::sha256(&hex::encode(hash))
            .map_err(|e| OciStorageError::InvalidContent(e.to_string()))?;

        // Verify digest matches
        if actual_digest.as_str() != expected_digest.as_str() {
            self.cleanup_upload(upload_id).await;
            return Err(OciStorageError::DigestMismatch {
                expected: expected_digest.to_string(),
                actual: actual_digest.to_string(),
            });
        }

        // Parse repository UUID
        let repository: ProjectUuid = state.repository.parse().map_err(|_e| {
            OciStorageError::InvalidContent(format!(
                "Invalid project UUID in upload state: {}",
                state.repository
            ))
        })?;

        // Copy to final blob location
        let blob_path = self.blob_path(&repository, &actual_digest);
        if let Some(parent) = blob_path.parent() {
            fs::create_dir_all(parent).await.map_err(|e| {
                OciStorageError::LocalStorage(format!("Failed to create blob directory: {e}"))
            })?;
        }
        fs::copy(&data_path, &blob_path)
            .await
            .map_err(|e| OciStorageError::LocalStorage(format!("Failed to copy blob: {e}")))?;

        // Clean up
        self.cleanup_upload(upload_id).await;

        Ok(actual_digest)
    }

    /// Cancels an in-progress upload
    pub async fn cancel_upload(&self, upload_id: &UploadId) -> Result<(), OciStorageError> {
        // Verify upload exists
        let _state = self.load_upload_state(upload_id).await?;
        self.cleanup_upload(upload_id).await;
        Ok(())
    }

    /// Cleans up upload files
    async fn cleanup_upload(&self, upload_id: &UploadId) {
        let upload_dir = self.upload_dir(upload_id);
        if let Err(e) = fs::remove_dir_all(&upload_dir).await {
            error!(self.log, "Failed to clean up upload directory"; "upload_id" => %upload_id, "error" => %e);
            crate::storage::report_cleanup_error(&self.log, "cleanup_upload: remove_dir_all", &e);
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

        let uploads_dir = self.uploads_dir();
        let upload_timeout = self.upload_timeout;
        let log = self.log.clone();
        let clock = self.clock.clone();

        tokio::spawn(async move {
            cleanup_stale_uploads_local(&log, &uploads_dir, upload_timeout, clock).await;
        });
    }

    // ==================== Blob Operations ====================

    /// Checks if a blob exists
    pub async fn blob_exists(
        &self,
        repository: &ProjectUuid,
        digest: &Digest,
    ) -> Result<bool, OciStorageError> {
        let path = self.blob_path(repository, digest);
        Ok(fs::try_exists(&path).await.unwrap_or(false))
    }

    /// Gets a blob's content and size (loads entire blob into memory)
    ///
    /// For large blobs, prefer `get_blob_stream` which streams the content.
    pub async fn get_blob(
        &self,
        repository: &ProjectUuid,
        digest: &Digest,
    ) -> Result<(Bytes, u64), OciStorageError> {
        let path = self.blob_path(repository, digest);
        let data = map_io_error(
            fs::read(&path).await,
            OciStorageError::BlobNotFound(digest.to_string()),
            "Failed to read blob",
        )?;

        let size = data.len() as u64;
        Ok((Bytes::from(data), size))
    }

    /// Gets a blob as a streaming body
    ///
    /// Returns a streaming body and the blob size. The content is streamed
    /// from disk rather than loaded entirely into memory.
    pub async fn get_blob_stream(
        &self,
        repository: &ProjectUuid,
        digest: &Digest,
    ) -> Result<(LocalBlobBody, u64), OciStorageError> {
        let path = self.blob_path(repository, digest);

        // Get file metadata for size
        let metadata = map_io_error(
            fs::metadata(&path).await,
            OciStorageError::BlobNotFound(digest.to_string()),
            "Failed to get blob metadata",
        )?;
        let size = metadata.len();

        // Open file for streaming
        let file = map_io_error(
            fs::File::open(&path).await,
            OciStorageError::BlobNotFound(digest.to_string()),
            "Failed to open blob file",
        )?;

        Ok((LocalBlobBody::new(file, size), size))
    }

    /// Gets blob metadata (size) without downloading content
    pub async fn get_blob_size(
        &self,
        repository: &ProjectUuid,
        digest: &Digest,
    ) -> Result<u64, OciStorageError> {
        let path = self.blob_path(repository, digest);
        let metadata = map_io_error(
            fs::metadata(&path).await,
            OciStorageError::BlobNotFound(digest.to_string()),
            "Failed to get blob metadata",
        )?;

        Ok(metadata.len())
    }

    /// Deletes a blob
    pub async fn delete_blob(
        &self,
        repository: &ProjectUuid,
        digest: &Digest,
    ) -> Result<(), OciStorageError> {
        let path = self.blob_path(repository, digest);
        match fs::remove_file(&path).await {
            Ok(()) => Ok(()),
            // File already deleted or never existed - that's fine
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(OciStorageError::LocalStorage(format!(
                "Failed to delete blob: {e}"
            ))),
        }
    }

    /// Mounts a blob from another repository (cross-repo blob mount)
    ///
    /// Attempts to copy the blob directly, avoiding a TOCTOU race between
    /// checking existence and copying. If the source blob doesn't exist,
    /// returns `Ok(false)`.
    pub async fn mount_blob(
        &self,
        from_repository: &ProjectUuid,
        to_repository: &ProjectUuid,
        digest: &Digest,
    ) -> Result<bool, OciStorageError> {
        let source_path = self.blob_path(from_repository, digest);
        let dest_path = self.blob_path(to_repository, digest);

        if let Some(parent) = dest_path.parent() {
            fs::create_dir_all(parent).await.map_err(|e| {
                OciStorageError::LocalStorage(format!("Failed to create blob directory: {e}"))
            })?;
        }

        match fs::copy(&source_path, &dest_path).await {
            Ok(_) => Ok(true),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(OciStorageError::LocalStorage(format!(
                "Failed to copy blob: {e}"
            ))),
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
        // Compute digest
        let mut hasher = Sha256::new();
        hasher.update(&content);
        let hash = hasher.finalize();
        // hex::encode always produces valid hex, so this is infallible in practice
        let digest = Digest::sha256(&hex::encode(hash))
            .map_err(|e| OciStorageError::InvalidContent(e.to_string()))?;

        // Store manifest by digest
        let manifest_path = self.manifest_path(repository, &digest);
        if let Some(parent) = manifest_path.parent() {
            fs::create_dir_all(parent).await.map_err(|e| {
                OciStorageError::LocalStorage(format!("Failed to create manifest directory: {e}"))
            })?;
        }
        fs::write(&manifest_path, &content)
            .await
            .map_err(|e| OciStorageError::LocalStorage(format!("Failed to write manifest: {e}")))?;

        // If a tag was provided, create a tag link
        if let Some(tag) = tag {
            let tag_path = self.tag_path(repository, tag);
            if let Some(parent) = tag_path.parent() {
                fs::create_dir_all(parent).await.map_err(|e| {
                    OciStorageError::LocalStorage(format!("Failed to create tag directory: {e}"))
                })?;
            }
            fs::write(&tag_path, digest.to_string())
                .await
                .map_err(|e| OciStorageError::LocalStorage(format!("Failed to write tag: {e}")))?;
        }

        // Check if manifest has a subject field (for referrers API)
        if let Some((subject_digest, descriptor)) =
            crate::types::build_referrer_descriptor(manifest, &digest, content.len())
        {
            // Store referrer link
            let referrer_path = self.referrer_path(repository, &subject_digest, &digest);
            if let Some(parent) = referrer_path.parent() {
                fs::create_dir_all(parent).await.map_err(|e| {
                    OciStorageError::LocalStorage(format!(
                        "Failed to create referrer directory: {e}"
                    ))
                })?;
            }
            fs::write(
                &referrer_path,
                serde_json::to_vec(&descriptor)
                    .map_err(|e| OciStorageError::Json(e.to_string()))?,
            )
            .await
            .map_err(|e| OciStorageError::LocalStorage(format!("Failed to write referrer: {e}")))?;
        }

        Ok(digest)
    }

    /// Gets a manifest by digest
    pub async fn get_manifest_by_digest(
        &self,
        repository: &ProjectUuid,
        digest: &Digest,
    ) -> Result<Bytes, OciStorageError> {
        let path = self.manifest_path(repository, digest);
        let data = map_io_error(
            fs::read(&path).await,
            OciStorageError::ManifestNotFound(digest.to_string()),
            "Failed to read manifest",
        )?;

        Ok(Bytes::from(data))
    }

    /// Resolves a tag to a digest
    pub async fn resolve_tag(
        &self,
        repository: &ProjectUuid,
        tag: &crate::types::Tag,
    ) -> Result<Digest, OciStorageError> {
        let path = self.tag_path(repository, tag);
        let data = map_io_error(
            fs::read_to_string(&path).await,
            OciStorageError::ManifestNotFound(tag.to_string()),
            "Failed to read tag",
        )?;

        data.trim()
            .parse()
            .map_err(|e: crate::types::DigestError| OciStorageError::InvalidContent(e.to_string()))
    }

    /// Lists tags for a repository with optional pagination
    ///
    /// - `limit`: Maximum number of tags to return
    /// - `start_after`: Tag to start listing after (for cursor-based pagination)
    ///
    /// Note: For local storage, we must read all directory entries first, then apply
    /// sorting and pagination. This is less efficient than S3 for very large repositories.
    pub async fn list_tags(
        &self,
        repository: &ProjectUuid,
        limit: Option<usize>,
        start_after: Option<&str>,
    ) -> Result<crate::storage::ListTagsResult, OciStorageError> {
        let tags_dir = self.repository_dir(repository).join("tags");

        if !fs::try_exists(&tags_dir).await.unwrap_or(false) {
            return Ok(crate::storage::ListTagsResult {
                tags: Vec::new(),
                has_more: false,
            });
        }

        let mut tags = Vec::new();
        let mut entries = fs::read_dir(&tags_dir).await.map_err(|e| {
            OciStorageError::LocalStorage(format!("Failed to read tags directory: {e}"))
        })?;

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| OciStorageError::LocalStorage(format!("Failed to read tag entry: {e}")))?
        {
            if let Some(name) = entry.file_name().to_str() {
                tags.push(name.to_owned());
            }
        }

        // Sort for consistent ordering (matches S3 behavior)
        tags.sort();

        // Apply cursor-based pagination: skip past start_after
        let tags = if let Some(start) = start_after {
            tags.into_iter()
                .skip_while(|t| t.as_str() <= start)
                .collect()
        } else {
            tags
        };

        // Apply limit and detect if more exist
        let has_more = limit.is_some_and(|l| tags.len() > l);
        let tags = if let Some(limit) = limit {
            tags.into_iter().take(limit).collect()
        } else {
            tags
        };

        Ok(crate::storage::ListTagsResult { tags, has_more })
    }

    /// Deletes a manifest by digest
    ///
    /// Also cleans up any referrer link if this manifest references another manifest
    /// via the `subject` field.
    pub async fn delete_manifest(
        &self,
        repository: &ProjectUuid,
        digest: &Digest,
    ) -> Result<(), OciStorageError> {
        let path = self.manifest_path(repository, digest);

        // Try to read the manifest first to check for subject field
        // If we can read it and it has a subject, clean up the referrer link
        if let Ok(data) = fs::read(&path).await
            && let Some(subject_digest) = crate::types::extract_subject_digest(&data)
        {
            let referrer_path = self.referrer_path(repository, &subject_digest, digest);
            if let Err(e) = fs::remove_file(&referrer_path).await
                && e.kind() != io::ErrorKind::NotFound
            {
                crate::storage::report_cleanup_error(
                    &self.log,
                    "delete_manifest: referrer link delete",
                    &e,
                );
            }
        }

        // Clean up any tags that point to this digest (best-effort)
        let tags_dir = self.repository_dir(repository).join("tags");
        if let Ok(mut entries) = fs::read_dir(&tags_dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                if let Ok(content) = fs::read_to_string(entry.path()).await
                    && let Ok(tag_digest) = content.trim().parse::<Digest>()
                    && tag_digest.as_str() == digest.as_str()
                    && let Err(e) = fs::remove_file(entry.path()).await
                    && e.kind() != io::ErrorKind::NotFound
                {
                    crate::storage::report_cleanup_error(
                        &self.log,
                        "delete_manifest: tag link delete",
                        &e,
                    );
                }
            }
        }

        // Delete the manifest itself
        match fs::remove_file(&path).await {
            Ok(()) => Ok(()),
            // File already deleted or never existed - that's fine
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(OciStorageError::LocalStorage(format!(
                "Failed to delete manifest: {e}"
            ))),
        }
    }

    /// Deletes a tag
    pub async fn delete_tag(
        &self,
        repository: &ProjectUuid,
        tag: &crate::types::Tag,
    ) -> Result<(), OciStorageError> {
        let path = self.tag_path(repository, tag);
        match fs::remove_file(&path).await {
            Ok(()) => Ok(()),
            // File already deleted or never existed - that's fine
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(OciStorageError::LocalStorage(format!(
                "Failed to delete tag: {e}"
            ))),
        }
    }

    /// Lists all manifests that reference a given digest via their subject field
    pub async fn list_referrers(
        &self,
        repository: &ProjectUuid,
        subject_digest: &Digest,
        artifact_type_filter: Option<&str>,
    ) -> Result<Vec<bencher_json::oci::OciDescriptor>, OciStorageError> {
        let referrers_dir = self.referrers_dir(repository, subject_digest);

        if !fs::try_exists(&referrers_dir).await.unwrap_or(false) {
            return Ok(Vec::new());
        }

        let mut referrers = Vec::new();
        let mut entries = fs::read_dir(&referrers_dir).await.map_err(|e| {
            OciStorageError::LocalStorage(format!("Failed to read referrers directory: {e}"))
        })?;

        while let Some(entry) = entries.next_entry().await.map_err(|e| {
            OciStorageError::LocalStorage(format!("Failed to read referrer entry: {e}"))
        })? {
            let Ok(data) = fs::read(entry.path()).await else {
                warn!(self.log, "Failed to read referrer file"; "path" => %entry.path().display());
                continue;
            };
            let Ok(descriptor) = serde_json::from_slice::<bencher_json::oci::OciDescriptor>(&data)
            else {
                warn!(self.log, "Failed to parse referrer JSON"; "path" => %entry.path().display());
                continue;
            };

            // Apply artifact type filter if specified
            if let Some(filter) = artifact_type_filter
                && descriptor.artifact_type.as_deref() != Some(filter)
            {
                continue;
            }

            referrers.push(descriptor);
        }

        Ok(referrers)
    }
}

/// Cleans up all stale uploads in the given uploads directory.
///
/// This is a standalone async function that can be spawned as a background task.
/// Individual upload cleanup failures are logged but don't stop processing.
async fn cleanup_stale_uploads_local(
    log: &Logger,
    uploads_dir: &Path,
    upload_timeout: u64,
    clock: crate::Clock,
) {
    let Ok(mut entries) = fs::read_dir(uploads_dir).await else {
        // Directory doesn't exist or can't be read - nothing to clean up
        return;
    };

    let now = clock.timestamp();
    let timeout_secs = i64::try_from(upload_timeout).unwrap_or(i64::MAX);

    loop {
        match entries.next_entry().await {
            Ok(Some(entry)) => {
                let Some(upload_id_str) = entry.file_name().to_str().map(ToOwned::to_owned) else {
                    continue;
                };

                // Validate upload ID format (we don't use the parsed value, just validate)
                if upload_id_str.parse::<UploadId>().is_err() {
                    continue;
                }

                // Try to load the state to check creation time.
                // If the state file is missing or unparseable, fall back to the
                // directory's modification time to decide staleness.  This avoids
                // a race where `start_upload` has created the directory but has
                // not yet written state.json — deleting the directory in that
                // window would break the in-progress upload.
                let state_path = entry.path().join("state.json");
                let is_stale = match fs::read(&state_path).await {
                    Ok(data) => match serde_json::from_slice::<UploadState>(&data) {
                        Ok(state) => now.saturating_sub(state.created_at) > timeout_secs,
                        Err(_) => dir_is_stale(&entry, now, timeout_secs).await,
                    },
                    Err(_) => dir_is_stale(&entry, now, timeout_secs).await,
                };

                // Remove stale uploads
                if is_stale && let Err(e) = fs::remove_dir_all(entry.path()).await {
                    error!(log, "Failed to remove stale upload"; "upload_id" => &upload_id_str, "error" => %e);
                }
            },
            Ok(None) => break,
            Err(e) => {
                warn!(log, "Error reading upload directory entry"; "error" => %e);
            },
        }
    }
}

/// Check whether a directory entry is stale based on its filesystem metadata.
///
/// Used as a fallback when `state.json` is missing or corrupt — the directory
/// modification time serves as a lower bound for its creation time.
async fn dir_is_stale(entry: &fs::DirEntry, now: i64, timeout_secs: i64) -> bool {
    let Ok(metadata) = entry.metadata().await else {
        // Can't read metadata — skip rather than risk deleting an active upload
        return false;
    };
    let Ok(modified) = metadata.modified() else {
        return false;
    };
    let modified_secs = i64::try_from(
        modified
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    )
    .unwrap_or(i64::MAX);
    let dir_age = now.saturating_sub(modified_secs);
    dir_age > timeout_secs
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;

    /// Create a test `OciLocalStorage` backed by a temporary directory.
    fn test_storage(tmp: &tempfile::TempDir) -> OciLocalStorage {
        let db_path = tmp.path().join("bencher.db");
        let log = Logger::root(slog::Discard, slog::o!());
        OciLocalStorage::new(log, &db_path, 3600, 0x4000_0000, crate::Clock::System)
    }

    /// Create a minimal OCI manifest JSON for testing
    fn test_manifest_json(config_digest: &str) -> String {
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

    fn test_repository() -> ProjectUuid {
        "00000000-0000-0000-0000-000000000001".parse().unwrap()
    }

    #[tokio::test]
    async fn put_and_get_manifest_by_digest() {
        let tmp = tempfile::tempdir().unwrap();
        let storage = test_storage(&tmp);
        let repo = test_repository();
        let content = test_manifest_json(
            "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        );
        let manifest = bencher_json::oci::Manifest::from_bytes(content.as_bytes()).unwrap();

        let digest = storage
            .put_manifest(&repo, Bytes::from(content.clone()), None, &manifest)
            .await
            .unwrap();

        let retrieved = storage
            .get_manifest_by_digest(&repo, &digest)
            .await
            .unwrap();
        assert_eq!(retrieved.as_ref(), content.as_bytes());
    }

    #[tokio::test]
    async fn put_manifest_with_tag_and_resolve() {
        let tmp = tempfile::tempdir().unwrap();
        let storage = test_storage(&tmp);
        let repo = test_repository();
        let content = test_manifest_json(
            "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        );
        let manifest = bencher_json::oci::Manifest::from_bytes(content.as_bytes()).unwrap();
        let tag: crate::types::Tag = "latest".parse().unwrap();

        let digest = storage
            .put_manifest(&repo, Bytes::from(content), Some(&tag), &manifest)
            .await
            .unwrap();

        let resolved = storage.resolve_tag(&repo, &tag).await.unwrap();
        assert_eq!(resolved.as_str(), digest.as_str());
    }

    #[tokio::test]
    async fn delete_manifest_removes_tags() {
        let tmp = tempfile::tempdir().unwrap();
        let storage = test_storage(&tmp);
        let repo = test_repository();
        let content = test_manifest_json(
            "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
        );
        let manifest = bencher_json::oci::Manifest::from_bytes(content.as_bytes()).unwrap();
        let tag1: crate::types::Tag = "v1".parse().unwrap();
        let tag2: crate::types::Tag = "v2".parse().unwrap();

        // Push manifest with tag1, then overwrite tag2 to point to the same digest
        let digest = storage
            .put_manifest(&repo, Bytes::from(content.clone()), Some(&tag1), &manifest)
            .await
            .unwrap();
        let _digest2 = storage
            .put_manifest(&repo, Bytes::from(content), Some(&tag2), &manifest)
            .await
            .unwrap();

        // Delete manifest by digest
        storage.delete_manifest(&repo, &digest).await.unwrap();

        // Both tags should be gone
        assert!(storage.resolve_tag(&repo, &tag1).await.is_err());
        assert!(storage.resolve_tag(&repo, &tag2).await.is_err());
    }

    #[tokio::test]
    async fn upload_round_trip() {
        let tmp = tempfile::tempdir().unwrap();
        let storage = test_storage(&tmp);
        let repo = test_repository();

        let upload_id = storage.start_upload(&repo).await.unwrap();

        let data = b"hello world blob data";
        let total = storage
            .append_upload(&upload_id, Bytes::from_static(data))
            .await
            .unwrap();
        assert_eq!(total, data.len() as u64);

        let expected_digest = Digest::from_sha256_bytes(data);
        let actual_digest = storage
            .complete_upload(&upload_id, &expected_digest)
            .await
            .unwrap();
        assert_eq!(actual_digest.as_str(), expected_digest.as_str());

        // Blob should now exist
        assert!(storage.blob_exists(&repo, &actual_digest).await.unwrap());
        let (blob_data, size) = storage.get_blob(&repo, &actual_digest).await.unwrap();
        assert_eq!(blob_data.as_ref(), data);
        assert_eq!(size, data.len() as u64);
    }

    #[tokio::test]
    async fn cancel_upload_cleans_up() {
        let tmp = tempfile::tempdir().unwrap();
        let storage = test_storage(&tmp);
        let repo = test_repository();

        let upload_id = storage.start_upload(&repo).await.unwrap();
        storage
            .append_upload(&upload_id, Bytes::from_static(b"some data"))
            .await
            .unwrap();

        storage.cancel_upload(&upload_id).await.unwrap();

        // Upload dir should be gone
        let upload_dir = storage.upload_dir(&upload_id);
        assert!(!upload_dir.exists());
    }

    #[tokio::test]
    async fn blob_exists_nonexistent() {
        let tmp = tempfile::tempdir().unwrap();
        let storage = test_storage(&tmp);
        let repo = test_repository();
        let digest = Digest::from_sha256_bytes(b"no such blob");

        assert!(!storage.blob_exists(&repo, &digest).await.unwrap());
    }

    #[tokio::test]
    async fn stale_upload_cleanup() {
        let tmp = tempfile::tempdir().unwrap();
        let storage = test_storage(&tmp);
        let repo = test_repository();

        let upload_id = storage.start_upload(&repo).await.unwrap();
        storage
            .append_upload(&upload_id, Bytes::from_static(b"stale data"))
            .await
            .unwrap();

        // Manually set created_at to epoch time to make it stale
        let state_path = storage.upload_state_path(&upload_id);
        let state_data = fs::read(&state_path).await.unwrap();
        let mut state: UploadState = serde_json::from_slice(&state_data).unwrap();
        state.created_at = 0;
        fs::write(&state_path, serde_json::to_vec(&state).unwrap())
            .await
            .unwrap();

        // Run cleanup with 1-second timeout
        let log = Logger::root(slog::Discard, slog::o!());
        cleanup_stale_uploads_local(&log, &storage.uploads_dir(), 1, crate::Clock::System).await;

        // Upload dir should be gone
        let upload_dir = storage.upload_dir(&upload_id);
        assert!(
            !upload_dir.exists(),
            "Stale upload directory should have been cleaned up"
        );
    }

    fn test_job_uuid() -> bencher_json::JobUuid {
        "00000000-0000-0000-0000-000000000099".parse().unwrap()
    }

    #[tokio::test]
    async fn put_and_get_job_output() {
        let tmp = tempfile::tempdir().unwrap();
        let storage = test_storage(&tmp);
        let repo = test_repository();
        let job = test_job_uuid();

        let output = bencher_json::runner::JsonJobOutput {
            exit_code: Some(0),
            stdout: Some("hello stdout".into()),
            stderr: Some("hello stderr".into()),
            output: None,
            error: None,
        };

        storage.put_job_output(repo, job, &output).await.unwrap();

        let retrieved = storage.get_job_output(repo, job).await.unwrap().unwrap();
        assert_eq!(retrieved.exit_code, Some(0));
        assert_eq!(retrieved.stdout.as_deref(), Some("hello stdout"));
        assert_eq!(retrieved.stderr.as_deref(), Some("hello stderr"));
        assert!(retrieved.output.is_none());
        assert!(retrieved.error.is_none());
    }

    #[tokio::test]
    async fn get_job_output_nonexistent() {
        let tmp = tempfile::tempdir().unwrap();
        let storage = test_storage(&tmp);
        let repo = test_repository();
        let job = test_job_uuid();

        let result = storage.get_job_output(repo, job).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn put_job_output_creates_directories() {
        let tmp = tempfile::tempdir().unwrap();
        let storage = test_storage(&tmp);
        let repo = test_repository();
        let job = test_job_uuid();

        let output = bencher_json::runner::JsonJobOutput {
            exit_code: Some(0),
            stdout: None,
            stderr: None,
            output: None,
            error: None,
        };

        // Directory doesn't exist yet
        let path = storage.job_output_path(repo, job);
        assert!(!path.exists());

        storage.put_job_output(repo, job, &output).await.unwrap();

        // Now the file exists
        assert!(path.exists());
    }

    #[tokio::test]
    async fn put_job_output_overwrites() {
        let tmp = tempfile::tempdir().unwrap();
        let storage = test_storage(&tmp);
        let repo = test_repository();
        let job = test_job_uuid();

        let output1 = bencher_json::runner::JsonJobOutput {
            exit_code: Some(0),
            stdout: Some("first".into()),
            stderr: None,
            output: None,
            error: None,
        };
        storage.put_job_output(repo, job, &output1).await.unwrap();

        let output2 = bencher_json::runner::JsonJobOutput {
            exit_code: Some(1),
            stdout: Some("second".into()),
            stderr: None,
            output: None,
            error: None,
        };
        storage.put_job_output(repo, job, &output2).await.unwrap();

        let retrieved = storage.get_job_output(repo, job).await.unwrap().unwrap();
        assert_eq!(retrieved.exit_code, Some(1));
        assert_eq!(retrieved.stdout.as_deref(), Some("second"));
    }

    #[tokio::test]
    async fn put_job_output_with_all_fields() {
        let tmp = tempfile::tempdir().unwrap();
        let storage = test_storage(&tmp);
        let repo = test_repository();
        let job = test_job_uuid();

        let output: bencher_json::runner::JsonJobOutput =
            serde_json::from_value(serde_json::json!({
                "exit_code": 42,
                "stdout": "full stdout",
                "stderr": "full stderr",
                "output": {
                    "/tmp/results.json": "{\"metric\": 42}",
                    "/tmp/log.txt": "some log"
                },
                "error": "something went wrong"
            }))
            .unwrap();

        storage.put_job_output(repo, job, &output).await.unwrap();

        let retrieved = storage.get_job_output(repo, job).await.unwrap().unwrap();
        assert_eq!(retrieved.exit_code, Some(42));
        assert_eq!(retrieved.stdout.as_deref(), Some("full stdout"));
        assert_eq!(retrieved.stderr.as_deref(), Some("full stderr"));
        assert_eq!(retrieved.error.as_deref(), Some("something went wrong"));
        let output_map = retrieved.output.unwrap();
        assert_eq!(output_map.len(), 2);
    }

    #[tokio::test]
    async fn put_job_output_with_minimal_fields() {
        let tmp = tempfile::tempdir().unwrap();
        let storage = test_storage(&tmp);
        let repo = test_repository();
        let job = test_job_uuid();

        let output = bencher_json::runner::JsonJobOutput {
            exit_code: Some(0),
            stdout: None,
            stderr: None,
            output: None,
            error: None,
        };

        storage.put_job_output(repo, job, &output).await.unwrap();

        let retrieved = storage.get_job_output(repo, job).await.unwrap().unwrap();
        assert_eq!(retrieved.exit_code, Some(0));
        assert!(retrieved.stdout.is_none());
        assert!(retrieved.stderr.is_none());
        assert!(retrieved.output.is_none());
        assert!(retrieved.error.is_none());
    }

    #[tokio::test]
    async fn put_job_output_different_projects() {
        let tmp = tempfile::tempdir().unwrap();
        let storage = test_storage(&tmp);
        let repo1: ProjectUuid = "00000000-0000-0000-0000-000000000001".parse().unwrap();
        let repo2: ProjectUuid = "00000000-0000-0000-0000-000000000002".parse().unwrap();
        let job = test_job_uuid();

        let output1 = bencher_json::runner::JsonJobOutput {
            exit_code: Some(0),
            stdout: Some("project1".into()),
            stderr: None,
            output: None,
            error: None,
        };
        let output2 = bencher_json::runner::JsonJobOutput {
            exit_code: Some(1),
            stdout: Some("project2".into()),
            stderr: None,
            output: None,
            error: None,
        };

        storage.put_job_output(repo1, job, &output1).await.unwrap();
        storage.put_job_output(repo2, job, &output2).await.unwrap();

        let r1 = storage.get_job_output(repo1, job).await.unwrap().unwrap();
        let r2 = storage.get_job_output(repo2, job).await.unwrap().unwrap();
        assert_eq!(r1.stdout.as_deref(), Some("project1"));
        assert_eq!(r2.stdout.as_deref(), Some("project2"));
    }

    #[tokio::test]
    async fn get_job_output_different_projects() {
        let tmp = tempfile::tempdir().unwrap();
        let storage = test_storage(&tmp);
        let repo1: ProjectUuid = "00000000-0000-0000-0000-000000000001".parse().unwrap();
        let repo2: ProjectUuid = "00000000-0000-0000-0000-000000000002".parse().unwrap();
        let job = test_job_uuid();

        let output = bencher_json::runner::JsonJobOutput {
            exit_code: Some(0),
            stdout: Some("only project1".into()),
            stderr: None,
            output: None,
            error: None,
        };

        storage.put_job_output(repo1, job, &output).await.unwrap();

        // Project1 has the output
        assert!(storage.get_job_output(repo1, job).await.unwrap().is_some());
        // Project2 does not
        assert!(storage.get_job_output(repo2, job).await.unwrap().is_none());
    }
}
