#![cfg(all(feature = "plus", feature = "testing"))]
//! Crash-recovery matrix: six kill points, each simulated deterministically.
//!
//! A "process crash" is the DROP of the step-driven engine at a precise step
//! boundary (optionally after an injected storage failure); recovery is a
//! rebuild via `SyncEngine::new_with_storage` over the same fixture and
//! replica directories with a fresh `FlakyStorage` (journal reset, no plan).
//! Every kill point shares one postcondition: the rebuilt engine quiesces,
//! ships one more transaction, a restore reproduces the source exactly, and
//! no epoch on the replica holds duplicate or overlapping segment ranges.
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

/// Shared fixtures: a scripted source database, a fault-injectable replica,
/// and an engine (behind an `Option`, so a test can crash it at any step
/// boundary) built over both with a deterministic clock.
#[cfg(test)]
pub(crate) mod harness {
    use std::collections::BTreeMap;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicI64, Ordering};

    use bencher_json::system::config::{JsonReplication, ReplicationTarget};
    use bencher_json::{Clock, DateTime};
    use bencher_replica::testing::{
        FailurePlan, FlakyStorage, OpKind, OpOutcome, WalFixture, assert_replica_equivalent,
    };
    use bencher_replica::{
        EngineState, GenerationId, LocalStorage, ReplicaConfig, ReplicaDb, ReplicaStorage,
        RestoreOutcome, SyncEngine, restore_if_missing,
    };
    use camino::{Utf8Path, Utf8PathBuf};

    /// Page size for every fixture database in this suite.
    const PAGE_SIZE: u32 = 4096;
    /// 2026-07-10T14:59:00Z, the deterministic clock start.
    const BASE_SECS: i64 = 1_783_695_540;

    pub(crate) fn dir_path(tmp: &tempfile::TempDir) -> &Utf8Path {
        Utf8Path::from_path(tmp.path()).expect("tempdir path is UTF-8")
    }

    pub(crate) fn logger() -> slog::Logger {
        slog::Logger::root(slog::Discard, slog::o!())
    }

    fn clock_for(secs: &Arc<AtomicI64>) -> Clock {
        let secs = Arc::clone(secs);
        Clock::Custom(Arc::new(move || {
            DateTime::try_from(secs.load(Ordering::SeqCst)).expect("valid clock seconds")
        }))
    }

    /// Flaky(Local) storage rooted at `root`, with an empty failure plan.
    fn flaky_over(root: &Utf8Path) -> ReplicaStorage {
        ReplicaStorage::Flaky(Box::new(FlakyStorage::new(
            ReplicaStorage::Local(LocalStorage::new(root.to_path_buf())),
            FailurePlan::new(),
        )))
    }

    /// Parse the `[start, end)` byte range out of a segment object key.
    fn segment_range(key: &str) -> (u64, u64) {
        let (_, file) = key.rsplit_once('/').expect("segment key has a directory");
        let range = file.strip_suffix(".wal.zst").expect("segment key suffix");
        let (start, end) = range.split_once('-').expect("segment range separator");
        (
            start.parse().expect("segment start offset"),
            end.parse().expect("segment end offset"),
        )
    }

    pub(crate) struct Harness {
        fixture: WalFixture,
        /// `None` between [`Harness::crash`] and [`Harness::rebuild_engine`].
        engine: Option<SyncEngine<()>>,
        config: ReplicaConfig,
        db: ReplicaDb<()>,
        clock_secs: Arc<AtomicI64>,
        replica_root: Utf8PathBuf,
        _fixture_tmp: tempfile::TempDir,
        _replica_tmp: tempfile::TempDir,
    }

    impl Harness {
        pub(crate) async fn new() -> Self {
            Self::with_config(|_| {}).await
        }

        pub(crate) async fn with_config<F>(overrides: F) -> Self
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
                false,
                flaky_over(&replica_root),
            )
            .await
            .expect("engine");
            Self {
                fixture,
                engine: Some(engine),
                config,
                db,
                clock_secs,
                replica_root,
                _fixture_tmp: fixture_tmp,
                _replica_tmp: replica_tmp,
            }
        }

        /// The scripted source database.
        pub(crate) fn fixture(&self) -> &WalFixture {
            &self.fixture
        }

        /// The live engine; panics after a crash and before a rebuild.
        pub(crate) fn engine(&self) -> &SyncEngine<()> {
            self.engine.as_ref().expect("the engine is running")
        }

        /// The live engine, mutably.
        pub(crate) fn engine_mut(&mut self) -> &mut SyncEngine<()> {
            self.engine.as_mut().expect("the engine is running")
        }

        /// The kill point: drop the engine mid-flight. Its open checkpoint
        /// connections, in-flight multipart upload, backoff state, and
        /// in-memory position all vanish, exactly as in a killed process.
        pub(crate) fn crash(&mut self) {
            let engine = self.engine.take();
            assert!(engine.is_some(), "a crash requires a running engine");
            drop(engine);
        }

        /// The recovery: rebuild the engine over the same fixture and
        /// replica directories. The storage wrapper is brand new (empty
        /// journal, no failure plan), so resume sees a healed replica.
        pub(crate) async fn rebuild_engine(&mut self) {
            assert!(
                self.engine.is_none(),
                "rebuild models a process restart: crash first"
            );
            self.engine = Some(
                SyncEngine::new_with_storage(
                    logger(),
                    self.config.clone(),
                    self.db.clone(),
                    clock_for(&self.clock_secs),
                    false,
                    flaky_over(&self.replica_root),
                )
                .await
                .expect("engine rebuild"),
            );
        }

        /// The fault-injection wrapper the engine was built with.
        pub(crate) fn flaky(&self) -> &FlakyStorage {
            if let ReplicaStorage::Flaky(flaky) = self.engine().storage() {
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

        /// Segment object keys whose put was injected to fail, in journal
        /// order (these never reached the backend).
        pub(crate) fn injected_segment_keys(&self) -> Vec<String> {
            self.flaky()
                .journal()
                .iter()
                .filter(|(op, outcome)| {
                    op.kind == OpKind::Put
                        && *outcome == OpOutcome::Injected
                        && op.key.ends_with(".wal.zst")
                })
                .map(|(op, _)| op.key.clone())
                .collect()
        }

        /// Drive `sync_once` until the engine is streaming (the fresh-replica
        /// bootstrap snapshot has completed).
        pub(crate) async fn until_streaming(&mut self) {
            for _ in 0..64 {
                if self.engine().state() == EngineState::Streaming {
                    return;
                }
                self.engine_mut()
                    .sync_once()
                    .await
                    .expect("sync_once during startup");
            }
            panic!(
                "engine never reached Streaming; state: {:?}",
                self.engine().state()
            );
        }

        /// Bootstrap plus one sync tick, so the initial WAL backlog is
        /// shipped and the engine is quiescent.
        pub(crate) async fn ready(&mut self) {
            self.until_streaming().await;
            self.engine_mut().sync_once().await.expect("backlog sync");
        }

        /// Drive `sync_once` until a fully quiet streaming tick: no shipped
        /// segments, no snapshot work, no backoff, no error. This absorbs
        /// whatever recovery work a rebuild scheduled (backlog re-ship or a
        /// forced new-generation snapshot).
        pub(crate) async fn drive_to_quiescence(&mut self) {
            for _ in 0..128 {
                let progress = self
                    .engine_mut()
                    .sync_once()
                    .await
                    .expect("quiescence sync");
                assert!(
                    progress.error.is_none(),
                    "recovery ticks over healed storage are clean: {progress:?}"
                );
                if self.engine().state() == EngineState::Streaming
                    && progress.shipped_segments == 0
                    && progress.snapshot.is_none()
                    && !progress.backing_off
                {
                    return;
                }
            }
            panic!("engine never quiesced; state: {:?}", self.engine().state());
        }

        /// Restore the replica into a scratch directory, assert logical
        /// equivalence with the live source database, and return the
        /// generation the restore was served from.
        pub(crate) async fn assert_restore_equivalent(&self) -> GenerationId {
            let target_tmp = tempfile::tempdir().expect("restore target tempdir");
            let target_db = dir_path(&target_tmp).join("restored.db");
            let outcome = restore_if_missing(&logger(), &self.config, &target_db)
                .await
                .expect("restore");
            let RestoreOutcome::Restored { generation, .. } = outcome else {
                panic!("expected Restored, got {outcome:?}");
            };
            assert_replica_equivalent(&self.fixture.db_path(), &target_db);
            generation
        }

        /// List every shipped segment on the replica and assert that within
        /// each (generation, epoch) directory the byte ranges are contiguous
        /// from offset 0 with strictly increasing starts: no duplicate,
        /// overlapping, or missing segment ranges survive a crash.
        pub(crate) async fn assert_contiguous_segments(&self) {
            let keys = self
                .engine()
                .storage()
                .list("generations")
                .await
                .expect("list replica");
            let mut epochs: BTreeMap<&str, Vec<(u64, u64)>> = BTreeMap::new();
            for key in &keys {
                if !key.ends_with(".wal.zst") {
                    continue;
                }
                let (dir, _file) = key.rsplit_once('/').expect("segment key has a directory");
                epochs.entry(dir).or_default().push(segment_range(key));
            }
            assert!(!epochs.is_empty(), "at least one epoch shipped segments");
            for (dir, mut ranges) in epochs {
                ranges.sort_unstable();
                let mut expected_start = 0u64;
                for (start, end) in ranges {
                    assert_eq!(
                        start, expected_start,
                        "{dir}: strictly increasing contiguous segment starts \
                         (no duplicate, overlapping, or missing ranges)"
                    );
                    assert!(end > start, "{dir}: segment [{start}, {end}) is non-empty");
                    expected_start = end;
                }
            }
        }

        /// The common crash postcondition: quiesce, prove the pipeline still
        /// ships (one more transaction plus a sync), and prove the replica
        /// restores to an exact copy with a duplicate-free segment layout.
        /// Returns the generation the restore was served from.
        pub(crate) async fn assert_crash_postcondition(&mut self) -> GenerationId {
            self.drive_to_quiescence().await;
            self.fixture
                .txn(&["INSERT INTO t (data) VALUES ('post-crash probe')"])
                .expect("post-crash txn");
            let progress = self
                .engine_mut()
                .sync_once()
                .await
                .expect("post-crash sync");
            assert!(
                progress.error.is_none(),
                "the post-crash sync is clean: {progress:?}"
            );
            let generation = self.assert_restore_equivalent().await;
            self.assert_contiguous_segments().await;
            generation
        }
    }
}

#[cfg(test)]
mod cases {
    use bencher_replica::testing::{CheckpointMode, FailurePlan, OpKind};
    use bencher_replica::{CheckpointOutcome, EngineState, SnapshotStatus, StorageError};
    use pretty_assertions::{assert_eq, assert_ne};

    use super::harness::Harness;

    /// Drive `sync_once` until the scheduled snapshot cycle reports
    /// `Finished`.
    async fn drive_snapshot_cycle(harness: &mut Harness) {
        for _ in 0..64 {
            let progress = harness
                .engine_mut()
                .sync_once()
                .await
                .expect("snapshot cycle sync");
            assert!(
                progress.error.is_none(),
                "snapshot cycle ticks are clean: {progress:?}"
            );
            if progress.snapshot == Some(SnapshotStatus::Finished) {
                return;
            }
        }
        panic!("the snapshot cycle never finished");
    }

    /// Count replica objects whose key ends with `suffix`.
    async fn object_count(harness: &Harness, suffix: &str) -> usize {
        harness
            .engine()
            .storage()
            .list("generations")
            .await
            .expect("list replica")
            .iter()
            .filter(|key| key.ends_with(suffix))
            .count()
    }

    /// `K_a`: crash with committed frames that never shipped. The frames
    /// survive in the local WAL; the rebuilt engine resumes at the replica
    /// tip and ships them on its very first sync.
    #[tokio::test]
    async fn crash_before_any_ship() {
        let mut harness = Harness::new().await;
        harness.ready().await;
        let shipped_offset = harness.engine().position().expect("position").offset;
        harness
            .fixture()
            .txn(&["INSERT INTO t (data) VALUES ('committed, never shipped')"])
            .expect("txn");
        harness.crash();
        harness.rebuild_engine().await;

        let position = harness.engine().position().expect("resumed position");
        assert_eq!(
            position.offset, shipped_offset,
            "resume from the replica LIST lands at the last shipped offset"
        );
        let progress = harness.engine_mut().sync_once().await.expect("first sync");
        assert!(
            progress.shipped_segments >= 1,
            "the crash-surviving frames ship on the FIRST sync after rebuild: {progress:?}"
        );
        harness.assert_crash_postcondition().await;
    }

    /// `K_b`: crash mid-segment-upload. The put is injected to fail (backoff
    /// armed), the process dies mid-backoff, and the rebuilt engine re-ships
    /// the segment under the exact same key: idempotent (epoch, start, end)
    /// naming means a retried upload can never duplicate a range.
    #[tokio::test]
    async fn crash_mid_segment_upload() {
        let mut harness = Harness::new().await;
        harness.ready().await;
        harness
            .fixture()
            .txn(&["INSERT INTO t (data) VALUES ('upload will fail')"])
            .expect("txn");
        harness
            .flaky()
            .set_plan(FailurePlan::new().fail_nth(OpKind::Put, 1));

        let progress = harness
            .engine_mut()
            .sync_once()
            .await
            .expect("failing sync");
        assert!(
            progress.error.is_some(),
            "the injected put surfaces as a tick error: {progress:?}"
        );
        let progress = harness.engine_mut().sync_once().await.expect("gated sync");
        assert!(
            progress.backing_off,
            "the backoff is armed when the process dies: {progress:?}"
        );
        let injected = harness.injected_segment_keys();
        assert_eq!(
            injected.len(),
            1,
            "exactly one injected segment put: {injected:?}"
        );
        let failed_key = injected.first().expect("the failed segment key").clone();
        let error = harness
            .engine()
            .storage()
            .get(&failed_key)
            .await
            .expect_err("the failed segment never reached the replica");
        assert!(
            matches!(error, StorageError::NotFound { .. }),
            "expected NotFound for the failed key, got {error}"
        );

        harness.crash();
        harness.rebuild_engine().await;
        let progress = harness
            .engine_mut()
            .sync_once()
            .await
            .expect("re-ship sync");
        assert!(
            progress.shipped_segments >= 1,
            "the lost segment re-ships after rebuild: {progress:?}"
        );
        assert_eq!(
            harness.shipped_segment_keys(),
            vec![failed_key],
            "the re-shipped segment reuses the exact key that failed mid-upload \
             (same epoch, start, end)"
        );
        harness.assert_crash_postcondition().await;
    }

    /// `K_c`: crash after a successful ship but before any checkpoint. The
    /// rebuilt engine re-derives its position from the replica LIST alone:
    /// a no-write sync issues zero puts (nothing re-ships, nothing is lost).
    #[tokio::test]
    async fn crash_after_ship_before_checkpoint() {
        let mut harness = Harness::new().await;
        harness.ready().await;
        harness
            .fixture()
            .txn(&["INSERT INTO t (data) VALUES ('shipped, not checkpointed')"])
            .expect("txn");
        harness.engine_mut().sync_once().await.expect("sync");
        let position = harness.engine().position().cloned().expect("position");

        harness.crash();
        harness.rebuild_engine().await;
        assert_eq!(
            harness.engine().position(),
            Some(&position),
            "the position is re-derived from the replica LIST alone \
             (exact offset and checksum)"
        );
        let progress = harness
            .engine_mut()
            .sync_once()
            .await
            .expect("no-write sync");
        assert_eq!(progress.shipped_segments, 0, "nothing new to ship");
        assert_eq!(
            harness.put_attempts(),
            0,
            "a no-write sync after rebuild issues zero storage puts: \
             no segment is ever shipped twice"
        );
        harness.assert_crash_postcondition().await;
    }

    /// `K_d`: crash mid-snapshot-upload. The half-built generation has no
    /// snapshot.json, so it is invisible to resume and restore; 25 hours
    /// later the scheduled snapshot cycle prunes the stale incomplete
    /// generation prefix.
    #[tokio::test]
    async fn crash_mid_snapshot_upload() {
        let mut harness = Harness::with_config(|json| {
            json.snapshot_throttle_mib = Some(1);
        })
        .await;
        harness.ready().await;
        // Grow the database FILE (not just the WAL) so the snapshot copy
        // spans multiple steps: big transaction, ship, full backfill.
        harness
            .fixture()
            .txn_touching_pages(800)
            .expect("big transaction");
        while harness
            .engine_mut()
            .ship_once()
            .await
            .expect("ship big txn")
            > 0
        {}
        let outcome = harness
            .engine_mut()
            .checkpoint_once()
            .await
            .expect("checkpoint");
        assert_eq!(
            outcome,
            CheckpointOutcome::Completed,
            "backfill the db file"
        );
        let bootstrap = harness.engine().generation().cloned().expect("generation");

        // Start a new-generation snapshot and kill it mid-upload: after two
        // steps the multipart upload into the new generation is open but
        // nothing is finalized.
        harness.engine_mut().trigger_snapshot();
        for step in 0..2 {
            let status = harness
                .engine_mut()
                .snapshot_step()
                .await
                .expect("snapshot step");
            assert_eq!(
                status,
                SnapshotStatus::InProgress,
                "step {step} leaves the snapshot in flight"
            );
        }
        harness.crash();
        harness.rebuild_engine().await;

        // The half generation is a directory without snapshot.json:
        // invisible to both resume and restore.
        let dirs = harness
            .engine()
            .storage()
            .list_dirs("generations")
            .await
            .expect("list generations");
        assert_eq!(
            dirs.len(),
            2,
            "the bootstrap generation plus the crashed half generation: {dirs:?}"
        );
        let half = dirs
            .iter()
            .find(|dir| dir.as_str() != bootstrap.as_str())
            .expect("the crashed generation directory")
            .clone();
        let error = harness
            .engine()
            .storage()
            .get(&format!("generations/{half}/snapshot.json"))
            .await
            .expect_err("the crashed generation must have no snapshot.json");
        assert!(
            matches!(error, StorageError::NotFound { .. }),
            "snapshot.json is absent, not another error: {error}"
        );

        let restored = harness.assert_crash_postcondition().await;
        assert_ne!(
            restored.as_str(),
            half.as_str(),
            "the crashed generation is never the restore source"
        );
        harness
            .engine()
            .storage()
            .get(&format!("generations/{}/snapshot.json", restored.as_str()))
            .await
            .expect("the restore source generation has snapshot.json");

        // 25 hours later the scheduled snapshot cycle runs; its prune reaps
        // the stale (> 24h) incomplete generation prefix.
        harness.advance(25 * 60 * 60);
        drive_snapshot_cycle(&mut harness).await;
        let dirs = harness
            .engine()
            .storage()
            .list_dirs("generations")
            .await
            .expect("list after prune");
        assert!(
            !dirs.contains(&half),
            "the stale incomplete generation was pruned: {dirs:?}"
        );
        // The new lineage still restores to an exact copy.
        harness.drive_to_quiescence().await;
        harness.assert_restore_equivalent().await;
        harness.assert_contiguous_segments().await;
    }

    /// `K_e`: crash after a completed checkpoint, after the WAL restarted
    /// (new salts), but before any new-epoch segment shipped. The advisory
    /// meta proves the old epoch was fully shipped through the checkpoint,
    /// so resume continues as epoch+1 in the SAME generation: no
    /// re-snapshot, no re-shipped old segments.
    #[tokio::test]
    async fn crash_after_checkpoint_before_new_epoch_ship() {
        let mut harness = Harness::new().await;
        harness.ready().await;
        harness
            .fixture()
            .txn(&["INSERT INTO t (data) VALUES ('seals the epoch')"])
            .expect("txn");
        harness.engine_mut().sync_once().await.expect("sync");
        let sealed = harness.engine().position().cloned().expect("position");
        let outcome = harness
            .engine_mut()
            .checkpoint_once()
            .await
            .expect("checkpoint");
        assert_eq!(
            outcome,
            CheckpointOutcome::Completed,
            "the full backfill seals the epoch"
        );
        let generation = harness.engine().generation().cloned().expect("generation");

        // The WAL is fully backfilled, so this write restarts it (new
        // salts); the process dies before the engine ever sees the new
        // cycle.
        harness
            .fixture()
            .txn(&["INSERT INTO t (data) VALUES ('first write of the new cycle')"])
            .expect("txn after checkpoint");
        harness.crash();
        harness.rebuild_engine().await;

        assert_eq!(
            harness.engine().state(),
            EngineState::Streaming,
            "the meta-verified resume streams immediately: no re-snapshot"
        );
        assert_eq!(
            harness.engine().generation(),
            Some(&generation),
            "the generation is unchanged across the crash"
        );
        let position = harness.engine().position().expect("resumed position");
        assert_eq!(
            position.epoch,
            sealed.epoch + 1,
            "resume continues as the next epoch"
        );
        assert_eq!(position.offset, 0, "the new epoch starts at offset 0");
        let salt = position.salt;

        let progress = harness
            .engine_mut()
            .sync_once()
            .await
            .expect("ship the new cycle");
        assert!(
            progress.shipped_segments >= 1,
            "the new cycle ships after rebuild: {progress:?}"
        );
        let epoch_dir = format!("{:010}-{:08x}{:08x}", sealed.epoch + 1, salt.0, salt.1);
        let keys = harness.shipped_segment_keys();
        assert!(
            !keys.is_empty() && keys.iter().all(|key| key.contains(&epoch_dir)),
            "every post-rebuild put lands in the NEW epoch directory {epoch_dir}; \
             no old segment re-ships: {keys:?}"
        );
        assert_eq!(
            object_count(&harness, "snapshot.json").await,
            1,
            "no new generation was created"
        );
        harness.assert_crash_postcondition().await;
    }

    /// `K_f`: crash, then (with the engine down) a stray writer restarts the
    /// WAL, a stray TRUNCATE checkpoint backfills those never-shipped
    /// frames, and another write restarts the WAL again, wiping them. The
    /// rebuilt engine cannot prove continuity across the buried cycle and
    /// must force a NEW generation; the fresh snapshot recaptures the
    /// buried transaction so nothing is lost from the replica.
    ///
    /// This test originally exposed a real engine gap: the meta-verified
    /// epoch+1 resume said nothing about how many WAL restarts happened
    /// while the process was down, so the buried cycle's commits (living
    /// only in the db file, backfilled by the stray TRUNCATE) silently never
    /// shipped. Resume now also proves salt CONTINUITY: `SQLite` increments
    /// WAL `salt1` by exactly one per restart, so the epoch+1 path requires
    /// `header.salt1 == meta.salt1 + 1` and diverges to a new generation on
    /// any larger jump.
    #[tokio::test]
    async fn crash_with_unshipped_frames_lost_to_wal_reset() {
        let mut harness = Harness::new().await;
        harness.ready().await;
        harness
            .fixture()
            .txn(&["INSERT INTO t (data) VALUES ('shipped and checkpointed')"])
            .expect("txn");
        harness.engine_mut().sync_once().await.expect("sync");
        let outcome = harness
            .engine_mut()
            .checkpoint_once()
            .await
            .expect("checkpoint");
        assert_eq!(
            outcome,
            CheckpointOutcome::Completed,
            "the epoch is sealed by a full backfill"
        );
        let old_generation = harness.engine().generation().cloned().expect("generation");
        harness.crash();

        // With the engine down: a stray transaction restarts the fully
        // backfilled WAL (legitimately: everything so far was shipped). Its
        // frames are never shipped. CREATE TABLE makes the eventual loss
        // visible: no later transaction rewrites the schema this touches,
        // so nothing downstream can mask a silently dropped cycle.
        let stray = harness.fixture().stray_conn().expect("stray conn");
        stray
            .execute_batch(
                "BEGIN IMMEDIATE;
                 CREATE TABLE buried (id INTEGER PRIMARY KEY, data TEXT);
                 INSERT INTO buried (data) VALUES ('never shipped');
                 COMMIT;",
            )
            .expect("stray txn");
        // A stray TRUNCATE checkpoint backfills the unshipped frames into
        // the db file, and the next write restarts the WAL a second time,
        // wiping every trace of them from the local WAL.
        harness
            .fixture()
            .checkpoint(CheckpointMode::Truncate)
            .expect("stray TRUNCATE");
        harness
            .fixture()
            .txn(&["INSERT INTO t (data) VALUES ('second restart cycle')"])
            .expect("txn after truncate");

        harness.rebuild_engine().await;
        assert_eq!(
            harness.engine().state(),
            EngineState::PendingSnapshot,
            "resume cannot prove continuity across the buried WAL cycle; \
             a new generation must be forced, never a silent epoch+1 resume"
        );
        let restored = harness.assert_crash_postcondition().await;
        assert_ne!(
            restored, old_generation,
            "a second generation appears and becomes the restore source"
        );
        assert_eq!(
            object_count(&harness, "snapshot.json").await,
            2,
            "both the old and the replacement generation are complete"
        );
    }
}
