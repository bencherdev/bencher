#![cfg(all(feature = "plus", feature = "testing"))]
//! Checkpoint critical section suite: ship-before-checkpoint (I1), the
//! PASSIVE-under-BEGIN-IMMEDIATE trick that closes the sneak-in race, and
//! the prime directive that app writers are never blocked for O(database)
//! work (I5).
//!
//! All tests use the multi-thread runtime: the engine's `spawn_blocking`
//! rusqlite calls run concurrently with real second-connection writers.
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
use serde as _;
use serde_json as _;
use sha2 as _;
use thiserror as _;
use uuid as _;
use zstd as _;
// Optional dependency enabled by the otel feature; unused by tests.
#[cfg(feature = "otel")]
use bencher_otel as _;

/// A slimmer sibling of the `tests/sync_engine.rs` harness: real fixture,
/// plain local replica (no fault injection), deterministic clock.
#[cfg(test)]
pub(crate) mod harness {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicI64, Ordering};

    use bencher_json::system::config::{JsonReplication, ReplicationTarget};
    use bencher_json::{Clock, DateTime};
    use bencher_replica::testing::{WalFixture, assert_replica_equivalent};
    use bencher_replica::{
        EngineState, ReplicaConfig, ReplicaDb, RestoreOutcome, SyncEngine, restore_if_missing,
    };
    use camino::Utf8Path;

    /// Page size for every fixture database in this suite.
    pub(crate) const PAGE_SIZE: u32 = 4096;
    /// 2026-07-10T14:59:00Z, the deterministic clock start.
    const BASE_SECS: i64 = 1_783_695_540;

    pub(crate) fn dir_path(tmp: &tempfile::TempDir) -> &Utf8Path {
        Utf8Path::from_path(tmp.path()).expect("tempdir path is UTF-8")
    }

    pub(crate) fn logger() -> slog::Logger {
        slog::Logger::root(slog::Discard, slog::o!())
    }

    pub(crate) struct Harness {
        pub fixture: WalFixture,
        pub engine: SyncEngine<()>,
        pub config: ReplicaConfig,
        pub _clock_secs: Arc<AtomicI64>,
        pub _fixture_tmp: tempfile::TempDir,
        pub _replica_tmp: tempfile::TempDir,
    }

    impl Harness {
        pub(crate) async fn new() -> Self {
            let fixture_tmp = tempfile::tempdir().expect("fixture tempdir");
            let replica_tmp = tempfile::tempdir().expect("replica tempdir");
            let fixture = WalFixture::new(dir_path(&fixture_tmp), PAGE_SIZE).expect("fixture");
            let replica_root = dir_path(&replica_tmp).to_path_buf();
            let config = ReplicaConfig::try_from(JsonReplication {
                target: ReplicationTarget::File {
                    path: replica_root.into_std_path_buf(),
                },
                sync_interval_secs: None,
                checkpoint_interval_secs: None,
                min_checkpoint_pages: None,
                snapshot_interval_secs: None,
                snapshot_throttle_mib: None,
                retention_generations: None,
                verification_interval_secs: None,
                shutdown_sync_timeout_secs: None,
            })
            .expect("config");
            let clock_secs = Arc::new(AtomicI64::new(BASE_SECS));
            let secs = Arc::clone(&clock_secs);
            let clock = Clock::Custom(Arc::new(move || {
                DateTime::try_from(secs.load(Ordering::SeqCst)).expect("valid clock seconds")
            }));
            let db = ReplicaDb {
                db_path: fixture.db_path(),
                writer: Arc::new(tokio::sync::Mutex::new(())),
                busy_timeout_ms: 5000,
            };
            let engine = SyncEngine::new(logger(), config.clone(), db, clock, false)
                .await
                .expect("engine");
            let mut harness = Self {
                fixture,
                engine,
                config,
                _clock_secs: clock_secs,
                _fixture_tmp: fixture_tmp,
                _replica_tmp: replica_tmp,
            };
            harness.ready().await;
            harness
        }

        /// Drive the bootstrap snapshot to Streaming and ship the backlog.
        async fn ready(&mut self) {
            for _ in 0..64 {
                if self.engine.state() == EngineState::Streaming {
                    self.engine.sync_once().await.expect("backlog sync");
                    return;
                }
                self.engine
                    .sync_once()
                    .await
                    .expect("sync_once during startup");
            }
            panic!(
                "engine never reached Streaming; state: {:?}",
                self.engine.state()
            );
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
}

#[cfg(test)]
mod cases {
    use std::io::Cursor;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::{Duration, Instant};

    use bencher_replica::testing::{ProbeResult, WriteProbe};
    use bencher_replica::{CheckpointOutcome, ReplicaMeta, WalScanner};
    use pretty_assertions::assert_eq;
    use tokio::sync::oneshot;

    use super::harness::Harness;

    /// Read the live WAL header salts of the fixture.
    fn wal_salt(harness: &Harness) -> (u32, u32) {
        let wal = harness.fixture.wal_bytes().expect("wal bytes");
        WalScanner::open(Cursor::new(wal))
            .expect("wal header")
            .expect("wal is not empty")
            .header()
            .salt
    }

    /// Every probe write must have succeeded; returns the longest observed
    /// block time.
    fn assert_all_writes_succeeded(results: Vec<ProbeResult>) -> Duration {
        assert!(!results.is_empty(), "the probe recorded at least one write");
        let mut max_blocked = Duration::ZERO;
        for probe in results {
            probe
                .result
                .expect("no probe write may fail (zero busy errors)");
            max_blocked = max_blocked.max(probe.blocked);
        }
        max_blocked
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn checkpoint_only_after_ship() {
        let mut harness = Harness::new().await;
        harness
            .fixture
            .txn(&["INSERT INTO t (data) VALUES ('unshipped')"])
            .expect("txn");
        let outcome = harness.engine.checkpoint_once().await.expect("checkpoint");
        assert_eq!(
            outcome,
            CheckpointOutcome::SkippedUnshipped,
            "unshipped committed frames block the checkpoint (I1)"
        );
        let shipped = harness.engine.ship_once().await.expect("ship");
        assert!(shipped >= 1, "the tail ships");
        let outcome = harness.engine.checkpoint_once().await.expect("checkpoint");
        assert_eq!(
            outcome,
            CheckpointOutcome::Completed,
            "once shipped, the checkpoint completes"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn checkpoint_completes_and_sets_meta_flag() {
        let mut harness = Harness::new().await;
        harness
            .fixture
            .txn(&["INSERT INTO t (data) VALUES ('to-checkpoint')"])
            .expect("txn");
        harness.engine.ship_once().await.expect("ship");
        let meta = ReplicaMeta::load(&harness.fixture.db_path())
            .expect("load meta")
            .expect("meta written by ship");
        assert!(
            !meta.epoch_shipped_through_checkpoint,
            "the flag is clear before any checkpoint"
        );

        let outcome = harness.engine.checkpoint_once().await.expect("checkpoint");
        assert_eq!(outcome, CheckpointOutcome::Completed, "full backfill");
        let meta = ReplicaMeta::load(&harness.fixture.db_path())
            .expect("load meta")
            .expect("meta written by checkpoint");
        assert!(
            meta.epoch_shipped_through_checkpoint,
            "Completed sets the epoch-shipped-through-checkpoint flag"
        );

        // Full backfill means the NEXT writer restarts the WAL: new salts.
        let salt_before = wal_salt(&harness);
        harness
            .fixture
            .txn(&["INSERT INTO t (data) VALUES ('restart')"])
            .expect("txn after checkpoint");
        let salt_after = wal_salt(&harness);
        assert!(
            salt_after != salt_before,
            "the WAL restarted after a completed checkpoint ({salt_before:?} vs {salt_after:?})"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn passive_checkpoint_never_blocks_writers() {
        let mut harness = Harness::new().await;
        let probe = WriteProbe::new(&harness.fixture.db_path(), Duration::from_secs(5));
        let (stop_tx, stop_rx) = oneshot::channel::<()>();
        let hammer = tokio::spawn(async move {
            probe
                .hammer(async move {
                    drop(stop_rx.await);
                })
                .await
        });

        // 5+ full ship+checkpoint cycles while the hammer runs. Outcomes
        // vary (the hammer commits between ship and checkpoint), but no
        // probe write may ever fail.
        for _cycle in 0..6 {
            harness.engine.ship_once().await.expect("ship");
            let _outcome = harness.engine.checkpoint_once().await.expect("checkpoint");
        }
        stop_tx.send(()).expect("stop the hammer");
        let results = hammer.await.expect("hammer task");
        let _max_blocked = assert_all_writes_succeeded(results);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn critical_section_bounded() {
        let mut harness = Harness::new().await;
        harness.fixture.txn_touching_pages(64).expect("seed pages");

        // A tokio-side probe AND a raw std::thread stray writer hammer the
        // database through the checkpoint cycles.
        let probe = WriteProbe::new(&harness.fixture.db_path(), Duration::from_secs(5));
        let (stop_tx, stop_rx) = oneshot::channel::<()>();
        let hammer = tokio::spawn(async move {
            probe
                .hammer(async move {
                    drop(stop_rx.await);
                })
                .await
        });
        let stray_stop = Arc::new(AtomicBool::new(false));
        let stray_flag = Arc::clone(&stray_stop);
        let stray_db_path = harness.fixture.db_path().to_string();
        let stray = std::thread::spawn(move || {
            let conn = rusqlite::Connection::open(&stray_db_path).expect("stray conn");
            conn.busy_timeout(Duration::from_secs(5)).expect("busy");
            let _pages: i64 = conn
                .query_row("PRAGMA wal_autocheckpoint = 0", [], |row| row.get(0))
                .expect("autocheckpoint off");
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS stray_writes(id INTEGER PRIMARY KEY, at TEXT)",
            )
            .expect("stray table");
            while !stray_flag.load(Ordering::SeqCst) {
                conn.execute("INSERT INTO stray_writes (at) VALUES ('stray')", [])
                    .expect("stray insert never times out");
                // A zero-gap loop would monopolize the write lock and
                // starve the other writers (a plain two-writer SQLite
                // contention problem, nothing to do with the checkpoint);
                // real stray writers (stats/credit sweeps) are paced.
                std::thread::sleep(Duration::from_millis(2));
            }
        });

        for _ in 0..6 {
            harness.engine.ship_once().await.expect("ship");
            harness.engine.checkpoint_once().await.expect("checkpoint");
        }
        stop_tx.send(()).expect("stop the hammer");
        stray_stop.store(true, Ordering::SeqCst);
        stray.join().expect("stray writer thread");
        let results = hammer.await.expect("hammer task");
        let max_blocked = assert_all_writes_succeeded(results);
        // Smoke bound with a generous margin: the write lock is only ever
        // held for O(WAL-tail) work (I5), so no writer waits anywhere near
        // its 5s busy budget.
        assert!(
            max_blocked < Duration::from_secs(2),
            "longest probe block {max_blocked:?} must stay far below the busy timeout"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn sneak_in_stray_commit_detected() {
        let mut harness = Harness::new().await;
        harness
            .fixture
            .txn(&["INSERT INTO t (data) VALUES ('shipped')"])
            .expect("txn");
        harness.engine.ship_once().await.expect("ship");

        // The litestream race: a stray commit lands AFTER the ship pass and
        // BEFORE the checkpoint. The locked section re-verifies the tail
        // under BEGIN IMMEDIATE, so the frame cannot be lost.
        let stray = harness.fixture.stray_conn().expect("stray conn");
        stray
            .execute("INSERT INTO t (data) VALUES ('sneak-in')", [])
            .expect("sneak-in insert");
        let outcome = harness.engine.checkpoint_once().await.expect("checkpoint");
        assert_eq!(
            outcome,
            CheckpointOutcome::SkippedUnshipped,
            "the sneak-in commit defers the checkpoint, it is never backfilled unshipped"
        );

        let shipped = harness.engine.ship_once().await.expect("ship sneak-in");
        assert!(shipped >= 1, "the sneak-in commit ships");
        let outcome = harness.engine.checkpoint_once().await.expect("checkpoint");
        assert_eq!(outcome, CheckpointOutcome::Completed, "then checkpoints");
        harness.assert_restore_equivalent().await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn long_reader_degrades_to_partial() {
        let mut harness = Harness::new().await;
        // Pin a read snapshot (like the online backup's long read).
        let reader = harness.fixture.stray_conn().expect("reader conn");
        reader.execute_batch("BEGIN").expect("begin read txn");
        let _count: i64 = reader
            .query_row("SELECT COUNT(*) FROM t", [], |row| row.get(0))
            .expect("pin the read snapshot");

        // Commits past the pinned mark, fully shipped.
        harness
            .fixture
            .txn(&["INSERT INTO t (data) VALUES ('past-the-mark')"])
            .expect("txn");
        harness
            .fixture
            .txn(&["INSERT INTO t (data) VALUES ('and-another')"])
            .expect("txn");
        harness.engine.ship_once().await.expect("ship");

        let probe = WriteProbe::new(&harness.fixture.db_path(), Duration::from_secs(5));
        let outcome = harness.engine.checkpoint_once().await.expect("checkpoint");
        assert_eq!(
            outcome,
            CheckpointOutcome::Partial,
            "a long reader pins the backfill mark: PASSIVE degrades to Partial"
        );
        let meta = ReplicaMeta::load(&harness.fixture.db_path())
            .expect("load meta")
            .expect("meta present");
        assert!(
            !meta.epoch_shipped_through_checkpoint,
            "Partial must NOT set the checkpoint flag"
        );
        // Writers are unaffected by the partial backfill.
        let result = probe.write_once().await;
        result.result.expect("probe write during the long read");

        // Release the reader: the next checkpoint completes. The probe
        // write above must ship first (I1).
        reader.execute_batch("COMMIT").expect("release the reader");
        harness.engine.ship_once().await.expect("ship probe write");
        let outcome = harness.engine.checkpoint_once().await.expect("checkpoint");
        assert_eq!(
            outcome,
            CheckpointOutcome::Completed,
            "full backfill once the reader releases its mark"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn busy_stray_writer_yields_skipped_busy() {
        let mut harness = Harness::new().await;
        harness
            .fixture
            .txn(&["INSERT INTO t (data) VALUES ('busy')"])
            .expect("txn");
        harness.engine.ship_once().await.expect("ship");

        // A stray writer holds the SQLite write lock across the checkpoint.
        let stray = harness.fixture.stray_conn().expect("stray conn");
        stray
            .execute_batch("BEGIN IMMEDIATE")
            .expect("hold the write lock");
        let started = Instant::now();
        let outcome = harness.engine.checkpoint_once().await.expect("checkpoint");
        let elapsed = started.elapsed();
        assert_eq!(
            outcome,
            CheckpointOutcome::SkippedBusy,
            "a held write lock skips the checkpoint, it does not escalate"
        );
        // The lock connection's busy budget is 1s; generous smoke margin.
        assert!(
            elapsed < Duration::from_secs(4),
            "SkippedBusy within the busy budget, took {elapsed:?}"
        );

        stray.execute_batch("ROLLBACK").expect("release the lock");
        let outcome = harness.engine.checkpoint_once().await.expect("checkpoint");
        assert_eq!(
            outcome,
            CheckpointOutcome::Completed,
            "the next attempt succeeds once the stray writer releases"
        );
    }
}
