use derive_more::Display;

use crate::WordStr;

pub mod alerts;
pub mod benchmarks;
pub mod branches;
pub mod metric_kinds;
pub mod perf;
pub mod projects;
pub mod reports;
pub mod results;
pub mod testbeds;
pub mod thresholds;

#[derive(Debug, Display, Clone, Copy)]
pub enum Resource {
    Alert,
    Benchmark,
    Branch,
    MetricKind,
    Perf,
    #[cfg(feature = "browser")]
    PerfImg,
    Project,
    Report,
    Result,
    Testbed,
    Threshold,
}

impl WordStr for Resource {
    fn singular(&self) -> &str {
        match self {
            Self::Alert => "alert",
            Self::Benchmark => "benchmark",
            Self::Branch => "branch",
            Self::MetricKind => "metric kind",
            Self::Perf => "benchmark perf",
            #[cfg(feature = "browser")]
            Self::PerfImg => "benchmark perf image",
            Self::Project => "project",
            Self::Report => "report",
            Self::Result => "result",
            Self::Testbed => "testbed",
            Self::Threshold => "threshold",
        }
    }

    fn plural(&self) -> &str {
        match self {
            Self::Alert => "alerts",
            Self::Benchmark => "benchmarks",
            Self::Branch => "branches",
            Self::MetricKind => "metric kinds",
            Self::Perf => "benchmark perfs",
            #[cfg(feature = "browser")]
            Self::PerfImg => "benchmark perfs image",
            Self::Project => "projects",
            Self::Report => "reports",
            Self::Result => "results",
            Self::Testbed => "testbeds",
            Self::Threshold => "thresholds",
        }
    }
}
