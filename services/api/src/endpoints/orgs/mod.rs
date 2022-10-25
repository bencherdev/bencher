use derive_more::Display;

use crate::WordStr;

pub mod alerts;
pub mod benchmarks;
pub mod branches;
pub mod members;
pub mod organizations;
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
    Member,
    Organization,
    OrganizationPermission,
    Perf,
    Project,
    Report,
    Testbed,
    Threshold,
}

impl WordStr for Resource {
    fn singular(&self) -> &str {
        match self {
            Self::Alert => "alert",
            Self::Benchmark => "benchmark",
            Self::Branch => "branch",
            Self::Member => "member",
            Self::Organization => "organization",
            Self::OrganizationPermission => "organization permission",
            Self::Perf => "benchmark perf",
            Self::Project => "project",
            Self::Report => "report",
            Self::Testbed => "testbed",
            Self::Threshold => "threshold",
        }
    }

    fn plural(&self) -> &str {
        match self {
            Self::Alert => "alerts",
            Self::Benchmark => "benchmarks",
            Self::Branch => "branches",
            Self::Member => "members",
            Self::Organization => "organizations",
            Self::OrganizationPermission => "organization permissions",
            Self::Perf => "benchmark perfs",
            Self::Project => "projects",
            Self::Report => "reports",
            Self::Testbed => "testbeds",
            Self::Threshold => "thresholds",
        }
    }
}
