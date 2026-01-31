use bencher_valid::{Sanitize, Secret};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// OCI Registry configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonOci {
    /// S3 storage configuration for OCI registry
    pub data_store: OciDataStore,
}

impl Sanitize for JsonOci {
    fn sanitize(&mut self) {
        self.data_store.sanitize();
    }
}

/// OCI Registry storage backend
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(tag = "service", rename_all = "snake_case")]
pub enum OciDataStore {
    AwsS3 {
        access_key_id: String,
        secret_access_key: Secret,
        /// S3 Access Point ARN with optional path prefix
        /// Format: arn:aws:s3:<region>:<account-id>:accesspoint/<bucket>[/oci-path]
        access_point: String,
    },
}

impl Sanitize for OciDataStore {
    fn sanitize(&mut self) {
        match self {
            Self::AwsS3 {
                secret_access_key, ..
            } => secret_access_key.sanitize(),
        }
    }
}
