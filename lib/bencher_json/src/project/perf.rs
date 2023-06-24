use chrono::{DateTime, Utc};
use ordered_float::OrderedFloat;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::ser::{self, SerializeStruct};
use serde::{Deserialize, Serialize, Serializer};
use url::Url;
use uuid::Uuid;

use crate::urlencoded::{
    from_millis, from_urlencoded, from_urlencoded_list, to_urlencoded, to_urlencoded_list,
    UrlEncodedError,
};
use crate::{JsonBenchmark, JsonBranch, JsonMetricKind, JsonProject, JsonTestbed, ResourceId};

use super::branch::JsonVersion;
use super::metric::JsonMetric;
use super::threshold::JsonStatistic;

const QUERY_KEYS: [&str; 6] = [
    "metric_kind",
    "branches",
    "testbeds",
    "benchmarks",
    "start_time",
    "end_time",
];

/// `JsonPerfQueryParams` is the actual query parameters accepted by the server.
/// All query parameter values are therefore scalar values.
/// Arrays are represented as comma separated lists.
/// Optional date times are simply stored as their millisecond representation.
/// `JsonPerfQueryParams` should always be converted into `JsonPerfQuery` for full type level validation.
#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonPerfQueryParams {
    pub title: Option<String>,
    pub metric_kind: String,
    pub branches: String,
    pub testbeds: String,
    pub benchmarks: String,
    pub start_time: Option<i64>,
    pub end_time: Option<i64>,
}

/// `JsonPerfQuery` is the full, strongly typed version of `JsonPerfQueryParams`.
/// It should always be used to validate `JsonPerfQueryParams`.
#[derive(Debug, Clone)]
pub struct JsonPerfQuery {
    pub metric_kind: ResourceId,
    pub branches: Vec<Uuid>,
    pub testbeds: Vec<Uuid>,
    pub benchmarks: Vec<Uuid>,
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
}

impl TryFrom<JsonPerfQueryParams> for JsonPerfQuery {
    type Error = UrlEncodedError;

    fn try_from(query_params: JsonPerfQueryParams) -> Result<Self, Self::Error> {
        let JsonPerfQueryParams {
            title: _,
            metric_kind,
            branches,
            testbeds,
            benchmarks,
            start_time,
            end_time,
        } = query_params;

        let metric_kind = from_urlencoded(&metric_kind)?;

        let branches = from_urlencoded_list(&branches)?;
        let testbeds = from_urlencoded_list(&testbeds)?;
        let benchmarks = from_urlencoded_list(&benchmarks)?;

        let start_time = if let Some(start_time) = start_time {
            Some(from_millis(start_time)?)
        } else {
            None
        };
        let end_time = if let Some(end_time) = end_time {
            Some(from_millis(end_time)?)
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

impl Serialize for JsonPerfQuery {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let urlencoded = self.urlencoded().map_err(ser::Error::custom)?;
        let mut state = serializer.serialize_struct("JsonPerfQuery", urlencoded.len())?;
        for (key, value) in urlencoded {
            state.serialize_field(key, &value)?;
        }
        state.end()
    }
}

impl JsonPerfQuery {
    pub fn to_url(
        &self,
        endpoint: &str,
        path: &str,
        query: &[(&str, Option<String>)],
    ) -> Result<Url, UrlEncodedError> {
        let mut url = Url::parse(endpoint)?;
        url.set_path(path);
        url.set_query(Some(&self.to_query_string(query)?));
        Ok(url)
    }

    pub fn to_query_string(
        &self,
        query: &[(&str, Option<String>)],
    ) -> Result<String, UrlEncodedError> {
        let urlencoded = self.urlencoded()?;
        let query = urlencoded.iter().chain(query).collect::<Vec<_>>();
        serde_urlencoded::to_string(query).map_err(Into::into)
    }

    fn urlencoded(&self) -> Result<[(&'static str, Option<String>); 6], UrlEncodedError> {
        let JsonPerfQuery {
            metric_kind,
            branches,
            testbeds,
            benchmarks,
            start_time,
            end_time,
        } = self;

        let metric_kind = Some(to_urlencoded(metric_kind)?);

        let branches = Some(to_urlencoded_list(branches)?);
        let testbeds = Some(to_urlencoded_list(testbeds)?);
        let benchmarks = Some(to_urlencoded_list(benchmarks)?);

        let start_time = if let Some(start_time) = start_time {
            Some(to_urlencoded(&start_time.timestamp_millis())?)
        } else {
            None
        };
        let end_time = if let Some(end_time) = end_time {
            Some(to_urlencoded(&end_time.timestamp_millis())?)
        } else {
            None
        };

        QUERY_KEYS
            .into_iter()
            .zip(
                [
                    metric_kind,
                    branches,
                    testbeds,
                    benchmarks,
                    start_time,
                    end_time,
                ]
                .into_iter(),
            )
            .collect::<Vec<_>>()
            .try_into()
            .map_err(UrlEncodedError::Vec)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonPerf {
    pub project: JsonProject,
    pub metric_kind: JsonMetricKind,
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub results: Vec<JsonPerfMetrics>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonPerfMetrics {
    pub branch: JsonBranch,
    pub testbed: JsonTestbed,
    pub benchmark: JsonBenchmark,
    pub metrics: Vec<JsonPerfMetric>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonPerfMetric {
    pub report: Uuid,
    pub iteration: u32,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub version: JsonVersion,
    pub metric: JsonMetric,
    // pub boundary: JsonPerfBoundary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonPerfBoundary {
    pub threshold: Uuid,
    pub statistic: JsonStatistic,
    pub left_side: Option<OrderedFloat<f64>>,
    pub right_side: Option<OrderedFloat<f64>>,
    pub alert: Option<Uuid>,
}

#[cfg(feature = "table")]
pub mod table {
    use std::fmt;

    use bencher_valid::GitHash;
    use chrono::{DateTime, Utc};
    use ordered_float::OrderedFloat;
    use tabled::{Table, TableIteratorExt, Tabled};

    use crate::{
        JsonBenchmark, JsonBranch, JsonMetric, JsonMetricKind, JsonPerf, JsonProject, JsonTestbed,
    };

    impl From<JsonPerf> for Table {
        fn from(json_perf: JsonPerf) -> Self {
            let mut perf_table = Vec::new();
            for result in json_perf.results {
                for metric in result.metrics {
                    perf_table.push(PerfTable {
                        project: json_perf.project.clone(),
                        metric_kind: json_perf.metric_kind.clone(),
                        branch: result.branch.clone(),
                        testbed: result.testbed.clone(),
                        benchmark: result.benchmark.clone(),
                        iteration: metric.iteration,
                        start_time: metric.start_time,
                        end_time: metric.end_time,
                        version_number: metric.version.number,
                        version_hash: VersionHash(metric.version.hash),
                        metric: metric.metric,
                        // left_side: BoundaryLimit(metric.boundary.left_side),
                        // right_side: BoundaryLimit(metric.boundary.right_side),
                    })
                }
            }
            perf_table.table()
        }
    }

    #[derive(Tabled)]
    pub struct PerfTable {
        #[tabled(rename = "Project")]
        pub project: JsonProject,
        #[tabled(rename = "Metric Kind")]
        pub metric_kind: JsonMetricKind,
        #[tabled(rename = "Branch")]
        pub branch: JsonBranch,
        #[tabled(rename = "Testbed")]
        pub testbed: JsonTestbed,
        #[tabled(rename = "Benchmark")]
        pub benchmark: JsonBenchmark,
        #[tabled(rename = "Iteration")]
        pub iteration: u32,
        #[tabled(rename = "Start Time")]
        pub start_time: DateTime<Utc>,
        #[tabled(rename = "End Time")]
        pub end_time: DateTime<Utc>,
        #[tabled(rename = "Version Number")]
        pub version_number: u32,
        #[tabled(rename = "Version Hash")]
        pub version_hash: VersionHash,
        #[tabled(rename = "Metric Value")]
        pub metric: JsonMetric,
        // #[tabled(rename = "Left Boundary Limit")]
        // pub left_side: BoundaryLimit,
        // #[tabled(rename = "Right Boundary Limit")]
        // pub right_side: BoundaryLimit,
    }

    pub struct VersionHash(Option<GitHash>);

    impl fmt::Display for VersionHash {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(
                f,
                "{}",
                if let Some(git_hash) = &self.0 {
                    git_hash.as_ref()
                } else {
                    ""
                }
            )
        }
    }

    pub struct BoundaryLimit(Option<OrderedFloat<f64>>);

    impl fmt::Display for BoundaryLimit {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(
                f,
                "{}",
                if let Some(limit) = &self.0 {
                    limit.to_string()
                } else {
                    "".into()
                }
            )
        }
    }
}
