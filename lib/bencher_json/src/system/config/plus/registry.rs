use std::path::PathBuf;

use bencher_valid::{Sanitize, Secret, Url};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Default upload timeout: 1 hour (3600 seconds)
pub const DEFAULT_UPLOAD_TIMEOUT_SECS: u64 = 3600;

/// Default maximum body size: 1 GiB (1,073,741,824 bytes)
pub const DEFAULT_MAX_BODY_SIZE: u64 = 0x4000_0000;

/// Default part size for blob uploads: 16 MiB (16,777,216 bytes).
///
/// An upload streams into a multipart upload in parts of this size, each held
/// in memory until it is stored. A session keeps the part size it started with.
pub const DEFAULT_CHUNK_SIZE: u64 = 16 * 1024 * 1024;

/// Minimum part size for blob uploads: 5 MiB (5,242,880 bytes), the smallest
/// part an S3 multipart upload allows.
pub const MIN_CHUNK_SIZE: u64 = 5 * 1024 * 1024;

/// Maximum part size for blob uploads: 5 GiB (5,368,709,120 bytes), the largest
/// part an S3 multipart upload allows.
pub const MAX_CHUNK_SIZE: u64 = 5 * 1024 * 1024 * 1024;

/// Default memory for upload part buffers across all requests: 512 MiB (536,870,912 bytes).
pub const DEFAULT_MAX_UPLOAD_MEMORY: u64 = 512 * 1024 * 1024;

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
    /// Memory in bytes for upload part buffers, shared by all requests.
    /// An upload that finds no free buffer waits for one.
    /// A value below one part is raised to one part.
    /// A request whose part does not fill within 5 minutes of taking its buffer fails,
    /// and its client resends it.
    /// Defaults to 512 MiB (536,870,912 bytes). See [`DEFAULT_MAX_UPLOAD_MEMORY`].
    #[serde(default = "default_max_upload_memory")]
    pub max_upload_memory: u64,
}

fn default_upload_timeout() -> u64 {
    DEFAULT_UPLOAD_TIMEOUT_SECS
}

fn default_max_body_size() -> u64 {
    DEFAULT_MAX_BODY_SIZE
}

fn default_max_upload_memory() -> u64 {
    DEFAULT_MAX_UPLOAD_MEMORY
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
        /// Part size in bytes for blob uploads, which stream into multipart uploads.
        /// Valid range: 5 MiB to 5 GiB. Defaults to 16 MiB (16,777,216 bytes).
        /// See [`DEFAULT_CHUNK_SIZE`], [`MIN_CHUNK_SIZE`], and [`MAX_CHUNK_SIZE`].
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
}
