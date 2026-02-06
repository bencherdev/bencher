use bencher_valid::{Sanitize, Secret};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Default upload timeout: 1 hour (3600 seconds)
pub const DEFAULT_UPLOAD_TIMEOUT_SECS: u64 = 3600;

/// Container registry configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonRegistry {
    /// S3 storage configuration for the container registry
    pub data_store: RegistryDataStore,
    /// Upload session timeout in seconds.
    /// Uploads older than this are cleaned up when new uploads start.
    /// Defaults to 3600 (1 hour).
    #[serde(default = "default_upload_timeout")]
    pub upload_timeout: u64,
}

fn default_upload_timeout() -> u64 {
    DEFAULT_UPLOAD_TIMEOUT_SECS
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
    AwsS3 {
        access_key_id: String,
        secret_access_key: Secret,
        /// S3 Access Point ARN with optional path prefix
        /// Format: arn:aws:s3:<region>:<account-id>:accesspoint/<bucket>[/path]
        access_point: String,
    },
}

impl Sanitize for RegistryDataStore {
    fn sanitize(&mut self) {
        match self {
            Self::AwsS3 {
                secret_access_key, ..
            } => secret_access_key.sanitize(),
        }
    }
}
