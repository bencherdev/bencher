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
use std::task::{Context, Poll};

use aws_sdk_s3::Client;
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::types::CompletedMultipartUpload;
use bencher_json::{ProjectResourceId, Secret, system::config::OciDataStore};
use bytes::Bytes;
use futures::stream::{self, StreamExt as _};
use hyper::body::Frame;
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use thiserror::Error;

use crate::local::{LocalBlobBody, OciLocalStorage};
use crate::types::{Digest, UploadId};

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
            }
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

    #[error("Invalid S3 ARN: {0}")]
    InvalidArn(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("JSON serialization error: {0}")]
    Json(String),
}

impl OciStorageError {
    /// Returns the appropriate HTTP status code for this storage error
    pub fn status_code(&self) -> http::StatusCode {
        match self {
            Self::UploadNotFound(_) | Self::BlobNotFound(_) | Self::ManifestNotFound(_) => {
                http::StatusCode::NOT_FOUND
            },
            Self::DigestMismatch { .. } | Self::InvalidContent(_) => http::StatusCode::BAD_REQUEST,
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
/// ## Buffer Chunk Storage (O(1) appends)
///
/// Instead of storing a single buffer that grows with each append (O(n²) I/O),
/// we store each incoming chunk as a separate S3 object. At completion, we
/// stream through all chunks to compute the hash and upload multipart parts.
///
/// This reduces append operations from O(n²) to O(n) total I/O.
#[derive(Debug, Serialize, Deserialize)]
struct UploadState {
    /// S3 multipart upload ID
    s3_upload_id: String,
    /// Repository name
    repository: String,
    /// Completed parts with their `ETag`s
    parts: Vec<CompletedPartInfo>,
    /// Number of buffer chunks stored (each chunk is a separate S3 object)
    buffer_chunk_count: u32,
    /// Total bytes across all buffer chunks
    buffer_total_size: u64,
}

/// Information about a completed S3 multipart upload part
#[derive(Debug, Serialize, Deserialize)]
struct CompletedPartInfo {
    part_number: i32,
    etag: String,
    size: u64,
}

impl UploadState {
    /// Total bytes in completed parts
    fn completed_parts_size(&self) -> u64 {
        self.parts.iter().map(|p| p.size).sum()
    }

    /// Total bytes uploaded (completed parts + buffer chunks)
    fn total_size(&self) -> u64 {
        self.completed_parts_size() + self.buffer_total_size
    }

    /// Next part number to use
    fn next_part_number(&self) -> i32 {
        // S3 part numbers are 1-indexed and max 10,000 parts
        // Safe to cast since we won't exceed 10,000 parts
        i32::try_from(self.parts.len()).unwrap_or(i32::MAX - 1) + 1
    }
}

/// OCI Storage implementation using S3 with multipart uploads
pub(crate) struct OciS3Storage {
    client: Client,
    config: OciStorageConfig,
}

/// OCI Storage backend - supports S3 or local filesystem
#[expect(
    private_interfaces,
    reason = "Users interact via methods, not variants"
)]
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
    pub fn try_from_config(
        data_store: Option<OciDataStore>,
        database_path: &Path,
    ) -> Result<Self, OciStorageError> {
        match data_store {
            Some(OciDataStore::AwsS3 {
                access_key_id,
                secret_access_key,
                access_point,
            }) => OciS3Storage::new(access_key_id, secret_access_key, &access_point)
                .map(OciStorage::S3),
            None => Ok(OciStorage::Local(OciLocalStorage::new(database_path))),
        }
    }

    // ==================== Upload Operations ====================

    /// Starts a new upload session
    pub async fn start_upload(
        &self,
        repository: &ProjectResourceId,
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
        repository: &ProjectResourceId,
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
        repository: &ProjectResourceId,
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
        repository: &ProjectResourceId,
        digest: &Digest,
    ) -> Result<(BlobBody, u64), OciStorageError> {
        match self {
            Self::S3(s3) => {
                let (body, size) = s3.get_blob_stream(repository, digest).await?;
                Ok((BlobBody::S3(body), size))
            }
            Self::Local(local) => {
                let (body, size) = local.get_blob_stream(repository, digest).await?;
                Ok((BlobBody::Local(body), size))
            }
        }
    }

    /// Gets blob metadata (size) without downloading content
    pub async fn get_blob_size(
        &self,
        repository: &ProjectResourceId,
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
        repository: &ProjectResourceId,
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
        from_repository: &ProjectResourceId,
        to_repository: &ProjectResourceId,
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
        repository: &ProjectResourceId,
        content: Bytes,
        tag: Option<&str>,
    ) -> Result<Digest, OciStorageError> {
        match self {
            Self::S3(s3) => s3.put_manifest(repository, content, tag).await,
            Self::Local(local) => local.put_manifest(repository, content, tag).await,
        }
    }

    /// Gets a manifest by digest
    pub async fn get_manifest_by_digest(
        &self,
        repository: &ProjectResourceId,
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
        repository: &ProjectResourceId,
        tag: &str,
    ) -> Result<Digest, OciStorageError> {
        match self {
            Self::S3(s3) => s3.resolve_tag(repository, tag).await,
            Self::Local(local) => local.resolve_tag(repository, tag).await,
        }
    }

    /// Lists all tags for a repository
    pub async fn list_tags(
        &self,
        repository: &ProjectResourceId,
    ) -> Result<Vec<String>, OciStorageError> {
        match self {
            Self::S3(s3) => s3.list_tags(repository).await,
            Self::Local(local) => local.list_tags(repository).await,
        }
    }

    /// Deletes a manifest by digest
    pub async fn delete_manifest(
        &self,
        repository: &ProjectResourceId,
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
        repository: &ProjectResourceId,
        tag: &str,
    ) -> Result<(), OciStorageError> {
        match self {
            Self::S3(s3) => s3.delete_tag(repository, tag).await,
            Self::Local(local) => local.delete_tag(repository, tag).await,
        }
    }

    /// Lists all manifests that reference a given digest via their subject field
    pub async fn list_referrers(
        &self,
        repository: &ProjectResourceId,
        subject_digest: &Digest,
        artifact_type_filter: Option<&str>,
    ) -> Result<Vec<serde_json::Value>, OciStorageError> {
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

impl OciS3Storage {
    /// Creates a new S3 storage instance
    fn new(
        access_key_id: String,
        secret_access_key: Secret,
        access_point: &str,
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

        Ok(Self { client, config })
    }

    // ==================== Key Generation ====================

    /// Returns the S3 key prefix for the given repository
    fn key_prefix(&self, repository: &ProjectResourceId) -> String {
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

    /// Returns the S3 key for a buffer chunk
    ///
    /// Buffer chunks are stored separately to avoid O(n²) read-modify-write
    /// operations. Each append creates a new chunk object.
    fn upload_buffer_chunk_key(&self, upload_id: &UploadId, chunk_num: u32) -> String {
        format!(
            "{}/{}/chunks/{:08}",
            self.global_prefix(),
            upload_id,
            chunk_num
        )
    }

    /// Returns the S3 key for the temporary upload data (multipart destination)
    fn upload_data_key(&self, upload_id: &UploadId) -> String {
        format!("{}/{}/data", self.global_prefix(), upload_id)
    }

    /// Returns the S3 key for a blob
    fn blob_key(&self, repository: &ProjectResourceId, digest: &Digest) -> String {
        format!(
            "{}/blobs/{}/{}",
            self.key_prefix(repository),
            digest.algorithm(),
            digest.hex_hash()
        )
    }

    /// Returns the S3 key for a manifest by digest
    fn manifest_key_by_digest(&self, repository: &ProjectResourceId, digest: &Digest) -> String {
        format!(
            "{}/manifests/sha256/{}",
            self.key_prefix(repository),
            digest.hex_hash()
        )
    }

    /// Returns the S3 key for a manifest tag link
    fn tag_link_key(&self, repository: &ProjectResourceId, tag: &str) -> String {
        format!("{}/tags/{}", self.key_prefix(repository), tag)
    }

    /// Returns the S3 key prefix for referrers to a given digest
    fn referrers_prefix(&self, repository: &ProjectResourceId, subject_digest: &Digest) -> String {
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
        repository: &ProjectResourceId,
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
                if e.raw_response().is_some_and(|r| r.status().as_u16() == 404) {
                    OciStorageError::UploadNotFound(upload_id.to_string())
                } else {
                    OciStorageError::S3(e.to_string())
                }
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

    /// Stores a buffer chunk to S3
    ///
    /// This is an O(1) operation - no read-modify-write cycle.
    async fn store_buffer_chunk(
        &self,
        upload_id: &UploadId,
        chunk_num: u32,
        data: Bytes,
    ) -> Result<(), OciStorageError> {
        let key = self.upload_buffer_chunk_key(upload_id, chunk_num);
        self.client
            .put_object()
            .bucket(&self.config.bucket_arn)
            .key(&key)
            .body(data.into())
            .send()
            .await
            .map_err(|e| OciStorageError::S3(e.to_string()))?;
        Ok(())
    }

    /// Loads a buffer chunk from S3
    async fn load_buffer_chunk(
        &self,
        upload_id: &UploadId,
        chunk_num: u32,
    ) -> Result<Bytes, OciStorageError> {
        let key = self.upload_buffer_chunk_key(upload_id, chunk_num);
        let response = self
            .client
            .get_object()
            .bucket(&self.config.bucket_arn)
            .key(&key)
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

    /// Deletes all upload-related objects from S3
    async fn cleanup_upload(&self, upload_id: &UploadId, chunk_count: u32) {
        // Best effort cleanup - ignore errors
        let state_key = self.upload_state_key(upload_id);
        let data_key = self.upload_data_key(upload_id);

        // Delete state
        let _unused = self
            .client
            .delete_object()
            .bucket(&self.config.bucket_arn)
            .key(&state_key)
            .send()
            .await;

        // Delete multipart data object
        let _unused = self
            .client
            .delete_object()
            .bucket(&self.config.bucket_arn)
            .key(&data_key)
            .send()
            .await;

        // Delete all buffer chunks
        for i in 0..chunk_count {
            let chunk_key = self.upload_buffer_chunk_key(upload_id, i);
            let _unused = self
                .client
                .delete_object()
                .bucket(&self.config.bucket_arn)
                .key(&chunk_key)
                .send()
                .await;
        }
    }

    // ==================== Upload Operations ====================

    /// Starts a new upload session using S3 multipart upload
    pub async fn start_upload(
        &self,
        repository: &ProjectResourceId,
    ) -> Result<UploadId, OciStorageError> {
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

        // Save initial state
        let state = UploadState {
            s3_upload_id,
            repository: repository.to_string(),
            parts: Vec::new(),
            buffer_chunk_count: 0,
            buffer_total_size: 0,
        };
        self.save_upload_state(&upload_id, &state).await?;

        Ok(upload_id)
    }

    /// Appends data to an in-progress upload
    ///
    /// Each append is stored as a separate chunk in S3 (O(1) operation).
    /// Chunks are combined and hashed at completion time.
    pub async fn append_upload(
        &self,
        upload_id: &UploadId,
        data: Bytes,
    ) -> Result<u64, OciStorageError> {
        // Load current state
        let mut state = self.load_upload_state(upload_id).await?;

        // Store new data as a separate chunk (O(1) - no read-modify-write)
        let chunk_num = state.buffer_chunk_count;
        let data_len = data.len() as u64;
        self.store_buffer_chunk(upload_id, chunk_num, data).await?;

        // Update state
        state.buffer_chunk_count += 1;
        state.buffer_total_size += data_len;

        // Save updated state
        self.save_upload_state(upload_id, &state).await?;

        Ok(state.total_size())
    }

    /// Gets the current size of an in-progress upload
    pub async fn get_upload_size(&self, upload_id: &UploadId) -> Result<u64, OciStorageError> {
        let state = self.load_upload_state(upload_id).await?;
        Ok(state.total_size())
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
        let chunk_count = state.buffer_chunk_count;
        let data_key = self.upload_data_key(upload_id);

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
        if chunk_count == 0 && state.parts.is_empty() {
            self.cleanup_upload(upload_id, 0).await;
            return Err(OciStorageError::InvalidContent(
                "Cannot complete upload with no data".to_owned(),
            ));
        }

        // Stream through chunks with incremental hashing
        let mut hasher = Sha256::new();
        let mut part_buffer = Vec::new();

        for chunk_num in 0..chunk_count {
            // Load chunk
            let chunk = self.load_buffer_chunk(upload_id, chunk_num).await?;

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
            self.cleanup_upload(upload_id, chunk_count).await;
            return Err(OciStorageError::InvalidContent(
                "Cannot complete upload with no data".to_owned(),
            ));
        }

        // Compute actual digest from incremental hash
        let hash = hasher.finalize();
        let actual_digest = Digest::sha256(&hex::encode(hash));

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
            self.cleanup_upload(upload_id, chunk_count).await;
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

        // Parse repository name
        let repository: ProjectResourceId =
            state
                .repository
                .parse()
                .map_err(|e: bencher_json::ValidError| {
                    OciStorageError::InvalidContent(e.to_string())
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
        self.cleanup_upload(upload_id, chunk_count).await;

        Ok(actual_digest)
    }

    /// Uploads a single part to S3 multipart upload
    async fn upload_multipart_part(
        &self,
        state: &mut UploadState,
        data_key: &str,
        data: Vec<u8>,
    ) -> Result<(), OciStorageError> {
        let part_number = state.next_part_number();
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
        // Load state to get S3 upload ID and chunk count
        let state = self.load_upload_state(upload_id).await?;
        let data_key = self.upload_data_key(upload_id);

        // Abort the S3 multipart upload
        let _unused = self
            .client
            .abort_multipart_upload()
            .bucket(&self.config.bucket_arn)
            .key(&data_key)
            .upload_id(&state.s3_upload_id)
            .send()
            .await;

        // Clean up (including all buffer chunks)
        self.cleanup_upload(upload_id, state.buffer_chunk_count)
            .await;

        Ok(())
    }

    // ==================== Blob Operations ====================

    /// Checks if a blob exists
    pub async fn blob_exists(
        &self,
        repository: &ProjectResourceId,
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
                if e.raw_response().is_some_and(|r| r.status().as_u16() == 404) {
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
        repository: &ProjectResourceId,
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
                if e.raw_response().is_some_and(|r| r.status().as_u16() == 404) {
                    OciStorageError::BlobNotFound(digest.to_string())
                } else {
                    OciStorageError::S3(e.to_string())
                }
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
        repository: &ProjectResourceId,
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
                if e.raw_response().is_some_and(|r| r.status().as_u16() == 404) {
                    OciStorageError::BlobNotFound(digest.to_string())
                } else {
                    OciStorageError::S3(e.to_string())
                }
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
        repository: &ProjectResourceId,
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
                if e.raw_response().is_some_and(|r| r.status().as_u16() == 404) {
                    OciStorageError::BlobNotFound(digest.to_string())
                } else {
                    OciStorageError::S3(e.to_string())
                }
            })?;

        Ok(response
            .content_length()
            .map_or(0, |len| u64::try_from(len).unwrap_or(0)))
    }

    /// Deletes a blob
    pub async fn delete_blob(
        &self,
        repository: &ProjectResourceId,
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
    pub async fn mount_blob(
        &self,
        from_repository: &ProjectResourceId,
        to_repository: &ProjectResourceId,
        digest: &Digest,
    ) -> Result<bool, OciStorageError> {
        // Check if blob exists in source
        if !self.blob_exists(from_repository, digest).await? {
            return Ok(false);
        }

        // Copy the blob to the new repository
        // For S3 Access Points, copy source must use the format:
        // arn:aws:s3:region:account-id:accesspoint/accesspoint-name/object/key
        let source_key = self.blob_key(from_repository, digest);
        let dest_key = self.blob_key(to_repository, digest);

        self.client
            .copy_object()
            .bucket(&self.config.bucket_arn)
            .copy_source(format!("{}/object/{}", self.config.bucket_arn, source_key))
            .key(&dest_key)
            .send()
            .await
            .map_err(|e| OciStorageError::S3(e.to_string()))?;

        Ok(true)
    }

    // ==================== Manifest Operations ====================

    /// Stores a manifest
    pub async fn put_manifest(
        &self,
        repository: &ProjectResourceId,
        content: Bytes,
        tag: Option<&str>,
    ) -> Result<Digest, OciStorageError> {
        // Compute digest
        let mut hasher = Sha256::new();
        hasher.update(&content);
        let hash = hasher.finalize();
        let digest = Digest::sha256(&hex::encode(hash));

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
            let tag_key = self.tag_link_key(repository, tag);
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
        if let Ok(manifest) = serde_json::from_slice::<serde_json::Value>(&content)
            && let Some(subject) = manifest.get("subject")
            && let Some(subject_digest_str) = subject.get("digest").and_then(|d| d.as_str())
            && let Ok(subject_digest) = subject_digest_str.parse::<Digest>()
        {
            // Extract descriptor info for the referrer
            let media_type = manifest
                .get("mediaType")
                .and_then(|m| m.as_str())
                .unwrap_or("application/vnd.oci.image.manifest.v1+json");
            let artifact_type = manifest
                .get("artifactType")
                .and_then(|a| a.as_str())
                .or_else(|| {
                    manifest
                        .get("config")
                        .and_then(|c| c.get("mediaType"))
                        .and_then(|m| m.as_str())
                });

            // Create referrer descriptor
            let mut descriptor = serde_json::json!({
                "mediaType": media_type,
                "digest": digest.to_string(),
                "size": content.len()
            });
            if let Some(at) = artifact_type
                && let Some(obj) = descriptor.as_object_mut()
            {
                obj.insert(
                    "artifactType".to_owned(),
                    serde_json::Value::String(at.to_owned()),
                );
            }
            if let Some(annotations) = manifest.get("annotations")
                && let Some(obj) = descriptor.as_object_mut()
            {
                obj.insert("annotations".to_owned(), annotations.clone());
            }

            // Store referrer link
            let referrer_key = self.referrer_key(repository, &subject_digest, &digest);
            self.client
                .put_object()
                .bucket(&self.config.bucket_arn)
                .key(&referrer_key)
                .body(serde_json::to_vec(&descriptor).unwrap_or_default().into())
                .send()
                .await
                .map_err(|e| OciStorageError::S3(e.to_string()))?;
        }

        Ok(digest)
    }

    /// Gets a manifest by digest
    pub async fn get_manifest_by_digest(
        &self,
        repository: &ProjectResourceId,
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
                if e.raw_response().is_some_and(|r| r.status().as_u16() == 404) {
                    OciStorageError::ManifestNotFound(digest.to_string())
                } else {
                    OciStorageError::S3(e.to_string())
                }
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
        repository: &ProjectResourceId,
        tag: &str,
    ) -> Result<Digest, OciStorageError> {
        let key = self.tag_link_key(repository, tag);
        let response = self
            .client
            .get_object()
            .bucket(&self.config.bucket_arn)
            .key(&key)
            .send()
            .await
            .map_err(|e| {
                if e.raw_response().is_some_and(|r| r.status().as_u16() == 404) {
                    OciStorageError::ManifestNotFound(tag.to_owned())
                } else {
                    OciStorageError::S3(e.to_string())
                }
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

    /// Lists all tags for a repository
    pub async fn list_tags(
        &self,
        repository: &ProjectResourceId,
    ) -> Result<Vec<String>, OciStorageError> {
        let prefix = format!("{}/tags/", self.key_prefix(repository));

        let mut tags = Vec::new();
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
                    if let Some(key) = object.key
                        && let Some(tag) = key.strip_prefix(&prefix)
                    {
                        tags.push(tag.to_owned());
                    }
                }
            }

            if response.is_truncated == Some(true) {
                continuation_token = response.next_continuation_token;
            } else {
                break;
            }
        }

        Ok(tags)
    }

    /// Deletes a manifest by digest
    ///
    /// Also cleans up any referrer link if this manifest references another manifest
    /// via the `subject` field.
    pub async fn delete_manifest(
        &self,
        repository: &ProjectResourceId,
        digest: &Digest,
    ) -> Result<(), OciStorageError> {
        // Try to read the manifest first to check for subject field
        // If we can read it and it has a subject, clean up the referrer link
        if let Ok(data) = self.get_manifest_by_digest(repository, digest).await
            && let Ok(manifest) = serde_json::from_slice::<serde_json::Value>(&data)
            && let Some(subject) = manifest.get("subject")
            && let Some(subject_digest_str) = subject.get("digest").and_then(|d| d.as_str())
            && let Ok(subject_digest) = subject_digest_str.parse::<Digest>()
        {
            // Delete the referrer link
            let referrer_key = self.referrer_key(repository, &subject_digest, digest);
            // Ignore errors - the referrer link may not exist or may have already been deleted
            drop(
                self.client
                    .delete_object()
                    .bucket(&self.config.bucket_arn)
                    .key(&referrer_key)
                    .send()
                    .await,
            );
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
        repository: &ProjectResourceId,
        tag: &str,
    ) -> Result<(), OciStorageError> {
        let key = self.tag_link_key(repository, tag);
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
        repository: &ProjectResourceId,
        subject_digest: &Digest,
        artifact_type_filter: Option<&str>,
    ) -> Result<Vec<serde_json::Value>, OciStorageError> {
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

        // Fetch referrer descriptors in parallel (up to 10 concurrent)
        let filter = artifact_type_filter.map(ToOwned::to_owned);
        let referrers: Vec<serde_json::Value> = stream::iter(keys)
            .map(|key| {
                let client = &self.client;
                let bucket = &self.config.bucket_arn;
                let filter = filter.clone();
                async move {
                    // Get the referrer descriptor
                    let Ok(resp) = client.get_object().bucket(bucket).key(&key).send().await else {
                        return None;
                    };
                    let Ok(data) = resp.body.collect().await else {
                        return None;
                    };
                    let Ok(descriptor) =
                        serde_json::from_slice::<serde_json::Value>(&data.into_bytes())
                    else {
                        return None;
                    };
                    // Apply artifact type filter if specified
                    if let Some(filter) = &filter {
                        let matches = descriptor
                            .get("artifactType")
                            .and_then(|a| a.as_str())
                            .is_some_and(|at| at == filter);
                        if !matches {
                            return None;
                        }
                    }
                    Some(descriptor)
                }
            })
            .buffer_unordered(10)
            .filter_map(|x| async move { x })
            .collect()
            .await;

        Ok(referrers)
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
