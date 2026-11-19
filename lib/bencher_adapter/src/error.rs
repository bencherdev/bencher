use std::fmt;

use bencher_json::{BmfVersion, project::report::Adapter};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AdapterError {
    #[error("Failed to validate: {0}")]
    Valid(#[from] bencher_json::ValidError),
    #[error("Failed to parse benchmark units: {0}")]
    BenchmarkUnits(String),
    #[error("Failed to convert results: {0}")]
    Convert(String),
    #[error("Results parsed as BMF version {parsed} but were expected at version {expected}")]
    BmfVersion {
        expected: BmfVersion,
        parsed: BmfVersion,
    },
    #[error("{0} reads results at BMF version 1 only")]
    V1Only(Adapter),
    #[error("`{java_jmh}` reads JMH 1.19 or later, and these results are {0}", java_jmh = Adapter::JavaJmh)]
    JmhLookalike(JmhLookalike),
}

/// A JMH-shaped report that `java_jmh` refuses because no benchmark in it names its JMH version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JmhLookalike {
    /// kotlinx-benchmark's report for a JS, Wasm, or Native target.
    Kotlinx,
    /// Swift package-benchmark's `--format jmh` export.
    SwiftBenchmark,
    /// JMH before 1.19, which writes no `jmhVersion`.
    JmhBefore119,
}

impl fmt::Display for JmhLookalike {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Kotlinx => write!(
                f,
                "a kotlinx-benchmark report: read it with `{}`",
                Adapter::KotlinKotlinx
            ),
            Self::SwiftBenchmark => write!(
                f,
                "a Swift package-benchmark JMH export: run the benchmarks with `--format influx` and read that with `swift_benchmark`"
            ),
            Self::JmhBefore119 => write!(
                f,
                "from JMH before 1.19, which writes no `jmhVersion`: upgrade JMH to 1.19 or later"
            ),
        }
    }
}
