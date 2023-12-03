#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::ser::{self, SerializeStruct};
use serde::{Deserialize, Serialize, Serializer};
use url::Url;

use crate::urlencoded::{from_urlencoded_list, to_urlencoded, to_urlencoded_list, UrlEncodedError};
use crate::{
    BenchmarkUuid, BranchUuid, DateTime, DateTimeMillis, JsonBenchmark, JsonBranch, JsonMetricKind,
    JsonProject, JsonTestbed, MetricKindUuid, ReportUuid, TestbedUuid,
};

use super::alert::JsonPerfAlert;
use super::boundary::JsonBoundary;
use super::branch::JsonVersion;
use super::metric::JsonMetric;
use super::threshold::JsonThresholdStatistic;

crate::typed_uuid::typed_uuid!(PerfUuid);

/// `JsonPerfQueryParams` is the actual query parameters accepted by the server.
/// All query parameter values are therefore scalar values.
/// Arrays are represented as comma separated lists.
/// Optional date times are simply stored as their millisecond representation.
/// `JsonPerfQueryParams` should always be converted into `JsonPerfQuery` for full type level validation.
#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonPerfQueryParams {
    pub title: Option<String>,
    pub metric_kinds: String,
    pub branches: String,
    pub testbeds: String,
    pub benchmarks: String,
    pub start_time: Option<DateTimeMillis>,
    pub end_time: Option<DateTimeMillis>,
}

/// `JsonPerfQuery` is the full, strongly typed version of `JsonPerfQueryParams`.
/// It should always be used to validate `JsonPerfQueryParams`.
#[typeshare::typeshare]
#[derive(Debug, Clone)]
pub struct JsonPerfQuery {
    pub metric_kinds: Vec<MetricKindUuid>,
    pub branches: Vec<BranchUuid>,
    pub testbeds: Vec<TestbedUuid>,
    pub benchmarks: Vec<BenchmarkUuid>,
    pub start_time: Option<DateTime>,
    pub end_time: Option<DateTime>,
}

impl TryFrom<JsonPerfQueryParams> for JsonPerfQuery {
    type Error = UrlEncodedError;

    fn try_from(query_params: JsonPerfQueryParams) -> Result<Self, Self::Error> {
        let JsonPerfQueryParams {
            title: _,
            metric_kinds,
            branches,
            testbeds,
            benchmarks,
            start_time,
            end_time,
        } = query_params;

        let metric_kinds = from_urlencoded_list(&metric_kinds)?;
        let branches = from_urlencoded_list(&branches)?;
        let testbeds = from_urlencoded_list(&testbeds)?;
        let benchmarks = from_urlencoded_list(&benchmarks)?;

        Ok(Self {
            metric_kinds,
            branches,
            testbeds,
            benchmarks,
            start_time: start_time.map(Into::into),
            end_time: end_time.map(Into::into),
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
        QUERY_KEYS
            .into_iter()
            .zip([
                Some(self.metric_kinds()),
                Some(self.branches()),
                Some(self.testbeds()),
                Some(self.benchmarks()),
                self.start_time_str(),
                self.end_time_str(),
            ])
            .collect::<Vec<_>>()
            .try_into()
            .map_err(UrlEncodedError::Vec)
    }

    pub fn metric_kinds(&self) -> String {
        to_urlencoded_list(&self.metric_kinds)
    }

    pub fn branches(&self) -> String {
        to_urlencoded_list(&self.branches)
    }

    pub fn testbeds(&self) -> String {
        to_urlencoded_list(&self.testbeds)
    }

    pub fn benchmarks(&self) -> String {
        to_urlencoded_list(&self.benchmarks)
    }

    pub fn start_time(&self) -> Option<DateTimeMillis> {
        self.start_time.map(Into::into)
    }

    pub fn end_time(&self) -> Option<DateTimeMillis> {
        self.end_time.map(Into::into)
    }

    fn start_time_str(&self) -> Option<String> {
        self.start_time().as_ref().map(to_urlencoded)
    }

    fn end_time_str(&self) -> Option<String> {
        self.end_time().as_ref().map(to_urlencoded)
    }
}

#[typeshare::typeshare]
#[derive(Debug, Clone, Copy, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum PerfQueryKey {
    MetricKinds,
    Branches,
    Testbeds,
    Benchmarks,
    StartTime,
    EndTime,
    // Console Keys
    LowerBoundary,
    UpperBoundary,
}

pub const METRIC_KINDS: &str = "metric_kinds";
pub const BRANCHES: &str = "branches";
pub const TESTBEDS: &str = "testbeds";
pub const BENCHMARKS: &str = "benchmarks";
pub const START_TIME: &str = "start_time";
pub const END_TIME: &str = "end_time";
const QUERY_KEYS: [&str; 6] = [
    METRIC_KINDS,
    BRANCHES,
    TESTBEDS,
    BENCHMARKS,
    START_TIME,
    END_TIME,
];
// Console Keys
pub const LOWER_BOUNDARY: &str = "lower_boundary";
pub const UPPER_BOUNDARY: &str = "upper_boundary";

#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonPerf {
    pub project: JsonProject,
    pub start_time: Option<DateTime>,
    pub end_time: Option<DateTime>,
    pub results: Vec<JsonPerfMetrics>,
}

#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonPerfMetrics {
    pub metric_kind: JsonMetricKind,
    pub branch: JsonBranch,
    pub testbed: JsonTestbed,
    pub benchmark: JsonBenchmark,
    pub metrics: Vec<JsonPerfMetric>,
}

#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonPerfMetric {
    pub report: ReportUuid,
    pub iteration: Iteration,
    pub start_time: DateTime,
    pub end_time: DateTime,
    pub version: JsonVersion,
    // Threshold is necessary for each metric as the statistic may change over time
    pub threshold: Option<JsonThresholdStatistic>,
    pub metric: JsonMetric,
    pub boundary: JsonBoundary,
    pub alert: Option<JsonPerfAlert>,
}

#[typeshare::typeshare]
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Hash, derive_more::Display, Serialize, Deserialize,
)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Integer))]
pub struct Iteration(pub u32);

#[cfg(feature = "db")]
mod iteration {
    use super::Iteration;

    impl From<usize> for Iteration {
        fn from(value: usize) -> Self {
            Self(u32::try_from(value).unwrap_or_default())
        }
    }

    impl<DB> diesel::serialize::ToSql<diesel::sql_types::Integer, DB> for Iteration
    where
        DB: diesel::backend::Backend,
        for<'a> i32: diesel::serialize::ToSql<diesel::sql_types::Integer, DB>
            + Into<<DB::BindCollector<'a> as diesel::query_builder::BindCollector<'a, DB>>::Buffer>,
    {
        fn to_sql<'b>(
            &'b self,
            out: &mut diesel::serialize::Output<'b, '_, DB>,
        ) -> diesel::serialize::Result {
            out.set_value(i32::try_from(self.0)?);
            Ok(diesel::serialize::IsNull::No)
        }
    }

    impl<DB> diesel::deserialize::FromSql<diesel::sql_types::Integer, DB> for Iteration
    where
        DB: diesel::backend::Backend,
        i32: diesel::deserialize::FromSql<diesel::sql_types::Integer, DB>,
    {
        fn from_sql(bytes: DB::RawValue<'_>) -> diesel::deserialize::Result<Self> {
            Ok(Self(u32::try_from(i32::from_sql(bytes)?)?))
        }
    }
}

#[cfg(feature = "table")]
pub mod table {
    use std::fmt;

    use bencher_valid::GitHash;
    use ordered_float::OrderedFloat;
    use tabled::{Table, Tabled};

    use crate::{
        project::branch::VersionNumber, DateTime, JsonBenchmark, JsonBranch, JsonMetric,
        JsonMetricKind, JsonPerf, JsonProject, JsonTestbed,
    };

    use super::Iteration;

    impl From<JsonPerf> for Table {
        fn from(json_perf: JsonPerf) -> Self {
            let mut perf_table = Vec::new();
            for result in json_perf.results {
                for metric in result.metrics {
                    perf_table.push(PerfTable {
                        project: json_perf.project.clone(),
                        metric_kind: result.metric_kind.clone(),
                        branch: result.branch.clone(),
                        testbed: result.testbed.clone(),
                        benchmark: result.benchmark.clone(),
                        iteration: metric.iteration,
                        start_time: metric.start_time,
                        end_time: metric.end_time,
                        version_number: metric.version.number,
                        version_hash: DisplayOption(metric.version.hash),
                        metric: metric.metric,
                        lower_limit: DisplayOption(metric.boundary.lower_limit),
                        upper_limit: DisplayOption(metric.boundary.upper_limit),
                    });
                }
            }
            Self::new(perf_table)
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
        pub iteration: Iteration,
        #[tabled(rename = "Start Time")]
        pub start_time: DateTime,
        #[tabled(rename = "End Time")]
        pub end_time: DateTime,
        #[tabled(rename = "Version Number")]
        pub version_number: VersionNumber,
        #[tabled(rename = "Version Hash")]
        pub version_hash: DisplayOption<GitHash>,
        #[tabled(rename = "Metric Value")]
        pub metric: JsonMetric,
        #[tabled(rename = "Lower Boundary Limit")]
        pub lower_limit: DisplayOption<OrderedFloat<f64>>,
        #[tabled(rename = "Upper Boundary Limit")]
        pub upper_limit: DisplayOption<OrderedFloat<f64>>,
    }

    pub struct DisplayOption<T>(Option<T>);

    impl<T> fmt::Display for DisplayOption<T>
    where
        T: fmt::Display,
    {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            if let Some(t) = &self.0 {
                write!(f, "{t}")
            } else {
                write!(f, "")
            }
        }
    }
}
