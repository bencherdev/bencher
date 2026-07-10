#![cfg(all(feature = "plus", feature = "testing"))]
//! End-to-end equivalence suite: seeded workloads drive the source database
//! and the step-driven engine together, then prove the governing invariant
//! `restore(replica) == source` at every commit boundary that matters (after
//! every completed checkpoint and after the final drain).
//!
//! Scripts come from `bencher_replica::testing::generate_workload`; the
//! smoke test's script is hand-written so a broken op type fails readably
//! before the seeded scripts do. On any failure the harness prints the seed,
//! the failing op index, and the full script.
//!
//! NOTE: `unused_crate_dependencies` cannot be handled with a crate-level
//! `#![expect]` here (see `tests/storage_contract.rs`); unused package
//! dependencies are referenced explicitly instead, as rustc recommends.

use async_compression as _;
use aws_credential_types as _;
use aws_sdk_s3 as _;
use bytes as _;
use futures as _;
use hex as _;
use rand as _;
use rusqlite as _;
use serde as _;
use serde_json as _;
use sha2 as _;
use thiserror as _;
use uuid as _;
use zstd as _;
// Optional dependency enabled by the otel feature; unused by tests.
#[cfg(feature = "otel")]
use bencher_otel as _;

/// Shared fixtures: a scripted source database, a workload environment over
/// a local replica, and restore-equivalence assertions.
#[cfg(test)]
pub(crate) mod harness {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicI64, Ordering};

    use bencher_json::system::config::{JsonReplication, ReplicationTarget};
    use bencher_json::{Clock, DateTime};
    use bencher_replica::testing::{
        FailurePlan, FlakyStorage, WalFixture, WorkloadEnv, WorkloadError, WorkloadOp,
        assert_replica_equivalent,
    };
    use bencher_replica::{
        EngineState, LocalStorage, ReplicaConfig, ReplicaDb, ReplicaStorage, RestoreOutcome,
        SyncEngine, restore_if_missing,
    };
    use camino::{Utf8Path, Utf8PathBuf};

    /// Page size for every fixture database in this suite.
    pub(crate) const PAGE_SIZE: u32 = 4096;
    /// 2026-07-10T14:59:00Z, the deterministic clock start.
    pub(crate) const BASE_SECS: i64 = 1_783_695_540;

    pub(crate) fn dir_path(tmp: &tempfile::TempDir) -> &Utf8Path {
        Utf8Path::from_path(tmp.path()).expect("tempdir path is UTF-8")
    }

    pub(crate) fn logger() -> slog::Logger {
        slog::Logger::root(slog::Discard, slog::o!())
    }

    pub(crate) fn clock_for(secs: &Arc<AtomicI64>) -> Clock {
        let secs = Arc::clone(secs);
        Clock::Custom(Arc::new(move || {
            DateTime::try_from(secs.load(Ordering::SeqCst)).expect("valid clock seconds")
        }))
    }

    /// Flaky(Local) storage rooted at `root`, with an empty failure plan.
    pub(crate) fn flaky_over(root: &Utf8Path) -> ReplicaStorage {
        ReplicaStorage::Flaky(Box::new(FlakyStorage::new(
            ReplicaStorage::Local(LocalStorage::new(root.to_path_buf())),
            FailurePlan::new(),
        )))
    }

    /// The equivalence rig: the source database and everything a
    /// `WorkloadRunner` needs, minus the engine itself (the runner owns it,
    /// so `RestartReplicator` can rebuild it in place).
    pub(crate) struct Rig {
        pub fixture: WalFixture,
        pub config: ReplicaConfig,
        pub db: ReplicaDb<()>,
        pub clock_secs: Arc<AtomicI64>,
        pub replica_root: Utf8PathBuf,
        pub _fixture_tmp: tempfile::TempDir,
        pub _replica_tmp: tempfile::TempDir,
    }

    impl Rig {
        pub(crate) fn new() -> Self {
            let fixture_tmp = tempfile::tempdir().expect("fixture tempdir");
            let replica_tmp = tempfile::tempdir().expect("replica tempdir");
            let fixture = WalFixture::new(dir_path(&fixture_tmp), PAGE_SIZE).expect("fixture");
            let replica_root = dir_path(&replica_tmp).to_path_buf();
            let json = JsonReplication {
                target: ReplicationTarget::File {
                    path: replica_root.clone().into_std_path_buf(),
                },
                sync_interval_secs: None,
                checkpoint_interval_secs: None,
                min_checkpoint_pages: None,
                snapshot_interval_secs: None,
                snapshot_throttle_mib: None,
                retention_generations: None,
                verification_interval_secs: None,
                shutdown_sync_timeout_secs: None,
            };
            let config = ReplicaConfig::try_from(json).expect("config");
            let clock_secs = Arc::new(AtomicI64::new(BASE_SECS));
            let db = ReplicaDb {
                db_path: fixture.db_path(),
                writer: Arc::new(tokio::sync::Mutex::new(())),
                busy_timeout_ms: 5000,
            };
            Self {
                fixture,
                config,
                db,
                clock_secs,
                replica_root,
                _fixture_tmp: fixture_tmp,
                _replica_tmp: replica_tmp,
            }
        }

        /// The pieces a `WorkloadRunner` needs to drive (and rebuild) the
        /// engine over this rig's directories.
        pub(crate) fn env(&self) -> WorkloadEnv<()> {
            WorkloadEnv {
                log: logger(),
                config: self.config.clone(),
                db: self.db.clone(),
                clock: clock_for(&self.clock_secs),
                replica_root: self.replica_root.clone(),
            }
        }

        /// Build an engine over the replica and drive the fresh-replica
        /// bootstrap snapshot to completion, ending quiescent in Streaming.
        pub(crate) async fn ready_engine(&self) -> SyncEngine<()> {
            let mut engine = SyncEngine::new_with_storage(
                logger(),
                self.config.clone(),
                self.db.clone(),
                clock_for(&self.clock_secs),
                false,
                flaky_over(&self.replica_root),
            )
            .await
            .expect("engine");
            for _ in 0..256 {
                if engine.state() == EngineState::Streaming {
                    break;
                }
                engine.sync_once().await.expect("bootstrap sync");
            }
            assert!(
                engine.state() == EngineState::Streaming,
                "engine never reached Streaming during bootstrap; state: {:?}",
                engine.state()
            );
            engine.sync_once().await.expect("backlog sync");
            engine
        }

        /// Restore the replica into a scratch directory and assert logical
        /// equivalence with the live source database.
        pub(crate) async fn assert_restore_equivalent(&self) {
            let target_tmp = tempfile::tempdir().expect("restore target tempdir");
            let target_db = dir_path(&target_tmp).join("restored.db");
            let outcome = restore_if_missing(&logger(), &self.config, &target_db)
                .await
                .expect("restore");
            assert!(
                matches!(outcome, RestoreOutcome::Restored { .. }),
                "expected Restored, got {outcome:?}"
            );
            assert_replica_equivalent(&self.fixture.db_path(), &target_db);
        }
    }

    /// Unwrap a workload result; on failure print the error (which carries
    /// the seed, the failing op index, and the op) plus the full script.
    pub(crate) fn expect_workload<T>(result: Result<T, WorkloadError>, script: &[WorkloadOp]) -> T {
        match result {
            Ok(value) => value,
            Err(error) => panic!(
                "{error}\nfull script ({} ops):\n{}",
                script.len(),
                format_script(script)
            ),
        }
    }

    fn format_script(script: &[WorkloadOp]) -> String {
        use std::fmt::Write as _;
        script
            .iter()
            .enumerate()
            .fold(String::new(), |mut out, (index, op)| {
                writeln!(out, "  {index:>4}: {op}").expect("write to string");
                out
            })
    }
}

#[cfg(test)]
mod cases {
    use bencher_replica::CheckpointOutcome;
    use bencher_replica::testing::{WorkloadOp, WorkloadRunner, generate_workload, run_workload};
    use pretty_assertions::assert_eq;

    use super::harness::{Rig, expect_workload};

    /// Ops per generated script (the soak uses more).
    const SCRIPT_LEN: usize = 200;

    /// A hand-written script exercising every `WorkloadOp` variant once (in
    /// a sensible order), so a broken op type fails here, readably, before
    /// the seeded scripts bury it.
    #[tokio::test]
    async fn equivalence_all_op_types_smoke() {
        let script = vec![
            WorkloadOp::CreateTable { table: 0 },
            WorkloadOp::Insert { table: 0, rows: 8 },
            WorkloadOp::CreateIndex { table: 0 },
            WorkloadOp::BlobWrite {
                table: 0,
                len: 64 * 1024,
            },
            WorkloadOp::Sync,
            WorkloadOp::Update { table: 0, rows: 4 },
            WorkloadOp::BigTxn { statements: 12 },
            WorkloadOp::UserVersionBump,
            WorkloadOp::Sync,
            WorkloadOp::Checkpoint,
            WorkloadOp::StrayWrite,
            WorkloadOp::Insert { table: 1, rows: 5 },
            WorkloadOp::Delete { table: 0, rows: 2 },
            WorkloadOp::Vacuum,
            WorkloadOp::Sync,
            WorkloadOp::Snapshot,
            WorkloadOp::Insert { table: 2, rows: 3 },
            WorkloadOp::RestartReplicator,
            WorkloadOp::Insert { table: 0, rows: 2 },
            WorkloadOp::DropTable { table: 1 },
            WorkloadOp::Sync,
            WorkloadOp::Checkpoint,
        ];
        // The hand-written script must exercise every variant.
        let mut kinds: Vec<&'static str> = script.iter().map(WorkloadOp::kind_name).collect();
        kinds.sort_unstable();
        kinds.dedup();
        assert_eq!(
            kinds.len(),
            WorkloadOp::VARIANT_COUNT,
            "smoke script covers every WorkloadOp variant"
        );

        let rig = Rig::new();
        let engine = rig.ready_engine().await;
        let seed = 424_242;
        let mut runner = expect_workload(
            WorkloadRunner::new(seed, script.clone(), rig.env(), engine),
            &script,
        );
        expect_workload(runner.run().await, &script);
        expect_workload(runner.drain().await, &script);
        rig.assert_restore_equivalent().await;
    }

    /// Generate, run, drain, restore, compare: one full equivalence pass.
    async fn run_seed(seed: u64, len: usize) {
        let rig = Rig::new();
        let engine = rig.ready_engine().await;
        let script = generate_workload(seed, len);
        let mut runner = expect_workload(
            run_workload(seed, script.clone(), rig.env(), engine).await,
            &script,
        );
        expect_workload(runner.drain().await, &script);
        rig.assert_restore_equivalent().await;
    }

    macro_rules! equivalence_seed {
        ($($name:ident => $seed:literal,)+) => {$(
            #[tokio::test]
            async fn $name() {
                // Boxed: the workload future is large (clippy::large_futures).
                Box::pin(run_seed($seed, SCRIPT_LEN)).await;
            }
        )+};
    }

    equivalence_seed! {
        equivalence_seed_0 => 0,
        equivalence_seed_1 => 1,
        equivalence_seed_2 => 2,
        equivalence_seed_3 => 3,
        equivalence_seed_4 => 4,
        equivalence_seed_5 => 5,
        equivalence_seed_6 => 6,
        equivalence_seed_7 => 7,
    }

    /// After every checkpoint that reports `Completed`, the replica must
    /// already restore to the exact committed source state: `Completed`
    /// means every WAL frame was shipped (I1) and backfilled, and the runner
    /// is between ops, so the live committed state IS the shipped state.
    #[tokio::test]
    async fn equivalence_restore_at_every_checkpoint() {
        let seed = 42;
        let rig = Rig::new();
        let engine = rig.ready_engine().await;
        let script = generate_workload(seed, 120);
        let mut runner = expect_workload(
            WorkloadRunner::new(seed, script.clone(), rig.env(), engine),
            &script,
        );
        let mut completed = 0u32;
        while let Some(applied) = expect_workload(runner.step().await, &script) {
            if applied.checkpoint == Some(CheckpointOutcome::Completed) {
                completed += 1;
                rig.assert_restore_equivalent().await;
            }
        }
        assert!(
            completed >= 1,
            "seed {seed} must complete at least one checkpoint, got {completed}"
        );
        expect_workload(runner.drain().await, &script);
        rig.assert_restore_equivalent().await;
    }

    /// Long soak: thousands of ops across multiple generations and
    /// replicator restarts.
    #[tokio::test]
    #[ignore = "soak: run manually or nightly"]
    async fn equivalence_soak() {
        Box::pin(run_seed(1337, 3000)).await;
    }
}
