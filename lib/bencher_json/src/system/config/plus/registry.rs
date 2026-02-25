use bencher_valid::{Sanitize, Secret, Url};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Default upload timeout: 1 hour (3600 seconds)
pub const DEFAULT_UPLOAD_TIMEOUT_SECS: u64 = 3600;

/// Default maximum body size: 1 GiB (1,073,741,824 bytes)
pub const DEFAULT_MAX_BODY_SIZE: u64 = 0x4000_0000;

/// Container registry configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonRegistry {
    /// The externally-reachable URL of the API server for OCI registry access.
    /// Defaults to `http://localhost:61016`.
    pub url: Option<Url>,
    /// Storage configuration for the container registry.
    /// Defaults to local filesystem storage if not provided.
    pub data_store: Option<RegistryDataStore>,
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
        self.data_store.sanitize();
    }
}

/// Container registry storage backend
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(tag = "service", rename_all = "snake_case")]
pub enum RegistryDataStore {
    Local,
    AwsS3 {
        /// AWS Access Key ID with permissions to read/write the specified S3 bucket
        access_key_id: String,
        /// AWS Secret Access Key with permissions to read/write the specified S3 bucket
        secret_access_key: Secret,
        /// S3 Access Point ARN with optional path prefix
        /// Format: arn:aws:s3:<region>:<account-id>:accesspoint/<bucket>[/path]
        access_point: String,
    },
}

impl Sanitize for RegistryDataStore {
    fn sanitize(&mut self) {
        match self {
            Self::Local => {},
            Self::AwsS3 {
                secret_access_key, ..
            } => secret_access_key.sanitize(),
        }
    }
}
