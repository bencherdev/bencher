//! Test-only infrastructure: fixtures, fault injection, workload generation,
//! and equivalence oracles.
//!
//! Compiled under `#[cfg(any(test, feature = "testing"))]`. The `testing`
//! feature exists so downstream integration tests (e.g. `lib/api_server`)
//! can drive the same machinery.

mod compare;
mod flaky;
mod probe;
mod synthetic_wal;
mod wal_fixture;
mod workload;

pub use compare::{assert_pages_equal, assert_replica_equivalent};
pub use flaky::{FailurePlan, FlakyMultipart, FlakyStorage, Op, OpKind, OpOutcome};
pub use probe::{ProbeResult, WriteProbe};
pub use synthetic_wal::SyntheticWal;
pub use wal_fixture::{CheckpointMode, FixtureError, WalFixture};
pub use workload::{
    AppliedOp, WorkloadEnv, WorkloadError, WorkloadOp, WorkloadOpError, WorkloadRunner,
    generate_workload, run_workload,
};
