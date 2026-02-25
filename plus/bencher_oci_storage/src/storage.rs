//! OCI Storage Layer - S3 and Local Filesystem Backends
//!
//! This module provides two storage backends:
//!
//! ## S3 Backend (default when configured)
//! - Uses S3 multipart uploads for scalability
//! - Upload state is stored in S3 for cross-instance consistency
//! - Chunks are buffered in S3 until they reach the 5MB minimum part size
//! - No in-memory state means horizontal scaling and restart resilience
//!
//! ## Local Filesystem Backend (fallback)
//! - Stores OCI artifacts on local disk
//! - Data is stored in an `oci` directory sibling to the database file
//! - Suitable for development and single-instance deployments

use std::path::Path;
use std::pin::Pin;
use std::str::FromStr;
use std::sync::atomic::{AtomicI64, Ordering};
use std::task::{Context, Poll};

use aws_sdk_s3::Client;
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::types::CompletedMultipartUpload;
use bencher_json::{
    ProjectUuid, Secret,
    system::config::{DEFAULT_MAX_BODY_SIZE, DEFAULT_UPLOAD_TIMEOUT_SECS, RegistryDataStore},
};
use bytes::Bytes;
use chrono::Utc;
use futures::stream::{self, StreamExt as _};
use hyper::body::Frame;
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use slog::Logger;
use thiserror::Error;

use bencher_json::Clock;

use crate::local::{LocalBlobBody, OciLocalStorage};
use crate::types::{Digest, UploadId};

pub(crate) fn report_cleanup_error(log: &Logger, context: &str, error: &impl std::fmt::Display) {
    slog::warn!(log, "OCI cleanup error"; "context" => context, "error" => %error);
    #[cfg(feature = "sentry")]
    sentry::capture_message(
        &format!("OCI cleanup error ({context}): {error}"),
        sentry::Level::Warning,
    );
}

/// A streaming body for blob content from S3
pub struct S3BlobBody {
    inner: ByteStream,
    size: u64,
}

impl hyper::body::Body for S3BlobBody {
    type Data = Bytes;
    type Error = Box<dyn std::error::Error + Send + Sync>;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
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

    fn size_hint(&self) -> hyper::body::SizeHint {
        hyper::body::SizeHint::with_exact(self.size)
    }
}

/// Unified blob body type that wraps either S3 or local filesystem streams
pub enum BlobBody {
    S3(S3BlobBody),
    Local(LocalBlobBody),
}

impl hyper::body::Body for BlobBody {
    type Data = Bytes;
    type Error = Box<dyn std::error::Error + Send + Sync>;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        match self.get_mut() {
            Self::S3(s3) => Pin::new(s3).poll_frame(cx),
            Self::Local(local) => Pin::new(local).poll_frame(cx),
        }
    }

    fn size_hint(&self) -> hyper::body::SizeHint {
        match self {
            Self::S3(s3) => s3.size_hint(),
            Self::Local(local) => local.size_hint(),
        }
    }
}

/// Minimum part size for S3 multipart upload (5MB)
/// S3 requires all parts except the last to be at least 5MB
const MIN_PART_SIZE: usize = 5 * 1024 * 1024;

/// Maximum concurrency for parallel S3/IO operations.
/// Clamped to available CPU parallelism, with this upper bound to prevent
/// excessive resource usage.
pub const MAX_CONCURRENCY: usize = 64;

/// Storage errors
#[derive(Debug, Error)]
pub enum OciStorageError {
    #[error("S3 error: {0}")]
    S3(String),

    #[error("Local storage error: {0}")]
    LocalStorage(String),

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

    #[error("Invalid S3 ARN: {0}")]
    InvalidArn(String),

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
            Self::S3(_)
            | Self::LocalStorage(_)
            | Self::InvalidArn(_)
            | Self::Config(_)
            | Self::Json(_) => http::StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

/// Configuration for OCI S3 storage
#[derive(Debug, Clone)]
pub struct OciStorageConfig {
    pub bucket_arn: String,
    pub prefix: Option<String>,
}

/// Upload state stored in S3 for cross-instance consistency
///
/// ## Buffer Chunk Storage (O(1) appends, race-free)
///
/// Instead of storing a single buffer that grows with each append (O(n²) I/O),
/// we store each incoming chunk as a separate S3 object with a unique key
/// (timestamp + UUID). At completion, we list all chunks, sort by key, and
/// stream through them to compute the hash and upload multipart parts.
///
/// This reduces append operations from O(n²) to O(n) total I/O, and eliminates
/// race conditions by not relying on shared mutable state for chunk numbering.
#[derive(Debug, Serialize, Deserialize)]
struct UploadState {
    /// S3 multipart upload ID
    s3_upload_id: String,
    /// Repository name
    repository: String,
    /// Completed parts with their `ETag`s
    parts: Vec<CompletedPartInfo>,
    /// Unix timestamp when the upload was created
    created_at: i64,
}

/// Information about a completed S3 multipart upload part
#[derive(Debug, Serialize, Deserialize)]
struct CompletedPartInfo {
    part_number: i32,
    etag: String,
    size: u64,
}

/// Maximum number of parts allowed in an S3 multipart upload
const MAX_S3_PARTS: usize = 10_000;

impl UploadState {
    /// Total bytes in completed parts
    fn completed_parts_size(&self) -> u64 {
        self.parts.iter().map(|p| p.size).sum()
    }

    /// Next part number to use
    fn next_part_number(&self) -> Result<i32, OciStorageError> {
        // S3 part numbers are 1-indexed and max 10,000 parts
        if self.parts.len() >= MAX_S3_PARTS {
            return Err(OciStorageError::S3(format!(
                "Maximum number of S3 parts ({MAX_S3_PARTS}) exceeded"
            )));
        }
        // Safe to cast: parts.len() < 10,000 which fits in i32
        let len = i32::try_from(self.parts.len())
            .map_err(|e| OciStorageError::S3(format!("Part count overflow: {e}")))?;
        Ok(len + 1)
    }
}

/// Result of listing tags with pagination support
pub struct ListTagsResult {
    /// The tags returned
    pub tags: Vec<String>,
    /// Whether more tags exist beyond the requested limit
    pub has_more: bool,
}

/// OCI Storage implementation using S3 with multipart uploads
pub struct OciS3Storage {
    client: Client,
    config: OciStorageConfig,
    /// Upload timeout in seconds for stale upload cleanup
    upload_timeout: u64,
    /// Maximum body size in bytes for uploads
    max_body_size: u64,
    /// Logger for error/warning reporting
    log: Logger,
    /// Concurrency limit for parallel referrer fetches
    concurrency: usize,
    /// Unix timestamp of the last stale upload cleanup (for debouncing)
    last_cleanup: AtomicI64,
    /// Clock for getting the current time (injectable for testing)
    clock: Clock,
}

/// OCI Storage backend - supports S3 or local filesystem
pub enum OciStorage {
    /// S3-based storage (recommended for production)
    S3(OciS3Storage),
    /// Local filesystem storage (for development/testing)
    Local(OciLocalStorage),
}

impl OciStorage {
    /// Creates a new OCI storage instance from configuration
    ///
    /// If S3 configuration is provided, uses S3 backend.
    /// Otherwise, falls back to local filesystem storage.
    ///
    /// The `upload_timeout` specifies how long (in seconds) before stale uploads
    /// are cleaned up. Pass `None` to use the default (1 hour).
    /// The `max_body_size` specifies the maximum body size in bytes.
    /// Pass `None` to use the default (1 GiB).
    pub fn try_from_config(
        log: Logger,
        data_store: Option<RegistryDataStore>,
        database_path: &Path,
        upload_timeout: Option<u64>,
        max_body_size: Option<u64>,
        clock: Option<Clock>,
    ) -> Result<Self, OciStorageError> {
        let timeout = upload_timeout.unwrap_or(DEFAULT_UPLOAD_TIMEOUT_SECS);
        let body_size = max_body_size.unwrap_or(DEFAULT_MAX_BODY_SIZE);
        let clock = clock.unwrap_or(Clock::System);
        match data_store {
            Some(RegistryDataStore::Local) | None => Ok(OciStorage::Local(OciLocalStorage::new(
                log,
                database_path,
                timeout,
                body_size,
                clock,
            ))),
            Some(RegistryDataStore::AwsS3 {
                access_key_id,
                secret_access_key,
                access_point,
            }) => OciS3Storage::new(
                log,
                access_key_id,
                secret_access_key,
                &access_point,
                timeout,
                body_size,
                clock,
            )
            .map(OciStorage::S3),
        }
    }

    /// Returns a view type for job output storage operations.
    pub fn job_output(&self) -> crate::job_output::JobOutput<'_> {
        crate::job_output::JobOutput::new(self)
    }

    /// Returns the configured maximum body size in bytes
    pub fn max_body_size(&self) -> u64 {
        match self {
            Self::S3(s3) => s3.max_body_size,
            Self::Local(local) => local.max_body_size(),
        }
    }

    // ==================== Upload Operations ====================

    /// Starts a new upload session
    pub async fn start_upload(
        &self,
        repository: &ProjectUuid,
    ) -> Result<UploadId, OciStorageError> {
        match self {
            Self::S3(s3) => s3.start_upload(repository).await,
            Self::Local(local) => local.start_upload(repository).await,
        }
    }

    /// Appends data to an in-progress upload
    pub async fn append_upload(
        &self,
        upload_id: &UploadId,
        data: Bytes,
    ) -> Result<u64, OciStorageError> {
        match self {
            Self::S3(s3) => s3.append_upload(upload_id, data).await,
            Self::Local(local) => local.append_upload(upload_id, data).await,
        }
    }

    /// Gets the current size of an in-progress upload
    pub async fn get_upload_size(&self, upload_id: &UploadId) -> Result<u64, OciStorageError> {
        match self {
            Self::S3(s3) => s3.get_upload_size(upload_id).await,
            Self::Local(local) => local.get_upload_size(upload_id).await,
        }
    }

    /// Completes an upload and stores the blob
    pub async fn complete_upload(
        &self,
        upload_id: &UploadId,
        expected_digest: &Digest,
    ) -> Result<Digest, OciStorageError> {
        match self {
            Self::S3(s3) => s3.complete_upload(upload_id, expected_digest).await,
            Self::Local(local) => local.complete_upload(upload_id, expected_digest).await,
        }
    }

    /// Validates that the upload session belongs to the expected repository
    pub async fn validate_upload_repository(
        &self,
        upload_id: &UploadId,
        expected_repository: &ProjectUuid,
    ) -> Result<(), OciStorageError> {
        match self {
            Self::S3(s3) => {
                s3.validate_upload_repository(upload_id, expected_repository)
                    .await
            },
            Self::Local(local) => {
                local
                    .validate_upload_repository(upload_id, expected_repository)
                    .await
            },
        }
    }

    /// Cancels an in-progress upload
    pub async fn cancel_upload(&self, upload_id: &UploadId) -> Result<(), OciStorageError> {
        match self {
            Self::S3(s3) => s3.cancel_upload(upload_id).await,
            Self::Local(local) => local.cancel_upload(upload_id).await,
        }
    }

    // ==================== Blob Operations ====================

    /// Checks if a blob exists
    pub async fn blob_exists(
        &self,
        repository: &ProjectUuid,
        digest: &Digest,
    ) -> Result<bool, OciStorageError> {
        match self {
            Self::S3(s3) => s3.blob_exists(repository, digest).await,
            Self::Local(local) => local.blob_exists(repository, digest).await,
        }
    }

    /// Gets a blob's content and size (loads entire blob into memory)
    ///
    /// For large blobs, prefer `get_blob_stream` which streams the content.
    pub async fn get_blob(
        &self,
        repository: &ProjectUuid,
        digest: &Digest,
    ) -> Result<(Bytes, u64), OciStorageError> {
        match self {
            Self::S3(s3) => s3.get_blob(repository, digest).await,
            Self::Local(local) => local.get_blob(repository, digest).await,
        }
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
        match self {
            Self::S3(s3) => {
                let (body, size) = s3.get_blob_stream(repository, digest).await?;
                Ok((BlobBody::S3(body), size))
            },
            Self::Local(local) => {
                let (body, size) = local.get_blob_stream(repository, digest).await?;
                Ok((BlobBody::Local(body), size))
            },
        }
    }

    /// Gets blob metadata (size) without downloading content
    pub async fn get_blob_size(
        &self,
        repository: &ProjectUuid,
        digest: &Digest,
    ) -> Result<u64, OciStorageError> {
        match self {
            Self::S3(s3) => s3.get_blob_size(repository, digest).await,
            Self::Local(local) => local.get_blob_size(repository, digest).await,
        }
    }

    /// Deletes a blob
    pub async fn delete_blob(
        &self,
        repository: &ProjectUuid,
        digest: &Digest,
    ) -> Result<(), OciStorageError> {
        match self {
            Self::S3(s3) => s3.delete_blob(repository, digest).await,
            Self::Local(local) => local.delete_blob(repository, digest).await,
        }
    }

    /// Mounts a blob from another repository (cross-repo blob mount)
    pub async fn mount_blob(
        &self,
        from_repository: &ProjectUuid,
        to_repository: &ProjectUuid,
        digest: &Digest,
    ) -> Result<bool, OciStorageError> {
        match self {
            Self::S3(s3) => s3.mount_blob(from_repository, to_repository, digest).await,
            Self::Local(local) => {
                local
                    .mount_blob(from_repository, to_repository, digest)
                    .await
            },
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
        match self {
            Self::S3(s3) => s3.put_manifest(repository, content, tag, manifest).await,
            Self::Local(local) => local.put_manifest(repository, content, tag, manifest).await,
        }
    }

    /// Gets a manifest by digest
    pub async fn get_manifest_by_digest(
        &self,
        repository: &ProjectUuid,
        digest: &Digest,
    ) -> Result<Bytes, OciStorageError> {
        match self {
            Self::S3(s3) => s3.get_manifest_by_digest(repository, digest).await,
            Self::Local(local) => local.get_manifest_by_digest(repository, digest).await,
        }
    }

    /// Resolves a tag to a digest
    pub async fn resolve_tag(
        &self,
        repository: &ProjectUuid,
        tag: &crate::types::Tag,
    ) -> Result<Digest, OciStorageError> {
        match self {
            Self::S3(s3) => s3.resolve_tag(repository, tag).await,
            Self::Local(local) => local.resolve_tag(repository, tag).await,
        }
    }

    /// Lists tags for a repository with optional pagination
    ///
    /// - `limit`: Maximum number of tags to return
    /// - `start_after`: Tag to start listing after (for cursor-based pagination)
    pub async fn list_tags(
        &self,
        repository: &ProjectUuid,
        limit: Option<usize>,
        start_after: Option<&str>,
    ) -> Result<ListTagsResult, OciStorageError> {
        match self {
            Self::S3(s3) => s3.list_tags(repository, limit, start_after).await,
            Self::Local(local) => local.list_tags(repository, limit, start_after).await,
        }
    }

    /// Deletes a manifest by digest
    pub async fn delete_manifest(
        &self,
        repository: &ProjectUuid,
        digest: &Digest,
    ) -> Result<(), OciStorageError> {
        match self {
            Self::S3(s3) => s3.delete_manifest(repository, digest).await,
            Self::Local(local) => local.delete_manifest(repository, digest).await,
        }
    }

    /// Deletes a tag
    pub async fn delete_tag(
        &self,
        repository: &ProjectUuid,
        tag: &crate::types::Tag,
    ) -> Result<(), OciStorageError> {
        match self {
            Self::S3(s3) => s3.delete_tag(repository, tag).await,
            Self::Local(local) => local.delete_tag(repository, tag).await,
        }
    }

    /// Lists all manifests that reference a given digest via their subject field
    pub async fn list_referrers(
        &self,
        repository: &ProjectUuid,
        subject_digest: &Digest,
        artifact_type_filter: Option<&str>,
    ) -> Result<Vec<bencher_json::oci::OciDescriptor>, OciStorageError> {
        match self {
            Self::S3(s3) => {
                s3.list_referrers(repository, subject_digest, artifact_type_filter)
                    .await
            },
            Self::Local(local) => {
                local
                    .list_referrers(repository, subject_digest, artifact_type_filter)
                    .await
            },
        }
    }
}

/// Check whether an S3 SDK error is a 404 Not Found response.
fn is_s3_not_found<E>(err: &aws_sdk_s3::error::SdkError<E>) -> bool {
    err.raw_response()
        .is_some_and(|r| r.status().as_u16() == 404)
}

impl OciS3Storage {
    /// Creates a new S3 storage instance
    fn new(
        log: Logger,
        access_key_id: String,
        secret_access_key: Secret,
        access_point: &str,
        upload_timeout: u64,
        max_body_size: u64,
        clock: Clock,
    ) -> Result<Self, OciStorageError> {
        // Parse the S3 ARN
        let arn = S3Arn::from_str(access_point)
            .map_err(|e| OciStorageError::InvalidArn(e.to_string()))?;

        // Create credentials
        let credentials = aws_credential_types::Credentials::new(
            access_key_id,
            secret_access_key,
            None,
            None,
            "bencher_oci",
        );
        let credentials_provider =
            aws_credential_types::provider::SharedCredentialsProvider::new(credentials);

        // Build S3 client
        let s3_config = aws_sdk_s3::Config::builder()
            .credentials_provider(credentials_provider)
            .region(aws_sdk_s3::config::Region::new(arn.region.clone()))
            .build();
        let client = Client::from_conf(s3_config);

        let config = OciStorageConfig {
            bucket_arn: arn.bucket_arn(),
            prefix: arn.bucket_path.clone(),
        };

        let concurrency = std::thread::available_parallelism()
            .map(std::num::NonZeroUsize::get)
            .unwrap_or(1)
            .clamp(1, MAX_CONCURRENCY);

        Ok(Self {
            client,
            config,
            upload_timeout,
            max_body_size,
            log,
            concurrency,
            last_cleanup: AtomicI64::new(0),
            clock,
        })
    }

    // ==================== S3 Error Helpers ====================

    /// Maps an S3 SDK error, converting 404 responses to the provided not-found error.
    fn map_s3_error<E: std::fmt::Display>(
        err: &aws_sdk_s3::error::SdkError<E>,
        not_found_error: OciStorageError,
    ) -> OciStorageError {
        if is_s3_not_found(err) {
            not_found_error
        } else {
            OciStorageError::S3(err.to_string())
        }
    }

    // ==================== Key Generation ====================

    /// Returns the S3 key prefix for the given repository
    fn key_prefix(&self, repository: &ProjectUuid) -> String {
        match &self.config.prefix {
            Some(prefix) => format!("{prefix}/{repository}"),
            None => repository.to_string(),
        }
    }

    /// Returns the global prefix (for upload staging area)
    fn global_prefix(&self) -> String {
        match &self.config.prefix {
            Some(prefix) => format!("{prefix}/_uploads"),
            None => "_uploads".to_owned(),
        }
    }

    /// Returns the S3 key for upload state metadata
    fn upload_state_key(&self, upload_id: &UploadId) -> String {
        format!("{}/{}/state.json", self.global_prefix(), upload_id)
    }

    /// Returns the S3 key prefix for buffer chunks
    ///
    /// Buffer chunks are stored separately to avoid O(n²) read-modify-write
    /// operations. Each append creates a new chunk object with a unique key.
    fn upload_chunks_prefix(&self, upload_id: &UploadId) -> String {
        format!("{}/{}/chunks/", self.global_prefix(), upload_id)
    }

    /// Returns a unique S3 key for a new buffer chunk
    ///
    /// Uses timestamp (nanoseconds) + UUID for uniqueness and ordering.
    /// The timestamp ensures chunks are sorted in creation order when listed.
    fn new_chunk_key(&self, upload_id: &UploadId) -> String {
        let timestamp = Utc::now().timestamp_nanos_opt().unwrap_or(0);
        let uuid = uuid::Uuid::new_v4();
        format!(
            "{}/{}/chunks/{:020}_{uuid}",
            self.global_prefix(),
            upload_id,
            timestamp
        )
    }

    /// Returns the S3 key for the temporary upload data (multipart destination)
    fn upload_data_key(&self, upload_id: &UploadId) -> String {
        format!("{}/{}/data", self.global_prefix(), upload_id)
    }

    /// Returns the S3 key for a blob
    fn blob_key(&self, repository: &ProjectUuid, digest: &Digest) -> String {
        format!(
            "{}/blobs/{}/{}",
            self.key_prefix(repository),
            digest.algorithm(),
            digest.hex_hash()
        )
    }

    /// Returns the S3 key for a manifest by digest
    fn manifest_key_by_digest(&self, repository: &ProjectUuid, digest: &Digest) -> String {
        format!(
            "{}/manifests/{}/{}",
            self.key_prefix(repository),
            digest.algorithm(),
            digest.hex_hash()
        )
    }

    /// Returns the S3 key for a manifest tag link
    fn tag_link_key(&self, repository: &ProjectUuid, tag: &str) -> String {
        format!("{}/tags/{}", self.key_prefix(repository), tag)
    }

    /// Returns the S3 key prefix for referrers to a given digest
    fn referrers_prefix(&self, repository: &ProjectUuid, subject_digest: &Digest) -> String {
        format!(
            "{}/referrers/{}/{}",
            self.key_prefix(repository),
            subject_digest.algorithm(),
            subject_digest.hex_hash()
        )
    }

    /// Returns the S3 key for a referrer link
    fn referrer_key(
        &self,
        repository: &ProjectUuid,
        subject_digest: &Digest,
        referrer_digest: &Digest,
    ) -> String {
        format!(
            "{}/{}-{}",
            self.referrers_prefix(repository, subject_digest),
            referrer_digest.algorithm(),
            referrer_digest.hex_hash()
        )
    }

    // ==================== Upload State Management ====================

    /// Validates that the upload session belongs to the expected repository
    async fn validate_upload_repository(
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

    /// Loads upload state from S3
    async fn load_upload_state(
        &self,
        upload_id: &UploadId,
    ) -> Result<UploadState, OciStorageError> {
        let key = self.upload_state_key(upload_id);
        let response = self
            .client
            .get_object()
            .bucket(&self.config.bucket_arn)
            .key(&key)
            .send()
            .await
            .map_err(|e| {
                Self::map_s3_error(&e, OciStorageError::UploadNotFound(upload_id.to_string()))
            })?;

        let data = response
            .body
            .collect()
            .await
            .map_err(|e| OciStorageError::S3(e.to_string()))?
            .into_bytes();

        serde_json::from_slice(&data).map_err(|e| OciStorageError::Json(e.to_string()))
    }

    /// Saves upload state to S3
    async fn save_upload_state(
        &self,
        upload_id: &UploadId,
        state: &UploadState,
    ) -> Result<(), OciStorageError> {
        let key = self.upload_state_key(upload_id);
        let data = serde_json::to_vec(state).map_err(|e| OciStorageError::Json(e.to_string()))?;

        self.client
            .put_object()
            .bucket(&self.config.bucket_arn)
            .key(&key)
            .body(data.into())
            .content_type("application/json")
            .send()
            .await
            .map_err(|e| OciStorageError::S3(e.to_string()))?;

        Ok(())
    }

    /// Stores a buffer chunk to S3 with a unique key
    ///
    /// This is an O(1) operation with no race conditions - each chunk gets
    /// a unique key based on timestamp + UUID, so concurrent appends don't
    /// interfere with each other.
    ///
    /// Returns the size of the stored chunk.
    async fn store_buffer_chunk(
        &self,
        upload_id: &UploadId,
        data: Bytes,
    ) -> Result<u64, OciStorageError> {
        let key = self.new_chunk_key(upload_id);
        let size = data.len() as u64;
        self.client
            .put_object()
            .bucket(&self.config.bucket_arn)
            .key(&key)
            .body(data.into())
            .send()
            .await
            .map_err(|e| OciStorageError::S3(e.to_string()))?;
        Ok(size)
    }

    /// Loads a buffer chunk from S3 by its full key
    async fn load_buffer_chunk_by_key(&self, key: &str) -> Result<Bytes, OciStorageError> {
        let response = self
            .client
            .get_object()
            .bucket(&self.config.bucket_arn)
            .key(key)
            .send()
            .await
            .map_err(|e| OciStorageError::S3(e.to_string()))?;

        let data = response
            .body
            .collect()
            .await
            .map_err(|e| OciStorageError::S3(e.to_string()))?
            .into_bytes();

        Ok(data)
    }

    /// Lists all buffer chunks for an upload, sorted by key (which ensures chronological order)
    ///
    /// Returns a list of (key, size) tuples.
    async fn list_buffer_chunks(
        &self,
        upload_id: &UploadId,
    ) -> Result<Vec<(String, u64)>, OciStorageError> {
        let prefix = self.upload_chunks_prefix(upload_id);
        let mut chunks = Vec::new();
        let mut continuation_token: Option<String> = None;

        loop {
            let mut request = self
                .client
                .list_objects_v2()
                .bucket(&self.config.bucket_arn)
                .prefix(&prefix);

            if let Some(token) = continuation_token.take() {
                request = request.continuation_token(token);
            }

            let response = request
                .send()
                .await
                .map_err(|e| OciStorageError::S3(e.to_string()))?;

            if let Some(contents) = response.contents {
                for object in contents {
                    if let Some(key) = object.key {
                        let size = if let Some(s) = object.size {
                            u64::try_from(s).unwrap_or(0)
                        } else {
                            slog::warn!(self.log, "S3 object missing size"; "key" => &key);
                            0
                        };
                        chunks.push((key, size));
                    }
                }
            }

            if response.is_truncated == Some(true) {
                continuation_token = response.next_continuation_token;
            } else {
                break;
            }
        }

        // Sort by key to ensure chronological order (keys are timestamp-prefixed)
        chunks.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(chunks)
    }

    /// Deletes all upload-related objects from S3
    ///
    /// Cleanup order: buffer chunks first, then data object, then state file last.
    /// This ordering ensures that if a crash occurs mid-cleanup, the state file
    /// still exists for discovery by `cleanup_stale_uploads_s3`, preventing
    /// permanently orphaned chunk objects.
    async fn cleanup_upload(&self, upload_id: &UploadId) {
        // Best effort cleanup - ignore errors
        let state_key = self.upload_state_key(upload_id);
        let data_key = self.upload_data_key(upload_id);

        // Delete all buffer chunks first (while state still exists for discovery)
        if let Ok(chunks) = self.list_buffer_chunks(upload_id).await {
            for (chunk_key, _size) in chunks {
                if let Err(e) = self
                    .client
                    .delete_object()
                    .bucket(&self.config.bucket_arn)
                    .key(&chunk_key)
                    .send()
                    .await
                {
                    report_cleanup_error(&self.log, "cleanup_upload: chunk delete", &e);
                }
            }
        }

        // Delete multipart data object
        if let Err(e) = self
            .client
            .delete_object()
            .bucket(&self.config.bucket_arn)
            .key(&data_key)
            .send()
            .await
        {
            report_cleanup_error(&self.log, "cleanup_upload: data delete", &e);
        }

        // Delete state last (so discovery still works if crash occurs above)
        if let Err(e) = self
            .client
            .delete_object()
            .bucket(&self.config.bucket_arn)
            .key(&state_key)
            .send()
            .await
        {
            report_cleanup_error(&self.log, "cleanup_upload: state delete", &e);
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

        let client = self.client.clone();
        let config = self.config.clone();
        let upload_timeout = self.upload_timeout;
        let log = self.log.clone();
        let clock = self.clock.clone();

        tokio::spawn(async move {
            cleanup_stale_uploads_s3(&log, client, config, upload_timeout, clock).await;
        });
    }

    // ==================== Upload Operations ====================

    /// Starts a new upload session using S3 multipart upload
    ///
    /// Also spawns a background task to clean up any stale uploads older than `upload_timeout`.
    pub async fn start_upload(
        &self,
        repository: &ProjectUuid,
    ) -> Result<UploadId, OciStorageError> {
        // Spawn background cleanup task (non-blocking)
        self.spawn_stale_upload_cleanup();

        let upload_id = UploadId::new();
        let data_key = self.upload_data_key(&upload_id);

        // Create S3 multipart upload
        let multipart = self
            .client
            .create_multipart_upload()
            .bucket(&self.config.bucket_arn)
            .key(&data_key)
            .send()
            .await
            .map_err(|e| OciStorageError::S3(e.to_string()))?;

        let s3_upload_id = multipart
            .upload_id()
            .ok_or_else(|| OciStorageError::S3("No upload ID returned".to_owned()))?
            .to_owned();

        // Save initial state with creation timestamp
        let state = UploadState {
            s3_upload_id,
            repository: repository.to_string(),
            parts: Vec::new(),
            created_at: self.clock.timestamp(),
        };
        self.save_upload_state(&upload_id, &state).await?;

        Ok(upload_id)
    }

    /// Appends data to an in-progress upload
    ///
    /// Each append is stored as a separate chunk in S3 with a unique key
    /// (timestamp + UUID). This is race-free because concurrent appends
    /// create independent objects rather than modifying shared state.
    ///
    /// Chunks are listed, sorted, and combined at completion time.
    ///
    /// Note: This method re-lists all buffer chunks to compute the current size.
    /// While this is O(n) in the number of chunks, it is intentionally chosen
    /// over tracking cumulative size in upload state to avoid race conditions
    /// with concurrent appends.
    pub async fn append_upload(
        &self,
        upload_id: &UploadId,
        data: Bytes,
    ) -> Result<u64, OciStorageError> {
        // Verify upload exists (we don't need to modify state for appends)
        let state = self.load_upload_state(upload_id).await?;

        // Calculate projected total size BEFORE storing the chunk
        let chunks = self.list_buffer_chunks(upload_id).await?;
        let buffer_total_size: u64 = chunks.iter().map(|(_key, size)| size).sum();
        let projected_total = state.completed_parts_size() + buffer_total_size + data.len() as u64;

        if projected_total > self.max_body_size {
            return Err(OciStorageError::SizeExceeded {
                size: projected_total,
                max: self.max_body_size,
            });
        }

        // Store data as a new chunk with unique key (race-free)
        self.store_buffer_chunk(upload_id, data).await?;

        Ok(projected_total)
    }

    /// Gets the current size of an in-progress upload
    ///
    /// Lists all buffer chunks to get the authoritative total size,
    /// avoiding race conditions from concurrent appends.
    pub async fn get_upload_size(&self, upload_id: &UploadId) -> Result<u64, OciStorageError> {
        let state = self.load_upload_state(upload_id).await?;
        let chunks = self.list_buffer_chunks(upload_id).await?;
        let buffer_total_size: u64 = chunks.iter().map(|(_key, size)| size).sum();
        Ok(state.completed_parts_size() + buffer_total_size)
    }

    /// Completes an upload and stores the blob
    ///
    /// This uses incremental hashing to avoid downloading the entire blob:
    /// 1. Streams through all buffer chunks, computing hash incrementally
    /// 2. Uploads parts to S3 multipart when buffer reaches 5MB
    /// 3. Verifies digest matches expected
    /// 4. Copies completed multipart to final blob location
    /// 5. Cleans up temporary objects
    ///
    /// If the server reboots mid-completion, the next attempt will abort the
    /// stale multipart upload and start fresh. The hash is always recomputed
    /// from all stored chunks, ensuring correctness.
    #[expect(
        clippy::too_many_lines,
        reason = "Complex upload completion logic benefits from being in one place"
    )]
    pub async fn complete_upload(
        &self,
        upload_id: &UploadId,
        expected_digest: &Digest,
    ) -> Result<Digest, OciStorageError> {
        // Load state
        let mut state = self.load_upload_state(upload_id).await?;
        let data_key = self.upload_data_key(upload_id);

        // List all buffer chunks (sorted by key for chronological order)
        let chunks = self.list_buffer_chunks(upload_id).await?;

        // If there are stale parts from an interrupted completion attempt,
        // abort the old multipart upload and start fresh to ensure consistency
        if !state.parts.is_empty() {
            let _unused = self
                .client
                .abort_multipart_upload()
                .bucket(&self.config.bucket_arn)
                .key(&data_key)
                .upload_id(&state.s3_upload_id)
                .send()
                .await;

            // Create new multipart upload
            let multipart = self
                .client
                .create_multipart_upload()
                .bucket(&self.config.bucket_arn)
                .key(&data_key)
                .send()
                .await
                .map_err(|e| OciStorageError::S3(e.to_string()))?;

            multipart
                .upload_id()
                .ok_or_else(|| OciStorageError::S3("No upload ID returned".to_owned()))?
                .clone_into(&mut state.s3_upload_id);
            state.parts.clear();
        }

        // Must have some data
        if chunks.is_empty() && state.parts.is_empty() {
            self.cleanup_upload(upload_id).await;
            return Err(OciStorageError::BlobUploadInvalidContent(
                "Cannot complete upload with no data".to_owned(),
            ));
        }

        // Stream through chunks with incremental hashing
        let mut hasher = Sha256::new();
        let mut part_buffer = Vec::new();

        for (chunk_key, _size) in &chunks {
            // Load chunk by key
            let chunk = self.load_buffer_chunk_by_key(chunk_key).await?;

            // Update hash incrementally (no egress cost - we're reading to process)
            hasher.update(&chunk);

            // Add to part buffer
            part_buffer.extend_from_slice(&chunk);

            // Upload complete parts when we reach 5MB threshold
            while part_buffer.len() >= MIN_PART_SIZE {
                let part_data: Vec<u8> = part_buffer.drain(..MIN_PART_SIZE).collect();
                self.upload_multipart_part(&mut state, &data_key, part_data)
                    .await?;
            }
        }

        // Upload any remaining data as the final part
        if !part_buffer.is_empty() {
            self.upload_multipart_part(&mut state, &data_key, part_buffer)
                .await?;
        }

        // Must have at least one part for S3 multipart completion
        if state.parts.is_empty() {
            self.cleanup_upload(upload_id).await;
            return Err(OciStorageError::BlobUploadInvalidContent(
                "Cannot complete upload with no data".to_owned(),
            ));
        }

        // Compute actual digest from incremental hash
        let hash = hasher.finalize();
        // hex::encode always produces valid hex, so this is infallible in practice
        let actual_digest = Digest::sha256(&hex::encode(hash))
            .map_err(|e| OciStorageError::InvalidContent(e.to_string()))?;

        // Verify digest matches BEFORE completing multipart (fail fast)
        if actual_digest.as_str() != expected_digest.as_str() {
            // Abort the multipart upload
            let _unused = self
                .client
                .abort_multipart_upload()
                .bucket(&self.config.bucket_arn)
                .key(&data_key)
                .upload_id(&state.s3_upload_id)
                .send()
                .await;
            self.cleanup_upload(upload_id).await;
            return Err(OciStorageError::DigestMismatch {
                expected: expected_digest.to_string(),
                actual: actual_digest.to_string(),
            });
        }

        // Build completed parts list for S3
        let completed_parts: Vec<_> = state
            .parts
            .iter()
            .map(|p| {
                aws_sdk_s3::types::CompletedPart::builder()
                    .part_number(p.part_number)
                    .e_tag(&p.etag)
                    .build()
            })
            .collect();

        // Complete the multipart upload
        self.client
            .complete_multipart_upload()
            .bucket(&self.config.bucket_arn)
            .key(&data_key)
            .upload_id(&state.s3_upload_id)
            .multipart_upload(
                CompletedMultipartUpload::builder()
                    .set_parts(Some(completed_parts))
                    .build(),
            )
            .send()
            .await
            .map_err(|e| OciStorageError::S3(e.to_string()))?;

        // Parse repository UUID
        let repository: ProjectUuid = state.repository.parse().map_err(|_e| {
            OciStorageError::InvalidContent(format!(
                "Invalid project UUID in upload state: {}",
                state.repository
            ))
        })?;

        // Copy to final blob location
        // For S3 Access Points, copy source must use the format:
        // arn:aws:s3:region:account-id:accesspoint/accesspoint-name/object/key
        let blob_key = self.blob_key(&repository, &actual_digest);
        self.client
            .copy_object()
            .bucket(&self.config.bucket_arn)
            .copy_source(format!("{}/object/{}", self.config.bucket_arn, data_key))
            .key(&blob_key)
            .send()
            .await
            .map_err(|e| OciStorageError::S3(e.to_string()))?;

        // Clean up temporary objects
        self.cleanup_upload(upload_id).await;

        Ok(actual_digest)
    }

    /// Uploads a single part to S3 multipart upload
    async fn upload_multipart_part(
        &self,
        state: &mut UploadState,
        data_key: &str,
        data: Vec<u8>,
    ) -> Result<(), OciStorageError> {
        let part_number = state.next_part_number()?;
        let part_size = data.len() as u64;

        let response = self
            .client
            .upload_part()
            .bucket(&self.config.bucket_arn)
            .key(data_key)
            .upload_id(&state.s3_upload_id)
            .part_number(part_number)
            .body(data.into())
            .send()
            .await
            .map_err(|e| OciStorageError::S3(e.to_string()))?;

        let etag = response
            .e_tag()
            .ok_or_else(|| OciStorageError::S3("No ETag returned for part".to_owned()))?
            .to_owned();

        state.parts.push(CompletedPartInfo {
            part_number,
            etag,
            size: part_size,
        });

        Ok(())
    }

    /// Cancels an in-progress upload
    pub async fn cancel_upload(&self, upload_id: &UploadId) -> Result<(), OciStorageError> {
        // Load state to get S3 upload ID
        let state = self.load_upload_state(upload_id).await?;
        let data_key = self.upload_data_key(upload_id);

        // Abort the S3 multipart upload
        if let Err(e) = self
            .client
            .abort_multipart_upload()
            .bucket(&self.config.bucket_arn)
            .key(&data_key)
            .upload_id(&state.s3_upload_id)
            .send()
            .await
        {
            report_cleanup_error(&self.log, "cancel_upload: abort multipart", &e);
        }

        // Clean up (lists and deletes all buffer chunks)
        self.cleanup_upload(upload_id).await;

        Ok(())
    }

    // ==================== Blob Operations ====================

    /// Checks if a blob exists
    pub async fn blob_exists(
        &self,
        repository: &ProjectUuid,
        digest: &Digest,
    ) -> Result<bool, OciStorageError> {
        let key = self.blob_key(repository, digest);
        match self
            .client
            .head_object()
            .bucket(&self.config.bucket_arn)
            .key(&key)
            .send()
            .await
        {
            Ok(_) => Ok(true),
            Err(e) => {
                if is_s3_not_found(&e) {
                    Ok(false)
                } else {
                    Err(OciStorageError::S3(e.to_string()))
                }
            },
        }
    }

    /// Gets a blob's content and size (loads entire blob into memory)
    ///
    /// For large blobs, prefer `get_blob_stream` which streams the content.
    pub async fn get_blob(
        &self,
        repository: &ProjectUuid,
        digest: &Digest,
    ) -> Result<(Bytes, u64), OciStorageError> {
        let key = self.blob_key(repository, digest);
        let response = self
            .client
            .get_object()
            .bucket(&self.config.bucket_arn)
            .key(&key)
            .send()
            .await
            .map_err(|e| {
                Self::map_s3_error(&e, OciStorageError::BlobNotFound(digest.to_string()))
            })?;

        let size = response
            .content_length()
            .map_or(0, |len| u64::try_from(len).unwrap_or(0));
        let data = response
            .body
            .collect()
            .await
            .map_err(|e| OciStorageError::S3(e.to_string()))?
            .into_bytes();

        Ok((data, size))
    }

    /// Gets a blob as a streaming body
    ///
    /// Returns a streaming body and the blob size. The content is streamed
    /// from S3 rather than loaded entirely into memory.
    pub async fn get_blob_stream(
        &self,
        repository: &ProjectUuid,
        digest: &Digest,
    ) -> Result<(S3BlobBody, u64), OciStorageError> {
        let key = self.blob_key(repository, digest);
        let response = self
            .client
            .get_object()
            .bucket(&self.config.bucket_arn)
            .key(&key)
            .send()
            .await
            .map_err(|e| {
                Self::map_s3_error(&e, OciStorageError::BlobNotFound(digest.to_string()))
            })?;

        let size = response
            .content_length()
            .map_or(0, |len| u64::try_from(len).unwrap_or(0));

        Ok((
            S3BlobBody {
                inner: response.body,
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
        let key = self.blob_key(repository, digest);
        let response = self
            .client
            .head_object()
            .bucket(&self.config.bucket_arn)
            .key(&key)
            .send()
            .await
            .map_err(|e| {
                Self::map_s3_error(&e, OciStorageError::BlobNotFound(digest.to_string()))
            })?;

        Ok(response
            .content_length()
            .map_or(0, |len| u64::try_from(len).unwrap_or(0)))
    }

    /// Deletes a blob
    pub async fn delete_blob(
        &self,
        repository: &ProjectUuid,
        digest: &Digest,
    ) -> Result<(), OciStorageError> {
        let key = self.blob_key(repository, digest);
        self.client
            .delete_object()
            .bucket(&self.config.bucket_arn)
            .key(&key)
            .send()
            .await
            .map_err(|e| OciStorageError::S3(e.to_string()))?;

        Ok(())
    }

    /// Mounts a blob from another repository (cross-repo blob mount)
    ///
    /// Attempts the copy directly and handles not-found, avoiding a TOCTOU race
    /// between checking existence and copying.
    pub async fn mount_blob(
        &self,
        from_repository: &ProjectUuid,
        to_repository: &ProjectUuid,
        digest: &Digest,
    ) -> Result<bool, OciStorageError> {
        let source_key = self.blob_key(from_repository, digest);
        let dest_key = self.blob_key(to_repository, digest);

        match self
            .client
            .copy_object()
            .bucket(&self.config.bucket_arn)
            .copy_source(format!("{}/object/{}", self.config.bucket_arn, source_key))
            .key(&dest_key)
            .send()
            .await
        {
            Ok(_) => Ok(true),
            Err(e) => {
                if is_s3_not_found(&e) {
                    Ok(false)
                } else {
                    Err(OciStorageError::S3(e.to_string()))
                }
            },
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
        let manifest_key = self.manifest_key_by_digest(repository, &digest);
        self.client
            .put_object()
            .bucket(&self.config.bucket_arn)
            .key(&manifest_key)
            .body(content.clone().into())
            .send()
            .await
            .map_err(|e| OciStorageError::S3(e.to_string()))?;

        // If a tag was provided, create a tag link
        if let Some(tag) = tag {
            let tag_key = self.tag_link_key(repository, tag.as_str());
            self.client
                .put_object()
                .bucket(&self.config.bucket_arn)
                .key(&tag_key)
                .body(digest.to_string().into_bytes().into())
                .send()
                .await
                .map_err(|e| OciStorageError::S3(e.to_string()))?;
        }

        // Check if manifest has a subject field (for referrers API)
        if let Some((subject_digest, descriptor)) =
            crate::types::build_referrer_descriptor(manifest, &digest, content.len())
        {
            // Store referrer link
            let referrer_key = self.referrer_key(repository, &subject_digest, &digest);
            self.client
                .put_object()
                .bucket(&self.config.bucket_arn)
                .key(&referrer_key)
                .body(
                    serde_json::to_vec(&descriptor)
                        .map_err(|e| OciStorageError::Json(e.to_string()))?
                        .into(),
                )
                .send()
                .await
                .map_err(|e| OciStorageError::S3(e.to_string()))?;
        }

        Ok(digest)
    }

    /// Gets a manifest by digest
    pub async fn get_manifest_by_digest(
        &self,
        repository: &ProjectUuid,
        digest: &Digest,
    ) -> Result<Bytes, OciStorageError> {
        let key = self.manifest_key_by_digest(repository, digest);
        let response = self
            .client
            .get_object()
            .bucket(&self.config.bucket_arn)
            .key(&key)
            .send()
            .await
            .map_err(|e| {
                Self::map_s3_error(&e, OciStorageError::ManifestNotFound(digest.to_string()))
            })?;

        let data = response
            .body
            .collect()
            .await
            .map_err(|e| OciStorageError::S3(e.to_string()))?
            .into_bytes();

        Ok(data)
    }

    /// Resolves a tag to a digest
    pub async fn resolve_tag(
        &self,
        repository: &ProjectUuid,
        tag: &crate::types::Tag,
    ) -> Result<Digest, OciStorageError> {
        let key = self.tag_link_key(repository, tag.as_str());
        let response = self
            .client
            .get_object()
            .bucket(&self.config.bucket_arn)
            .key(&key)
            .send()
            .await
            .map_err(|e| {
                Self::map_s3_error(&e, OciStorageError::ManifestNotFound(tag.to_string()))
            })?;

        let data = response
            .body
            .collect()
            .await
            .map_err(|e| OciStorageError::S3(e.to_string()))?
            .into_bytes();

        let digest_str = String::from_utf8(data.to_vec())
            .map_err(|e| OciStorageError::InvalidContent(e.to_string()))?;

        digest_str
            .trim()
            .parse()
            .map_err(|e: crate::types::DigestError| OciStorageError::InvalidContent(e.to_string()))
    }

    /// Lists tags for a repository with optional pagination
    ///
    /// - `limit`: Maximum number of tags to return
    /// - `start_after`: Tag to start listing after (for cursor-based pagination)
    ///
    /// Returns tags sorted lexicographically by S3.
    pub async fn list_tags(
        &self,
        repository: &ProjectUuid,
        limit: Option<usize>,
        start_after: Option<&str>,
    ) -> Result<ListTagsResult, OciStorageError> {
        let prefix = format!("{}/tags/", self.key_prefix(repository));

        let mut tags = Vec::new();
        let mut continuation_token: Option<String> = None;

        // Calculate how many we need to fetch - one extra to detect if more exist
        let fetch_limit = limit.map(|l| l + 1);
        let mut remaining = fetch_limit;

        loop {
            let mut request = self
                .client
                .list_objects_v2()
                .bucket(&self.config.bucket_arn)
                .prefix(&prefix);

            // Apply max_keys limit if specified
            if let Some(rem) = remaining {
                // rem.min(1000) is always <= 1000 which fits in i32
                let max_keys = i32::try_from(rem.min(1000)).unwrap_or(1000);
                request = request.max_keys(max_keys);
            }

            // Apply continuation token or start_after (mutually exclusive in S3 API)
            if let Some(token) = continuation_token.take() {
                request = request.continuation_token(token);
            } else if let Some(start) = start_after {
                // Only apply start_after on the first request
                request = request.start_after(format!("{prefix}{start}"));
            }

            let response = request
                .send()
                .await
                .map_err(|e| OciStorageError::S3(e.to_string()))?;

            let count_before = tags.len();
            if let Some(contents) = response.contents {
                for object in contents {
                    if let Some(key) = object.key
                        && let Some(tag) = key.strip_prefix(&prefix)
                    {
                        tags.push(tag.to_owned());

                        // Check if we've collected enough (fetch_limit = limit + 1)
                        if let Some(fl) = fetch_limit
                            && tags.len() >= fl
                        {
                            let has_more = limit.is_some_and(|l| tags.len() > l);
                            if let Some(l) = limit {
                                tags.truncate(l);
                            }
                            return Ok(ListTagsResult { tags, has_more });
                        }
                    }
                }
            }

            // Update remaining count based on items added this iteration
            let added_this_iteration = tags.len() - count_before;
            if let Some(rem) = remaining.as_mut() {
                *rem = rem.saturating_sub(added_this_iteration);
                if *rem == 0 {
                    break;
                }
            }

            if response.is_truncated == Some(true) {
                continuation_token = response.next_continuation_token;
            } else {
                break;
            }
        }

        let has_more = limit.is_some_and(|l| tags.len() > l);
        if let Some(l) = limit {
            tags.truncate(l);
        }
        Ok(ListTagsResult { tags, has_more })
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
        // Try to read the manifest first to check for subject field
        // If we can read it and it has a subject, clean up the referrer link
        if let Ok(data) = self.get_manifest_by_digest(repository, digest).await
            && let Some(subject_digest) = crate::types::extract_subject_digest(&data)
        {
            let referrer_key = self.referrer_key(repository, &subject_digest, digest);
            if let Err(e) = self
                .client
                .delete_object()
                .bucket(&self.config.bucket_arn)
                .key(&referrer_key)
                .send()
                .await
            {
                report_cleanup_error(&self.log, "delete_manifest: referrer link delete", &e);
            }
        }

        // Clean up any tags that point to this digest (best-effort)
        if let Ok(result) = self.list_tags(repository, None, None).await {
            let tags_to_delete: Vec<String> = stream::iter(
                result
                    .tags
                    .into_iter()
                    .filter_map(|tag_name| {
                        tag_name
                            .parse::<crate::types::Tag>()
                            .ok()
                            .map(|tag| (tag_name, tag))
                    })
                    .map(|(tag_name, tag)| async move {
                        match self.resolve_tag(repository, &tag).await {
                            Ok(tag_digest) if tag_digest.as_str() == digest.as_str() => {
                                Some(tag_name)
                            },
                            _ => None,
                        }
                    }),
            )
            .buffer_unordered(self.concurrency)
            .filter_map(|x| async { x })
            .collect()
            .await;

            for tag_name in tags_to_delete {
                let tag_key = self.tag_link_key(repository, &tag_name);
                if let Err(e) = self
                    .client
                    .delete_object()
                    .bucket(&self.config.bucket_arn)
                    .key(&tag_key)
                    .send()
                    .await
                {
                    report_cleanup_error(&self.log, "delete_manifest: tag link delete", &e);
                }
            }
        }

        // Delete the manifest itself
        let key = self.manifest_key_by_digest(repository, digest);
        self.client
            .delete_object()
            .bucket(&self.config.bucket_arn)
            .key(&key)
            .send()
            .await
            .map_err(|e| OciStorageError::S3(e.to_string()))?;

        Ok(())
    }

    /// Deletes a tag (removes the tag link, not the manifest itself)
    pub async fn delete_tag(
        &self,
        repository: &ProjectUuid,
        tag: &crate::types::Tag,
    ) -> Result<(), OciStorageError> {
        let key = self.tag_link_key(repository, tag.as_str());
        self.client
            .delete_object()
            .bucket(&self.config.bucket_arn)
            .key(&key)
            .send()
            .await
            .map_err(|e| OciStorageError::S3(e.to_string()))?;

        Ok(())
    }

    /// Lists all manifests that reference a given digest via their subject field
    ///
    /// Uses parallel fetches (up to 10 concurrent) for improved performance.
    pub async fn list_referrers(
        &self,
        repository: &ProjectUuid,
        subject_digest: &Digest,
        artifact_type_filter: Option<&str>,
    ) -> Result<Vec<bencher_json::oci::OciDescriptor>, OciStorageError> {
        let prefix = self.referrers_prefix(repository, subject_digest);

        // First, collect all keys
        let mut keys = Vec::new();
        let mut continuation_token: Option<String> = None;

        loop {
            let mut request = self
                .client
                .list_objects_v2()
                .bucket(&self.config.bucket_arn)
                .prefix(&prefix);

            if let Some(token) = continuation_token.take() {
                request = request.continuation_token(token);
            }

            let response = request
                .send()
                .await
                .map_err(|e| OciStorageError::S3(e.to_string()))?;

            if let Some(contents) = response.contents {
                for object in contents {
                    if let Some(key) = object.key {
                        keys.push(key);
                    }
                }
            }

            if response.is_truncated == Some(true) {
                continuation_token = response.next_continuation_token;
            } else {
                break;
            }
        }

        // Fetch referrer descriptors in parallel
        let filter = artifact_type_filter.map(ToOwned::to_owned);
        let log = &self.log;
        let referrers: Vec<bencher_json::oci::OciDescriptor> = stream::iter(keys)
            .map(|key| {
                let client = &self.client;
                let bucket = &self.config.bucket_arn;
                let filter = filter.clone();
                async move {
                    // Get the referrer descriptor
                    let Ok(resp) = client.get_object().bucket(bucket).key(&key).send().await else {
                        slog::warn!(log, "Failed to fetch referrer from S3"; "key" => &key);
                        return None;
                    };
                    let Ok(data) = resp.body.collect().await else {
                        slog::warn!(log, "Failed to collect referrer body from S3"; "key" => &key);
                        return None;
                    };
                    let Ok(descriptor) = serde_json::from_slice::<bencher_json::oci::OciDescriptor>(
                        &data.into_bytes(),
                    ) else {
                        slog::warn!(log, "Failed to parse referrer JSON from S3"; "key" => &key);
                        return None;
                    };
                    // Apply artifact type filter if specified
                    if let Some(filter) = &filter
                        && descriptor.artifact_type.as_deref() != Some(filter.as_str())
                    {
                        return None;
                    }
                    Some(descriptor)
                }
            })
            .buffer_unordered(self.concurrency)
            .filter_map(|x| async move { x })
            .collect()
            .await;

        Ok(referrers)
    }

    // ==================== Job Output ====================

    /// S3 key for a job output blob.
    fn job_output_key(&self, project: ProjectUuid, job: bencher_json::JobUuid) -> String {
        format!("{}/output/v0/jobs/{job}", self.key_prefix(&project))
    }

    pub(crate) async fn put_job_output(
        &self,
        project: ProjectUuid,
        job: bencher_json::JobUuid,
        output: &bencher_json::runner::JsonJobOutput,
    ) -> Result<(), OciStorageError> {
        let key = self.job_output_key(project, job);
        let data = serde_json::to_vec(output).map_err(|e| OciStorageError::Json(e.to_string()))?;

        self.client
            .put_object()
            .bucket(&self.config.bucket_arn)
            .key(&key)
            .body(data.into())
            .content_type("application/json")
            // Job output is immutable once stored (terminal jobs don't change),
            // so allow aggressive caching.
            .cache_control("public, max-age=31536000, immutable")
            .send()
            .await
            .map_err(|e| OciStorageError::S3(e.to_string()))?;

        Ok(())
    }

    pub(crate) async fn get_job_output(
        &self,
        project: ProjectUuid,
        job: bencher_json::JobUuid,
    ) -> Result<Option<bencher_json::runner::JsonJobOutput>, OciStorageError> {
        let key = self.job_output_key(project, job);

        match self
            .client
            .get_object()
            .bucket(&self.config.bucket_arn)
            .key(&key)
            .send()
            .await
        {
            Ok(response) => {
                let data = response
                    .body
                    .collect()
                    .await
                    .map_err(|e| OciStorageError::S3(e.to_string()))?
                    .into_bytes();
                let output = serde_json::from_slice(&data)
                    .map_err(|e| OciStorageError::Json(e.to_string()))?;
                Ok(Some(output))
            },
            Err(e) => {
                if is_s3_not_found(&e) {
                    Ok(None)
                } else {
                    Err(OciStorageError::S3(e.to_string()))
                }
            },
        }
    }
}

// ==================== S3 ARN Parsing ====================

const ARN_PREFIX: &str = "arn";
const S3_SERVICE: &str = "s3";
const ACCESSPOINT: &str = "accesspoint";

#[derive(Debug, Clone)]
struct S3Arn {
    partition: String,
    region: String,
    account_id: String,
    bucket_name: String,
    bucket_path: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum S3ArnError {
    #[error("Missing S3 ARN prefix")]
    NoPrefix,
    #[error("Invalid S3 ARN prefix: {0}")]
    BadPrefix(String),
    #[error("Missing S3 ARN partition")]
    NoPartition,
    #[error("Missing S3 ARN service")]
    NoService,
    #[error("Invalid S3 ARN service: {0}")]
    BadService(String),
    #[error("Missing S3 ARN region")]
    NoRegion,
    #[error("Missing S3 ARN account ID")]
    NoAccountId,
    #[error("Missing S3 ARN resource")]
    NoResource,
    #[error("Unexpected extra data in S3 ARN: {0:?}")]
    Remainder(Vec<String>),
    #[error("Missing S3 ARN access point")]
    NoAccessPoint,
    #[error("Invalid S3 ARN access point: {0}")]
    BadAccessPoint(String),
    #[error("Invalid S3 ARN bucket name: {0}")]
    BadBucketName(String),
}

impl FromStr for S3Arn {
    type Err = S3ArnError;

    fn from_str(arn: &str) -> Result<Self, Self::Err> {
        let mut parts = arn.splitn(6, ':');
        let arn_part = parts.next().ok_or(S3ArnError::NoPrefix)?;
        if arn_part != ARN_PREFIX {
            return Err(S3ArnError::BadPrefix(arn_part.to_owned()));
        }
        let partition = parts.next().ok_or(S3ArnError::NoPartition)?.to_owned();
        let service = parts.next().ok_or(S3ArnError::NoService)?.to_owned();
        if service != S3_SERVICE {
            return Err(S3ArnError::BadService(service));
        }
        let region = parts.next().ok_or(S3ArnError::NoRegion)?.to_owned();
        let account_id = parts.next().ok_or(S3ArnError::NoAccountId)?.to_owned();
        let resource = parts.next().ok_or(S3ArnError::NoResource)?.to_owned();

        let remainder = parts.map(ToOwned::to_owned).collect::<Vec<_>>();
        if !remainder.is_empty() {
            return Err(S3ArnError::Remainder(remainder));
        }

        let (accesspoint, resource_path) =
            resource.split_once('/').ok_or(S3ArnError::NoAccessPoint)?;
        if accesspoint != ACCESSPOINT {
            return Err(S3ArnError::BadAccessPoint(accesspoint.to_owned()));
        }

        let (bucket_name, bucket_path) =
            if let Some((bucket_name, bucket_path)) = resource_path.split_once('/') {
                (bucket_name.to_owned(), Some(bucket_path.to_owned()))
            } else {
                (resource_path.to_owned(), None)
            };

        if bucket_name.is_empty() {
            return Err(S3ArnError::BadBucketName(bucket_name));
        }

        Ok(Self {
            partition,
            region,
            account_id,
            bucket_name,
            bucket_path,
        })
    }
}

impl S3Arn {
    fn bucket_arn(&self) -> String {
        format!(
            "{ARN_PREFIX}:{partition}:{S3_SERVICE}:{region}:{account_id}:{ACCESSPOINT}/{bucket_name}",
            partition = self.partition,
            region = self.region,
            account_id = self.account_id,
            bucket_name = self.bucket_name
        )
    }
}

// ==================== Stale Upload Cleanup ====================

/// Cleans up all stale uploads in S3 that have exceeded the timeout.
///
/// This is a standalone async function that can be spawned as a background task.
#[expect(
    clippy::too_many_lines,
    clippy::cognitive_complexity,
    reason = "Stale upload cleanup logic is self-contained and benefits from being in one place"
)]
async fn cleanup_stale_uploads_s3(
    log: &Logger,
    client: Client,
    config: OciStorageConfig,
    upload_timeout: u64,
    clock: Clock,
) {
    let global_prefix = match &config.prefix {
        Some(prefix) => format!("{prefix}/_uploads"),
        None => "_uploads".to_owned(),
    };

    // List upload directories (with pagination)
    let prefix = format!("{global_prefix}/");
    let mut all_prefixes = Vec::new();
    let mut continuation_token: Option<String> = None;

    loop {
        let mut request = client
            .list_objects_v2()
            .bucket(&config.bucket_arn)
            .prefix(&prefix)
            .delimiter("/");

        if let Some(token) = continuation_token.take() {
            request = request.continuation_token(token);
        }

        let Ok(response) = request.send().await else {
            slog::warn!(
                log,
                "S3 stale upload cleanup: failed to list upload prefixes"
            );
            report_cleanup_error(
                log,
                "stale_upload: list prefixes",
                &"S3 list request failed",
            );
            return;
        };

        if let Some(prefixes) = response.common_prefixes {
            all_prefixes.extend(prefixes);
        }

        if response.is_truncated == Some(true) {
            continuation_token = response.next_continuation_token;
        } else {
            break;
        }
    }

    let (now, os_now) = clock.timestamps();
    let timeout_secs = i64::try_from(upload_timeout).unwrap_or(i64::MAX);

    for prefix in all_prefixes {
        let Some(prefix_str) = prefix.prefix else {
            continue;
        };

        // Extract upload ID from prefix (format: "_uploads/{upload_id}/")
        let upload_id_str = prefix_str
            .trim_start_matches(&format!("{global_prefix}/"))
            .trim_end_matches('/');

        let Ok(upload_id) = upload_id_str.parse::<UploadId>() else {
            slog::warn!(log, "S3 stale upload cleanup: failed to parse upload ID"; "upload_id" => upload_id_str);
            report_cleanup_error(
                log,
                "stale_upload: parse upload ID",
                &format!("Invalid upload ID: {upload_id_str}"),
            );
            continue;
        };

        // Load the state to check creation time.
        // If the state file is missing, unreadable, or unparseable, fall back
        // to the prefix's newest object timestamp to decide staleness.  This
        // avoids a race where `start_upload` has created the S3 multipart
        // upload but has not yet written state.json.
        let state_key = format!("{global_prefix}/{upload_id}/state.json");
        let state: Option<UploadState> = match client
            .get_object()
            .bucket(&config.bucket_arn)
            .key(&state_key)
            .send()
            .await
        {
            Ok(response) => match response.body.collect().await {
                Ok(data) => serde_json::from_slice::<UploadState>(&data.into_bytes()).ok(),
                Err(_) => None,
            },
            Err(_) => None,
        };

        // `now` (from Clock) is used for `state.created_at` because both are
        // in the application time domain.
        // `os_now` is used for S3 `LastModified` because both are in the OS
        // wall-clock time domain.
        let is_stale = match &state {
            Some(s) => now.saturating_sub(s.created_at) > timeout_secs,
            None => {
                // No valid state — check the newest object in the prefix to
                // avoid deleting a freshly-created upload whose state.json
                // has not been written yet.
                prefix_is_stale(
                    &client,
                    &config,
                    &format!("{global_prefix}/{upload_id}/"),
                    os_now,
                    timeout_secs,
                )
                .await
            },
        };

        if is_stale {
            // Abort the S3 multipart upload if we have the upload ID
            let data_key = format!("{global_prefix}/{upload_id}/data");
            if let Some(state) = &state
                && let Err(e) = client
                    .abort_multipart_upload()
                    .bucket(&config.bucket_arn)
                    .key(&data_key)
                    .upload_id(&state.s3_upload_id)
                    .send()
                    .await
            {
                report_cleanup_error(log, "stale_upload: abort multipart", &e);
            }

            // Delete all buffer chunks first (while state still exists for discovery)
            let chunks_prefix = format!("{global_prefix}/{upload_id}/chunks/");
            let mut continuation_token: Option<String> = None;
            loop {
                let mut request = client
                    .list_objects_v2()
                    .bucket(&config.bucket_arn)
                    .prefix(&chunks_prefix);

                if let Some(token) = continuation_token.take() {
                    request = request.continuation_token(token);
                }

                let Ok(response) = request.send().await else {
                    break;
                };

                if let Some(contents) = response.contents {
                    for object in contents {
                        if let Some(chunk_key) = object.key
                            && let Err(e) = client
                                .delete_object()
                                .bucket(&config.bucket_arn)
                                .key(&chunk_key)
                                .send()
                                .await
                        {
                            report_cleanup_error(log, "stale_upload: chunk delete", &e);
                        }
                    }
                }

                if response.is_truncated == Some(true) {
                    continuation_token = response.next_continuation_token;
                } else {
                    break;
                }
            }

            // Delete data file
            if let Err(e) = client
                .delete_object()
                .bucket(&config.bucket_arn)
                .key(&data_key)
                .send()
                .await
            {
                report_cleanup_error(log, "stale_upload: data delete", &e);
            }

            // Delete state last (so discovery still works if crash occurs above)
            if let Err(e) = client
                .delete_object()
                .bucket(&config.bucket_arn)
                .key(&state_key)
                .send()
                .await
            {
                report_cleanup_error(log, "stale_upload: state delete", &e);
            }
        }
    }
}

/// Check whether an S3 prefix with no valid state.json is stale by inspecting
/// the `LastModified` timestamp of its newest object.
///
/// Returns `false` (not stale) if we can't determine the age, to avoid
/// accidentally deleting an in-progress upload whose state hasn't been written yet.
async fn prefix_is_stale(
    client: &Client,
    config: &OciStorageConfig,
    prefix: &str,
    now: i64,
    timeout_secs: i64,
) -> bool {
    let mut newest: i64 = 0;
    let mut found_any = false;
    let mut continuation_token: Option<String> = None;

    loop {
        let mut request = client
            .list_objects_v2()
            .bucket(&config.bucket_arn)
            .prefix(prefix);

        if let Some(token) = continuation_token.take() {
            request = request.continuation_token(token);
        }

        let Ok(response) = request.send().await else {
            return false;
        };

        if let Some(contents) = &response.contents {
            for obj in contents {
                found_any = true;
                if let Some(lm) = obj.last_modified {
                    newest = newest.max(lm.secs());
                }
            }
        }

        if response.is_truncated == Some(true) {
            continuation_token = response.next_continuation_token;
        } else {
            break;
        }
    }

    if !found_any {
        // No objects at all under this prefix — truly empty, safe to consider stale
        return true;
    }

    now.saturating_sub(newest) > timeout_secs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn s3_arn_valid_without_path() {
        let arn: S3Arn = "arn:aws:s3:us-east-1:123456789012:accesspoint/my-bucket"
            .parse()
            .unwrap();
        assert_eq!(arn.partition, "aws");
        assert_eq!(arn.region, "us-east-1");
        assert_eq!(arn.account_id, "123456789012");
        assert_eq!(arn.bucket_name, "my-bucket");
        assert!(arn.bucket_path.is_none());
    }

    #[test]
    fn s3_arn_valid_with_path() {
        let arn: S3Arn = "arn:aws:s3:eu-west-1:987654321098:accesspoint/my-bucket/some/prefix"
            .parse()
            .unwrap();
        assert_eq!(arn.partition, "aws");
        assert_eq!(arn.region, "eu-west-1");
        assert_eq!(arn.account_id, "987654321098");
        assert_eq!(arn.bucket_name, "my-bucket");
        assert_eq!(arn.bucket_path.as_deref(), Some("some/prefix"));
    }

    #[test]
    fn s3_arn_bucket_arn_round_trip() {
        let arn: S3Arn = "arn:aws:s3:us-west-2:111111111111:accesspoint/test-bucket"
            .parse()
            .unwrap();
        assert_eq!(
            arn.bucket_arn(),
            "arn:aws:s3:us-west-2:111111111111:accesspoint/test-bucket"
        );
    }

    #[test]
    fn s3_arn_bucket_arn_strips_path() {
        let arn: S3Arn = "arn:aws:s3:us-west-2:111111111111:accesspoint/test-bucket/path"
            .parse()
            .unwrap();
        assert_eq!(
            arn.bucket_arn(),
            "arn:aws:s3:us-west-2:111111111111:accesspoint/test-bucket"
        );
    }

    #[test]
    fn s3_arn_missing_prefix() {
        let result = "not-arn:aws:s3:us-east-1:123:accesspoint/b".parse::<S3Arn>();
        assert!(matches!(result, Err(S3ArnError::BadPrefix(_))));
    }

    #[test]
    fn s3_arn_bad_service() {
        let result = "arn:aws:ec2:us-east-1:123:accesspoint/b".parse::<S3Arn>();
        assert!(matches!(result, Err(S3ArnError::BadService(_))));
    }

    #[test]
    fn s3_arn_missing_accesspoint() {
        let result = "arn:aws:s3:us-east-1:123:bucket-only".parse::<S3Arn>();
        assert!(matches!(result, Err(S3ArnError::NoAccessPoint)));
    }

    #[test]
    fn s3_arn_empty_bucket_name() {
        let result = "arn:aws:s3:us-east-1:123:accesspoint/".parse::<S3Arn>();
        assert!(matches!(result, Err(S3ArnError::BadBucketName(_))));
    }

    #[test]
    fn s3_arn_missing_resource() {
        let result = "arn:aws:s3:us-east-1:123".parse::<S3Arn>();
        assert!(matches!(result, Err(S3ArnError::NoResource)));
    }

    #[test]
    fn s3_arn_gov_cloud_partition() {
        let arn: S3Arn = "arn:aws-us-gov:s3:us-gov-west-1:123:accesspoint/gov-bucket"
            .parse()
            .unwrap();
        assert_eq!(arn.partition, "aws-us-gov");
        assert_eq!(arn.region, "us-gov-west-1");
        assert_eq!(arn.bucket_name, "gov-bucket");
    }
}
