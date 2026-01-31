//! OCI Storage Layer - S3 Backend

use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

use aws_sdk_s3::Client;
use bencher_json::{system::config::OciDataStore, Secret};
use bytes::Bytes;
use sha2::{Digest as _, Sha256};
use thiserror::Error;
use tokio::sync::Mutex;

use crate::types::{Digest, RepositoryName, UploadId};

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
}

/// Configuration for OCI S3 storage
#[derive(Debug, Clone)]
pub struct OciStorageConfig {
    pub bucket_arn: String,
    pub prefix: Option<String>,
}

/// In-progress upload state
struct UploadState {
    repository: RepositoryName,
    chunks: Vec<Bytes>,
    total_size: u64,
}

/// OCI Storage implementation using S3
pub struct OciStorage {
    client: Client,
    config: OciStorageConfig,
    // In-progress uploads (in memory for now, could be moved to S3 multipart)
    uploads: Arc<Mutex<HashMap<String, UploadState>>>,
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

        Ok(Self {
            client,
            config,
            uploads: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// Creates a new OCI storage instance (for testing or direct use)
    pub fn new(client: Client, config: OciStorageConfig) -> Self {
        Self {
            client,
            config,
            uploads: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Returns the S3 key prefix for the given repository
    fn key_prefix(&self, repository: &RepositoryName) -> String {
        match &self.config.prefix {
            Some(prefix) => format!("{prefix}/{repository}"),
            None => repository.to_string(),
        }
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

    /// Starts a new upload session
    pub async fn start_upload(
        &self,
        repository: &RepositoryName,
    ) -> Result<UploadId, OciStorageError> {
        let upload_id = UploadId::new();
        let state = UploadState {
            repository: repository.clone(),
            chunks: Vec::new(),
            total_size: 0,
        };

        let mut uploads = self.uploads.lock().await;
        uploads.insert(upload_id.to_string(), state);

        Ok(upload_id)
    }

    /// Appends data to an in-progress upload
    pub async fn append_upload(
        &self,
        upload_id: &UploadId,
        data: Bytes,
    ) -> Result<u64, OciStorageError> {
        let mut uploads = self.uploads.lock().await;
        let state = uploads
            .get_mut(upload_id.as_str())
            .ok_or_else(|| OciStorageError::UploadNotFound(upload_id.to_string()))?;

        state.total_size += data.len() as u64;
        state.chunks.push(data);

        Ok(state.total_size)
    }

    /// Gets the current size of an in-progress upload
    pub async fn get_upload_size(&self, upload_id: &UploadId) -> Result<u64, OciStorageError> {
        let uploads = self.uploads.lock().await;
        let state = uploads
            .get(upload_id.as_str())
            .ok_or_else(|| OciStorageError::UploadNotFound(upload_id.to_string()))?;

        Ok(state.total_size)
    }

    /// Completes an upload and stores the blob
    pub async fn complete_upload(
        &self,
        upload_id: &UploadId,
        expected_digest: &Digest,
    ) -> Result<Digest, OciStorageError> {
        // Remove the upload state
        let state = {
            let mut uploads = self.uploads.lock().await;
            uploads
                .remove(upload_id.as_str())
                .ok_or_else(|| OciStorageError::UploadNotFound(upload_id.to_string()))?
        };

        // Concatenate all chunks
        let capacity = usize::try_from(state.total_size).unwrap_or(usize::MAX);
        let mut data = Vec::with_capacity(capacity);
        for chunk in state.chunks {
            data.extend_from_slice(&chunk);
        }

        // Compute the actual digest
        let mut hasher = Sha256::new();
        hasher.update(&data);
        let hash = hasher.finalize();
        let actual_digest = Digest::sha256(&hex::encode(hash));

        // Verify digest matches
        if actual_digest.as_str() != expected_digest.as_str() {
            return Err(OciStorageError::DigestMismatch {
                expected: expected_digest.to_string(),
                actual: actual_digest.to_string(),
            });
        }

        // Store the blob in S3
        let key = self.blob_key(&state.repository, &actual_digest);
        self.client
            .put_object()
            .bucket(&self.config.bucket_arn)
            .key(&key)
            .body(data.into())
            .send()
            .await
            .map_err(|e| OciStorageError::S3(e.to_string()))?;

        Ok(actual_digest)
    }

    /// Cancels an in-progress upload
    pub async fn cancel_upload(&self, upload_id: &UploadId) -> Result<(), OciStorageError> {
        let mut uploads = self.uploads.lock().await;
        uploads
            .remove(upload_id.as_str())
            .ok_or_else(|| OciStorageError::UploadNotFound(upload_id.to_string()))?;
        Ok(())
    }

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
                // Check if error is NotFound (404)
                if e.raw_response()
                    .is_some_and(|r| r.status().as_u16() == 404)
                    
                {
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
                if e.raw_response()
                    .is_some_and(|r| r.status().as_u16() == 404)
                    
                {
                    OciStorageError::BlobNotFound(digest.to_string())
                } else {
                    OciStorageError::S3(e.to_string())
                }
            })?;

        let size = response.content_length().map_or(0, |len| u64::try_from(len).unwrap_or(0));
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
                if e.raw_response()
                    .is_some_and(|r| r.status().as_u16() == 404)
                    
                {
                    OciStorageError::BlobNotFound(digest.to_string())
                } else {
                    OciStorageError::S3(e.to_string())
                }
            })?;

        Ok(response.content_length().map_or(0, |len| u64::try_from(len).unwrap_or(0)))
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
            // Store the digest as the tag content
            self.client
                .put_object()
                .bucket(&self.config.bucket_arn)
                .key(&tag_key)
                .body(digest.to_string().into_bytes().into())
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
                if e.raw_response()
                    .is_some_and(|r| r.status().as_u16() == 404)
                    
                {
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
                if e.raw_response()
                    .is_some_and(|r| r.status().as_u16() == 404)
                    
                {
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

        let digest_str =
            String::from_utf8(data.to_vec()).map_err(|e| OciStorageError::InvalidContent(e.to_string()))?;

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
        let source_key = self.blob_key(from_repository, digest);
        let dest_key = self.blob_key(to_repository, digest);

        self.client
            .copy_object()
            .bucket(&self.config.bucket_arn)
            .copy_source(format!("{}/{}", self.config.bucket_arn, source_key))
            .key(&dest_key)
            .send()
            .await
            .map_err(|e| OciStorageError::S3(e.to_string()))?;

        Ok(true)
    }
}

// S3 ARN parsing (similar to lib/bencher_schema/src/context/database.rs)

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
