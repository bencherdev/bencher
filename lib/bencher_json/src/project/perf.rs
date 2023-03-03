use chrono::serde::ts_milliseconds_option::deserialize as from_milli_ts;
use chrono::serde::ts_milliseconds_option::serialize as to_milli_ts;
use chrono::{DateTime, Utc};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::urlencoded::comma_separated_list;
use crate::urlencoded::urlencoded_list;
use crate::urlencoded::UrlEncodedError;
use crate::ResourceId;

use super::metric::JsonMetric;

// TODO Figure out why
// Dropshot requires query parameters to be scalar
// The chrono::serde helper does not honor the optionality of an Option<DateTime<Utc>>
// So a second round of marshaling is required
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonPerfQueryParams {
    pub metric_kind: ResourceId,
    pub branches: String,
    pub testbeds: String,
    pub benchmarks: String,
    pub start_time: Option<i64>,
    pub end_time: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonPerfQuery {
    pub metric_kind: ResourceId,
    pub branches: Vec<Uuid>,
    pub testbeds: Vec<Uuid>,
    pub benchmarks: Vec<Uuid>,
    #[serde(serialize_with = "to_milli_ts")]
    #[serde(deserialize_with = "from_milli_ts")]
    pub start_time: Option<DateTime<Utc>>,
    #[serde(serialize_with = "to_milli_ts")]
    #[serde(deserialize_with = "from_milli_ts")]
    pub end_time: Option<DateTime<Utc>>,
}

impl TryFrom<JsonPerfQueryParams> for JsonPerfQuery {
    type Error = UrlEncodedError;

    fn try_from(query_params: JsonPerfQueryParams) -> Result<Self, Self::Error> {
        let JsonPerfQueryParams {
            metric_kind,
            branches,
            testbeds,
            benchmarks,
            start_time,
            end_time,
        } = query_params;

        let branches = comma_separated_list(&branches)?;
        let testbeds = comma_separated_list(&testbeds)?;
        let benchmarks = comma_separated_list(&benchmarks)?;

        let start_time = if let Some(start_time) = start_time {
            Some(serde_json::from_value(serde_json::json!(start_time))?)
        } else {
            None
        };
        let end_time = if let Some(end_time) = end_time {
            Some(serde_json::from_value(serde_json::json!(end_time))?)
        } else {
            None
        };

        Ok(Self {
            metric_kind,
            branches,
            testbeds,
            benchmarks,
            start_time,
            end_time,
        })
    }
}

impl TryFrom<JsonPerfQuery> for JsonPerfQueryParams {
    type Error = UrlEncodedError;

    fn try_from(query: JsonPerfQuery) -> Result<Self, Self::Error> {
        let JsonPerfQuery {
            metric_kind,
            branches,
            testbeds,
            benchmarks,
            start_time,
            end_time,
        } = query;

        let branches = urlencoded_list(&branches)?;
        let testbeds = urlencoded_list(&testbeds)?;
        let benchmarks = urlencoded_list(&benchmarks)?;

        let start_time = start_time.map(|t| t.timestamp_millis());
        let end_time = end_time.map(|t| t.timestamp_millis());

        Ok(Self {
            metric_kind,
            branches,
            testbeds,
            benchmarks,
            start_time,
            end_time,
        })
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonPerf {
    pub metric_kind: Uuid,
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub results: Vec<JsonPerfMetrics>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonPerfMetrics {
    pub branch: Uuid,
    pub testbed: Uuid,
    pub benchmark: Uuid,
    pub metrics: Vec<JsonPerfMetric>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonPerfMetric {
    pub uuid: Uuid,
    pub iteration: u32,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub version_number: u32,
    pub version_hash: Option<String>,
    pub metric: JsonMetric,
}
