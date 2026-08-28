use std::collections::BTreeMap;

use bencher_valid::MetricName;
use ordered_float::OrderedFloat;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::ser::{self, SerializeStruct as _};
use serde::{Deserialize, Serialize, Serializer};
use url::Url;

#[cfg(feature = "plus")]
use crate::SpecUuid;
use crate::urlencoded::{
    UrlEncodedError, from_urlencoded_list, from_urlencoded_nullable_list, to_urlencoded,
    to_urlencoded_element_list, to_urlencoded_list, to_urlencoded_optional_list,
};
use crate::{
    BenchmarkUuid, BranchUuid, DateTime, DateTimeMillis, HeadUuid, JsonBenchmark, JsonBranch,
    JsonMeasure, JsonParameter, JsonProject, JsonTestbed, MeasureUuid, ParameterSet, ReportUuid,
    TestbedUuid,
};

use super::alert::JsonPerfAlert;
use super::boundary::JsonBoundary;
use super::head::JsonVersion;
use super::metric::JsonMetricTriple;
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
    /// An optional comma separated list of branch head UUIDs.
    /// To not specify a particular branch head leave an empty entry in the list.
    pub heads: Option<String>,
    /// A comma separated list of testbed UUIDs to query.
    pub testbeds: String,
    /// An optional comma separated list of testbed spec UUIDs.
    /// To not specify a particular testbed spec leave an empty entry in the list.
    pub specs: Option<String>,
    /// A comma separated list of benchmark UUIDs to query.
    pub benchmarks: String,
    /// An optional comma separated list of URL encoded parameter sets to filter on.
    /// A variant is queried when at least one of them is a subset of its
    /// parameter set: every key the filter names, with the same value.
    /// Leaving this off queries every variant.
    pub parameters: Option<String>,
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
    /// An optional comma separated list of branch head UUIDs.
    /// To not specify a particular branch head leave an empty entry in the list.
    pub heads: Option<String>,
    /// A comma separated list of testbed UUIDs to query.
    pub testbeds: String,
    /// An optional comma separated list of testbed spec UUIDs.
    /// To not specify a particular testbed spec leave an empty entry in the list.
    pub specs: Option<String>,
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
            specs,
            benchmarks,
            measures,
            start_time,
            end_time,
        } = query;
        Self {
            branches,
            heads,
            testbeds,
            specs,
            benchmarks,
            // No filter, so every variant is plotted.
            parameters: None,
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
    pub heads: Vec<Option<HeadUuid>>,
    pub testbeds: Vec<TestbedUuid>,
    #[cfg(feature = "plus")]
    pub specs: Vec<Option<SpecUuid>>,
    pub benchmarks: Vec<BenchmarkUuid>,
    /// The parameters filter, OR across its elements. Empty matches every variant.
    pub parameters: Vec<ParameterSet>,
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
            specs,
            benchmarks,
            parameters,
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
        // An empty string is not a list of one empty element, it is no filter.
        let parameters = match parameters.as_deref() {
            Some(parameters) if !parameters.is_empty() => from_urlencoded_list(parameters)?,
            _ => Vec::new(),
        };
        let measures = from_urlencoded_list(&measures)?;

        // Guarantee that the `heads` array is the same length as the `branches` array.
        let heads = size_heads_to_branches(&branches, &heads);

        // Guarantee that the `specs` array is the same length as the `testbeds` array.
        #[cfg(feature = "plus")]
        let specs = {
            let specs = from_urlencoded_nullable_list(specs.as_deref())?;
            size_specs_to_testbeds(&testbeds, &specs)
        };
        #[cfg(not(feature = "plus"))]
        let _specs = specs;

        Ok(Self {
            branches,
            heads,
            testbeds,
            #[cfg(feature = "plus")]
            specs,
            benchmarks,
            parameters,
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
fn size_heads_to_branches(
    branches: &[BranchUuid],
    heads: &[Option<HeadUuid>],
) -> Vec<Option<HeadUuid>> {
    (0..branches.len())
        .map(|i| heads.get(i).copied().flatten())
        .collect()
}

// Guarantee that the `specs` array is the same length as the `testbeds` array.
// It is okay for there to be less specs than testbeds.
// They will just be set to `None`.
// But there should never be more specs than testbeds.
// Those extra specs will just be ignored.
#[cfg(feature = "plus")]
fn size_specs_to_testbeds(
    testbeds: &[TestbedUuid],
    specs: &[Option<SpecUuid>],
) -> Vec<Option<SpecUuid>> {
    (0..testbeds.len())
        .map(|i| specs.get(i).copied().flatten())
        .collect()
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

    fn urlencoded(&self) -> Result<[(&'static str, Option<String>); 9], UrlEncodedError> {
        QUERY_KEYS
            .into_iter()
            .zip([
                Some(self.branches()),
                self.heads(),
                Some(self.testbeds()),
                self.specs(),
                Some(self.benchmarks()),
                self.parameters(),
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

    pub fn specs(&self) -> Option<String> {
        #[cfg(feature = "plus")]
        {
            if self.specs.is_empty() {
                None
            } else {
                Some(to_urlencoded_optional_list(&self.specs))
            }
        }
        #[cfg(not(feature = "plus"))]
        {
            None
        }
    }

    pub fn testbeds(&self) -> String {
        to_urlencoded_list(&self.testbeds)
    }

    pub fn benchmarks(&self) -> String {
        to_urlencoded_list(&self.benchmarks)
    }

    /// A parameter set spells commas, so its elements are encoded with the
    /// separator escaped rather than left literal the way a UUID may be.
    pub fn parameters(&self) -> Option<String> {
        if self.parameters.is_empty() {
            None
        } else {
            Some(to_urlencoded_element_list(&self.parameters))
        }
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
    Specs,
    Benchmarks,
    Parameters,
    Measures,
    StartTime,
    EndTime,
}

pub const BRANCHES: &str = "branches";
pub const HEADS: &str = "heads";
pub const TESTBEDS: &str = "testbeds";
pub const SPECS: &str = "specs";
pub const BENCHMARKS: &str = "benchmarks";
pub const PARAMETERS: &str = "parameters";
pub const MEASURES: &str = "measures";
pub const START_TIME: &str = "start_time";
pub const END_TIME: &str = "end_time";
const QUERY_KEYS: [&str; 9] = [
    BRANCHES, HEADS, TESTBEDS, SPECS, BENCHMARKS, PARAMETERS, MEASURES, START_TIME, END_TIME,
];

#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonPerf {
    pub project: JsonProject,
    pub start_time: Option<DateTime>,
    pub end_time: Option<DateTime>,
    pub results: Vec<JsonPerfLine>,
}

/// One line of a perf query: one variant of one benchmark, on one branch, one
/// testbed, and one measure.
#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonPerfLine {
    pub branch: JsonBranch,
    pub testbed: JsonTestbed,
    pub benchmark: JsonBenchmark,
    /// The parameter set this line plots.
    pub parameter: JsonParameter,
    pub measure: JsonMeasure,
    pub metrics: Vec<JsonPerfMetrics>,
}

/// One point of a perf line: everything one measure of one variant measured, in
/// one iteration of one report.
#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonPerfMetrics {
    pub report: ReportUuid,
    pub iteration: Iteration,
    pub start_time: DateTime,
    pub end_time: DateTime,
    pub version: JsonVersion,
    /// Every metric this measure ingested, keyed by name.
    #[typeshare(typescript(type = "Record<string, JsonMetricEntry>"))]
    pub metrics: BTreeMap<MetricName, JsonMetricEntry>,

    /// Deprecated. The metric triple, reconstructed from the `value` row and its
    /// `lower_value`/`upper_value` siblings. Retained for compatibility with older
    /// clients and removed in a future release.
    ///
    /// Absent when the measure carries no `value` name, which BMF v1 permits. Never
    /// absent for anything an older client could produce.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metric: Option<JsonMetricTriple>,
    /// Deprecated. The threshold that checked the `value` row, if any.
    // The threshold model is necessary for each metric as it may change over time
    pub threshold: Option<JsonThresholdModel>,
    /// Deprecated. The boundary computed for the `value` row, if any.
    pub boundary: Option<JsonBoundary>,
    /// Deprecated. The alert raised on the `value` row's boundary, if any.
    pub alert: Option<JsonPerfAlert>,
}

/// Exactly one `metric` row: the metric's value and every threshold that checked it.
#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonMetricEntry {
    pub value: OrderedFloat<f64>,
    /// Every threshold that checked this metric, with the boundary it produced
    /// and any alert that boundary raised. Absent when nothing checked it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub boundaries: Option<Vec<JsonPerfBoundary>>,
}

/// A threshold and the boundary it produced, with any alert that boundary raised.
#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonPerfBoundary {
    pub threshold: JsonThresholdModel,
    pub boundary: JsonBoundary,
    pub alert: Option<JsonPerfAlert>,
}

#[cfg(feature = "table")]
pub mod table {
    use std::fmt;

    use bencher_valid::GitHash;
    use ordered_float::OrderedFloat;
    use tabled::{Table, Tabled};

    use crate::{
        DateTime, JsonBenchmark, JsonBranch, JsonMeasure, JsonMetricTriple, JsonPerf, JsonProject,
        JsonTestbed,
        project::{head::VersionNumber, report::Iteration},
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
                        metric: DisplayOption(metric.metric),
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
        pub metric: DisplayOption<JsonMetricTriple>,
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
