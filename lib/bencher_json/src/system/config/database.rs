use std::path::PathBuf;

use bencher_valid::{Sanitize, Secret};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonDatabase {
    pub file: PathBuf,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_store: Option<DataStore>,
}

impl Sanitize for JsonDatabase {
    fn sanitize(&mut self) {
        self.data_store.sanitize();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(tag = "service", rename_all = "snake_case")]
pub enum DataStore {
    AwsS3 {
        access_key_id: String,
        secret_access_key: Secret,
        // arn:aws:s3:<region>:<account-id>:accesspoint/<resource>[/backup-dir-path]
        // https://docs.aws.amazon.com/AmazonS3/latest/userguide/using-access-points.html
        access_point: String,
    },
}

impl Sanitize for DataStore {
    fn sanitize(&mut self) {
        match self {
            Self::AwsS3 {
                secret_access_key, ..
            } => secret_access_key.sanitize(),
        }
    }
}
