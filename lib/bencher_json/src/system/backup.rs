use bencher_valid::DateTime;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonBackup {
    /// Compress the database backup with gzip.
    /// This operation runs first.
    pub compress: Option<bool>,
    /// Save the database backup to this data store.
    /// This operation runs second.
    pub data_store: Option<JsonDataStore>,
    // TODO remove in due time
    #[serde(alias = "remove")]
    /// Remove the local copy of the database backup.
    /// This operation runs third.
    pub rm: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum JsonDataStore {
    AwsS3,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonBackupCreated {
    pub created: DateTime,
}
