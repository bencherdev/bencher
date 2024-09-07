use std::fmt;

use bencher_valid::{DateTime, DateTimeMillis, GitHash};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    urlencoded::{from_urlencoded, to_urlencoded, UrlEncodedError},
    JsonAlert, JsonBenchmark, JsonBoundary, JsonMeasure, JsonMetric, JsonProject, JsonTestbed,
    JsonUser, NameId,
};

use super::{
    branch::{JsonBranchVersion, JsonNewStartPoint},
    threshold::JsonThresholdModel,
};

crate::typed_uuid::typed_uuid!(ReportUuid);

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonNewReport {
    /// Branch UUID, slug, or name.
    /// If the branch does not exist, it will be created.
    pub branch: NameId,
    /// Full `git` commit hash.
    /// All reports with the same `git` commit hash will be considered part of the same branch version.
    /// This can be useful for tracking the performance of a specific commit across multiple testbeds.
    pub hash: Option<GitHash>,
    /// The start point for the report branch.
    /// If the branch already exists and the start point is not provided, the current branch will be used.
    /// If the branch already exists and the start point provided is different, a new branch will be created from the new start point.
    /// If the branch does not exist, the start point will be used to create a new branch.
    /// If a new branch is created with a start point,
    /// all branch versions from the start point branch will be shallow copied over to the new branch.
    /// That is, all historical metrics data for the start point branch will appear in queries for the branch.
    /// For example, pull request branches often use their base branch as their start point branch.
    /// If a new branch is created, it is not kept in sync with the start point branch.
    pub start_point: Option<JsonReportStartPoint>,
    /// Testbed UUID, slug, or name.
    /// If the testbed does not exist, it will be created.
    pub testbed: NameId,
    /// Start time for the report. Must be an ISO 8601 formatted string.
    pub start_time: DateTime,
    /// End time for the report. Must be an ISO 8601 formatted string.
    pub end_time: DateTime,
    /// An array of benchmarks results.
    pub results: Vec<String>,
    /// Settings for how to handle the report.
    pub settings: Option<JsonReportSettings>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonReportStartPoint {
    /// The UUID, slug, or name of the branch to use as the start point.
    pub branch: Option<NameId>,
    /// The full git hash of the branch to use as the start point.
    /// Requires the `branch` field to be set.
    pub hash: Option<GitHash>,
    /// If set to `true`, the thresholds from the start point branch will be deep copied to the new branch.
    /// This can be useful for pull request branches that should have the same thresholds as their target branch.
    /// Requires the `branch` field to be set.
    pub thresholds: Option<bool>,
    /// Reset the branch to an empty state.
    /// If the branch already exists, a new empty branch will be created.
    /// If a start point is provided, the new branch will begin at that start point.
    pub reset: Option<bool>,
}

impl JsonReportStartPoint {
    pub fn to_new_start_point(&self) -> Option<JsonNewStartPoint> {
        let JsonReportStartPoint {
            branch,
            hash,
            thresholds,
            reset: _,
        } = self;
        Some(JsonNewStartPoint {
            branch: branch.clone()?,
            hash: hash.clone(),
            thresholds: *thresholds,
        })
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonReportSettings {
    /// The benchmark harness adapter for parsing the benchmark results.
    /// If no adapter is specified, then the Magic adapter will be used.
    pub adapter: Option<Adapter>,
    /// Benchmark harness suggested central tendency (ie average).
    /// Some benchmarking harnesses provide multiple averages, such as mean and median.
    pub average: Option<JsonAverage>,
    /// Fold multiple results into a single result using the selected operation.
    /// This can be useful for taking the min, max, mean, or median of the benchmark results.
    pub fold: Option<JsonFold>,
}

const MAGIC_INT: i32 = 0;
const JSON_INT: i32 = 10;
const RUST_INT: i32 = 20;
const RUST_BENCH_INT: i32 = 21;
const RUST_CRITERION_INT: i32 = 22;
const RUST_IAI_INT: i32 = 23;
const RUST_IAI_CALLGRIND_INT: i32 = 24;
const CPP_INT: i32 = 30;
const CPP_GOOGLE_INT: i32 = 31;
const CPP_CATCH2_INT: i32 = 32;
const GO_INT: i32 = 40;
const GO_BENCH_INT: i32 = 41;
const JAVA_INT: i32 = 50;
const JAVA_JMH_INT: i32 = 51;
const C_SHARP_INT: i32 = 60;
const C_SHARP_DOT_NET_INT: i32 = 61;
const JS_INT: i32 = 70;
const JS_BENCHMARK_INT: i32 = 71;
const JS_TIME_INT: i32 = 72;
const PYTHON_INT: i32 = 80;
const PYTHON_ASV_INT: i32 = 81;
const PYTHON_PYTEST_INT: i32 = 82;
const RUBY_INT: i32 = 90;
const RUBY_BENCHMARK_INT: i32 = 91;
const SHELL_INT: i32 = 100;
const SHELL_HYPERFINE_INT: i32 = 101;

#[typeshare::typeshare]
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Integer))]
#[serde(rename_all = "snake_case")]
#[repr(i32)]
pub enum Adapter {
    #[default]
    Magic = MAGIC_INT,
    Json = JSON_INT,
    Rust = RUST_INT,
    RustBench = RUST_BENCH_INT,
    RustCriterion = RUST_CRITERION_INT,
    RustIai = RUST_IAI_INT,
    RustIaiCallgrind = RUST_IAI_CALLGRIND_INT,
    Cpp = CPP_INT,
    CppGoogle = CPP_GOOGLE_INT,
    CppCatch2 = CPP_CATCH2_INT,
    Go = GO_INT,
    GoBench = GO_BENCH_INT,
    Java = JAVA_INT,
    JavaJmh = JAVA_JMH_INT,
    CSharp = C_SHARP_INT,
    CSharpDotNet = C_SHARP_DOT_NET_INT,
    Js = JS_INT,
    JsBenchmark = JS_BENCHMARK_INT,
    JsTime = JS_TIME_INT,
    Python = PYTHON_INT,
    PythonAsv = PYTHON_ASV_INT,
    PythonPytest = PYTHON_PYTEST_INT,
    Ruby = RUBY_INT,
    RubyBenchmark = RUBY_BENCHMARK_INT,
    Shell = SHELL_INT,
    ShellHyperfine = SHELL_HYPERFINE_INT,
}

impl fmt::Display for Adapter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Magic => write!(f, "magic"),
            Self::Json => write!(f, "json"),
            Self::Rust => write!(f, "rust"),
            Self::RustBench => write!(f, "rust_bench"),
            Self::RustCriterion => write!(f, "rust_criterion"),
            Self::RustIai => write!(f, "rust_iai"),
            Self::RustIaiCallgrind => write!(f, "rust_iai_callgrind"),
            Self::Cpp => write!(f, "cpp"),
            Self::CppGoogle => write!(f, "cpp_google"),
            Self::CppCatch2 => write!(f, "cpp_catch2"),
            Self::Go => write!(f, "go"),
            Self::GoBench => write!(f, "go_bench"),
            Self::Java => write!(f, "java"),
            Self::JavaJmh => write!(f, "java_jmh"),
            Self::CSharp => write!(f, "c_sharp"),
            Self::CSharpDotNet => write!(f, "c_sharp_dot_net"),
            Self::Js => write!(f, "js"),
            Self::JsBenchmark => write!(f, "js_benchmark"),
            Self::JsTime => write!(f, "js_time"),
            Self::Python => write!(f, "python"),
            Self::PythonAsv => write!(f, "python_asv"),
            Self::PythonPytest => write!(f, "python_pytest"),
            Self::Ruby => write!(f, "ruby"),
            Self::RubyBenchmark => write!(f, "ruby_benchmark"),
            Self::Shell => write!(f, "shell"),
            Self::ShellHyperfine => write!(f, "shell_hyperfine"),
        }
    }
}

#[cfg(feature = "db")]
mod adapter {
    use super::{
        Adapter, CPP_CATCH2_INT, CPP_GOOGLE_INT, CPP_INT, C_SHARP_DOT_NET_INT, C_SHARP_INT,
        GO_BENCH_INT, GO_INT, JAVA_INT, JAVA_JMH_INT, JSON_INT, JS_BENCHMARK_INT, JS_INT,
        JS_TIME_INT, MAGIC_INT, PYTHON_ASV_INT, PYTHON_INT, PYTHON_PYTEST_INT, RUBY_BENCHMARK_INT,
        RUBY_INT, RUST_BENCH_INT, RUST_CRITERION_INT, RUST_IAI_CALLGRIND_INT, RUST_IAI_INT,
        RUST_INT, SHELL_HYPERFINE_INT, SHELL_INT,
    };

    #[derive(Debug, thiserror::Error)]
    pub enum AdapterError {
        #[error("Invalid adapter value: {0}")]
        Invalid(i32),
    }

    impl<DB> diesel::serialize::ToSql<diesel::sql_types::Integer, DB> for Adapter
    where
        DB: diesel::backend::Backend,
        i32: diesel::serialize::ToSql<diesel::sql_types::Integer, DB>,
    {
        fn to_sql<'b>(
            &'b self,
            out: &mut diesel::serialize::Output<'b, '_, DB>,
        ) -> diesel::serialize::Result {
            match self {
                Self::Magic => MAGIC_INT.to_sql(out),
                Self::Json => JSON_INT.to_sql(out),
                Self::Rust => RUST_INT.to_sql(out),
                Self::RustBench => RUST_BENCH_INT.to_sql(out),
                Self::RustCriterion => RUST_CRITERION_INT.to_sql(out),
                Self::RustIai => RUST_IAI_INT.to_sql(out),
                Self::RustIaiCallgrind => RUST_IAI_CALLGRIND_INT.to_sql(out),
                Self::Cpp => CPP_INT.to_sql(out),
                Self::CppGoogle => CPP_GOOGLE_INT.to_sql(out),
                Self::CppCatch2 => CPP_CATCH2_INT.to_sql(out),
                Self::Go => GO_INT.to_sql(out),
                Self::GoBench => GO_BENCH_INT.to_sql(out),
                Self::Java => JAVA_INT.to_sql(out),
                Self::JavaJmh => JAVA_JMH_INT.to_sql(out),
                Self::CSharp => C_SHARP_INT.to_sql(out),
                Self::CSharpDotNet => C_SHARP_DOT_NET_INT.to_sql(out),
                Self::Js => JS_INT.to_sql(out),
                Self::JsBenchmark => JS_BENCHMARK_INT.to_sql(out),
                Self::JsTime => JS_TIME_INT.to_sql(out),
                Self::Python => PYTHON_INT.to_sql(out),
                Self::PythonAsv => PYTHON_ASV_INT.to_sql(out),
                Self::PythonPytest => PYTHON_PYTEST_INT.to_sql(out),
                Self::Ruby => RUBY_INT.to_sql(out),
                Self::RubyBenchmark => RUBY_BENCHMARK_INT.to_sql(out),
                Self::Shell => SHELL_INT.to_sql(out),
                Self::ShellHyperfine => SHELL_HYPERFINE_INT.to_sql(out),
            }
        }
    }

    impl<DB> diesel::deserialize::FromSql<diesel::sql_types::Integer, DB> for Adapter
    where
        DB: diesel::backend::Backend,
        i32: diesel::deserialize::FromSql<diesel::sql_types::Integer, DB>,
    {
        fn from_sql(bytes: DB::RawValue<'_>) -> diesel::deserialize::Result<Self> {
            match i32::from_sql(bytes)? {
                MAGIC_INT => Ok(Self::Magic),
                JSON_INT => Ok(Self::Json),
                RUST_INT => Ok(Self::Rust),
                RUST_BENCH_INT => Ok(Self::RustBench),
                RUST_CRITERION_INT => Ok(Self::RustCriterion),
                RUST_IAI_INT => Ok(Self::RustIai),
                RUST_IAI_CALLGRIND_INT => Ok(Self::RustIaiCallgrind),
                CPP_INT => Ok(Self::Cpp),
                CPP_GOOGLE_INT => Ok(Self::CppGoogle),
                CPP_CATCH2_INT => Ok(Self::CppCatch2),
                GO_INT => Ok(Self::Go),
                GO_BENCH_INT => Ok(Self::GoBench),
                JAVA_INT => Ok(Self::Java),
                JAVA_JMH_INT => Ok(Self::JavaJmh),
                C_SHARP_INT => Ok(Self::CSharp),
                C_SHARP_DOT_NET_INT => Ok(Self::CSharpDotNet),
                JS_INT => Ok(Self::Js),
                JS_BENCHMARK_INT => Ok(Self::JsBenchmark),
                JS_TIME_INT => Ok(Self::JsTime),
                PYTHON_INT => Ok(Self::Python),
                PYTHON_ASV_INT => Ok(Self::PythonAsv),
                PYTHON_PYTEST_INT => Ok(Self::PythonPytest),
                RUBY_INT => Ok(Self::Ruby),
                RUBY_BENCHMARK_INT => Ok(Self::RubyBenchmark),
                SHELL_INT => Ok(Self::Shell),
                SHELL_HYPERFINE_INT => Ok(Self::ShellHyperfine),
                value => Err(Box::new(AdapterError::Invalid(value))),
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum JsonAverage {
    #[default]
    Mean,
    Median,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum JsonFold {
    Min,
    Max,
    Mean,
    Median,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonReports(pub Vec<JsonReport>);

crate::from_vec!(JsonReports[JsonReport]);

#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonReport {
    pub uuid: ReportUuid,
    pub user: JsonUser,
    pub project: JsonProject,
    pub branch: JsonBranchVersion,
    pub testbed: JsonTestbed,
    pub start_time: DateTime,
    pub end_time: DateTime,
    pub adapter: Adapter,
    pub results: JsonReportResults,
    pub alerts: JsonReportAlerts,
    pub created: DateTime,
}

#[typeshare::typeshare]
pub type JsonReportResults = Vec<JsonReportIteration>;

#[typeshare::typeshare]
pub type JsonReportIteration = Vec<JsonReportResult>;

#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonReportResult {
    pub iteration: Iteration,
    pub benchmark: JsonBenchmark,
    pub measures: Vec<JsonReportMeasure>,
}

#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonReportMeasure {
    pub measure: JsonMeasure,
    pub metric: JsonMetric,
    pub threshold: Option<JsonThresholdModel>,
    pub boundary: Option<JsonBoundary>,
}

#[typeshare::typeshare]
pub type JsonReportAlerts = Vec<JsonAlert>;

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonReportQueryParams {
    /// Filter by branch UUID, slug, or name exact match.
    pub branch: Option<String>,
    /// Filter by testbed UUID, slug, or name exact match.
    pub testbed: Option<String>,
    /// Filter for reports after the given date time in milliseconds.
    pub start_time: Option<DateTimeMillis>,
    /// Filter for reports before the given date time in milliseconds.
    pub end_time: Option<DateTimeMillis>,
    /// If set to `true`, only return reports with an archived branch or testbed.
    /// If not set or set to `false`, only returns reports with non-archived branches and testbeds.
    pub archived: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct JsonReportQuery {
    pub branch: Option<NameId>,
    pub testbed: Option<NameId>,
    pub start_time: Option<DateTime>,
    pub end_time: Option<DateTime>,
    pub archived: Option<bool>,
}

impl TryFrom<JsonReportQueryParams> for JsonReportQuery {
    type Error = UrlEncodedError;

    fn try_from(query_params: JsonReportQueryParams) -> Result<Self, Self::Error> {
        let JsonReportQueryParams {
            branch,
            testbed,
            start_time,
            end_time,
            archived,
        } = query_params;

        let branch = if let Some(branch) = branch {
            Some(from_urlencoded(&branch)?)
        } else {
            None
        };
        let testbed = if let Some(testbed) = testbed {
            Some(from_urlencoded(&testbed)?)
        } else {
            None
        };

        Ok(Self {
            branch,
            testbed,
            start_time: start_time.map(Into::into),
            end_time: end_time.map(Into::into),
            archived,
        })
    }
}

impl JsonReportQuery {
    pub fn branch(&self) -> Option<String> {
        self.branch.as_ref().map(to_urlencoded)
    }

    pub fn testbed(&self) -> Option<String> {
        self.testbed.as_ref().map(to_urlencoded)
    }

    pub fn start_time(&self) -> Option<DateTimeMillis> {
        self.start_time.map(Into::into)
    }

    pub fn end_time(&self) -> Option<DateTimeMillis> {
        self.end_time.map(Into::into)
    }
}

#[typeshare::typeshare]
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Hash, derive_more::Display, Serialize, Deserialize,
)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Integer))]
pub struct Iteration(pub u32);

impl From<Iteration> for u32 {
    fn from(value: Iteration) -> Self {
        value.0
    }
}

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
