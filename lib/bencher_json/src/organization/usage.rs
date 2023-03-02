#![cfg(feature = "plus")]

use chrono::serde::ts_milliseconds::deserialize as from_milli_ts;
use chrono::serde::ts_milliseconds::serialize as to_milli_ts;
use chrono::{DateTime, Utc};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonUsage {
    #[serde(serialize_with = "to_milli_ts")]
    #[serde(deserialize_with = "from_milli_ts")]
    pub start: DateTime<Utc>,
    #[serde(serialize_with = "to_milli_ts")]
    #[serde(deserialize_with = "from_milli_ts")]
    pub end: DateTime<Utc>,
}
