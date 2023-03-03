use chrono::serde::ts_milliseconds_option::deserialize as from_milli_ts;
use chrono::serde::ts_milliseconds_option::serialize as to_milli_ts;
use chrono::{DateTime, Utc};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

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

#[derive(Debug, Error)]
pub enum UrlEncodedError {
    #[error("{0}")]
    SerdeJson(#[from] serde_json::Error),
    #[error("{0}")]
    Serialize(#[from] serde_urlencoded::ser::Error),
    #[error("{0}")]
    Deserialize(#[from] serde_urlencoded::de::Error),
    #[error("{0}")]
    Uuid(#[from] uuid::Error),
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

const COMMA: &str = "%2C";

fn comma_separated_list(list: &str) -> Result<Vec<Uuid>, UrlEncodedError> {
    let mut values = Vec::new();
    for value in list.split(COMMA) {
        values.push(value.parse()?);
    }
    Ok(values)
}

fn urlencoded_list<T>(values: &[T]) -> Result<String, UrlEncodedError>
where
    T: Serialize,
{
    let mut list: Option<String> = None;
    for value in values {
        let element = urlencoded(value)?;
        if let Some(list) = list.as_mut() {
            list.push_str(COMMA);
            list.push_str(&element);
        } else {
            list = Some(element);
        }
    }

    Ok(list.unwrap_or_default())
}

fn urlencoded<T>(value: T) -> Result<String, UrlEncodedError>
where
    T: Serialize,
{
    const KEY: &str = "_x";
    const KEY_EQUAL: &str = "_x=";
    Ok(serde_urlencoded::to_string([(KEY, value)])?
        .strip_prefix(KEY_EQUAL)
        .unwrap_or_default()
        .to_string())
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
