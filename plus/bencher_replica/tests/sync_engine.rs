#![cfg(all(feature = "plus", feature = "testing"))]
//! Sync engine suite: shipping, resume, backoff, snapshots, and pruning.
//!
//! Every test drives the step-driven engine core directly (`sync_once`,
//! `ship_once`, `checkpoint_once`, `snapshot_step`, `prune_once`) against a
//! real `SQLite` database (`WalFixture`) and a fault-injectable replica
//! (`FlakyStorage` over `LocalStorage`), with all scheduling under an
//! injected `Clock`.
//!
//! NOTE: `unused_crate_dependencies` cannot be handled with a crate-level
//! `#![expect]` here (see `tests/storage_contract.rs`); unused package
//! dependencies are referenced explicitly instead, as rustc recommends.

use async_compression as _;
use aws_credential_types as _;
use aws_sdk_s3 as _;
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

/// Shared fixtures: a scripted source database, a fault-injectable replica,
/// and an engine built over both with a deterministic clock.
#[cfg(test)]
pub(crate) mod harness {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicI64, Ordering};

    use bencher_json::system::config::{JsonReplication, ReplicationTarget};
    use bencher_json::{Clock, DateTime};
    use bencher_replica::testing::{
        FailurePlan, FlakyStorage, OpKind, OpOutcome, WalFixture, assert_replica_equivalent,
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

    /// Parse the `[start, end)` byte range out of a segment object key.
    pub(crate) fn segment_range(key: &str) -> (u64, u64) {
        let (_, file) = key.rsplit_once('/').expect("segment key has a directory");
        let range = file.strip_suffix(".wal.zst").expect("segment key suffix");
        let (start, end) = range.split_once('-').expect("segment range separator");
        (
            start.parse().expect("segment start offset"),
            end.parse().expect("segment end offset"),
        )
    }

    pub(crate) struct Harness {
        pub fixture: WalFixture,
        pub engine: SyncEngine<()>,
        pub config: ReplicaConfig,
        pub db: ReplicaDb<()>,
        pub clock_secs: Arc<AtomicI64>,
        pub replica_root: Utf8PathBuf,
        pub shadow: bool,
        pub _fixture_tmp: tempfile::TempDir,
        pub _replica_tmp: tempfile::TempDir,
    }

    impl Harness {
        pub(crate) async fn new() -> Self {
            Self::with_config(|_| {}).await
        }

        pub(crate) async fn with_config<F>(overrides: F) -> Self
        where
            F: FnOnce(&mut JsonReplication),
        {
            Self::with_shadow_config(false, overrides).await
        }

        pub(crate) async fn with_shadow_config<F>(shadow: bool, overrides: F) -> Self
        where
            F: FnOnce(&mut JsonReplication),
        {
            let fixture_tmp = tempfile::tempdir().expect("fixture tempdir");
            let replica_tmp = tempfile::tempdir().expect("replica tempdir");
            let fixture = WalFixture::new(dir_path(&fixture_tmp), PAGE_SIZE).expect("fixture");
            let replica_root = dir_path(&replica_tmp).to_path_buf();
            let mut json = JsonReplication {
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
            overrides(&mut json);
            let config = ReplicaConfig::try_from(json).expect("config");
            let clock_secs = Arc::new(AtomicI64::new(BASE_SECS));
            let db = ReplicaDb {
                db_path: fixture.db_path(),
                writer: Arc::new(tokio::sync::Mutex::new(())),
                busy_timeout_ms: 5000,
            };
            let engine = SyncEngine::new_with_storage(
                logger(),
                config.clone(),
                db.clone(),
                clock_for(&clock_secs),
                shadow,
                flaky_over(&replica_root),
            )
            .await
            .expect("engine");
            Self {
                fixture,
                engine,
                config,
                db,
                clock_secs,
                replica_root,
                shadow,
                _fixture_tmp: fixture_tmp,
                _replica_tmp: replica_tmp,
            }
        }

        /// The fault-injection wrapper the engine was built with.
        pub(crate) fn flaky(&self) -> &FlakyStorage {
            if let ReplicaStorage::Flaky(flaky) = self.engine.storage() {
                flaky
            } else {
                panic!("harness engine always wraps Flaky storage")
            }
        }

        /// Advance the injected clock by `secs`.
        pub(crate) fn advance(&self, secs: i64) {
            self.clock_secs.fetch_add(secs, Ordering::SeqCst);
        }

        /// Number of put operations attempted so far (any outcome).
        pub(crate) fn put_attempts(&self) -> usize {
            self.flaky()
                .journal()
                .iter()
                .filter(|(op, _)| op.kind == OpKind::Put)
                .count()
        }

        /// Segment object keys successfully shipped so far, in journal order.
        pub(crate) fn shipped_segment_keys(&self) -> Vec<String> {
            self.flaky()
                .journal()
                .iter()
                .filter(|(op, outcome)| {
                    op.kind == OpKind::Put
                        && *outcome == OpOutcome::Ok
                        && op.key.ends_with(".wal.zst")
                })
                .map(|(op, _)| op.key.clone())
                .collect()
        }

        /// Drive `sync_once` until the engine is streaming (the fresh-replica
        /// bootstrap snapshot has completed).
        pub(crate) async fn until_streaming(&mut self) {
            for _ in 0..64 {
                if self.engine.state() == EngineState::Streaming {
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

        /// Bootstrap plus one sync tick, so the initial WAL backlog is
        /// shipped and the engine is quiescent.
        pub(crate) async fn ready(&mut self) {
            self.until_streaming().await;
            self.engine.sync_once().await.expect("backlog sync");
        }

        /// Replace the engine (dropping the old one) with a freshly resumed
        /// engine over the same replica; the flaky journal starts empty.
        pub(crate) async fn rebuild_engine(&mut self) {
            self.engine = SyncEngine::new_with_storage(
                logger(),
                self.config.clone(),
                self.db.clone(),
                clock_for(&self.clock_secs),
                self.shadow,
                flaky_over(&self.replica_root),
            )
            .await
            .expect("engine rebuild");
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
    use std::time::Duration;

    use bencher_json::DateTime;
    use bencher_replica::testing::{CheckpointMode, FailurePlan, OpKind, WriteProbe};
    use bencher_replica::{
        CheckpointOutcome, EngineState, GenerationId, Replicator, SnapshotStatus, VerifyReport,
        WalScanner, decompress_segment,
    };
    use bytes::Bytes;
    use pretty_assertions::assert_eq;

    use super::harness::{BASE_SECS, Harness, PAGE_SIZE, clock_for, logger, segment_range};

    /// Read the live WAL header salts of the fixture.
    fn wal_salt(harness: &Harness) -> (u32, u32) {
        let wal = harness.fixture.wal_bytes().expect("wal bytes");
        WalScanner::open(Cursor::new(wal))
            .expect("wal header")
            .expect("wal is not empty")
            .header()
            .salt
    }

    /// Count objects on the replica whose key ends with `suffix`.
    async fn object_count(harness: &Harness, suffix: &str) -> usize {
        harness
            .engine
            .storage()
            .list("generations")
            .await
            .expect("list replica")
            .iter()
            .filter(|key| key.ends_with(suffix))
            .count()
    }

    #[tokio::test]
    async fn sync_ships_nothing_when_no_new_commits() {
        let mut harness = Harness::new().await;
        harness.ready().await;
        harness
            .fixture
            .txn(&["INSERT INTO t (data) VALUES ('one')"])
            .expect("txn");
        let progress = harness.engine.sync_once().await.expect("sync");
        assert_eq!(progress.shipped_segments, 1, "the new commit ships");

        let puts_before = harness.put_attempts();
        let progress = harness.engine.sync_once().await.expect("quiet sync");
        assert_eq!(progress.shipped_segments, 0, "nothing new to ship");
        assert_eq!(
            harness.put_attempts(),
            puts_before,
            "a quiet tick issues no storage puts"
        );
    }

    #[tokio::test]
    async fn sync_ships_only_delta_frames() {
        let mut harness = Harness::new().await;
        harness.ready().await;
        harness
            .fixture
            .txn(&["INSERT INTO t (data) VALUES ('a')"])
            .expect("txn a");
        harness.engine.sync_once().await.expect("sync a");
        let end_a = harness.engine.position().expect("position").offset;
        harness
            .fixture
            .txn(&["INSERT INTO t (data) VALUES ('b')"])
            .expect("txn b");
        harness.engine.sync_once().await.expect("sync b");
        let end_b = harness.engine.position().expect("position").offset;
        assert!(end_b > end_a, "position advances: {end_a} -> {end_b}");

        let keys = harness.shipped_segment_keys();
        assert!(keys.len() >= 2, "at least two segments shipped: {keys:?}");
        let (start_b, key_end_b) = segment_range(keys.last().expect("segment b"));
        let (_, key_end_a) = segment_range(keys.get(keys.len() - 2).expect("segment a"));
        assert_eq!(
            start_b, key_end_a,
            "the second segment starts exactly at the first segment's end"
        );
        assert_eq!(key_end_a, end_a, "segment a covers through position a");
        assert_eq!(key_end_b, end_b, "segment b covers through position b");
    }

    #[tokio::test]
    async fn first_segment_of_epoch_contains_wal_header() {
        let mut harness = Harness::new().await;
        harness.ready().await;
        let keys = harness.shipped_segment_keys();
        let first = keys.first().expect("an epoch-0 segment shipped");
        let (start, end) = segment_range(first);
        assert_eq!(start, 0, "the first segment of an epoch starts at 0");

        let compressed = harness
            .engine
            .storage()
            .get(first)
            .await
            .expect("get first segment");
        let raw = decompress_segment(&compressed).expect("decompress");
        assert_eq!(
            raw.len(),
            usize::try_from(end).expect("segment end"),
            "the stored segment covers [0, end)"
        );
        // The stored bytes parse as a self-contained, checksum-valid WAL.
        let mut scanner = WalScanner::open(Cursor::new(raw.clone()))
            .expect("stored segment parses as a WAL")
            .expect("stored segment has a header");
        let mut last_end = 0;
        while let Some(chunk) = scanner.next_committed(u64::MAX).expect("scan stored WAL") {
            last_end = chunk.end_offset;
        }
        assert_eq!(
            last_end,
            u64::try_from(raw.len()).expect("raw len"),
            "every stored frame verifies against the embedded header"
        );
    }

    #[tokio::test]
    async fn restart_resumes_from_replica_list() {
        let mut harness = Harness::new().await;
        harness.ready().await;
        harness
            .fixture
            .txn(&["INSERT INTO t (data) VALUES ('pre-restart')"])
            .expect("txn");
        harness.engine.sync_once().await.expect("sync");
        let position = harness.engine.position().cloned().expect("position");

        harness.rebuild_engine().await;
        assert_eq!(
            harness.engine.position(),
            Some(&position),
            "resume from the replica LIST reproduces the exact position (offset and checksum)"
        );
        // A no-write sync re-ships nothing.
        let progress = harness.engine.sync_once().await.expect("quiet sync");
        assert_eq!(progress.shipped_segments, 0, "no re-ship after restart");
        assert_eq!(harness.put_attempts(), 0, "no puts after restart");

        // A new write ships exactly the gap.
        harness
            .fixture
            .txn(&["INSERT INTO t (data) VALUES ('post-restart')"])
            .expect("txn");
        harness.engine.sync_once().await.expect("sync delta");
        let keys = harness.shipped_segment_keys();
        assert_eq!(keys.len(), 1, "exactly one delta segment: {keys:?}");
        let (start, _) = segment_range(keys.first().expect("delta segment"));
        assert_eq!(start, position.offset, "the delta starts at the old end");
        harness.assert_restore_equivalent().await;
    }

    #[tokio::test]
    async fn restart_after_checkpoint_resumes_as_next_epoch() {
        let mut harness = Harness::new().await;
        harness.ready().await;
        harness
            .fixture
            .txn(&["INSERT INTO t (data) VALUES ('epoch-0')"])
            .expect("txn");
        harness.engine.sync_once().await.expect("sync");
        let epoch_before = harness.engine.position().expect("position").epoch;
        let outcome = harness.engine.checkpoint_once().await.expect("checkpoint");
        assert_eq!(outcome, CheckpointOutcome::Completed, "full backfill");

        // The WAL is fully backfilled, so this write restarts it (new salts).
        harness
            .fixture
            .txn(&["INSERT INTO t (data) VALUES ('epoch-1')"])
            .expect("txn after checkpoint");

        harness.rebuild_engine().await;
        let position = harness.engine.position().expect("resumed position");
        assert_eq!(
            position.epoch,
            epoch_before + 1,
            "meta-verified resume continues as the next epoch, no re-snapshot"
        );
        assert_eq!(position.offset, 0, "the new epoch starts at offset 0");
        assert_eq!(
            position.salt,
            wal_salt(&harness),
            "the new epoch binds the local WAL header salts"
        );
        harness.engine.sync_once().await.expect("ship new epoch");
        harness.assert_restore_equivalent().await;
    }

    #[tokio::test]
    async fn restart_with_unshipped_frames_lost_to_reset_forces_new_generation() {
        let mut harness = Harness::new().await;
        harness.ready().await;
        harness
            .fixture
            .txn(&["INSERT INTO t (data) VALUES ('shipped')"])
            .expect("txn a");
        harness.engine.sync_once().await.expect("sync");
        let old_generation = harness.engine.generation().cloned().expect("generation");

        // Unshipped commit, then a stray TRUNCATE checkpoint resets the WAL:
        // the unshipped frames are gone from the WAL (they live only in the
        // db file now) and the next write starts a new salt cycle.
        harness
            .fixture
            .txn(&["INSERT INTO t (data) VALUES ('unshipped')"])
            .expect("txn b");
        harness
            .fixture
            .checkpoint(CheckpointMode::Truncate)
            .expect("stray TRUNCATE");
        harness
            .fixture
            .txn(&["INSERT INTO t (data) VALUES ('new-cycle')"])
            .expect("txn c");

        harness.rebuild_engine().await;
        assert_eq!(
            harness.engine.state(),
            EngineState::PendingSnapshot,
            "salt change with unshipped loss resolves to a new generation, never silently"
        );
        harness.ready().await;
        assert!(
            harness
                .engine
                .generation()
                .is_some_and(|generation| { generation != &old_generation }),
            "a new generation was created"
        );
        harness.assert_restore_equivalent().await;
    }

    #[tokio::test]
    async fn stray_writer_frames_picked_up() {
        let mut harness = Harness::new().await;
        harness.ready().await;
        let stray = harness.fixture.stray_conn().expect("stray conn");
        stray
            .execute("INSERT INTO t (data) VALUES ('stray')", [])
            .expect("stray insert");
        let progress = harness.engine.sync_once().await.expect("sync");
        assert!(
            progress.shipped_segments >= 1,
            "stray-connection commits replicate: {progress:?}"
        );
        harness.assert_restore_equivalent().await;
    }

    #[tokio::test]
    async fn storage_outage_defers_checkpoint_and_catches_up() {
        let mut harness = Harness::new().await;
        harness.ready().await;
        harness
            .flaky()
            .set_plan(FailurePlan::new().fail_all(OpKind::Put));

        harness
            .fixture
            .txn(&["INSERT INTO t (data) VALUES ('outage-1')"])
            .expect("txn 1");
        let wal_len_1 = harness.fixture.wal_bytes().expect("wal").len();
        let progress = harness.engine.sync_once().await.expect("sync in outage");
        assert!(
            progress.error.is_some(),
            "the tick reports the storage error class: {progress:?}"
        );
        let progress = harness.engine.sync_once().await.expect("gated sync");
        assert!(
            progress.backing_off,
            "the next tick is gated by backoff: {progress:?}"
        );

        harness
            .fixture
            .txn(&["INSERT INTO t (data) VALUES ('outage-2')"])
            .expect("txn 2");
        let wal_len_2 = harness.fixture.wal_bytes().expect("wal").len();
        assert!(
            wal_len_2 > wal_len_1,
            "the WAL grows monotonically during the outage ({wal_len_1} -> {wal_len_2})"
        );
        let outcome = harness.engine.checkpoint_once().await.expect("checkpoint");
        assert_eq!(
            outcome,
            CheckpointOutcome::SkippedUnshipped,
            "no checkpoint while unshipped frames exist (I1): the WAL is the buffer"
        );
        harness
            .fixture
            .txn(&["INSERT INTO t (data) VALUES ('outage-3')"])
            .expect("txn 3");
        let wal_len_3 = harness.fixture.wal_bytes().expect("wal").len();
        assert!(wal_len_3 > wal_len_2, "the WAL keeps growing");

        // Heal, wait out the backoff, and catch up in order.
        harness.flaky().heal();
        harness.advance(400);
        let progress = harness.engine.sync_once().await.expect("catch-up sync");
        assert!(progress.error.is_none(), "healed tick: {progress:?}");
        assert!(
            progress.shipped_segments >= 1,
            "the backlog ships after healing: {progress:?}"
        );
        let keys = harness.shipped_segment_keys();
        let mut expected_start = None;
        for key in &keys {
            let (start, end) = segment_range(key);
            if let Some(expected) = expected_start {
                assert_eq!(start, expected, "segments ship in order: {keys:?}");
            }
            expected_start = Some(end);
        }
        harness.assert_restore_equivalent().await;
    }

    #[tokio::test]
    async fn backoff_delays_follow_sequence() {
        let mut harness = Harness::new().await;
        harness.ready().await;
        harness
            .flaky()
            .set_plan(FailurePlan::new().fail_all(OpKind::Put));
        harness
            .fixture
            .txn(&["INSERT INTO t (data) VALUES ('backoff')"])
            .expect("txn");

        // The bootstrap already issued puts; count attempts from here.
        let base = harness.put_attempts();

        // Attempt at t0 fails and arms a 1s delay.
        harness.engine.sync_once().await.expect("sync t0");
        assert_eq!(harness.put_attempts(), base + 1, "first attempt at t0");
        harness.engine.sync_once().await.expect("sync t0 again");
        assert_eq!(harness.put_attempts(), base + 1, "gated before t0+1");

        harness.advance(1); // t0+1
        harness.engine.sync_once().await.expect("sync t0+1");
        assert_eq!(harness.put_attempts(), base + 2, "second attempt at t0+1");

        harness.advance(1); // t0+2
        harness.engine.sync_once().await.expect("sync t0+2");
        assert_eq!(
            harness.put_attempts(),
            base + 2,
            "gated before t0+3 (2s delay)"
        );

        harness.advance(1); // t0+3
        harness.engine.sync_once().await.expect("sync t0+3");
        assert_eq!(harness.put_attempts(), base + 3, "third attempt at t0+3");

        harness.advance(3); // t0+6
        harness.engine.sync_once().await.expect("sync t0+6");
        assert_eq!(
            harness.put_attempts(),
            base + 3,
            "gated before t0+7 (4s delay)"
        );

        harness.advance(1); // t0+7
        harness.engine.sync_once().await.expect("sync t0+7");
        assert_eq!(harness.put_attempts(), base + 4, "fourth attempt at t0+7");
    }

    #[tokio::test]
    async fn snapshot_end_to_end_restore_equivalent() {
        let mut harness = Harness::new().await;
        harness.ready().await;
        harness
            .fixture
            .txn(&["INSERT INTO t (data) VALUES ('before-snapshot')"])
            .expect("txn");
        harness.engine.sync_once().await.expect("sync");
        let old_generation = harness.engine.generation().cloned().expect("generation");

        harness.engine.trigger_snapshot();
        for step in 0..1000 {
            let status = harness.engine.snapshot_step().await.expect("snapshot step");
            if status == SnapshotStatus::Finished {
                break;
            }
            assert!(step < 999, "snapshot never finished");
        }
        let new_generation = harness.engine.generation().cloned().expect("generation");
        assert!(
            new_generation != old_generation,
            "a snapshot always creates a new generation"
        );
        assert_eq!(
            harness.engine.state(),
            EngineState::Streaming,
            "back to streaming after the snapshot"
        );
        // The accumulated WAL backlog re-ships into the new generation.
        harness
            .engine
            .sync_once()
            .await
            .expect("post-snapshot sync");
        harness.assert_restore_equivalent().await;
    }

    #[tokio::test]
    async fn snapshot_writes_proceed_between_steps() {
        // 1 MiB copy budget per step forces a multi-step copy.
        let mut harness = Harness::with_config(|json| {
            json.snapshot_throttle_mib = Some(1);
        })
        .await;
        harness.ready().await;
        // Grow the database FILE (not just the WAL): big transaction, ship,
        // full checkpoint.
        harness
            .fixture
            .txn_touching_pages(800)
            .expect("big transaction");
        harness.engine.ship_once().await.expect("ship big txn");
        let outcome = harness.engine.checkpoint_once().await.expect("checkpoint");
        assert_eq!(
            outcome,
            CheckpointOutcome::Completed,
            "backfill the db file"
        );

        let probe = WriteProbe::new(&harness.fixture.db_path(), Duration::from_secs(5));
        harness.engine.trigger_snapshot();
        let mut steps = 0u32;
        loop {
            let status = harness.engine.snapshot_step().await.expect("snapshot step");
            steps += 1;
            // The anti-Litestream regression: a real second connection
            // writes IMMEDIATELY between snapshot steps; the snapshot holds
            // no locks of any kind.
            let result = probe.write_once().await;
            result
                .result
                .expect("write between snapshot steps succeeds");
            assert!(
                result.blocked < Duration::from_secs(2),
                "write between snapshot steps must not block: {:?}",
                result.blocked
            );
            if status == SnapshotStatus::Finished {
                break;
            }
            assert!(steps < 1000, "snapshot never finished");
        }
        assert!(
            steps > 4,
            "the throttled copy must span multiple steps, got {steps}"
        );
        // The probe writes accumulated in the WAL ship into the new
        // generation and survive a restore.
        harness
            .engine
            .sync_once()
            .await
            .expect("post-snapshot sync");
        harness.assert_restore_equivalent().await;
    }

    /// An external checkpointer (even a TRUNCATE checkpoint after a stray
    /// write) mid-snapshot must NOT tear or abort the snapshot: the body
    /// comes from a single-step online backup taken through the pager, so
    /// it is transactionally consistent no matter who checkpoints. This is
    /// what makes shadow mode (Litestream checkpointing freely) viable.
    #[tokio::test]
    async fn snapshot_survives_external_checkpoint_mid_copy() {
        let mut harness = Harness::with_config(|json| {
            json.snapshot_throttle_mib = Some(1);
        })
        .await;
        harness.ready().await;
        harness
            .fixture
            .txn_touching_pages(800)
            .expect("big transaction");
        harness.engine.ship_once().await.expect("ship big txn");
        let outcome = harness.engine.checkpoint_once().await.expect("checkpoint");
        assert_eq!(
            outcome,
            CheckpointOutcome::Completed,
            "backfill the db file"
        );
        assert_eq!(
            object_count(&harness, "snapshot.json").await,
            1,
            "bootstrap"
        );

        // Enter the multi-step Copying phase (the backup already happened
        // at CreateGeneration; the steps upload the scratch file).
        harness.engine.trigger_snapshot();
        for _ in 0..3 {
            let status = harness.engine.snapshot_step().await.expect("snapshot step");
            assert_eq!(status, SnapshotStatus::InProgress, "copy still running");
        }
        // An external checkpointer mutates the LIVE db file mid-upload: a
        // stray write that grows the database, then a TRUNCATE checkpoint.
        // The scratch upload is unaffected.
        let stray = harness.fixture.stray_conn().expect("stray conn");
        let big = "x".repeat(64 * 1024);
        stray
            .execute("INSERT INTO t (data) VALUES (?1)", [&big])
            .expect("stray insert");
        harness
            .fixture
            .checkpoint(CheckpointMode::Truncate)
            .expect("external checkpoint");

        // The snapshot completes despite the external churn.
        for _ in 0..256 {
            if harness.engine.state() != EngineState::Snapshotting {
                break;
            }
            harness.engine.snapshot_step().await.expect("snapshot step");
        }
        assert_eq!(
            object_count(&harness, "snapshot.json").await,
            2,
            "the snapshot committed"
        );
        // The stray write (which the WAL restart could have buried) is
        // recaptured: divergence handling plus the fresh snapshot keep the
        // replica equivalent end to end.
        for _ in 0..64 {
            let progress = harness.engine.sync_once().await.expect("recovery sync");
            if progress.shipped_segments == 0
                && harness.engine.state() == EngineState::Streaming
                && !progress.backing_off
            {
                break;
            }
            harness.advance(1);
        }
        if harness.engine.state() == EngineState::Snapshotting
            || object_count(&harness, "snapshot.json").await > 2
        {
            // A divergence-triggered replacement snapshot may be running;
            // drive it home.
            for _ in 0..256 {
                if harness.engine.state() == EngineState::Streaming {
                    break;
                }
                harness.engine.sync_once().await.expect("replacement sync");
            }
        }
        harness.engine.sync_once().await.expect("final drain");
        harness.assert_restore_equivalent().await;
    }

    #[tokio::test]
    async fn prune_keeps_newest_n_and_reaps_stale_incomplete() {
        let mut harness = Harness::with_config(|json| {
            json.retention_generations = Some(2);
        })
        .await;
        harness.until_streaming().await;
        let current = harness.engine.generation().cloned().expect("generation");

        let old = |age_secs: i64, suffix: u32| {
            GenerationId::new(
                DateTime::try_from(BASE_SECS - age_secs).expect("timestamp"),
                suffix,
            )
        };
        let complete_old = old(200_000, 1);
        let complete_mid = old(150_000, 2);
        let stale_incomplete = old(100_000, 3); // > 24h without snapshot.json
        let fresh_incomplete = old(3_600, 4); // 1h old, still in flight
        let storage = harness.engine.storage();
        for generation in [&complete_old, &complete_mid] {
            storage
                .put(
                    &format!("generations/{}/snapshot.db.zst", generation.as_str()),
                    Bytes::from_static(b"body"),
                )
                .await
                .expect("put body");
            storage
                .put(
                    &format!("generations/{}/snapshot.json", generation.as_str()),
                    Bytes::from_static(b"{}"),
                )
                .await
                .expect("put marker");
        }
        for generation in [&stale_incomplete, &fresh_incomplete] {
            storage
                .put(
                    &format!("generations/{}/snapshot.db.zst", generation.as_str()),
                    Bytes::from_static(b"body"),
                )
                .await
                .expect("put orphan body");
        }

        harness.engine.prune_once().await.expect("prune");
        let dirs = harness
            .engine
            .storage()
            .list_dirs("generations")
            .await
            .expect("list generations");
        assert_eq!(
            dirs,
            vec![
                complete_mid.as_str().to_owned(),
                fresh_incomplete.as_str().to_owned(),
                current.as_str().to_owned(),
            ],
            "keep the newest 2 complete generations plus fresh incomplete ones"
        );
    }

    #[tokio::test]
    async fn new_generation_epoch_zero_rebinding() {
        let mut harness = Harness::new().await;
        harness.ready().await;
        harness
            .fixture
            .txn(&["INSERT INTO t (data) VALUES ('pre')"])
            .expect("txn");
        harness.engine.sync_once().await.expect("sync");
        let outcome = harness.engine.checkpoint_once().await.expect("checkpoint");
        assert_eq!(outcome, CheckpointOutcome::Completed, "full backfill");

        // Snapshot with a fully backfilled WAL and no writes in between: the
        // boundary binds to the old salt cycle.
        harness.engine.trigger_snapshot();
        for step in 0..1000 {
            let status = harness.engine.snapshot_step().await.expect("snapshot step");
            if status == SnapshotStatus::Finished {
                break;
            }
            assert!(step < 999, "snapshot never finished");
        }
        let new_generation = harness.engine.generation().cloned().expect("generation");
        let boundary_salt = harness.engine.position().expect("position").salt;

        // A post-snapshot write restarts the WAL BEFORE any epoch-0 segment
        // ships: epoch 0 must rebind to the new cycle, not leave an empty
        // epoch behind.
        harness
            .fixture
            .txn(&["INSERT INTO t (data) VALUES ('post-snapshot')"])
            .expect("txn after snapshot");
        let new_salt = wal_salt(&harness);
        assert!(
            new_salt != boundary_salt,
            "the write after a full backfill restarts the WAL"
        );
        let progress = harness.engine.sync_once().await.expect("sync");
        assert!(
            progress.shipped_segments >= 1,
            "the restarted cycle ships: {progress:?}"
        );
        let position = harness.engine.position().expect("position");
        assert_eq!(position.epoch, 0, "epoch numbering stays contiguous from 0");
        assert_eq!(position.salt, new_salt, "epoch 0 rebound to the new salts");

        // Exactly one epoch directory exists, named for epoch 0 with the
        // NEW salts.
        let prefix = format!("generations/{}/wal/", new_generation.as_str());
        let keys = harness
            .engine
            .storage()
            .list(&prefix)
            .await
            .expect("list epoch dirs");
        let mut epoch_dirs: Vec<String> = keys
            .iter()
            .filter_map(|key| {
                key.strip_prefix(&prefix)
                    .and_then(|rest| rest.split_once('/'))
                    .map(|(dir, _)| dir.to_owned())
            })
            .collect();
        epoch_dirs.dedup();
        assert_eq!(
            epoch_dirs,
            vec![format!("{:010}-{:08x}{:08x}", 0, new_salt.0, new_salt.1)],
            "one epoch dir: epoch 0 under the new salts"
        );
        harness.assert_restore_equivalent().await;
    }

    #[tokio::test]
    async fn replicator_shutdown_ships_tail() {
        // Pre-populate a valid generation with a manually driven engine.
        let mut harness = Harness::new().await;
        harness.ready().await;
        harness
            .fixture
            .txn(&["INSERT INTO t (data) VALUES ('shipped')"])
            .expect("txn");
        harness.engine.sync_once().await.expect("sync");
        harness.rebuild_engine().await; // drop the active engine

        // An unshipped tail, then the production shell: start + shutdown.
        // final_sync ships the tail without any tick having elapsed.
        harness
            .fixture
            .txn(&["INSERT INTO t (data) VALUES ('tail')"])
            .expect("tail txn");
        let handle = Replicator::start(
            logger(),
            harness.config.clone(),
            harness.db.clone(),
            clock_for(&harness.clock_secs),
            false,
        );
        handle
            .shutdown(Duration::from_secs(10))
            .await
            .expect("shutdown");
        harness.assert_restore_equivalent().await;
    }

    /// A post-power-loss rewind fork: the local WAL still reaches the
    /// shipped offset with a VALID checksum chain, but the content differs
    /// from what the replica stored (with `synchronous = NORMAL`, committed
    /// frames can be lost from the page cache and re-written differently).
    /// Resume must compare content against the replica tip, not just chain
    /// length, and force a new generation.
    #[tokio::test]
    async fn resume_rewind_fork_forces_new_generation() {
        let mut harness = Harness::new().await;
        harness.ready().await;
        harness
            .fixture
            .txn(&["INSERT INTO t (data) VALUES ('pre fork')"])
            .expect("txn");
        harness.engine.sync_once().await.expect("sync");
        let before = harness.engine.generation().cloned();

        // Simulate the fork by doctoring the LAST frame in place: same
        // length, same commit flag, different payload, RECOMPUTED cumulative
        // checksum, so the local chain is fully valid but the bytes differ
        // from the shipped segment.
        let wal_path = harness.fixture.wal_path();
        let mut wal = std::fs::read(&wal_path).expect("read wal");
        let frame_size = 24 + PAGE_SIZE as usize;
        assert!(wal.len() >= 32 + 2 * frame_size, "need at least two frames");
        let last = wal.len() - frame_size;
        // Seed: the cumulative checksum stored in the PREVIOUS frame header.
        let prev = last - frame_size;
        let prev_header: [u8; 24] = wal[prev..prev + 24].try_into().expect("frame header");
        let seed = bencher_replica::FrameHeader::parse(&prev_header).checksum;
        let header: [u8; 24] = wal[..24].try_into().expect("wal header prefix");
        // Manual byte math: the workspace warns on from_be_bytes/to_be_bytes.
        let big_endian = header[3] & 1 == 1;
        // Flip a payload byte, then recompute the frame checksum over the
        // first 8 header bytes plus the full page payload.
        wal[last + 24 + 100] ^= 0xFF;
        let mut input = Vec::with_capacity(8 + PAGE_SIZE as usize);
        input.extend_from_slice(&wal[last..last + 8]);
        input.extend_from_slice(&wal[last + 24..last + frame_size]);
        let (c1, c2) = bencher_replica::wal_checksum(big_endian, seed, &input);
        for (index, checksum) in [c1, c2].into_iter().enumerate() {
            let at = last + 16 + index * 4;
            wal[at] = u8::try_from((checksum >> 24) & 0xff).expect("byte");
            wal[at + 1] = u8::try_from((checksum >> 16) & 0xff).expect("byte");
            wal[at + 2] = u8::try_from((checksum >> 8) & 0xff).expect("byte");
            wal[at + 3] = u8::try_from(checksum & 0xff).expect("byte");
        }
        std::fs::write(&wal_path, wal).expect("write doctored wal");

        // Resume must detect the fork and start a new generation.
        harness.rebuild_engine().await;
        harness.until_streaming().await;
        harness.engine.sync_once().await.expect("backlog");
        let after = harness.engine.generation().cloned();
        assert_ne!(before, after, "a rewind fork must force a new generation");
        harness.assert_restore_equivalent().await;
    }

    /// A WAL cycle buried behind the engine's back BETWEEN ticks (stray
    /// restart, stray TRUNCATE checkpoint, stray restart) jumps salt1 by
    /// more than one: the live epoch transition must refuse the shortcut
    /// and force a new generation, and the fresh snapshot recaptures the
    /// buried commits.
    #[tokio::test]
    async fn live_buried_wal_cycle_forces_new_generation() {
        let mut harness = Harness::new().await;
        harness.ready().await;
        harness
            .fixture
            .txn(&["INSERT INTO t (data) VALUES ('shipped')"])
            .expect("txn");
        harness.engine.sync_once().await.expect("sync");
        let outcome = harness.engine.checkpoint_once().await.expect("checkpoint");
        assert_eq!(outcome, CheckpointOutcome::Completed);

        // Bury a whole cycle: restart (fully backfilled WAL), TRUNCATE
        // checkpoint backfills the never-shipped frames, restart again.
        harness
            .fixture
            .txn(&["CREATE TABLE buried (id INTEGER PRIMARY KEY, data TEXT)"])
            .expect("buried txn");
        harness
            .fixture
            .checkpoint(CheckpointMode::Truncate)
            .expect("stray truncate");
        harness
            .fixture
            .txn(&["INSERT INTO buried (data) VALUES ('after burial')"])
            .expect("post burial txn");

        // The live transition detects the salt discontinuity and diverges.
        let before = harness.engine.generation().cloned();
        harness.engine.sync_once().await.expect("sync");
        harness.until_streaming().await;
        harness.engine.sync_once().await.expect("backlog");
        let after = harness.engine.generation().cloned();
        assert_ne!(before, after, "a buried cycle must force a new generation");
        // The buried table survives on the replica via the fresh snapshot.
        harness.assert_restore_equivalent().await;
    }

    /// The salt-collision poison scenario, end to end. A generation's epoch 1
    /// is present but corrupt (parseable keys, broken checksum chain). After a
    /// volume loss the restore soft-stops at epoch 0 and pins an advisory meta
    /// to epoch 0, while the replica tip is still the corrupt epoch 1. On the
    /// following resume the engine must NOT bind fresh salts and extend the
    /// poisoned generation (that is what would leave it with two epoch-1
    /// directories, which `plan_epochs` can only ever soft-stop on): the
    /// `meta.epoch == tip.epoch` proof fails, so resume diverges to a brand-new
    /// generation. This is the guard that keeps the pessimistic collision
    /// handling in `plan_epochs` unreachable in practice.
    #[tokio::test]
    async fn resume_after_soft_stop_below_corrupt_epoch_forces_new_generation() {
        use std::sync::Arc;

        use bencher_replica::{
            LocalStorage, ReplicaDb, ReplicaMeta, ReplicaStorage, RestoreOutcome, SyncEngine,
            compress_segment, restore_if_missing,
        };

        use super::harness::dir_path;

        let mut harness = Harness::new().await;
        harness.ready().await;
        let poisoned = harness.engine.generation().cloned().expect("generation");

        // Epoch 0: ship one commit and checkpoint it, then restart the WAL
        // (new salts) into epoch 1 and ship that too.
        harness
            .fixture
            .txn(&["INSERT INTO t (data) VALUES ('epoch-0')"])
            .expect("epoch 0 txn");
        harness.engine.sync_once().await.expect("ship epoch 0");
        let outcome = harness.engine.checkpoint_once().await.expect("checkpoint");
        assert_eq!(outcome, CheckpointOutcome::Completed, "full backfill");
        harness
            .fixture
            .txn(&["INSERT INTO t (data) VALUES ('epoch-1')"])
            .expect("epoch 1 txn");
        harness.engine.sync_once().await.expect("ship epoch 1");

        // Corrupt epoch 1 IN PLACE so the object stays valid (recompressed,
        // same byte length) while the WAL checksum chain breaks: parseable
        // keys, invalid content. A later restore plans the epoch, then
        // soft-stops when the assembled WAL fails chain validation.
        let epoch1_key = harness
            .shipped_segment_keys()
            .into_iter()
            .find(|key| key.contains("/0000000001-"))
            .expect("an epoch-1 segment shipped");
        let compressed = harness
            .engine
            .storage()
            .get(&epoch1_key)
            .await
            .expect("get epoch-1 segment");
        let mut raw = decompress_segment(&compressed).expect("decompress epoch-1 segment");
        raw[24 + 50] ^= 0xff;
        let recompressed = compress_segment(&raw).expect("recompress tampered segment");
        harness
            .engine
            .storage()
            .put(&epoch1_key, Bytes::from(recompressed))
            .await
            .expect("put tampered epoch-1 segment");

        // Volume loss: restore into a fresh target. Replay soft-stops at
        // epoch 0 (the corrupt epoch 1 is discarded) and writes an advisory
        // meta pinned to epoch 0.
        let target_tmp = tempfile::tempdir().expect("restore target tempdir");
        let target_db = dir_path(&target_tmp).join("bencher.db");
        let restored = restore_if_missing(&logger(), &harness.config, &target_db)
            .await
            .expect("restore boots on epoch 0");
        assert!(
            matches!(restored, RestoreOutcome::Restored { .. }),
            "restore boots on the last good epoch, got {restored:?}"
        );
        let restored_meta = ReplicaMeta::load(&target_db)
            .expect("load restored meta")
            .expect("restore writes an advisory meta");
        assert_eq!(
            restored_meta.epoch, 0,
            "restore soft-stopped below the corrupt epoch 1, pinning the meta to epoch 0"
        );

        // Resume over the restored volume against the same replica. The tip is
        // the still-present epoch 1, but the meta records epoch 0: the
        // meta-verified epoch+1 path is refused and the engine diverges to a
        // fresh generation instead of colliding a second epoch-1 directory
        // into the poisoned generation.
        let db = ReplicaDb {
            db_path: target_db.clone(),
            writer: Arc::new(tokio::sync::Mutex::new(())),
            busy_timeout_ms: 5000,
        };
        let resumed = SyncEngine::new_with_storage(
            logger(),
            harness.config.clone(),
            db,
            clock_for(&harness.clock_secs),
            false,
            ReplicaStorage::Local(LocalStorage::new(harness.replica_root.clone())),
        )
        .await
        .expect("resume engine");
        assert_eq!(
            resumed.state(),
            EngineState::PendingSnapshot,
            "resume refuses to extend the poisoned generation and forces a fresh one"
        );
        assert_eq!(
            resumed.generation(),
            None,
            "no lineage is bound; a brand-new generation will be snapshotted next, \
             so the poisoned generation {} keeps its single epoch-1 directory",
            poisoned.as_str()
        );
    }

    /// Verification pins the shipped position and passes on a faithful
    /// replica; app writes proceed while the fingerprint and restore run.
    #[tokio::test]
    async fn verify_passes_after_writes() {
        let mut harness = Harness::new().await;
        harness.ready().await;
        harness
            .fixture
            .txn(&["INSERT INTO t (data) VALUES ('verified')"])
            .expect("txn");
        let report = harness.engine.verify_once().await.expect("verify");
        assert_eq!(report, Some(VerifyReport::Pass));
    }

    /// A fresh engine with no bound position cannot verify yet.
    #[tokio::test]
    async fn verify_unavailable_without_position() {
        let mut harness = Harness::new().await;
        // No bootstrap: the replica is empty and no position is bound.
        let report = harness.engine.verify_once().await.expect("verify");
        assert_eq!(report, None);
    }

    /// A tampered replica object fails verification, and (outside the
    /// rate-limit window) forces a new generation on the next tick.
    #[tokio::test]
    async fn verify_fail_on_tampered_replica_triggers_new_generation() {
        let mut harness = Harness::new().await;
        harness.ready().await;
        harness
            .fixture
            .txn(&["INSERT INTO t (data) VALUES ('will be tampered')"])
            .expect("txn");
        harness.engine.sync_once().await.expect("sync");

        // Corrupt the newest shipped segment directly on the local replica.
        let key = harness
            .shipped_segment_keys()
            .pop()
            .expect("a shipped segment");
        let object = harness.replica_root.join(&key);
        std::fs::write(&object, b"garbage, not zstd").expect("tamper");

        // Move past the retrigger rate limit so the failure forces a new
        // generation.
        harness.advance(7 * 60 * 60);
        let report = harness.engine.verify_once().await.expect("verify");
        assert!(
            matches!(report, Some(VerifyReport::Fail { .. })),
            "expected Fail, got {report:?}"
        );
        // The failure schedules a new generation: the next tick snapshots,
        // and the replica heals to full equivalence again.
        let progress = harness.engine.sync_once().await.expect("sync");
        assert!(
            progress.snapshot.is_some() || harness.engine.state() == EngineState::Snapshotting,
            "verification failure must trigger a new generation"
        );
        harness.until_streaming().await;
        harness.engine.sync_once().await.expect("backlog");
        harness.assert_restore_equivalent().await;
    }

    /// Scheduled verification runs through `sync_once` once the interval
    /// elapses on the injected clock.
    #[tokio::test]
    async fn verify_scheduled_via_sync_once() {
        let mut harness = Harness::with_config(|json| {
            json.verification_interval_secs = Some(60);
        })
        .await;
        harness.ready().await;
        harness
            .fixture
            .txn(&["INSERT INTO t (data) VALUES ('scheduled')"])
            .expect("txn");

        // Not due yet: no verification report.
        let progress = harness.engine.sync_once().await.expect("sync");
        assert_eq!(progress.verify, None);

        harness.advance(61);
        let progress = harness.engine.sync_once().await.expect("sync");
        assert_eq!(progress.verify, Some(VerifyReport::Pass));
    }

    /// A verification EXECUTION error (here: the replica cannot be read for
    /// the restore) must not arm the global ship backoff. Shipping proceeds
    /// on the failing tick and, crucially, on the very next tick.
    #[tokio::test]
    async fn verify_error_does_not_block_shipping() {
        let mut harness = Harness::with_config(|json| {
            json.verification_interval_secs = Some(60);
        })
        .await;
        harness.ready().await;
        // Verification restores the replica by reading (Get) its objects;
        // fail all Gets so verification cannot COMPLETE (an execution error,
        // distinct from a content Fail). Shipping only Puts, so it is
        // unaffected.
        harness
            .flaky()
            .set_plan(FailurePlan::new().fail_all(OpKind::Get));
        harness
            .fixture
            .txn(&["INSERT INTO t (data) VALUES ('a')"])
            .expect("txn a");
        harness.advance(61);
        let progress = harness
            .engine
            .sync_once()
            .await
            .expect("sync with verify error");
        assert!(
            progress.shipped_segments >= 1,
            "the write ships despite the verify error: {progress:?}"
        );
        assert!(
            progress.error.is_some(),
            "the verify execution error is reported: {progress:?}"
        );
        assert!(
            !progress.backing_off,
            "the failing tick is not itself gated"
        );

        // The decoupling: the verify error must NOT arm the global ship
        // backoff, so the very next tick ships a new write immediately.
        harness
            .fixture
            .txn(&["INSERT INTO t (data) VALUES ('b')"])
            .expect("txn b");
        let progress = harness.engine.sync_once().await.expect("next tick");
        assert!(
            !progress.backing_off,
            "a verify error must not throttle WAL shipping: {progress:?}"
        );
        assert!(
            progress.shipped_segments >= 1,
            "the next write ships immediately: {progress:?}"
        );
    }

    /// Sole mode: an EXTERNAL checkpointer restarting the WAL between the
    /// snapshot backup and finalize must ABORT the generation (no
    /// snapshot.json), never rebind epoch 0 onto a body that may be missing
    /// buried frames. A fresh, consistent generation then replaces it.
    #[tokio::test]
    async fn sole_finalize_aborts_on_external_wal_restart() {
        let mut harness = Harness::with_config(|json| {
            json.snapshot_throttle_mib = Some(1);
        })
        .await;
        harness.ready().await;
        // Grow the database FILE so the copy spans multiple steps and there is
        // room to interfere after the backup.
        harness
            .fixture
            .txn_touching_pages(400)
            .expect("big transaction");
        while harness.engine.ship_once().await.expect("ship big txn") > 0 {}
        let outcome = harness.engine.checkpoint_once().await.expect("checkpoint");
        assert_eq!(
            outcome,
            CheckpointOutcome::Completed,
            "backfill the db file"
        );
        let bootstrap_markers = object_count(&harness, "snapshot.json").await;

        harness.engine.trigger_snapshot();
        // Drive past CreateGeneration (records the boundary salt) into Copying.
        harness
            .engine
            .snapshot_step()
            .await
            .expect("ship tail step");
        harness
            .engine
            .snapshot_step()
            .await
            .expect("create generation step");
        assert_eq!(harness.engine.state(), EngineState::Snapshotting);

        // The engine's own checkpoints are suppressed while a snapshot runs,
        // so this WAL restart is provably external: TRUNCATE resets the WAL,
        // then a write starts a NEW salt cycle.
        harness
            .fixture
            .checkpoint(CheckpointMode::Truncate)
            .expect("external truncate");
        harness
            .fixture
            .txn(&["INSERT INTO t (data) VALUES ('buried new cycle')"])
            .expect("new cycle txn");

        // The Finalize step must abort (Err) rather than commit a torn marker.
        let mut aborted = false;
        for _ in 0..256 {
            if harness.engine.state() != EngineState::Snapshotting {
                break;
            }
            if harness.engine.snapshot_step().await.is_err() {
                aborted = true;
                break;
            }
        }
        assert!(
            aborted,
            "sole-mode finalize aborts on an external WAL restart"
        );
        assert_eq!(
            object_count(&harness, "snapshot.json").await,
            bootstrap_markers,
            "the aborted generation committed no snapshot.json"
        );

        // The engine scheduled a fresh generation; drive it home and confirm
        // the replica restores to an exact copy of the (now-larger) source.
        for _ in 0..256 {
            let progress = harness.engine.sync_once().await.expect("recovery sync");
            if harness.engine.state() == EngineState::Streaming
                && progress.shipped_segments == 0
                && progress.snapshot.is_none()
                && !progress.backing_off
            {
                break;
            }
            harness.advance(1);
        }
        assert_eq!(
            object_count(&harness, "snapshot.json").await,
            bootstrap_markers + 1,
            "a fresh consistent generation replaces the aborted one"
        );
        harness.assert_restore_equivalent().await;
    }

    /// Shadow mode: the same external WAL restart between backup and finalize
    /// is legitimate (Litestream owns checkpoints), so the marker IS still
    /// committed (the boundary rebinds; the shadow replica is disposable).
    #[tokio::test]
    async fn shadow_finalize_commits_despite_external_wal_restart() {
        let mut harness = Harness::with_shadow_config(true, |json| {
            json.snapshot_throttle_mib = Some(1);
        })
        .await;
        harness.until_streaming().await;
        harness
            .fixture
            .txn_touching_pages(400)
            .expect("big transaction");
        while harness.engine.ship_once().await.expect("ship big txn") > 0 {}
        let before = object_count(&harness, "snapshot.json").await;

        harness.engine.trigger_snapshot();
        harness
            .engine
            .snapshot_step()
            .await
            .expect("ship tail step");
        harness
            .engine
            .snapshot_step()
            .await
            .expect("create generation step");
        assert_eq!(harness.engine.state(), EngineState::Snapshotting);

        // An external checkpointer (Litestream) legitimately restarts the WAL.
        harness
            .fixture
            .checkpoint(CheckpointMode::Truncate)
            .expect("external truncate");
        harness
            .fixture
            .txn(&["INSERT INTO t (data) VALUES ('litestream cycle')"])
            .expect("new cycle txn");

        // Shadow mode keeps committing: the snapshot finishes and the marker
        // lands (unlike sole mode, which would abort).
        for step in 0..256 {
            let status = harness
                .engine
                .snapshot_step()
                .await
                .expect("shadow snapshot commits despite the restart");
            if status == SnapshotStatus::Finished {
                break;
            }
            assert!(step < 255, "shadow snapshot never finished");
        }
        assert_eq!(
            object_count(&harness, "snapshot.json").await,
            before + 1,
            "shadow mode commits the marker despite the external WAL restart"
        );
        assert_eq!(
            harness.engine.state(),
            EngineState::Streaming,
            "back to streaming after the shadow snapshot"
        );
    }

    /// Shadow-mode burn-in: a real external checkpointer commits and TRUNCATE-
    /// checkpoints the WAL repeatedly (several restarts) between engine ticks.
    /// The engine ships across every restart, never checkpoints itself, and
    /// the final replica restores to an exact copy of the source.
    #[tokio::test]
    async fn shadow_burn_in_ships_across_external_checkpoints() {
        let mut harness = Harness::with_shadow_config(true, |_| {}).await;
        harness.until_streaming().await;
        harness.engine.sync_once().await.expect("initial backlog");

        for round in 0..5 {
            // A stray writer commits a frame in the current cycle.
            let stray = harness.fixture.stray_conn().expect("stray conn");
            let data = format!("round-{round}");
            stray
                .execute("INSERT INTO t (data) VALUES (?1)", [&data])
                .expect("stray insert");
            // The engine ships that frame BEFORE the checkpoint (I1: nothing
            // unshipped is ever backfilled).
            harness.engine.sync_once().await.expect("ship the cycle");
            // Then an external TRUNCATE checkpoint restarts the WAL.
            harness
                .fixture
                .checkpoint(CheckpointMode::Truncate)
                .expect("external truncate");
            harness
                .engine
                .sync_once()
                .await
                .expect("sync after the restart");
        }

        // The engine's own checkpoints are always skipped in shadow mode.
        let outcome = harness.engine.checkpoint_once().await.expect("checkpoint");
        assert_eq!(
            outcome,
            CheckpointOutcome::SkippedShadow,
            "shadow mode never checkpoints"
        );

        // Drain and confirm the final replica is logically equivalent.
        for _ in 0..64 {
            let progress = harness.engine.sync_once().await.expect("drain");
            if harness.engine.state() == EngineState::Streaming
                && progress.shipped_segments == 0
                && progress.snapshot.is_none()
                && !progress.backing_off
            {
                break;
            }
            harness.advance(1);
        }
        harness.assert_restore_equivalent().await;
    }

    /// Prune deletes the `snapshot.json` marker FIRST, so a crash (here: an
    /// injected failure of the unordered `delete_prefix` batch) mid-prune
    /// leaves an invisible markerless generation, never a marker without its
    /// body that resume/restore would trust.
    #[tokio::test]
    async fn prune_deletes_marker_before_body() {
        let mut harness = Harness::with_config(|json| {
            json.retention_generations = Some(1);
        })
        .await;
        harness.until_streaming().await;
        let current = harness.engine.generation().cloned().expect("generation");

        // An older COMPLETE generation that retention (1) must prune.
        let old = GenerationId::new(
            DateTime::try_from(BASE_SECS - 200_000).expect("timestamp"),
            1,
        );
        let body_key = format!("generations/{}/snapshot.db.zst", old.as_str());
        let marker_key = format!("generations/{}/snapshot.json", old.as_str());
        let storage = harness.engine.storage();
        storage
            .put(&body_key, Bytes::from_static(b"body"))
            .await
            .expect("put body");
        storage
            .put(&marker_key, Bytes::from_static(b"{}"))
            .await
            .expect("put marker");

        // Fail the unordered prefix-delete batch (a separate op from the
        // single marker delete). The marker delete runs first and succeeds;
        // the prefix batch then fails, and the prune surfaces the error.
        harness
            .flaky()
            .set_plan(FailurePlan::new().fail_all(OpKind::DeletePrefix));
        let result = harness.engine.prune_once().await;
        assert!(result.is_err(), "the failed prefix delete surfaces");

        let storage = harness.engine.storage();
        let marker = storage.get(&marker_key).await;
        assert!(
            matches!(marker, Err(bencher_replica::StorageError::NotFound { .. })),
            "the marker was deleted first (generation now invisible): {marker:?}"
        );
        assert!(
            storage.get(&body_key).await.is_ok(),
            "the body remains after the failed prefix delete"
        );
        assert_eq!(
            harness.engine.generation(),
            Some(&current),
            "the current generation is untouched"
        );
    }

    /// The poison classification (the last-resort fatal safety net): an
    /// oversized transaction qualifies, transient/control errors do not. In
    /// normal operation the ship path intercepts an oversized transaction and
    /// re-snapshots instead (see `oversized_committed_txn_re_snapshots_and_drains`),
    /// so this classification only fires for an unconverted escape.
    #[test]
    fn poison_classification() {
        use bencher_replica::{SyncError, WalError};
        assert!(
            SyncError::TransactionTooLarge {
                bytes: 5,
                max_bytes: 4
            }
            .is_poison()
        );
        assert!(
            SyncError::Wal(WalError::TransactionTooLarge {
                bytes: 5,
                max_bytes: 4
            })
            .is_poison()
        );
        assert!(!SyncError::TaskExited.is_poison());
        assert!(!SyncError::SnapshotBoundaryDiverged.is_poison());
    }

    /// An oversized COMMITTED transaction cannot ship (restore rejects the
    /// segment) and, being durable, cannot be split retroactively. It must NOT
    /// wedge or crash the engine: the ship path forces a re-snapshot that
    /// captures its state, pins epoch 0 at the boundary, and drains the WAL via
    /// a checkpoint so shipping resumes on a fresh cycle. The replica stays
    /// restorable throughout and ends logically equivalent to the source.
    #[tokio::test]
    async fn oversized_committed_txn_re_snapshots_and_drains() {
        let mut harness = Harness::new().await;
        harness.ready().await;
        // Impose a small per-transaction cap and rebuild the engine to resume
        // with it (there is no JSON knob, so mutate the resolved config). The
        // cap is above a normal single-page write (~4 KiB) but well below the
        // deliberately oversized transaction below.
        harness.config.max_transaction_bytes = 16 * 1024;
        harness.rebuild_engine().await;
        let bootstrap = harness.engine.generation().cloned().expect("generation");

        // A committed transaction far over the cap (dozens of pages).
        harness
            .fixture
            .txn_touching_pages(16)
            .expect("oversized committed txn");

        // Ship detects it and schedules a re-snapshot; it is NOT a fatal error.
        let progress = harness.engine.sync_once().await.expect("sync (no fatal)");
        assert!(
            progress.error.is_none(),
            "an oversized committed transaction is not a fatal error: {progress:?}"
        );

        // Drive the re-snapshot to completion (position pins at the boundary).
        for _ in 0..256 {
            if harness.engine.state() == EngineState::Streaming {
                break;
            }
            harness.engine.sync_once().await.expect("re-snapshot tick");
        }
        let drained_gen = harness.engine.generation().cloned().expect("generation");
        assert_ne!(
            drained_gen, bootstrap,
            "a fresh generation captured the oversized transaction"
        );

        // Pinned at the boundary: the ship path ships nothing until the drain.
        let progress = harness.engine.sync_once().await.expect("pinned ship");
        assert_eq!(
            progress.shipped_segments, 0,
            "epoch 0 is pinned at the boundary, awaiting the drain"
        );

        // Drain: a checkpoint backfills the WAL below the boundary (driven
        // directly to bypass the min-pages threshold for this small fixture).
        let outcome = harness
            .engine
            .checkpoint_once()
            .await
            .expect("drain checkpoint");
        assert_eq!(
            outcome,
            CheckpointOutcome::Completed,
            "the WAL below the boundary backfills, allowing the restart"
        );

        // A write restarts the WAL; the ship path rebinds epoch 0 to the fresh
        // cycle and resumes shipping.
        harness
            .fixture
            .txn(&["INSERT INTO t (data) VALUES ('after drain')"])
            .expect("post-drain write");
        let progress = harness.engine.sync_once().await.expect("resume shipping");
        assert!(
            progress.error.is_none() && progress.shipped_segments >= 1,
            "shipping resumes on a fresh cycle after the drain: {progress:?}"
        );

        // The oversized transaction lives in the snapshot body, the post-drain
        // write in an epoch-0 segment: restore reproduces the source exactly.
        harness.assert_restore_equivalent().await;
    }

    /// The checkpoint gate bounds its unshipped-commit scan with
    /// `max_transaction_bytes`: an oversized run aborts the scan (never read in
    /// full inside the critical section) and is treated CONSERVATIVELY as
    /// unshipped, deferring the checkpoint (invariant I1) rather than
    /// backfilling an ambiguous tail.
    #[tokio::test]
    async fn checkpoint_gate_defers_on_oversized_unshipped_commit() {
        let mut harness = Harness::new().await;
        harness.ready().await;
        harness.config.max_transaction_bytes = 16 * 1024;
        harness.rebuild_engine().await;
        // A committed transaction over the cap, sitting unshipped above the
        // resumed position.
        harness
            .fixture
            .txn_touching_pages(16)
            .expect("oversized committed txn");
        let outcome = harness.engine.checkpoint_once().await.expect("checkpoint");
        assert_eq!(
            outcome,
            CheckpointOutcome::SkippedUnshipped,
            "an oversized unshipped commit defers the checkpoint conservatively"
        );
    }
}
