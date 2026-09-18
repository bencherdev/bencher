use std::path::PathBuf;

use bencher_valid::{Sanitize, Secret, Url};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Default upload timeout: 1 hour (3600 seconds)
pub const DEFAULT_UPLOAD_TIMEOUT_SECS: u64 = 3600;

/// Default maximum body size: 1 GiB (1,073,741,824 bytes)
pub const DEFAULT_MAX_BODY_SIZE: u64 = 0x4000_0000;

/// Default chunk size for upload buffering: 5 MB (5,242,880 bytes).
///
/// HTTP request bodies arrive as small network frames (typically 8-64 KB).
/// Without batching, each frame would be stored as a separate object,
/// creating thousands of objects per layer and making both upload
/// and completion extremely slow. This value controls the minimum batch
/// size before flushing to the object store.
///
/// 5 MB also matches the S3 multipart upload minimum part size,
/// so chunks stored at this size can be efficiently assembled during
/// upload completion.
pub const DEFAULT_CHUNK_SIZE: u64 = 5 * 1024 * 1024;

/// Maximum chunk size for upload buffering: 5 GB (5,368,709,120 bytes).
///
/// The S3 multipart upload maximum part size is 5 GiB.
/// We use 5 GB as a practical upper bound.
pub const MAX_CHUNK_SIZE: u64 = 5 * 1024 * 1024 * 1024;

/// Container registry configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonRegistry {
    /// The externally-reachable URL of the API server for OCI registry access.
    /// Defaults to `http://localhost:6610`.
    pub url: Option<Url>,
    /// Storage configuration for the container registry.
    /// Defaults to local filesystem storage if not provided.
    pub storage: Option<RegistryStorage>,
    /// Upload session timeout in seconds.
    /// Uploads older than this are cleaned up when new uploads start.
    /// Defaults to 3600 (1 hour).
    #[serde(default = "default_upload_timeout")]
    pub upload_timeout: u64,
    /// Maximum body size in bytes for blob and manifest uploads.
    /// Requests exceeding this limit are rejected with 413 Payload Too Large.
    /// Defaults to 1 GiB (1,073,741,824 bytes).
    #[serde(default = "default_max_body_size")]
    pub max_body_size: u64,
}

fn default_upload_timeout() -> u64 {
    DEFAULT_UPLOAD_TIMEOUT_SECS
}

fn default_max_body_size() -> u64 {
    DEFAULT_MAX_BODY_SIZE
}

impl Sanitize for JsonRegistry {
    fn sanitize(&mut self) {
        self.storage.sanitize();
    }
}

/// Container registry storage backend
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(tag = "scheme", rename_all = "snake_case")]
pub enum RegistryStorage {
    File {
        /// Directory for registry data.
        /// Defaults to a `registry` directory beside the database file.
        #[serde(skip_serializing_if = "Option::is_none")]
        path: Option<PathBuf>,
    },
    S3 {
        /// Bucket that holds the registry data
        bucket: String,
        /// Key prefix inside the bucket
        #[serde(skip_serializing_if = "Option::is_none")]
        path: Option<String>,
        /// Endpoint URL, required for any store other than AWS
        #[serde(skip_serializing_if = "Option::is_none")]
        endpoint: Option<String>,
        /// Region, required for AWS.
        /// Defaults to `auto`, which S3-compatible stores expect.
        #[serde(skip_serializing_if = "Option::is_none")]
        region: Option<String>,
        /// Access Key ID with permissions to read/write the specified bucket
        access_key_id: String,
        /// Secret Access Key with permissions to read/write the specified bucket
        secret_access_key: Secret,
        /// Minimum chunk size in bytes for buffering upload data before storing.
        /// Valid range: 5 MB to 5 GB. Defaults to 5 MB (5,242,880 bytes).
        /// See [`DEFAULT_CHUNK_SIZE`] and [`MAX_CHUNK_SIZE`].
        #[serde(skip_serializing_if = "Option::is_none")]
        chunk_size: Option<u64>,
    },
}

impl Sanitize for RegistryStorage {
    fn sanitize(&mut self) {
        match self {
            Self::File { .. } => {},
            Self::S3 {
                secret_access_key, ..
            } => secret_access_key.sanitize(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{JsonRegistry, RegistryStorage};

    fn registry(storage: &str) -> JsonRegistry {
        serde_json::from_str(&format!(r#"{{"url": null, "storage": {storage}}}"#)).unwrap()
    }

    #[test]
    fn file_without_path() {
        let registry = registry(r#"{"scheme": "file"}"#);
        let Some(RegistryStorage::File { path }) = registry.storage else {
            panic!("expected File")
        };
        assert!(path.is_none());
    }

    #[test]
    fn file_with_path() {
        let registry = registry(r#"{"scheme": "file", "path": "/data/registry"}"#);
        let Some(RegistryStorage::File { path }) = registry.storage else {
            panic!("expected File")
        };
        assert_eq!(path.unwrap(), PathBuf::from("/data/registry"));
    }

    #[test]
    fn s3_minimal() {
        let registry = registry(
            r#"{"scheme": "s3", "bucket": "my-bucket", "access_key_id": "key", "secret_access_key": "secret"}"#,
        );
        let Some(RegistryStorage::S3 {
            bucket,
            path,
            endpoint,
            region,
            access_key_id,
            secret_access_key,
            chunk_size,
        }) = registry.storage
        else {
            panic!("expected S3")
        };
        assert_eq!(bucket, "my-bucket");
        assert!(path.is_none());
        assert!(endpoint.is_none());
        assert!(region.is_none());
        assert!(chunk_size.is_none());
        assert_eq!(access_key_id, "key");
        assert_eq!(secret_access_key.as_ref(), "secret");
    }

    #[test]
    fn s3_full() {
        let registry = registry(
            r#"{"scheme": "s3", "bucket": "my-bucket", "path": "registry", "endpoint": "https://s3.example.com", "region": "auto", "access_key_id": "key", "secret_access_key": "secret", "chunk_size": 5242880}"#,
        );
        let Some(RegistryStorage::S3 {
            path,
            endpoint,
            region,
            chunk_size,
            ..
        }) = registry.storage
        else {
            panic!("expected S3")
        };
        assert_eq!(path.as_deref(), Some("registry"));
        assert_eq!(endpoint.as_deref(), Some("https://s3.example.com"));
        assert_eq!(region.as_deref(), Some("auto"));
        assert_eq!(chunk_size, Some(5_242_880));
    }

    #[test]
    fn missing_storage_is_none() {
        let registry: JsonRegistry = serde_json::from_str(r#"{"url": null}"#).unwrap();
        assert!(registry.storage.is_none());
    }

    #[test]
    fn data_store_is_no_longer_a_field() {
        let registry: JsonRegistry = serde_json::from_str(
            r#"{"url": null, "data_store": {"service": "aws_s3", "access_key_id": "key", "secret_access_key": "secret", "access_point": "arn:aws:s3:us-east-1:123456789012:accesspoint/my-bucket"}}"#,
        )
        .unwrap();
        assert!(registry.storage.is_none());
    }

    #[test]
    fn sanitize_hides_the_secret() {
        use bencher_valid::Sanitize as _;
        let mut registry = registry(
            r#"{"scheme": "s3", "bucket": "my-bucket", "access_key_id": "key", "secret_access_key": "my_secret"}"#,
        );
        registry.sanitize();
        let Some(RegistryStorage::S3 {
            secret_access_key, ..
        }) = registry.storage
        else {
            panic!("expected S3")
        };
        assert_eq!(secret_access_key.as_ref(), "************");
    }

    #[test]
    fn sanitize_leaves_file_alone() {
        use bencher_valid::Sanitize as _;
        let mut registry = registry(r#"{"scheme": "file", "path": "/data/registry"}"#);
        registry.sanitize();
        let Some(RegistryStorage::File { path }) = registry.storage else {
            panic!("expected File")
        };
        assert_eq!(path.unwrap(), PathBuf::from("/data/registry"));
    }
}
