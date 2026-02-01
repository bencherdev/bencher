//! OCI Storage Layer - Local Filesystem Backend
//!
//! This implementation stores OCI artifacts on the local filesystem,
//! sibling to the database file. If the database is at `data/bencher.db`,
//! OCI data will be stored under `data/oci/`.

use std::path::{Path, PathBuf};

use bytes::Bytes;
use sha2::{Digest as _, Sha256};
use tokio::fs;
use tokio::io::AsyncWriteExt as _;

use bencher_json::ProjectResourceId;

use crate::storage::OciStorageError;
use crate::types::{Digest, UploadId};

/// Upload state stored on disk
#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct UploadState {
    /// Repository name
    repository: String,
    /// Total bytes uploaded so far
    size: u64,
}

/// OCI Storage implementation using local filesystem
pub(crate) struct OciLocalStorage {
    /// Base directory for OCI storage (e.g., `data/oci`)
    base_dir: PathBuf,
}

impl OciLocalStorage {
    /// Creates a new local OCI storage instance
    ///
    /// The `database_path` is the path to the `SQLite` database file.
    /// OCI data will be stored in an `oci` subdirectory next to it.
    pub fn new(database_path: &Path) -> Self {
        let base_dir = database_path
            .parent()
            .map_or_else(|| PathBuf::from("oci"), |p| p.join("oci"));

        Self { base_dir }
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
    fn repository_dir(&self, repository: &ProjectResourceId) -> PathBuf {
        self.base_dir.join(repository.to_string())
    }

    /// Returns the path for a blob
    fn blob_path(&self, repository: &ProjectResourceId, digest: &Digest) -> PathBuf {
        self.repository_dir(repository)
            .join("blobs")
            .join(digest.algorithm())
            .join(digest.hex_hash())
    }

    /// Returns the path for a manifest by digest
    fn manifest_path(&self, repository: &ProjectResourceId, digest: &Digest) -> PathBuf {
        self.repository_dir(repository)
            .join("manifests")
            .join("sha256")
            .join(digest.hex_hash())
    }

    /// Returns the path for a tag link
    fn tag_path(&self, repository: &ProjectResourceId, tag: &str) -> PathBuf {
        self.repository_dir(repository).join("tags").join(tag)
    }

    /// Returns the directory for referrers to a given digest
    fn referrers_dir(&self, repository: &ProjectResourceId, subject_digest: &Digest) -> PathBuf {
        self.repository_dir(repository)
            .join("referrers")
            .join(subject_digest.algorithm())
            .join(subject_digest.hex_hash())
    }

    /// Returns the path for a referrer link
    fn referrer_path(
        &self,
        repository: &ProjectResourceId,
        subject_digest: &Digest,
        referrer_digest: &Digest,
    ) -> PathBuf {
        self.referrers_dir(repository, subject_digest).join(format!(
            "{}-{}",
            referrer_digest.algorithm(),
            referrer_digest.hex_hash()
        ))
    }

    // ==================== Upload State Management ====================

    /// Loads upload state from disk
    async fn load_upload_state(
        &self,
        upload_id: &UploadId,
    ) -> Result<UploadState, OciStorageError> {
        let path = self.upload_state_path(upload_id);
        let data = fs::read(&path).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                OciStorageError::UploadNotFound(upload_id.to_string())
            } else {
                OciStorageError::LocalStorage(format!("Failed to read upload state: {e}"))
            }
        })?;

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
    pub async fn start_upload(
        &self,
        repository: &ProjectResourceId,
    ) -> Result<UploadId, OciStorageError> {
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

        // Save initial state
        let state = UploadState {
            repository: repository.to_string(),
            size: 0,
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
        // Load current state
        let mut state = self.load_upload_state(upload_id).await?;

        // Append data to file
        let data_path = self.upload_data_path(upload_id);
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

        // Update state
        state.size += data.len() as u64;
        self.save_upload_state(upload_id, &state).await?;

        Ok(state.size)
    }

    /// Gets the current size of an in-progress upload
    pub async fn get_upload_size(&self, upload_id: &UploadId) -> Result<u64, OciStorageError> {
        let state = self.load_upload_state(upload_id).await?;
        Ok(state.size)
    }

    /// Completes an upload and stores the blob
    pub async fn complete_upload(
        &self,
        upload_id: &UploadId,
        expected_digest: &Digest,
    ) -> Result<Digest, OciStorageError> {
        // Load state
        let state = self.load_upload_state(upload_id).await?;

        // Read the uploaded data
        let data_path = self.upload_data_path(upload_id);
        let data = fs::read(&data_path).await.map_err(|e| {
            OciStorageError::LocalStorage(format!("Failed to read upload data: {e}"))
        })?;

        if data.is_empty() {
            self.cleanup_upload(upload_id).await;
            return Err(OciStorageError::InvalidContent(
                "Cannot complete upload with no data".to_owned(),
            ));
        }

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
        let repository: ProjectResourceId = state
            .repository
            .parse()
            .map_err(|e: bencher_json::ValidError| {
                OciStorageError::InvalidContent(e.to_string())
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
        let _unused = fs::remove_dir_all(&upload_dir).await;
    }

    // ==================== Blob Operations ====================

    /// Checks if a blob exists
    pub async fn blob_exists(
        &self,
        repository: &ProjectResourceId,
        digest: &Digest,
    ) -> Result<bool, OciStorageError> {
        let path = self.blob_path(repository, digest);
        Ok(fs::try_exists(&path).await.unwrap_or(false))
    }

    /// Gets a blob's content and size
    pub async fn get_blob(
        &self,
        repository: &ProjectResourceId,
        digest: &Digest,
    ) -> Result<(Bytes, u64), OciStorageError> {
        let path = self.blob_path(repository, digest);
        let data = fs::read(&path).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                OciStorageError::BlobNotFound(digest.to_string())
            } else {
                OciStorageError::LocalStorage(format!("Failed to read blob: {e}"))
            }
        })?;

        let size = data.len() as u64;
        Ok((Bytes::from(data), size))
    }

    /// Gets blob metadata (size) without downloading content
    pub async fn get_blob_size(
        &self,
        repository: &ProjectResourceId,
        digest: &Digest,
    ) -> Result<u64, OciStorageError> {
        let path = self.blob_path(repository, digest);
        let metadata = fs::metadata(&path).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                OciStorageError::BlobNotFound(digest.to_string())
            } else {
                OciStorageError::LocalStorage(format!("Failed to get blob metadata: {e}"))
            }
        })?;

        Ok(metadata.len())
    }

    /// Deletes a blob
    pub async fn delete_blob(
        &self,
        repository: &ProjectResourceId,
        digest: &Digest,
    ) -> Result<(), OciStorageError> {
        let path = self.blob_path(repository, digest);
        match fs::remove_file(&path).await {
            Ok(()) => Ok(()),
            // File already deleted or never existed - that's fine
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(OciStorageError::LocalStorage(format!(
                "Failed to delete blob: {e}"
            ))),
        }
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
        let source_path = self.blob_path(from_repository, digest);
        let dest_path = self.blob_path(to_repository, digest);

        if let Some(parent) = dest_path.parent() {
            fs::create_dir_all(parent).await.map_err(|e| {
                OciStorageError::LocalStorage(format!("Failed to create blob directory: {e}"))
            })?;
        }

        fs::copy(&source_path, &dest_path)
            .await
            .map_err(|e| OciStorageError::LocalStorage(format!("Failed to copy blob: {e}")))?;

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
                serde_json::to_vec(&descriptor).unwrap_or_default(),
            )
            .await
            .map_err(|e| OciStorageError::LocalStorage(format!("Failed to write referrer: {e}")))?;
        }

        Ok(digest)
    }

    /// Gets a manifest by digest
    pub async fn get_manifest_by_digest(
        &self,
        repository: &ProjectResourceId,
        digest: &Digest,
    ) -> Result<Bytes, OciStorageError> {
        let path = self.manifest_path(repository, digest);
        let data = fs::read(&path).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                OciStorageError::ManifestNotFound(digest.to_string())
            } else {
                OciStorageError::LocalStorage(format!("Failed to read manifest: {e}"))
            }
        })?;

        Ok(Bytes::from(data))
    }

    /// Resolves a tag to a digest
    pub async fn resolve_tag(
        &self,
        repository: &ProjectResourceId,
        tag: &str,
    ) -> Result<Digest, OciStorageError> {
        let path = self.tag_path(repository, tag);
        let data = fs::read_to_string(&path).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                OciStorageError::ManifestNotFound(tag.to_owned())
            } else {
                OciStorageError::LocalStorage(format!("Failed to read tag: {e}"))
            }
        })?;

        data.trim()
            .parse()
            .map_err(|e: crate::types::DigestError| OciStorageError::InvalidContent(e.to_string()))
    }

    /// Lists all tags for a repository
    pub async fn list_tags(
        &self,
        repository: &ProjectResourceId,
    ) -> Result<Vec<String>, OciStorageError> {
        let tags_dir = self.repository_dir(repository).join("tags");

        if !tags_dir.exists() {
            return Ok(Vec::new());
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

        Ok(tags)
    }

    /// Deletes a manifest by digest
    pub async fn delete_manifest(
        &self,
        repository: &ProjectResourceId,
        digest: &Digest,
    ) -> Result<(), OciStorageError> {
        let path = self.manifest_path(repository, digest);
        match fs::remove_file(&path).await {
            Ok(()) => Ok(()),
            // File already deleted or never existed - that's fine
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(OciStorageError::LocalStorage(format!(
                "Failed to delete manifest: {e}"
            ))),
        }
    }

    /// Deletes a tag
    pub async fn delete_tag(
        &self,
        repository: &ProjectResourceId,
        tag: &str,
    ) -> Result<(), OciStorageError> {
        let path = self.tag_path(repository, tag);
        match fs::remove_file(&path).await {
            Ok(()) => Ok(()),
            // File already deleted or never existed - that's fine
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(OciStorageError::LocalStorage(format!(
                "Failed to delete tag: {e}"
            ))),
        }
    }

    /// Lists all manifests that reference a given digest via their subject field
    pub async fn list_referrers(
        &self,
        repository: &ProjectResourceId,
        subject_digest: &Digest,
        artifact_type_filter: Option<&str>,
    ) -> Result<Vec<serde_json::Value>, OciStorageError> {
        let referrers_dir = self.referrers_dir(repository, subject_digest);

        if !referrers_dir.exists() {
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
                continue;
            };
            let Ok(descriptor) = serde_json::from_slice::<serde_json::Value>(&data) else {
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

        Ok(referrers)
    }
}
