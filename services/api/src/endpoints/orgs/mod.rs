use derive_more::Display;

use crate::WordStr;

pub mod benchmarks;
pub mod branches;
pub mod perf;
pub mod projects;
pub mod reports;
pub mod testbeds;
pub mod thresholds;

#[derive(Debug, Display, Clone, Copy)]
pub enum Resource {
    Benchmark,
    Branch,
    Organization,
    Perf,
    Project,
    Report,
    Testbed,
    Threshold,
}

impl WordStr for Resource {
    fn singular(&self) -> &str {
        match self {
            Self::Benchmark => "benchmark",
            Self::Branch => "branch",
            Self::Perf => "benchmark perf",
            Self::Organization => "organization",
            Self::Project => "project",
            Self::Report => "report",
            Self::Testbed => "testbed",
            Self::Threshold => "threshold",
        }
    }

    fn plural(&self) -> &str {
        match self {
            Self::Benchmark => "benchmarks",
            Self::Branch => "branches",
            Self::Perf => "benchmark perfs",
            Self::Organization => "organizations",
            Self::Project => "projects",
            Self::Report => "reports",
            Self::Testbed => "testbeds",
            Self::Threshold => "thresholds",
        }
    }
}
