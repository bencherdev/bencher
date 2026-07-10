#![cfg(all(feature = "plus", feature = "testing"))]
//! Fault-injection matrix: eight storage-failure scenarios (F1-F8).
//!
//! Every scenario follows arrange -> act -> assert: arrange a scripted
//! [`bencher_replica::testing::FailurePlan`] on the Flaky(Local) replica
//! storage, act by driving the step-driven engine (`sync_once`,
//! `checkpoint_once`, `snapshot_step`, `prune_once`) with all scheduling
//! under an injected `Clock` (never a sleep), and assert on the operation
//! journal, the on-disk replica state, and end-to-end restore equivalence
//! where meaningful.
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

/// Shared fixtures (private copy of the `tests/sync_engine.rs` harness): a
/// scripted source database, a fault-injectable replica, and an engine built
/// over both with a deterministic clock.
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
        EngineState, GenerationId, LocalStorage, ReplicaConfig, ReplicaDb, ReplicaStorage,
        RestoreOutcome, SnapshotStatus, SyncEngine, SyncError, restore_if_missing,
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

    /// Flaky(Local) storage rooted at `root` with the given failure plan.
    pub(crate) fn flaky_with(root: &Utf8Path, plan: FailurePlan) -> ReplicaStorage {
        ReplicaStorage::Flaky(Box::new(FlakyStorage::new(
            ReplicaStorage::Local(LocalStorage::new(root.to_path_buf())),
            plan,
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

    /// Every file under `root`, as sorted root-relative path strings.
    /// Used to prove a failed resume uploaded nothing at all.
    pub(crate) fn replica_file_set(root: &Utf8Path) -> Vec<String> {
        fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
            let Ok(entries) = std::fs::read_dir(dir) else {
                return;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk(&path, out);
                } else {
                    out.push(path);
                }
            }
        }
        let mut paths = Vec::new();
        walk(root.as_std_path(), &mut paths);
        let mut files: Vec<String> = paths
            .iter()
            .map(|path| {
                path.strip_prefix(root.as_std_path())
                    .unwrap_or(path)
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        files.sort();
        files
    }

    pub(crate) struct Harness {
        pub fixture: WalFixture,
        pub engine: SyncEngine<()>,
        pub config: ReplicaConfig,
        pub db: ReplicaDb<()>,
        pub clock_secs: Arc<AtomicI64>,
        pub replica_root: Utf8PathBuf,
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
                flaky_with(&replica_root, FailurePlan::new()),
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

        /// Current injected clock second.
        pub(crate) fn now_secs(&self) -> i64 {
            self.clock_secs.load(Ordering::SeqCst)
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

        /// Replace the engine (dropping the old one on success) with a
        /// freshly resumed engine over the same replica, built on storage
        /// with the given failure plan (the plan is live BEFORE
        /// construction). On `Err` the previous engine is untouched; the
        /// flaky journal starts empty either way.
        pub(crate) async fn rebuild_engine_with_plan(
            &mut self,
            plan: FailurePlan,
        ) -> Result<(), SyncError> {
            self.engine = SyncEngine::new_with_storage(
                logger(),
                self.config.clone(),
                self.db.clone(),
                clock_for(&self.clock_secs),
                false,
                flaky_with(&self.replica_root, plan),
            )
            .await?;
            Ok(())
        }

        /// Trigger a new-generation snapshot and drive it to completion.
        pub(crate) async fn drive_snapshot(&mut self) {
            self.engine.trigger_snapshot();
            for _ in 0..1000 {
                let status = self.engine.snapshot_step().await.expect("snapshot step");
                if status == SnapshotStatus::Finished {
                    return;
                }
            }
            panic!("snapshot never finished");
        }

        /// On-disk directory of a generation under the local replica root.
        pub(crate) fn generation_dir(&self, generation: &GenerationId) -> Utf8PathBuf {
            self.replica_root
                .join("generations")
                .join(generation.as_str())
        }

        /// Restore the replica into a scratch directory, assert logical
        /// equivalence with the live source database, and return the
        /// generation the restore picked.
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
    }
}

#[cfg(test)]
mod cases {
    use bencher_replica::testing::{FailurePlan, OpKind, OpOutcome};
    use bencher_replica::{
        CheckpointOutcome, EngineState, RestoreError, SnapshotStatus, StorageError, SyncError,
        restore_if_missing,
    };
    use pretty_assertions::assert_eq;

    use super::harness::{Harness, dir_path, logger, replica_file_set, segment_range};

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

    /// F1: a single injected put arms the backoff; once the clock passes the
    /// delay the SAME segment ships exactly once (one injected attempt, one
    /// successful put for the same key) and the replica restores equivalent.
    #[tokio::test]
    async fn put_fails_once_then_heals() {
        let mut harness = Harness::new().await;
        harness.ready().await;
        harness
            .flaky()
            .set_plan(FailurePlan::new().fail_nth(OpKind::Put, 1));
        harness
            .fixture
            .txn(&["INSERT INTO t (data) VALUES ('flaky-once')"])
            .expect("txn");

        let progress = harness.engine.sync_once().await.expect("failing sync");
        assert!(
            progress.error.is_some(),
            "the injected put arms a backoff: {progress:?}"
        );
        assert_eq!(
            progress.shipped_segments, 0,
            "nothing ships on the failed tick"
        );

        let progress = harness.engine.sync_once().await.expect("gated sync");
        assert!(
            progress.backing_off,
            "the next tick is gated until the clock advances: {progress:?}"
        );

        harness.advance(1);
        let progress = harness.engine.sync_once().await.expect("retry sync");
        assert!(progress.error.is_none(), "the retry succeeds: {progress:?}");
        assert_eq!(
            progress.shipped_segments, 1,
            "the pending segment ships exactly once"
        );

        let key = harness
            .shipped_segment_keys()
            .pop()
            .expect("a shipped segment");
        let outcomes: Vec<OpOutcome> = harness
            .flaky()
            .journal()
            .into_iter()
            .filter(|(op, _)| op.kind == OpKind::Put && op.key == key)
            .map(|(_, outcome)| outcome)
            .collect();
        assert_eq!(
            outcomes,
            vec![OpOutcome::Injected, OpOutcome::Ok],
            "the same key sees exactly one injected attempt then one successful ship"
        );
        harness.assert_restore_equivalent().await;
    }

    /// F2: under a persistent put outage the spacing between real put
    /// attempts follows the exact capped doubling sequence 1, 2, 4, ...,
    /// 256, 300, 300; healing recovers, and a success resets the backoff so
    /// the next failure starts back at 1s.
    #[tokio::test]
    async fn backoff_caps_at_max() {
        let mut harness = Harness::new().await;
        harness.ready().await;
        harness
            .flaky()
            .set_plan(FailurePlan::new().fail_all(OpKind::Put));
        harness
            .fixture
            .txn(&["INSERT INTO t (data) VALUES ('cap')"])
            .expect("txn");

        // Step the clock one second at a time; a tick either attempts a put
        // (journal grows) or is backoff-gated. Record the clock second of
        // every real attempt.
        let mut attempt_secs = Vec::new();
        let mut seen = harness.put_attempts();
        for _ in 0..2000 {
            if attempt_secs.len() == 12 {
                break;
            }
            let progress = harness.engine.sync_once().await.expect("outage sync");
            let attempts = harness.put_attempts();
            if attempts > seen {
                seen = attempts;
                assert!(
                    !progress.backing_off,
                    "an attempt tick is never backoff-gated: {progress:?}"
                );
                assert!(
                    progress.error.is_some(),
                    "every attempt during the outage fails: {progress:?}"
                );
                attempt_secs.push(harness.now_secs());
            } else {
                assert!(
                    progress.backing_off,
                    "a no-attempt tick during the outage is backoff-gated: {progress:?}"
                );
                harness.advance(1);
            }
        }
        assert_eq!(attempt_secs.len(), 12, "twelve attempts were observed");
        let deltas: Vec<i64> = attempt_secs
            .iter()
            .zip(attempt_secs.iter().skip(1))
            .map(|(previous, next)| next - previous)
            .collect();
        assert_eq!(
            deltas,
            vec![1, 2, 4, 8, 16, 32, 64, 128, 256, 300, 300],
            "attempt spacing doubles from 1s and caps at 300s"
        );

        // Heal and wait out the final (capped) delay: the backlog ships.
        harness.flaky().heal();
        harness.advance(301);
        let progress = harness.engine.sync_once().await.expect("healed sync");
        assert!(
            progress.error.is_none(),
            "the healed tick succeeds: {progress:?}"
        );
        assert!(
            progress.shipped_segments >= 1,
            "the backlog ships after healing: {progress:?}"
        );

        // The success reset the backoff: a fresh failure starts at 1s again,
        // not at the 300s cap.
        harness
            .flaky()
            .set_plan(FailurePlan::new().fail_nth(OpKind::Put, 1));
        harness
            .fixture
            .txn(&["INSERT INTO t (data) VALUES ('reset')"])
            .expect("txn");
        let progress = harness.engine.sync_once().await.expect("failing sync");
        assert!(
            progress.error.is_some(),
            "the induced failure arms a fresh backoff: {progress:?}"
        );
        let progress = harness.engine.sync_once().await.expect("gated sync");
        assert!(
            progress.backing_off,
            "gated within the fresh 1s delay: {progress:?}"
        );
        harness.advance(1);
        let progress = harness.engine.sync_once().await.expect("retry sync");
        assert!(
            progress.error.is_none() && progress.shipped_segments >= 1,
            "one second later the retry ships: the backoff restarted at 1s, not 300s: {progress:?}"
        );
        harness.assert_restore_equivalent().await;
    }

    /// F3: during an outage the WAL is the buffer (it grows every round and
    /// no checkpoint may complete while unshipped frames exist); after
    /// healing the backlog ships strictly in order, the checkpoint
    /// completes, and the replica restores equivalent.
    #[tokio::test]
    async fn outage_wal_grows_then_catchup() {
        let mut harness = Harness::new().await;
        harness.ready().await;
        harness
            .flaky()
            .set_plan(FailurePlan::new().fail_all(OpKind::Put));

        let mut wal_len = harness.fixture.wal_bytes().expect("wal").len();
        for round in 0..5u32 {
            // Past the backoff cap: every round makes a real attempt.
            harness.advance(400);
            let statement = format!("INSERT INTO t (data) VALUES ('outage-{round}')");
            harness.fixture.txn(&[statement.as_str()]).expect("txn");
            let grown = harness.fixture.wal_bytes().expect("wal").len();
            assert!(
                grown > wal_len,
                "round {round}: the WAL buffers the outage, growing monotonically ({wal_len} -> {grown})"
            );
            wal_len = grown;
            let progress = harness.engine.sync_once().await.expect("outage sync");
            assert!(
                progress.error.is_some(),
                "round {round}: the ship attempt fails: {progress:?}"
            );
            let outcome = harness.engine.checkpoint_once().await.expect("checkpoint");
            assert_eq!(
                outcome,
                CheckpointOutcome::SkippedUnshipped,
                "round {round}: no checkpoint completes while unshipped frames exist (I1)"
            );
        }

        // Heal, clear the backoff, and drive to quiescence.
        harness.flaky().heal();
        harness.advance(400);
        let mut quiescent = false;
        for _ in 0..100 {
            let progress = harness.engine.sync_once().await.expect("catch-up sync");
            assert!(
                progress.error.is_none(),
                "healed ticks succeed: {progress:?}"
            );
            if progress.shipped_segments == 0 {
                quiescent = true;
                break;
            }
        }
        assert!(quiescent, "the backlog fully ships after healing");

        // Every successful ship, bootstrap included, landed strictly in
        // (epoch, start) order and contiguously.
        let keys = harness.shipped_segment_keys();
        assert!(!keys.is_empty(), "segments were shipped: {keys:?}");
        for (previous, next) in keys.iter().zip(keys.iter().skip(1)) {
            assert!(
                previous < next,
                "segments ship in strictly increasing key order: {keys:?}"
            );
        }
        let mut expected_start = None;
        for key in &keys {
            let (start, end) = segment_range(key);
            if let Some(expected) = expected_start {
                assert_eq!(start, expected, "segments are contiguous: {keys:?}");
            }
            expected_start = Some(end);
        }

        let outcome = harness.engine.checkpoint_once().await.expect("checkpoint");
        assert_eq!(
            outcome,
            CheckpointOutcome::Completed,
            "with everything shipped the checkpoint completes"
        );
        harness.assert_restore_equivalent().await;
    }

    /// F4: a directory-listing failure at resume (the first replica read the
    /// engine makes) surfaces as a retryable error, creates no generation, and
    /// uploads nothing (an unreachable replica must never read as an empty
    /// one); a healed retry resumes the EXISTING generation at the exact
    /// position.
    #[tokio::test]
    async fn list_error_at_resume_no_new_generation() {
        let mut harness = Harness::new().await;
        harness.ready().await;
        harness
            .fixture
            .txn(&["INSERT INTO t (data) VALUES ('resume me')"])
            .expect("txn");
        harness.engine.sync_once().await.expect("sync");
        let generation = harness.engine.generation().cloned().expect("generation");
        let position = harness.engine.position().cloned().expect("position");

        // "Process crash": the old engine is dropped on rebuild; the failing
        // construction below never touches it.
        let files_before = replica_file_set(&harness.replica_root);
        let error = harness
            .rebuild_engine_with_plan(FailurePlan::new().fail_all(OpKind::ListDirs))
            .await
            .expect_err("resume against an unreachable replica must fail");
        assert!(
            error.is_retryable(),
            "an unreachable replica at boot is retryable, never fatal: {error}"
        );
        assert!(
            matches!(
                error,
                SyncError::Storage(StorageError::Injected {
                    op: "list_dirs",
                    ..
                })
            ),
            "the injected directory-listing error surfaces unchanged: {error}"
        );
        assert_eq!(
            replica_file_set(&harness.replica_root),
            files_before,
            "a failed resume creates no generation and uploads nothing"
        );

        // Heal (same replica root) and retry: the existing generation is
        // resumed, nothing re-ships, and no new generation dir appears.
        harness
            .rebuild_engine_with_plan(FailurePlan::new())
            .await
            .expect("healed resume");
        assert_eq!(
            harness.engine.state(),
            EngineState::Streaming,
            "the healed resume streams immediately (no snapshot)"
        );
        assert_eq!(
            harness.engine.generation(),
            Some(&generation),
            "the EXISTING generation is resumed"
        );
        assert_eq!(
            harness.engine.position(),
            Some(&position),
            "the exact position is recovered from the replica LIST"
        );
        let progress = harness.engine.sync_once().await.expect("quiet sync");
        assert_eq!(progress.shipped_segments, 0, "nothing re-ships");
        assert_eq!(harness.put_attempts(), 0, "no puts after a clean resume");
        let generation_dirs =
            std::fs::read_dir(harness.replica_root.join("generations").as_std_path())
                .expect("generations dir")
                .count();
        assert_eq!(generation_dirs, 1, "exactly one generation dir on disk");
        harness.assert_restore_equivalent().await;
    }

    /// F5: orphaned junk on the replica (a crashed `.partial-` upload and a
    /// bogus half-object under the WAL prefix) is invisible to shipping and
    /// restore, is never deleted by shipping, and is reaped only when its
    /// whole generation is pruned.
    #[tokio::test]
    async fn orphan_partial_upload_ignored() {
        let mut harness = Harness::with_config(|json| {
            json.retention_generations = Some(1);
        })
        .await;
        harness.ready().await;
        let old_generation = harness.engine.generation().cloned().expect("generation");
        let old_dir = harness.generation_dir(&old_generation);

        // Plant the orphans by hand, as a crashed uploader would leave them.
        let partial_orphan = old_dir.join("snapshot.db.zst.partial-garbage");
        std::fs::write(partial_orphan.as_std_path(), b"crashed multipart part")
            .expect("plant partial orphan");
        let wal_dir = old_dir.join("wal");
        std::fs::create_dir_all(wal_dir.as_std_path()).expect("wal dir");
        let half_object = wal_dir.join("0000000000-halfobject");
        std::fs::write(half_object.as_std_path(), b"half an object").expect("plant half object");

        // Shipping proceeds unbothered.
        harness
            .fixture
            .txn(&["INSERT INTO t (data) VALUES ('after orphans')"])
            .expect("txn");
        let progress = harness.engine.sync_once().await.expect("sync");
        assert!(
            progress.error.is_none() && progress.shipped_segments >= 1,
            "shipping ignores the orphans: {progress:?}"
        );

        // Restore ignores them too (list filters the partial infix; the
        // unparseable key is skipped with a warning).
        let restored = harness.assert_restore_equivalent().await;
        assert_eq!(
            restored, old_generation,
            "restore uses the real generation despite the orphans"
        );
        assert!(
            partial_orphan.as_std_path().exists(),
            "shipping never deletes the partial orphan"
        );
        assert!(
            half_object.as_std_path().exists(),
            "shipping never deletes the half object"
        );

        // A new generation under retention 1 prunes the old generation
        // wholesale, orphans included.
        harness.drive_snapshot().await;
        let new_generation = harness.engine.generation().cloned().expect("generation");
        assert!(
            new_generation != old_generation,
            "the snapshot created a new generation"
        );
        assert!(
            !old_dir.as_std_path().exists(),
            "pruning removes the whole old generation directory"
        );
        assert!(
            !partial_orphan.as_std_path().exists(),
            "pruning reaps the partial orphan with its generation"
        );
        assert!(
            !half_object.as_std_path().exists(),
            "pruning reaps the half object with its generation"
        );

        harness.engine.sync_once().await.expect("backlog sync");
        let restored = harness.assert_restore_equivalent().await;
        assert_eq!(restored, new_generation, "restore picks the new generation");
    }

    /// F6: a corrupt (truncated) snapshot body makes the startup restore
    /// fail HARD, leaving no file at the target path and no `.restore` /
    /// `.restore-wal` / `.restore-shm` scratch leftovers behind.
    #[tokio::test]
    async fn get_fails_mid_restore_no_half_db() {
        let mut harness = Harness::new().await;
        harness.ready().await;
        harness
            .fixture
            .txn(&["INSERT INTO t (data) VALUES ('tamper target')"])
            .expect("txn");
        harness.engine.sync_once().await.expect("sync");
        let generation = harness.engine.generation().cloned().expect("generation");

        // Corrupt the snapshot body in place: truncate it to half. The
        // restore fails on the broken zstd stream (or, failing that, on the
        // sha256 verification).
        let snapshot_path = harness.generation_dir(&generation).join("snapshot.db.zst");
        let body = std::fs::read(snapshot_path.as_std_path()).expect("snapshot body");
        assert!(body.len() >= 2, "the snapshot body is non-trivial");
        std::fs::write(
            snapshot_path.as_std_path(),
            body.get(..body.len() >> 1).expect("half the body"),
        )
        .expect("truncate snapshot");

        let target_tmp = tempfile::tempdir().expect("target tempdir");
        let target_db = dir_path(&target_tmp).join("restored.db");
        let error = restore_if_missing(&logger(), &harness.config, &target_db)
            .await
            .expect_err("a truncated snapshot must fail the restore");
        assert!(
            matches!(
                error,
                RestoreError::SnapshotDownload { .. } | RestoreError::SnapshotChecksum { .. }
            ),
            "the failure names the snapshot, not a generic IO error: {error}"
        );

        assert!(
            !target_db.as_std_path().exists(),
            "no half-restored database at the target path"
        );
        let leftovers: Vec<String> = std::fs::read_dir(dir_path(&target_tmp).as_std_path())
            .expect("read target dir")
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            leftovers,
            Vec::<String>::new(),
            "a failed restore leaves no scratch files behind"
        );
    }

    /// F7: delete failures during pruning are logged and swallowed (the
    /// snapshot completes and sync keeps working); after healing, the next
    /// prune deletes the old generation and restore picks the newest.
    #[tokio::test]
    async fn delete_fails_during_prune() {
        let mut harness = Harness::with_config(|json| {
            json.retention_generations = Some(1);
        })
        .await;
        harness.ready().await;
        let old_generation = harness.engine.generation().cloned().expect("generation");
        harness
            .fixture
            .txn(&["INSERT INTO t (data) VALUES ('pre-prune')"])
            .expect("txn");
        harness.engine.sync_once().await.expect("sync");

        harness
            .flaky()
            .set_plan(FailurePlan::new().fail_all(OpKind::DeletePrefix));
        // The snapshot completes even though its finalize-time prune fails.
        harness.drive_snapshot().await;
        let new_generation = harness.engine.generation().cloned().expect("generation");
        assert!(
            new_generation != old_generation,
            "the snapshot created a new generation"
        );
        let injected_deletes = harness
            .flaky()
            .journal()
            .iter()
            .filter(|(op, outcome)| {
                op.kind == OpKind::DeletePrefix && *outcome == OpOutcome::Injected
            })
            .count();
        assert!(
            injected_deletes >= 1,
            "the prune attempted a delete_prefix and it was injected"
        );
        assert!(
            harness
                .generation_dir(&old_generation)
                .as_std_path()
                .exists(),
            "the old generation survives the failed prune"
        );
        // Marker-first prune: the snapshot.json delete precedes the injected
        // delete_prefix, so the surviving old generation is markerless
        // (invisible to resume/restore), never a marker without a body.
        assert!(
            !harness
                .generation_dir(&old_generation)
                .join("snapshot.json")
                .as_std_path()
                .exists(),
            "the failed prune already removed the old generation's marker"
        );

        // Sync keeps working through the prune failure.
        assert_eq!(
            harness.engine.state(),
            EngineState::Streaming,
            "the engine keeps streaming"
        );
        harness
            .fixture
            .txn(&["INSERT INTO t (data) VALUES ('post-prune-failure')"])
            .expect("txn");
        let progress = harness
            .engine
            .sync_once()
            .await
            .expect("sync keeps working");
        assert!(
            progress.error.is_none() && progress.shipped_segments >= 1,
            "prune failures never poison the sync loop: {progress:?}"
        );

        // Heal and prune again. The marker-first failed prune left the old
        // generation markerless, so it is classified incomplete
        // (indistinguishable from a still-uploading snapshot) and is reaped
        // as stale-incomplete rather than immediately: advance past the 24h
        // stale cutoff so the healed prune reaps it.
        harness.flaky().heal();
        harness.advance(25 * 60 * 60);
        harness.engine.prune_once().await.expect("healed prune");
        assert!(
            !harness
                .generation_dir(&old_generation)
                .as_std_path()
                .exists(),
            "the old generation is reaped once deletes heal and it goes stale"
        );
        assert!(
            harness
                .generation_dir(&new_generation)
                .as_std_path()
                .exists(),
            "the current generation is never pruned"
        );
        let restored = harness.assert_restore_equivalent().await;
        assert_eq!(
            restored, new_generation,
            "restore picks the newest generation"
        );
    }

    /// F8: an injected multipart write aborts the snapshot; the aborted
    /// generation stays invisible (no snapshot.json, no visible body), the
    /// engine retriggers on its own, and after healing a fresh snapshot
    /// completes and restore picks it.
    #[tokio::test]
    async fn multipart_write_fails_mid_snapshot() {
        // 1 MiB copy budget per step forces multiple multipart writes.
        let mut harness = Harness::with_config(|json| {
            json.snapshot_throttle_mib = Some(1);
        })
        .await;
        harness.ready().await;
        // Grow the database FILE (not just the WAL) so the copy spans
        // several parts: big transaction, ship, full checkpoint.
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
            "one bootstrap generation before the fault"
        );

        harness
            .flaky()
            .set_plan(FailurePlan::new().fail_nth(OpKind::MultipartWrite, 2));
        harness.engine.trigger_snapshot();
        let mut surfaced = None;
        for _ in 0..64 {
            match harness.engine.snapshot_step().await {
                Ok(SnapshotStatus::InProgress) => {},
                Ok(SnapshotStatus::Finished) => {
                    panic!("the snapshot must not finish past an injected multipart write")
                },
                Err(error) => {
                    surfaced = Some(error);
                    break;
                },
            }
        }
        let error = surfaced.expect("the injected multipart write must surface");
        assert!(
            matches!(
                error,
                SyncError::Storage(StorageError::Injected {
                    op: "multipart_write",
                    ..
                })
            ),
            "the snapshot aborts on the injected multipart write: {error}"
        );
        assert_eq!(
            object_count(&harness, "snapshot.json").await,
            1,
            "no snapshot.json for the aborted generation: it stays invisible"
        );
        assert_eq!(
            object_count(&harness, "snapshot.db.zst").await,
            1,
            "the aborted body upload never became visible"
        );

        // The abort scheduled a retrigger: the next tick starts over.
        harness.flaky().heal();
        harness.advance(5);
        harness.engine.sync_once().await.expect("retrigger tick");
        assert_eq!(
            harness.engine.state(),
            EngineState::Snapshotting,
            "the aborted snapshot retriggers after healing"
        );
        for _ in 0..64 {
            if object_count(&harness, "snapshot.json").await == 2 {
                break;
            }
            harness.engine.sync_once().await.expect("recovery sync");
        }
        assert_eq!(
            object_count(&harness, "snapshot.json").await,
            2,
            "the retried snapshot completes"
        );
        let generation = harness.engine.generation().cloned().expect("generation");
        harness.engine.sync_once().await.expect("backlog sync");
        let restored = harness.assert_restore_equivalent().await;
        assert_eq!(
            restored, generation,
            "restore picks the completed generation"
        );
    }

    /// F9: a segment put lands server-side but reports failure (a lost 200).
    /// Seeing failure, the engine retries and re-ships the SAME segment; the
    /// key is put twice (applied-but-reported-failed, then a clean overwrite),
    /// and the idempotent re-ship leaves the replica correct and restorable.
    /// Exercises retry idempotency under ambiguous success.
    #[tokio::test]
    async fn put_reports_failure_after_applying_reships() {
        let mut harness = Harness::new().await;
        harness.ready().await;
        // The next segment put reaches the backend but returns an error.
        harness.flaky().set_plan(
            FailurePlan::new()
                .fail_matching(Some(OpKind::Put), ".wal.zst", 1)
                .after(),
        );
        harness
            .fixture
            .txn(&["INSERT INTO t (data) VALUES ('lost-200')"])
            .expect("txn");

        let progress = harness.engine.sync_once().await.expect("ambiguous sync");
        assert!(
            progress.error.is_some(),
            "the lost-200 arms a backoff even though the segment reached the backend: {progress:?}"
        );
        assert_eq!(
            progress.shipped_segments, 0,
            "nothing counts as shipped on the failed tick"
        );

        harness.advance(1);
        let progress = harness.engine.sync_once().await.expect("retry sync");
        assert!(progress.error.is_none(), "the retry succeeds: {progress:?}");
        assert_eq!(
            progress.shipped_segments, 1,
            "the pending segment re-ships exactly once"
        );

        // The same key was put twice: applied-but-reported-failed, then a
        // clean re-ship. Put is idempotent, so the replica ends correct.
        let key = harness
            .shipped_segment_keys()
            .pop()
            .expect("a shipped segment");
        let outcomes: Vec<OpOutcome> = harness
            .flaky()
            .journal()
            .into_iter()
            .filter(|(op, _)| op.kind == OpKind::Put && op.key == key)
            .map(|(_, outcome)| outcome)
            .collect();
        assert_eq!(
            outcomes,
            vec![OpOutcome::InjectedAfter, OpOutcome::Ok],
            "the key is put twice: applied-but-reported-failed, then a clean re-ship"
        );
        harness.assert_restore_equivalent().await;
    }
}
