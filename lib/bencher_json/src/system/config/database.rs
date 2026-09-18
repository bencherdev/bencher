use std::{num::NonZeroU32, path::PathBuf};

#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonDatabase {
    pub file: PathBuf,
    /// The database busy timeout in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub busy_timeout: Option<u32>,
    /// The database page cache size in KiB for the writer connection
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_size: Option<NonZeroU32>,
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::JsonDatabase;

    #[test]
    fn data_store_is_ignored() {
        let database: JsonDatabase = serde_json::from_str(
            r#"{
                "file": "/var/lib/bencher/data/bencher.db",
                "data_store": {
                    "service": "aws_s3",
                    "access_key_id": "access_key_id",
                    "secret_access_key": "secret_access_key",
                    "access_point": "arn:aws:s3:us-east-1:000000000000:accesspoint/backup"
                },
                "busy_timeout": 5000,
                "cache_size": 1000
            }"#,
        )
        .unwrap();
        let JsonDatabase {
            file,
            busy_timeout,
            cache_size,
        } = database;
        assert_eq!(file, PathBuf::from("/var/lib/bencher/data/bencher.db"));
        assert_eq!(busy_timeout, Some(5000));
        assert_eq!(cache_size.map(u32::from), Some(1000));
    }
}
