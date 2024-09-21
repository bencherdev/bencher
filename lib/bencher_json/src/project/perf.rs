#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::ser::{self, SerializeStruct};
use serde::{Deserialize, Serialize, Serializer};
use url::Url;

use crate::urlencoded::{
    from_urlencoded_list, from_urlencoded_nullable_list, to_urlencoded, to_urlencoded_list,
    to_urlencoded_optional_list, UrlEncodedError,
};
use crate::{
    BenchmarkUuid, BranchUuid, DateTime, DateTimeMillis, JsonBenchmark, JsonBranch, JsonMeasure,
    JsonProject, JsonTestbed, MeasureUuid, ReferenceUuid, ReportUuid, TestbedUuid,
};

use super::alert::JsonPerfAlert;
use super::boundary::JsonBoundary;
use super::metric::JsonMetric;
use super::reference::JsonVersion;
use super::report::Iteration;
use super::threshold::JsonThresholdModel;

crate::typed_uuid::typed_uuid!(ReportBenchmarkUuid);

/// `JsonPerfQueryParams` is the actual query parameters accepted by the server.
/// All query parameter values are therefore scalar values.
/// Arrays are represented as comma separated lists.
/// Optional date times are simply stored as their millisecond representation.
/// `JsonPerfQueryParams` should always be converted into `JsonPerfQuery` for full type level validation.
#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonPerfQueryParams {
    /// A comma separated list of branch UUIDs to query.
    pub branches: String,
    /// An optional comma separated list of branch head reference UUIDs.
    /// To not specify a particular branch head leave an empty entry in the list.
    pub heads: Option<String>,
    /// A comma separated list of testbed UUIDs to query.
    pub testbeds: String,
    /// A comma separated list of benchmark UUIDs to query.
    pub benchmarks: String,
    /// A comma separated list of measure UUIDs to query.
    pub measures: String,
    /// Search for metrics after the given date time in milliseconds.
    pub start_time: Option<DateTimeMillis>,
    /// Search for metrics before the given date time in milliseconds.
    pub end_time: Option<DateTimeMillis>,
}

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonPerfImgQueryParams {
    /// The title for the perf plot.
    /// If not provided, the project name will be used.
    pub title: Option<String>,
    /// A comma separated list of branch UUIDs to query.
    pub branches: String,
    /// An optional comma separated list of branch head reference UUIDs.
    /// To not specify a particular branch head leave an empty entry in the list.
    pub heads: Option<String>,
    /// A comma separated list of testbed UUIDs to query.
    pub testbeds: String,
    /// A comma separated list of benchmark UUIDs to query.
    pub benchmarks: String,
    /// A comma separated list of measure UUIDs to query.
    pub measures: String,
    /// Search for metrics after the given date time in milliseconds.
    pub start_time: Option<DateTimeMillis>,
    /// Search for metrics before the given date time in milliseconds.
    pub end_time: Option<DateTimeMillis>,
}

impl From<JsonPerfImgQueryParams> for JsonPerfQueryParams {
    fn from(query: JsonPerfImgQueryParams) -> Self {
        let JsonPerfImgQueryParams {
            title: _,
            branches,
            heads,
            testbeds,
            benchmarks,
            measures,
            start_time,
            end_time,
        } = query;
        Self {
            branches,
            heads,
            testbeds,
            benchmarks,
            measures,
            start_time,
            end_time,
        }
    }
}

/// `JsonPerfQuery` is the full, strongly typed version of `JsonPerfQueryParams`.
/// It should always be used to validate `JsonPerfQueryParams`.
#[typeshare::typeshare]
#[derive(Debug, Clone)]
pub struct JsonPerfQuery {
    pub branches: Vec<BranchUuid>,
    pub heads: Vec<Option<ReferenceUuid>>,
    pub testbeds: Vec<TestbedUuid>,
    pub benchmarks: Vec<BenchmarkUuid>,
    pub measures: Vec<MeasureUuid>,
    pub start_time: Option<DateTime>,
    pub end_time: Option<DateTime>,
}

impl TryFrom<JsonPerfQueryParams> for JsonPerfQuery {
    type Error = UrlEncodedError;

    fn try_from(query_params: JsonPerfQueryParams) -> Result<Self, Self::Error> {
        let JsonPerfQueryParams {
            branches,
            heads,
            testbeds,
            benchmarks,
            measures,
            start_time,
            end_time,
        } = query_params;

        if branches.is_empty() {
            return Err(UrlEncodedError::EmptyBranches);
        }
        if testbeds.is_empty() {
            return Err(UrlEncodedError::EmptyTestbeds);
        }
        if benchmarks.is_empty() {
            return Err(UrlEncodedError::EmptyBenchmarks);
        }
        if measures.is_empty() {
            return Err(UrlEncodedError::EmptyMeasures);
        }

        let branches = from_urlencoded_list(&branches)?;
        let heads = from_urlencoded_nullable_list(heads.as_deref())?;
        let testbeds = from_urlencoded_list(&testbeds)?;
        let benchmarks = from_urlencoded_list(&benchmarks)?;
        let measures = from_urlencoded_list(&measures)?;

        // Guarantee that the `heads` array is the same length as the `branches` array.
        let heads = pad_heads_to_branches(branches.len(), &heads);

        Ok(Self {
            branches,
            heads,
            testbeds,
            benchmarks,
            measures,
            start_time: start_time.map(Into::into),
            end_time: end_time.map(Into::into),
        })
    }
}

// Guarantee that the `heads` array is the same length as the `branches` array.
// It is okay for their to be less heads than branches.
// They will just be set to `None`.
// But there should never be more heads than branches.
// Those extra heads will just be ignored.
fn pad_heads_to_branches(
    branches_len: usize,
    heads: &[Option<ReferenceUuid>],
) -> Vec<Option<ReferenceUuid>> {
    let mut branch_heads = Vec::with_capacity(branches_len);
    for i in 0..branches_len {
        branch_heads.push(heads.get(i).copied().flatten());
    }
    branch_heads
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
        console_url: &str,
        path: &str,
        query: &[(&str, Option<String>)],
    ) -> Result<Url, UrlEncodedError> {
        let mut url = Url::parse(console_url)?;
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

    fn urlencoded(&self) -> Result<[(&'static str, Option<String>); 7], UrlEncodedError> {
        QUERY_KEYS
            .into_iter()
            .zip([
                Some(self.branches()),
                self.heads(),
                Some(self.testbeds()),
                Some(self.benchmarks()),
                Some(self.measures()),
                self.start_time_str(),
                self.end_time_str(),
            ])
            .collect::<Vec<_>>()
            .try_into()
            .map_err(UrlEncodedError::Vec)
    }

    pub fn branches(&self) -> String {
        to_urlencoded_list(&self.branches)
    }

    pub fn heads(&self) -> Option<String> {
        if self.heads.is_empty() {
            None
        } else {
            Some(to_urlencoded_optional_list(&self.heads))
        }
    }

    pub fn testbeds(&self) -> String {
        to_urlencoded_list(&self.testbeds)
    }

    pub fn benchmarks(&self) -> String {
        to_urlencoded_list(&self.benchmarks)
    }

    pub fn measures(&self) -> String {
        to_urlencoded_list(&self.measures)
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
    Branches,
    Heads,
    Testbeds,
    Benchmarks,
    Measures,
    StartTime,
    EndTime,
}

pub const BRANCHES: &str = "branches";
pub const HEADS: &str = "heads";
pub const TESTBEDS: &str = "testbeds";
pub const BENCHMARKS: &str = "benchmarks";
pub const MEASURES: &str = "measures";
pub const START_TIME: &str = "start_time";
pub const END_TIME: &str = "end_time";
const QUERY_KEYS: [&str; 7] = [
    BRANCHES, HEADS, TESTBEDS, BENCHMARKS, MEASURES, START_TIME, END_TIME,
];

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
    pub branch: JsonBranch,
    pub testbed: JsonTestbed,
    pub benchmark: JsonBenchmark,
    pub measure: JsonMeasure,
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
    pub metric: JsonMetric,
    // The threshold model is necessary for each metric as it may change over time
    pub threshold: Option<JsonThresholdModel>,
    pub boundary: Option<JsonBoundary>,
    pub alert: Option<JsonPerfAlert>,
}

#[cfg(feature = "table")]
pub mod table {
    use std::fmt;

    use bencher_valid::GitHash;
    use ordered_float::OrderedFloat;
    use tabled::{Table, Tabled};

    use crate::{
        project::{reference::VersionNumber, report::Iteration},
        DateTime, JsonBenchmark, JsonBranch, JsonMeasure, JsonMetric, JsonPerf, JsonProject,
        JsonTestbed,
    };

    impl From<JsonPerf> for Table {
        fn from(json_perf: JsonPerf) -> Self {
            let mut perf_table = Vec::new();
            for result in json_perf.results {
                for metric in result.metrics {
                    let (baseline, lower_limit, upper_limit) =
                        if let Some(boundary) = metric.boundary {
                            (
                                DisplayOption(boundary.baseline),
                                DisplayOption(boundary.lower_limit),
                                DisplayOption(boundary.upper_limit),
                            )
                        } else {
                            (
                                DisplayOption::default(),
                                DisplayOption::default(),
                                DisplayOption::default(),
                            )
                        };
                    perf_table.push(PerfTable {
                        project: json_perf.project.clone(),
                        branch: result.branch.clone(),
                        testbed: result.testbed.clone(),
                        benchmark: result.benchmark.clone(),
                        measure: result.measure.clone(),
                        iteration: metric.iteration,
                        start_time: metric.start_time,
                        end_time: metric.end_time,
                        version_number: metric.version.number,
                        version_hash: DisplayOption(metric.version.hash),
                        metric: metric.metric,
                        baseline,
                        lower_limit,
                        upper_limit,
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
        #[tabled(rename = "Branch")]
        pub branch: JsonBranch,
        #[tabled(rename = "Testbed")]
        pub testbed: JsonTestbed,
        #[tabled(rename = "Benchmark")]
        pub benchmark: JsonBenchmark,
        #[tabled(rename = "Measure")]
        pub measure: JsonMeasure,
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
        #[tabled(rename = "Boundary Baseline")]
        pub baseline: DisplayOption<OrderedFloat<f64>>,
        #[tabled(rename = "Lower Boundary Limit")]
        pub lower_limit: DisplayOption<OrderedFloat<f64>>,
        #[tabled(rename = "Upper Boundary Limit")]
        pub upper_limit: DisplayOption<OrderedFloat<f64>>,
    }

    #[derive(Default)]
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
