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
