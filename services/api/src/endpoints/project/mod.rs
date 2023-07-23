use derive_more::Display;

use crate::WordStr;

pub mod allowed;
pub mod benchmarks;
pub mod branches;
pub mod metric_kinds;
pub mod perf;
pub mod projects;
pub mod reports;
pub mod testbeds;
pub mod thresholds;

#[derive(Debug, Display, Clone, Copy)]
pub enum Resource {
    Alert,
    Benchmark,
    Branch,
    MetricKind,
    Perf,
    PerfImg,
    Project,
    ProjectPermission,
    Report,
    Result,
    Statistic,
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
            Self::PerfImg => "benchmark perf image",
            Self::Project => "project",
            Self::ProjectPermission => "project permission",
            Self::Report => "report",
            Self::Result => "result",
            Self::Statistic => "statistic",
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
            Self::PerfImg => "benchmark perfs image",
            Self::Project => "projects",
            Self::ProjectPermission => "project permissions",
            Self::Report => "reports",
            Self::Result => "results",
            Self::Statistic => "statistics",
            Self::Testbed => "testbeds",
            Self::Threshold => "thresholds",
        }
    }
}
