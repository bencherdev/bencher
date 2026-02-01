//! OCI Storage Layer - S3 Backend with Multipart Upload Support
//!
//! This implementation uses S3 multipart uploads for scalability:
//! - Upload state is stored in S3 for cross-instance consistency
//! - Chunks are buffered in S3 until they reach the 5MB minimum part size
//! - No in-memory state means horizontal scaling and restart resilience

use std::str::FromStr;

use aws_sdk_s3::Client;
use aws_sdk_s3::types::CompletedMultipartUpload;
use bencher_json::{system::config::OciDataStore, Secret};
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use thiserror::Error;

use crate::types::{Digest, RepositoryName, UploadId};

/// Minimum part size for S3 multipart upload (5MB)
/// S3 requires all parts except the last to be at least 5MB
const MIN_PART_SIZE: usize = 5 * 1024 * 1024;

/// Storage errors
#[derive(Debug, Error)]
pub enum OciStorageError {
    #[error("S3 error: {0}")]
    S3(String),

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
            }
            Self::DigestMismatch { .. } | Self::InvalidContent(_) => http::StatusCode::BAD_REQUEST,
            Self::S3(_) | Self::InvalidArn(_) | Self::Config(_) | Self::Json(_) => {
                http::StatusCode::INTERNAL_SERVER_ERROR
            }
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
#[derive(Debug, Serialize, Deserialize)]
struct UploadState {
    /// S3 multipart upload ID
    s3_upload_id: String,
    /// Repository name
    repository: String,
    /// Completed parts with their `ETag`s
    parts: Vec<CompletedPartInfo>,
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
    fn completed_size(&self) -> u64 {
        self.parts.iter().map(|p| p.size).sum()
    }

    /// Next part number to use
    fn next_part_number(&self) -> i32 {
        // S3 part numbers are 1-indexed and max 10,000 parts
        // Safe to cast since we won't exceed 10,000 parts
        i32::try_from(self.parts.len()).unwrap_or(i32::MAX - 1) + 1
    }
}

/// OCI Storage implementation using S3 with multipart uploads
pub struct OciStorage {
    client: Client,
    config: OciStorageConfig,
}

impl OciStorage {
    /// Creates a new OCI storage instance from configuration
    pub fn try_from_config(data_store: OciDataStore) -> Result<Self, OciStorageError> {
        match data_store {
            OciDataStore::AwsS3 {
                access_key_id,
                secret_access_key,
                access_point,
            } => Self::new_s3(access_key_id, secret_access_key, &access_point),
        }
    }

    /// Creates a new OCI storage instance with S3 backend
    fn new_s3(
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

    /// Creates a new OCI storage instance (for testing or direct use)
    pub fn new(client: Client, config: OciStorageConfig) -> Self {
        Self { client, config }
    }

    // ==================== Key Generation ====================

    /// Returns the S3 key prefix for the given repository
    fn key_prefix(&self, repository: &RepositoryName) -> String {
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

    /// Returns the S3 key for upload buffer (chunks < 5MB)
    fn upload_buffer_key(&self, upload_id: &UploadId) -> String {
        format!("{}/{}/buffer", self.global_prefix(), upload_id)
    }

    /// Returns the S3 key for the temporary upload data (multipart destination)
    fn upload_data_key(&self, upload_id: &UploadId) -> String {
        format!("{}/{}/data", self.global_prefix(), upload_id)
    }

    /// Returns the S3 key for a blob
    fn blob_key(&self, repository: &RepositoryName, digest: &Digest) -> String {
        format!(
            "{}/blobs/{}/{}",
            self.key_prefix(repository),
            digest.algorithm(),
            digest.hex_hash()
        )
    }

    /// Returns the S3 key for a manifest by digest
    fn manifest_key_by_digest(&self, repository: &RepositoryName, digest: &Digest) -> String {
        format!(
            "{}/manifests/sha256/{}",
            self.key_prefix(repository),
            digest.hex_hash()
        )
    }

    /// Returns the S3 key for a manifest tag link
    fn tag_link_key(&self, repository: &RepositoryName, tag: &str) -> String {
        format!("{}/tags/{}", self.key_prefix(repository), tag)
    }

    /// Returns the S3 key prefix for referrers to a given digest
    fn referrers_prefix(&self, repository: &RepositoryName, subject_digest: &Digest) -> String {
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
        repository: &RepositoryName,
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
    async fn load_upload_state(&self, upload_id: &UploadId) -> Result<UploadState, OciStorageError> {
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

    /// Loads upload buffer from S3 (returns empty vec if not found)
    async fn load_upload_buffer(&self, upload_id: &UploadId) -> Result<Vec<u8>, OciStorageError> {
        let key = self.upload_buffer_key(upload_id);
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
                Ok(data.to_vec())
            }
            Err(e) => {
                if e.raw_response().is_some_and(|r| r.status().as_u16() == 404) {
                    Ok(Vec::new())
                } else {
                    Err(OciStorageError::S3(e.to_string()))
                }
            }
        }
    }

    /// Saves upload buffer to S3 (deletes if empty)
    async fn save_upload_buffer(
        &self,
        upload_id: &UploadId,
        buffer: &[u8],
    ) -> Result<(), OciStorageError> {
        let key = self.upload_buffer_key(upload_id);

        if buffer.is_empty() {
            // Delete the buffer object if empty
            let _unused = self
                .client
                .delete_object()
                .bucket(&self.config.bucket_arn)
                .key(&key)
                .send()
                .await;
            Ok(())
        } else {
            self.client
                .put_object()
                .bucket(&self.config.bucket_arn)
                .key(&key)
                .body(buffer.to_vec().into())
                .send()
                .await
                .map_err(|e| OciStorageError::S3(e.to_string()))?;
            Ok(())
        }
    }

    /// Deletes all upload-related objects from S3
    async fn cleanup_upload(&self, upload_id: &UploadId) {
        // Best effort cleanup - ignore errors
        let state_key = self.upload_state_key(upload_id);
        let buffer_key = self.upload_buffer_key(upload_id);
        let data_key = self.upload_data_key(upload_id);

        let _unused = self
            .client
            .delete_object()
            .bucket(&self.config.bucket_arn)
            .key(&state_key)
            .send()
            .await;
        let _unused = self
            .client
            .delete_object()
            .bucket(&self.config.bucket_arn)
            .key(&buffer_key)
            .send()
            .await;
        let _unused = self
            .client
            .delete_object()
            .bucket(&self.config.bucket_arn)
            .key(&data_key)
            .send()
            .await;
    }

    // ==================== Upload Operations ====================

    /// Starts a new upload session using S3 multipart upload
    pub async fn start_upload(
        &self,
        repository: &RepositoryName,
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
        };
        self.save_upload_state(&upload_id, &state).await?;

        Ok(upload_id)
    }

    /// Appends data to an in-progress upload
    ///
    /// Data is buffered in S3 until we accumulate enough for a 5MB part,
    /// then uploaded as an S3 multipart part.
    pub async fn append_upload(
        &self,
        upload_id: &UploadId,
        data: Bytes,
    ) -> Result<u64, OciStorageError> {
        // Load current state and buffer
        let mut state = self.load_upload_state(upload_id).await?;
        let mut buffer = self.load_upload_buffer(upload_id).await?;

        // Append new data to buffer
        buffer.extend_from_slice(&data);

        // Upload complete parts (5MB each)
        while buffer.len() >= MIN_PART_SIZE {
            let part_data: Vec<u8> = buffer.drain(..MIN_PART_SIZE).collect();
            self.upload_part(upload_id, &mut state, part_data).await?;
        }

        // Calculate total size
        let total_size = state.completed_size() + buffer.len() as u64;

        // Save updated state and buffer
        self.save_upload_state(upload_id, &state).await?;
        self.save_upload_buffer(upload_id, &buffer).await?;

        Ok(total_size)
    }

    /// Uploads a single part to S3 multipart upload
    async fn upload_part(
        &self,
        upload_id: &UploadId,
        state: &mut UploadState,
        data: Vec<u8>,
    ) -> Result<(), OciStorageError> {
        let data_key = self.upload_data_key(upload_id);
        let part_number = state.next_part_number();
        let part_size = data.len() as u64;

        let response = self
            .client
            .upload_part()
            .bucket(&self.config.bucket_arn)
            .key(&data_key)
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

    /// Gets the current size of an in-progress upload
    pub async fn get_upload_size(&self, upload_id: &UploadId) -> Result<u64, OciStorageError> {
        let state = self.load_upload_state(upload_id).await?;
        let buffer = self.load_upload_buffer(upload_id).await?;
        Ok(state.completed_size() + buffer.len() as u64)
    }

    /// Completes an upload and stores the blob
    ///
    /// This:
    /// 1. Uploads any remaining buffer as the final part
    /// 2. Completes the S3 multipart upload
    /// 3. Downloads and verifies the digest
    /// 4. Copies to the final blob location
    /// 5. Cleans up temporary objects
    pub async fn complete_upload(
        &self,
        upload_id: &UploadId,
        expected_digest: &Digest,
    ) -> Result<Digest, OciStorageError> {
        // Load state and buffer
        let mut state = self.load_upload_state(upload_id).await?;
        let buffer = self.load_upload_buffer(upload_id).await?;

        // Upload any remaining buffer as the final part
        if !buffer.is_empty() {
            self.upload_part(upload_id, &mut state, buffer).await?;
        }

        // Must have at least one part
        if state.parts.is_empty() {
            self.cleanup_upload(upload_id).await;
            return Err(OciStorageError::InvalidContent(
                "Cannot complete upload with no data".to_owned(),
            ));
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

        let data_key = self.upload_data_key(upload_id);

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

        // Download and verify digest
        let response = self
            .client
            .get_object()
            .bucket(&self.config.bucket_arn)
            .key(&data_key)
            .send()
            .await
            .map_err(|e| OciStorageError::S3(e.to_string()))?;

        let data = response
            .body
            .collect()
            .await
            .map_err(|e| OciStorageError::S3(e.to_string()))?
            .into_bytes();

        // Compute actual digest
        let mut hasher = Sha256::new();
        hasher.update(&data);
        let hash = hasher.finalize();
        let actual_digest = Digest::sha256(&hex::encode(hash));

        // Verify digest matches
        if actual_digest.as_str() != expected_digest.as_str() {
            self.cleanup_upload(upload_id).await;
            return Err(OciStorageError::DigestMismatch {
                expected: expected_digest.to_string(),
                actual: actual_digest.to_string(),
            });
        }

        // Parse repository name
        let repository: RepositoryName = state
            .repository
            .parse()
            .map_err(|e: crate::types::RepositoryNameError| {
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
        self.cleanup_upload(upload_id).await;

        Ok(actual_digest)
    }

    /// Cancels an in-progress upload
    pub async fn cancel_upload(&self, upload_id: &UploadId) -> Result<(), OciStorageError> {
        // Load state to get S3 upload ID
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

        // Clean up
        self.cleanup_upload(upload_id).await;

        Ok(())
    }

    // ==================== Blob Operations ====================

    /// Checks if a blob exists
    pub async fn blob_exists(
        &self,
        repository: &RepositoryName,
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
            }
        }
    }

    /// Gets a blob's content and size
    pub async fn get_blob(
        &self,
        repository: &RepositoryName,
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

    /// Gets blob metadata (size) without downloading content
    pub async fn get_blob_size(
        &self,
        repository: &RepositoryName,
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
        repository: &RepositoryName,
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
        from_repository: &RepositoryName,
        to_repository: &RepositoryName,
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
        repository: &RepositoryName,
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
        repository: &RepositoryName,
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
        repository: &RepositoryName,
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
        repository: &RepositoryName,
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
    pub async fn delete_manifest(
        &self,
        repository: &RepositoryName,
        digest: &Digest,
    ) -> Result<(), OciStorageError> {
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
        repository: &RepositoryName,
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
    pub async fn list_referrers(
        &self,
        repository: &RepositoryName,
        subject_digest: &Digest,
        artifact_type_filter: Option<&str>,
    ) -> Result<Vec<serde_json::Value>, OciStorageError> {
        let prefix = self.referrers_prefix(repository, subject_digest);

        let mut referrers = Vec::new();
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
                    let Some(key) = object.key else {
                        continue;
                    };
                    // Get the referrer descriptor
                    let Ok(resp) = self
                        .client
                        .get_object()
                        .bucket(&self.config.bucket_arn)
                        .key(&key)
                        .send()
                        .await
                    else {
                        continue;
                    };
                    let Ok(data) = resp.body.collect().await else {
                        continue;
                    };
                    let Ok(descriptor) =
                        serde_json::from_slice::<serde_json::Value>(&data.into_bytes())
                    else {
                        continue;
                    };
                    // Apply artifact type filter if specified
                    if let Some(filter) = artifact_type_filter {
                        let matches = descriptor
                            .get("artifactType")
                            .and_then(|a| a.as_str())
                            .is_some_and(|at| at == filter);
                        if !matches {
                            continue;
                        }
                    }
                    referrers.push(descriptor);
                }
            }

            if response.is_truncated == Some(true) {
                continuation_token = response.next_continuation_token;
            } else {
                break;
            }
        }

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
