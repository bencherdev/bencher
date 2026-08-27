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
    /// A grid point is queried when at least one of them is a subset of its
    /// parameter set: every key the filter names, with the same value.
    /// Leaving this off queries every grid point.
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
    /// An optional comma separated list of URL encoded parameter sets to filter on.
    /// A grid point is queried when at least one of them is a subset of its
    /// parameter set: every key the filter names, with the same value.
    /// Leaving this off queries every grid point.
    pub parameters: Option<String>,
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
            parameters,
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
            parameters,
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
    /// The parameters filter, OR across its elements. Empty matches every grid point.
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
        // An absent filter and an empty one both mean the same thing: no filter.
        // Spelling it out here keeps the empty string out of the list reader, which
        // rejects an empty element.
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

    /// The parameters filter, or nothing at all when there is no filter.
    ///
    /// A parameter set spells commas, so its elements are encoded with the separator
    /// escaped rather than left literal the way a UUID may be.
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
    pub results: Vec<JsonPerfMetrics>,
}

/// One line of a perf query: one grid point of one benchmark, on one branch, one
/// testbed, and one measure.
///
/// The parameter set sits between the benchmark and the measure because a line is
/// what a grid point plots: two parameter sets of one benchmark are two lines, not
/// one line with the points interleaved.
#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonPerfMetrics {
    pub branch: JsonBranch,
    pub testbed: JsonTestbed,
    pub benchmark: JsonBenchmark,
    /// The parameter set this line plots.
    pub parameter: JsonParameter,
    pub measure: JsonMeasure,
    pub metrics: Vec<JsonPerfMetric>,
}

/// One point of a perf line: everything one measure of one grid point measured, in
/// one iteration of one report.
#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonPerfMetric {
    pub report: ReportUuid,
    pub iteration: Iteration,
    pub start_time: DateTime,
    pub end_time: DateTime,
    pub version: JsonVersion,
    /// Every named scalar this measure ingested, keyed by name.
    #[typeshare(typescript(type = "Record<string, JsonMetricEntry>"))]
    pub metrics: BTreeMap<MetricName, JsonMetricEntry>,

    /// Deprecated. Reconstructed from the `value` row and its
    /// `lower_value`/`upper_value` siblings. Retained for compatibility with older
    /// clients and removed in a future release.
    ///
    /// Absent when the measure carries no `value` name, which BMF v1 permits. Never
    /// absent for anything an older client could produce.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metric: Option<JsonMetricTriple>,
    /// Deprecated. The threshold that gated the `value` row, if any.
    // The threshold model is necessary for each metric as it may change over time
    pub threshold: Option<JsonThresholdModel>,
    /// Deprecated. The boundary computed for the `value` row, if any.
    pub boundary: Option<JsonBoundary>,
    /// Deprecated. The alert raised on the `value` row's boundary, if any.
    pub alert: Option<JsonPerfAlert>,
}

/// Exactly one `metric` row: one named scalar and every threshold that gated it.
#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonMetricEntry {
    pub value: OrderedFloat<f64>,
    /// Every threshold that gated this named scalar, with the boundary it produced
    /// and any alert that boundary raised. Absent when nothing gated it.
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
    use tabled::{
        Table, Tabled,
        settings::{Remove, location::ByColumnName},
    };

    use crate::{
        DateTime, JsonBenchmark, JsonBranch, JsonMeasure, JsonMetricTriple, JsonPerf, JsonProject,
        JsonTestbed,
        project::{head::VersionNumber, report::Iteration},
    };

    /// The header of the column that names each line's grid point.
    ///
    /// A project that never reported a parameter set has nothing to tell its lines
    /// apart by, so the column is removed rather than filled with `{}`, and this is
    /// what names it for removal. The `tabled` rename attribute takes a literal, so
    /// the field below spells the same string out.
    const PARAMETERS: &str = "Parameters";

    impl From<JsonPerf> for Table {
        fn from(json_perf: JsonPerf) -> Self {
            // One non-empty set anywhere in the query is what makes the column worth
            // a column: without one, every line is the benchmark's only grid point.
            let grid = json_perf
                .results
                .iter()
                .any(|result| !result.parameter.set.is_empty());

            let mut perf_table = Vec::new();
            for result in json_perf.results {
                let parameters = result.parameter.set.canonical();
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
                        parameters: parameters.clone(),
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
            let mut table = Self::new(perf_table);
            if !grid {
                table.with(Remove::column(ByColumnName::new(PARAMETERS)));
            }
            table
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
        /// The canonical spelling of the grid point this line plots.
        ///
        /// The column sits between the benchmark and the measure, the way the
        /// parameter set sits between them in a perf result, and it is removed
        /// entirely when no line of the query plots a non-empty set.
        #[tabled(rename = "Parameters")]
        pub parameters: String,
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

    #[cfg(test)]
    mod tests {
        use tabled::Table;

        use crate::JsonPerf;

        /// A one line perf query whose benchmark plots `set`.
        fn line(benchmark: &str, set: &str, value: f64) -> String {
            format!(
                r#"{{
                    "branch": {{
                        "uuid": "7d7e73de-78c2-43f7-bc2a-da31a5b9a819",
                        "project": "c7fd3581-73d1-443c-b30f-6aa5c1c516cf",
                        "name": "master",
                        "slug": "master",
                        "head": {{
                            "uuid": "7d7e73de-78c2-43f7-bc2a-da31a5b9a819",
                            "start_point": null,
                            "version": null,
                            "created": "2023-07-02T12:53:33Z",
                            "replaced": null
                        }},
                        "created": "2023-07-02T12:53:33Z",
                        "modified": "2023-07-02T12:53:33Z"
                    }},
                    "testbed": {{
                        "uuid": "e095df48-52a6-474b-aaa7-1a8546c235b6",
                        "project": "c7fd3581-73d1-443c-b30f-6aa5c1c516cf",
                        "name": "base",
                        "slug": "base",
                        "created": "2023-07-02T12:53:33Z",
                        "modified": "2023-07-02T12:53:33Z"
                    }},
                    "benchmark": {{
                        "uuid": "dbb90f5c-e7e2-438c-9533-ce86792174ee",
                        "project": "c7fd3581-73d1-443c-b30f-6aa5c1c516cf",
                        "name": "{benchmark}",
                        "slug": "dbb90f5c-e7e2-438c-9533-ce86792174ee",
                        "created": "2023-07-02T12:53:33Z",
                        "modified": "2023-07-02T12:53:33Z"
                    }},
                    "parameter": {{
                        "uuid": "b23b1a5e-0f4f-4b8a-9a35-2b7a2f5f0a2f",
                        "benchmark": "dbb90f5c-e7e2-438c-9533-ce86792174ee",
                        "set": {set},
                        "created": "2023-07-02T12:53:33Z",
                        "modified": "2023-07-02T12:53:33Z",
                        "archived": null
                    }},
                    "measure": {{
                        "uuid": "61a385d0-f19d-4f20-895a-e3c684ec6cbc",
                        "project": "c7fd3581-73d1-443c-b30f-6aa5c1c516cf",
                        "name": "Latency",
                        "slug": "latency",
                        "units": "nanoseconds (ns)",
                        "created": "2023-07-02T12:53:33Z",
                        "modified": "2023-07-02T12:53:33Z"
                    }},
                    "metrics": [
                        {{
                            "report": "ef582192-c7f4-47a0-8668-55cf7d99d8cc",
                            "iteration": 0,
                            "start_time": "2023-07-02T12:53:33Z",
                            "end_time": "2023-07-02T12:53:33Z",
                            "version": {{ "number": 0, "hash": null }},
                            "threshold": null,
                            "boundary": null,
                            "alert": null,
                            "metrics": {{ "value": {{ "value": {value} }} }},
                            "metric": {{
                                "uuid": "00000000-0000-0000-0000-000000000000",
                                "value": {value},
                                "lower_value": null,
                                "upper_value": null
                            }}
                        }}
                    ]
                }}"#
            )
        }

        fn json_perf(lines: &[String]) -> JsonPerf {
            let results = lines.join(",");
            serde_json::from_str(&format!(
                r#"{{
                    "project": {{
                        "uuid": "c7fd3581-73d1-443c-b30f-6aa5c1c516cf",
                        "organization": "4142ce9a-f0a0-44d5-94cd-fc76c77d9098",
                        "name": "The Computer",
                        "slug": "the-computer",
                        "url": null,
                        "visibility": "public",
                        "bmf_version": 0,
                        "created": "2023-07-02T12:53:33Z",
                        "modified": "2023-07-02T12:53:33Z"
                    }},
                    "start_time": null,
                    "end_time": null,
                    "results": [{results}]
                }}"#
            ))
            .expect("Failed to parse perf JSON")
        }

        fn table(lines: &[String]) -> String {
            Table::from(json_perf(lines)).to_string()
        }

        // A query whose every line plots the empty parameter set is the query every
        // project made before a benchmark could have more than one grid point. It
        // prints exactly the table it always printed, column for column.
        #[test]
        fn table_without_grid_points() {
            let table = table(&[line("bencher::mock_0", "{}", 7.0)]);
            assert_eq!(
                table,
                concat!(
                    "+--------------+--------+---------+-----------------+---------------------------+-----------+-------------------------+-------------------------+----------------+--------------+--------------+-------------------+----------------------+----------------------+\n",
                    "| Project      | Branch | Testbed | Benchmark       | Measure                   | Iteration | Start Time              | End Time                | Version Number | Version Hash | Metric Value | Boundary Baseline | Lower Boundary Limit | Upper Boundary Limit |\n",
                    "+--------------+--------+---------+-----------------+---------------------------+-----------+-------------------------+-------------------------+----------------+--------------+--------------+-------------------+----------------------+----------------------+\n",
                    "| The Computer | master | base    | bencher::mock_0 | Latency: nanoseconds (ns) | 0         | 2023-07-02 12:53:33 UTC | 2023-07-02 12:53:33 UTC | 0              |              | 7            |                   |                      |                      |\n",
                    "+--------------+--------+---------+-----------------+---------------------------+-----------+-------------------------+-------------------------+----------------+--------------+--------------+-------------------+----------------------+----------------------+",
                ),
                "the table a project without grid points prints"
            );
        }

        // One non-empty set anywhere in the query earns the column, and every line
        // spells the set it plots, the empty set among them as `{}`.
        #[test]
        fn table_with_grid_points() {
            let table = table(&[
                line("bencher::mock_0", "{}", 7.0),
                line("bencher::mock_0", r#"{"size_mb": 16}"#, 8.0),
            ]);
            assert!(table.contains("Parameters"), "unexpected table: {table}");
            assert!(table.contains("| {} "), "unexpected table: {table}");
            assert!(
                table.contains(r#"| {"size_mb":16} "#),
                "unexpected table: {table}"
            );
        }
    }
}
