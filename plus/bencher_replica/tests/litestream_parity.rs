#![cfg(all(feature = "plus", feature = "testing"))]
//! Litestream parity suite: cross-checks ported from Litestream v0.3.x's
//! test suite (github.com/benbjohnson/litestream, tag v0.3.13).
//!
//! - The `TestChecksum` golden vector from `litestream_test.go` (`OnePass` and
//!   `Incremental`), an incremental-equals-one-pass property over both checksum
//!   byte orders, and a big-endian scanner round-trip (the golden WAL fixture
//!   is little-endian only).
//! - Offline WAL-wrap at resume, after `TestDB_Sync/OverwritePrevPosition`:
//!   pins exactly which buried-cycle subcases resume detects (forced new
//!   generation) and which it provably cannot; for the latter the asserted
//!   property is the SAFETY backstop (the restore-and-compare verification
//!   reports the divergence and a fresh generation heals the replica), not
//!   the limitation itself.
//! - The WAL file deleted outright while the process is down, after
//!   `TestDB_Sync/NoWAL`.
//!
//! NOTE: `unused_crate_dependencies` cannot be handled with a crate-level
//! `#![expect]` here (see `tests/storage_contract.rs`); unused package
//! dependencies are referenced explicitly instead, as rustc recommends.

use async_compression as _;
use aws_credential_types as _;
use aws_sdk_s3 as _;
use bytes as _;
use futures as _;
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

/// Shared fixtures: a scripted source database and an engine (behind an
/// `Option`, so a test can stop it and mutate the on-disk state while it is
/// down) over a local replica with a deterministic clock.
#[cfg(test)]
pub(crate) mod harness {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicI64, Ordering};

    use bencher_json::system::config::{JsonReplication, ReplicationTarget};
    use bencher_json::{Clock, DateTime};
    use bencher_replica::testing::{WalFixture, assert_replica_equivalent};
    use bencher_replica::{
        EngineState, GenerationId, LocalStorage, ReplicaConfig, ReplicaDb, ReplicaStorage,
        RestoreOutcome, SyncEngine, restore_if_missing,
    };
    use camino::{Utf8Path, Utf8PathBuf};

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

    fn clock_for(secs: &Arc<AtomicI64>) -> Clock {
        let secs = Arc::clone(secs);
        Clock::Custom(Arc::new(move || {
            DateTime::try_from(secs.load(Ordering::SeqCst)).expect("valid clock seconds")
        }))
    }

    pub(crate) struct Harness {
        /// `None` after [`Harness::close_fixture`] (all source connections
        /// closed, modeling a fully dead process).
        fixture: Option<WalFixture>,
        /// `None` between [`Harness::crash`] and [`Harness::rebuild_engine`].
        engine: Option<SyncEngine<()>>,
        config: ReplicaConfig,
        db: ReplicaDb<()>,
        clock_secs: Arc<AtomicI64>,
        replica_root: Utf8PathBuf,
        db_path: Utf8PathBuf,
        wal_path: Utf8PathBuf,
        _fixture_tmp: tempfile::TempDir,
        _replica_tmp: tempfile::TempDir,
    }

    impl Harness {
        pub(crate) async fn new() -> Self {
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
            let engine = SyncEngine::new_with_storage(
                logger(),
                config.clone(),
                db.clone(),
                clock_for(&clock_secs),
                false,
                ReplicaStorage::Local(LocalStorage::new(replica_root.clone())),
            )
            .await
            .expect("engine");
            let db_path = fixture.db_path();
            let wal_path = fixture.wal_path();
            Self {
                fixture: Some(fixture),
                engine: Some(engine),
                config,
                db,
                clock_secs,
                replica_root,
                db_path,
                wal_path,
                _fixture_tmp: fixture_tmp,
                _replica_tmp: replica_tmp,
            }
        }

        /// The scripted source database; panics after [`Harness::close_fixture`].
        pub(crate) fn fixture(&self) -> &WalFixture {
            self.fixture.as_ref().expect("the fixture is open")
        }

        /// Path to the source database file (valid even after
        /// [`Harness::close_fixture`]).
        pub(crate) fn db_path(&self) -> &Utf8Path {
            &self.db_path
        }

        /// Path to the source WAL file (valid even after
        /// [`Harness::close_fixture`]).
        pub(crate) fn wal_path(&self) -> &Utf8Path {
            &self.wal_path
        }

        /// Drop the fixture (and with it the last scripted connection).
        /// Callers that must avoid `SQLite`'s close-time checkpoint keep a
        /// holder connection open across this call; see
        /// `close_source_without_checkpoint`.
        pub(crate) fn close_fixture(&mut self) {
            let fixture = self.fixture.take();
            assert!(fixture.is_some(), "the fixture is only closed once");
            drop(fixture);
        }

        /// The live engine; panics after a stop and before a rebuild.
        pub(crate) fn engine(&self) -> &SyncEngine<()> {
            self.engine.as_ref().expect("the engine is running")
        }

        /// The live engine, mutably.
        pub(crate) fn engine_mut(&mut self) -> &mut SyncEngine<()> {
            self.engine.as_mut().expect("the engine is running")
        }

        /// Stop the engine: drop it mid-flight, exactly as in a killed or
        /// shut-down process (the replicator is offline from here on).
        pub(crate) fn crash(&mut self) {
            let engine = self.engine.take();
            assert!(engine.is_some(), "a crash requires a running engine");
            drop(engine);
        }

        /// The recovery: rebuild the engine over the same fixture and
        /// replica directories, resolving the resume decision table.
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
                    ReplicaStorage::Local(LocalStorage::new(self.replica_root.clone())),
                )
                .await
                .expect("engine rebuild"),
            );
        }

        /// Advance the injected clock by `secs`.
        pub(crate) fn advance(&self, secs: i64) {
            self.clock_secs.fetch_add(secs, Ordering::SeqCst);
        }

        /// Drive `sync_once` until the engine is streaming (any pending
        /// new-generation snapshot has completed).
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
            assert_replica_equivalent(&self.db_path, &target_db);
            generation
        }
    }
}

#[cfg(test)]
mod cases {
    use std::io::Cursor;

    use bencher_replica::testing::{CheckpointMode, SyntheticWal};
    use bencher_replica::{
        CheckpointOutcome, EngineState, ReplicaMeta, VerifyReport, WalScanner, wal_checksum,
    };
    use camino::Utf8Path;
    use pretty_assertions::{assert_eq, assert_ne};

    use super::harness::Harness;

    // 1. Checksum parity with Litestream's TestChecksum
    //
    // Golden vector lifted verbatim from litestream_test.go at tag v0.3.13
    // (https://raw.githubusercontent.com/benbjohnson/litestream/v0.3.13/litestream_test.go).
    // The input is the checksum-covered prefix of a real WAL header (24
    // bytes: magic 0x377f0682, format 3007000, page size 4096, sequence 0,
    // salts 0x52382eac / 0x857b1a4e), the first 8 bytes of a commit-frame
    // header (page 2, db size 2), and one 4096-byte page.

    /// The 24 checksum-covered WAL header bytes of the Litestream vector.
    const LITESTREAM_WAL_HEADER_PREFIX_HEX: &str =
        "377f0682002de218000010000000000052382eac857b1a4e";
    /// The first 8 bytes of the vector's frame header (page number, db size).
    const LITESTREAM_FRAME_HEADER_PREFIX_HEX: &str = "0000000200000002";
    /// The vector's 4096-byte page payload.
    const LITESTREAM_PAGE_HEX: &str = concat!(
        "0d000000080fe0000ffc0ff80ff40ff00fec0fe80fe40fe00000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0208020902070209020602090205020902040209020302090202020902010209",
    );
    /// Litestream's expected running checksum after the 24 header bytes.
    const LITESTREAM_HEADER_CHECKSUM: (u32, u32) = (0x8115_3b65, 0x8717_8e8f);
    /// Litestream's expected checksum over the whole vector.
    const LITESTREAM_ONE_PASS_CHECKSUM: (u32, u32) = (0xdc2f_3e84, 0x5404_88d3);

    /// The full one-pass input: header prefix, frame header prefix, page.
    fn litestream_vector() -> Vec<u8> {
        let hex_input = format!(
            "{LITESTREAM_WAL_HEADER_PREFIX_HEX}{LITESTREAM_FRAME_HEADER_PREFIX_HEX}{LITESTREAM_PAGE_HEX}"
        );
        let input = hex::decode(hex_input).expect("golden vector hex");
        assert_eq!(input.len(), 24 + 8 + 4096, "vector covers header + frame");
        input
    }

    /// Litestream `TestChecksum/OnePass`: the exact input produces the exact
    /// pair, checksummed in a single call (little-endian word order, seeded
    /// with zero, exactly as `litestream.Checksum` was invoked).
    #[test]
    fn checksum_golden_vector_matches_litestream_one_pass() {
        let input = litestream_vector();
        assert_eq!(
            wal_checksum(false, (0, 0), &input),
            LITESTREAM_ONE_PASS_CHECKSUM,
            "one-pass checksum over the Litestream vector"
        );
    }

    /// Litestream `TestChecksum/Incremental`: the same result when split
    /// across calls at the same boundaries Litestream used (header, then
    /// frame-header prefix, then page), including the intermediate value
    /// after the header bytes.
    #[test]
    fn checksum_golden_vector_matches_litestream_incremental() {
        let input = litestream_vector();
        let after_header = wal_checksum(false, (0, 0), &input[..24]);
        assert_eq!(
            after_header, LITESTREAM_HEADER_CHECKSUM,
            "running checksum after the 24 WAL header bytes"
        );
        let after_frame_header = wal_checksum(false, after_header, &input[24..32]);
        assert_eq!(
            wal_checksum(false, after_frame_header, &input[32..]),
            LITESTREAM_ONE_PASS_CHECKSUM,
            "incremental checksum equals the one-pass result"
        );
    }

    /// The property behind Litestream's Incremental subtest, generalized:
    /// splitting the input at any 8-byte boundary (including degenerate
    /// empty sides and one call per 8-byte word) never changes the result,
    /// in both checksum byte orders; and the byte order actually matters.
    #[test]
    fn checksum_incremental_equals_one_pass_both_orders() {
        // Deterministic non-repeating-ish buffer; no wall clock, no RNG.
        let data: Vec<u8> = (0u32..4096)
            .map(|index| u8::try_from(index.wrapping_mul(31).wrapping_add(7) & 0xff).expect("byte"))
            .collect();
        let mut one_pass_by_order = Vec::new();
        for big_endian in [false, true] {
            let one_pass = wal_checksum(big_endian, (0, 0), &data);
            for split in [0usize, 8, 16, 24, 512, 1008, 2048, 4088, 4096] {
                let mid = wal_checksum(big_endian, (0, 0), &data[..split]);
                assert_eq!(
                    wal_checksum(big_endian, mid, &data[split..]),
                    one_pass,
                    "split at {split} (big_endian: {big_endian})"
                );
            }
            // One call per 8-byte word: the finest possible split.
            let mut running = (0, 0);
            for word_pair in data.chunks(8) {
                running = wal_checksum(big_endian, running, word_pair);
            }
            assert_eq!(
                running, one_pass,
                "word-by-word chain (big_endian: {big_endian})"
            );
            one_pass_by_order.push(one_pass);
        }
        assert_ne!(
            one_pass_by_order[0], one_pass_by_order[1],
            "the byte-order flag must change the result; identical values \
             would mean the flag is ignored"
        );
    }

    /// The golden fixture (`wal_le_4096.wal`) is little-endian only: this
    /// pins the big-endian path end to end. A synthetic BE WAL (independent
    /// checksum implementation, magic 0x377f0683) round-trips through
    /// `WalScanner`: header parse, per-commit chunking, mid-chain resume,
    /// commit detection, and an uncommitted tail that never ships.
    #[test]
    fn big_endian_synthetic_wal_round_trips_scanner() {
        const PAGE: u32 = 512;
        const SALT: (u32, u32) = (0x0102_0304, 0x0506_0708);
        let frame_size = 24 + u64::from(PAGE);
        let page_of = |fill: u8| vec![fill; usize::try_from(PAGE).expect("page size")];
        let build = |big_endian: bool| {
            SyntheticWal::new(PAGE, big_endian, SALT)
                .frame(1, &page_of(0x11))
                .commit_frame(2, &page_of(0x22), 2)
                .commit_frame(3, &page_of(0x33), 3)
                .frame(4, &page_of(0x44))
                .bytes()
        };
        let bytes = build(true);

        let mut scanner = WalScanner::open(Cursor::new(bytes.clone()))
            .expect("BE WAL parses")
            .expect("BE WAL has a header");
        let header = *scanner.header();
        assert_eq!(header.magic, 0x377f_0683, "big-endian checksum magic");
        assert!(header.big_endian_checksum());
        assert_eq!(header.salt, SALT);

        // Per-commit chunking: the two-frame transaction, then the third
        // frame's commit; the trailing open frame is never surfaced.
        let first = scanner
            .next_committed(1)
            .expect("scan first chunk")
            .expect("first commit");
        assert_eq!(first.end_offset, 32 + 2 * frame_size, "first commit ends");
        assert_eq!(first.commit_count, 1);
        let second = scanner
            .next_committed(1)
            .expect("scan second chunk")
            .expect("second commit");
        assert_eq!(second.end_offset, 32 + 3 * frame_size, "second commit ends");
        assert!(
            scanner
                .next_committed(u64::MAX)
                .expect("scan tail")
                .is_none(),
            "the uncommitted BE tail frame is never surfaced"
        );

        // Mid-chain resume from the recorded (offset, checksum) reproduces
        // the exact tail bytes.
        let mut resumed = WalScanner::resume(
            Cursor::new(bytes),
            header,
            first.end_offset,
            first.checksum_at_end,
        )
        .expect("resume mid-chain");
        let tail = resumed
            .next_committed(u64::MAX)
            .expect("scan resumed tail")
            .expect("resumed commit");
        assert_eq!(tail.bytes, second.bytes, "resume reproduces the tail");
        assert_eq!(tail.checksum_at_end, second.checksum_at_end);

        // Same logical content under the LE magic stores a DIFFERENT frame
        // checksum: proof that the byte-order flag reaches the chain (both
        // still scan to the same end offset).
        let le_bytes = build(false);
        let be_bytes = build(true);
        let checksum_at = |bytes: &[u8]| bytes[32 + 16..32 + 24].to_vec();
        assert_ne!(
            checksum_at(&be_bytes),
            checksum_at(&le_bytes),
            "BE and LE chains must store different frame checksums"
        );
    }

    // 2. Offline WAL-wrap at resume (Litestream TestDB_Sync/OverwritePrevPosition)

    /// Read the current committed extent and header salts of a WAL file.
    fn wal_extent_and_salt(wal_path: &Utf8Path) -> (u64, (u32, u32)) {
        let wal = std::fs::read(wal_path).expect("read wal");
        let mut scanner = WalScanner::open(Cursor::new(wal))
            .expect("wal parses")
            .expect("wal has a header");
        let salt = scanner.header().salt;
        let mut end = 32u64;
        while let Some(chunk) = scanner.next_committed(u64::MAX).expect("scan wal") {
            end = chunk.end_offset;
        }
        (end, salt)
    }

    /// Assemble a big-endian `u32` from `bytes[at..at + 4]`.
    fn be_u32_at(bytes: &[u8], at: usize) -> u32 {
        (u32::from(bytes[at]) << 24)
            | (u32::from(bytes[at + 1]) << 16)
            | (u32::from(bytes[at + 2]) << 8)
            | u32::from(bytes[at + 3])
    }

    /// Write `value` big-endian into `bytes[at..at + 4]`.
    fn write_be_u32(bytes: &mut [u8], at: usize, value: u32) {
        bytes[at] = u8::try_from((value >> 24) & 0xff).expect("byte");
        bytes[at + 1] = u8::try_from((value >> 16) & 0xff).expect("byte");
        bytes[at + 2] = u8::try_from((value >> 8) & 0xff).expect("byte");
        bytes[at + 3] = u8::try_from(value & 0xff).expect("byte");
    }

    /// Rewrite a WAL file's salt1 in place (header and every frame copy),
    /// recomputing the header self-checksum and the whole frame checksum
    /// chain so the file is fully valid under the new salt. Simulates the
    /// 1-in-2^32 case where a brand-new WAL's RANDOM salt1 collides with the
    /// exact value the salt-continuity check expects.
    fn doctor_wal_salt1(wal_path: &Utf8Path, salt1: u32) {
        let mut wal = std::fs::read(wal_path).expect("read wal");
        assert!(wal.len() >= 32, "wal has a full header");
        // Manual byte math throughout: the workspace warns on
        // from_be_bytes/to_be_bytes.
        let big_endian = wal[3] & 1 == 1;
        let page_size = usize::try_from(be_u32_at(&wal, 8)).expect("page size");
        let frame_size = 24 + page_size;
        write_be_u32(&mut wal, 16, salt1);
        let header_prefix = wal[..24].to_vec();
        let mut running = wal_checksum(big_endian, (0, 0), &header_prefix);
        write_be_u32(&mut wal, 24, running.0);
        write_be_u32(&mut wal, 28, running.1);
        let mut offset = 32;
        while offset + frame_size <= wal.len() {
            write_be_u32(&mut wal, offset + 8, salt1);
            let head = wal[offset..offset + 8].to_vec();
            let page = wal[offset + 24..offset + frame_size].to_vec();
            let after_head = wal_checksum(big_endian, running, &head);
            let computed = wal_checksum(big_endian, after_head, &page);
            write_be_u32(&mut wal, offset + 16, computed.0);
            write_be_u32(&mut wal, offset + 20, computed.1);
            running = computed;
            offset += frame_size;
        }
        std::fs::write(wal_path, wal).expect("write doctored wal");
    }

    /// Close every source connection WITHOUT `SQLite`'s close-time
    /// checkpoint, modeling a killed process. A same-process test cannot
    /// simply delete the WAL under live connections (the unix VFS caches
    /// the shm node per inode, so every later connection would hit I/O
    /// errors), and closing them normally would run the close-time
    /// checkpoint, backfilling the WAL and turning "files deleted while
    /// down" into a plain checkpoint reset. Instead a holder connection
    /// pins the WAL open (WAL-mode shared lock) while the fixture closes,
    /// then closes itself with `SQLITE_DBCONFIG_NO_CKPT_ON_CLOSE`.
    fn close_source_without_checkpoint(harness: &mut Harness) {
        let holder = rusqlite::Connection::open(harness.db_path()).expect("holder conn");
        holder
            .set_db_config(
                rusqlite::config::DbConfig::SQLITE_DBCONFIG_NO_CKPT_ON_CLOSE,
                true,
            )
            .expect("no ckpt on close");
        // Touch the database so the holder actually opens the WAL and holds
        // the shared lock that blocks the fixture's closing checkpoint.
        let _tables: i64 = holder
            .query_row("SELECT COUNT(*) FROM sqlite_master", [], |row| row.get(0))
            .expect("holder query");
        harness.close_fixture();
        drop(holder);
    }

    /// A fresh, independent connection to the source database (busy timeout
    /// set, autocheckpoint disabled), for writes after the fixture's own
    /// connection is gone.
    fn fresh_conn(db_path: &Utf8Path) -> rusqlite::Connection {
        let conn = rusqlite::Connection::open(db_path).expect("fresh conn");
        conn.busy_timeout(std::time::Duration::from_secs(5))
            .expect("busy timeout");
        let _pages: i64 = conn
            .query_row("PRAGMA wal_autocheckpoint = 0", [], |row| row.get(0))
            .expect("autocheckpoint off");
        conn
    }

    /// Seed the resume proof: ship one commit, complete a checkpoint (the
    /// advisory meta now proves "epoch shipped through checkpoint"), and
    /// return the sealed position.
    async fn seal_epoch(harness: &mut Harness) -> bencher_replica::Position {
        harness.ready().await;
        harness
            .fixture()
            .txn(&["INSERT INTO t (data) VALUES ('sealed')"])
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
            "the full backfill seals the epoch and writes the meta proof"
        );
        harness.engine().position().cloned().expect("position")
    }

    /// Offline WAL wrap, detected subcase: while the replicator is down the
    /// WAL is checkpointed, reset, and rewritten PAST the previously
    /// recorded position, burying a whole never-shipped cycle. Two restarts
    /// happen (write, external RESTART checkpoint, write), so salt1 jumps by
    /// two; resume proves the discontinuity and forces a new generation
    /// whose snapshot recaptures the buried commits.
    ///
    /// Deterministic construction: a RESTART-mode checkpoint (not TRUNCATE)
    /// keeps the WAL file in place, so the second restart increments the
    /// EXISTING header's salt1 instead of minting random salts.
    #[tokio::test]
    async fn offline_wal_wrap_buried_cycle_forces_new_generation_at_resume() {
        let mut harness = Harness::new().await;
        let sealed = seal_epoch(&mut harness).await;
        let old_generation = harness.engine().generation().cloned().expect("generation");
        harness.crash();

        // Down: a commit restarts the fully backfilled WAL (salt1 + 1); its
        // frames never ship. CREATE TABLE makes the eventual loss visible.
        harness
            .fixture()
            .txn(&[
                "CREATE TABLE buried (id INTEGER PRIMARY KEY, data TEXT)",
                "INSERT INTO buried (data) VALUES ('never shipped')",
            ])
            .expect("buried txn");
        // An external RESTART checkpoint backfills the buried cycle, then
        // the next write restarts the WAL again (salt1 + 2) and rewrites it
        // from offset 0, past the previously recorded position.
        harness
            .fixture()
            .checkpoint(CheckpointMode::Restart)
            .expect("external RESTART checkpoint");
        harness
            .fixture()
            .txn_touching_pages(32)
            .expect("big txn after restart");

        // Guards: this is the OverwritePrevPosition shape (the rewritten
        // cycle extends past the sealed offset) with a salt jump of exactly
        // two.
        let (extent, salt) = wal_extent_and_salt(&harness.fixture().wal_path());
        assert!(
            extent > sealed.offset,
            "the rewritten cycle must extend past the previously shipped \
             offset ({extent} <= {})",
            sealed.offset
        );
        assert_eq!(
            salt.0,
            sealed.salt.0.wrapping_add(2),
            "two WAL restarts while down: salt1 jumps by two"
        );

        harness.rebuild_engine().await;
        assert_eq!(
            harness.engine().state(),
            EngineState::PendingSnapshot,
            "resume proves the salt discontinuity (jump greater than one) \
             and forces a new generation, never a silent epoch+1 resume"
        );
        harness.until_streaming().await;
        harness.engine_mut().sync_once().await.expect("backlog");
        let restored = harness.assert_restore_equivalent().await;
        assert_ne!(
            restored, old_generation,
            "the fresh generation (recapturing the buried commits) is the \
             restore source"
        );
    }

    /// Offline WAL wrap, provably undetectable subcase: a single external
    /// checkpoint-plus-restart increments salt1 by exactly one, which is
    /// indistinguishable at resume from a legitimate restart of our own
    /// fully-shipped WAL (the residual limitation documented on
    /// `transition_epoch` and the resume path in sync.rs).
    ///
    /// With the WAL present, burial plus a plus-one jump additionally
    /// requires the fresh WAL's RANDOM salt1 to collide with the expected
    /// value (probability 2^-32); the collision is simulated by doctoring
    /// the WAL's salt1 in place with a recomputed checksum chain. Resume
    /// then takes the meta-verified epoch+1 path and the buried cycle is
    /// silently absent from the replica.
    ///
    /// The asserted property is the SAFETY backstop, not the miss: the
    /// restore-and-compare verification reports the divergence and the
    /// forced fresh generation heals the replica. Litestream's resume
    /// re-reads the last shipped frame's bytes before trusting a position,
    /// which refuses this state outright (at the cost of a new generation on
    /// EVERY restart-while-down); a last-frame re-verification at startup
    /// would close the gap here too.
    #[tokio::test]
    async fn offline_salt_collision_slips_resume_until_verify_backstop() {
        let mut harness = Harness::new().await;
        let sealed = seal_epoch(&mut harness).await;
        let old_generation = harness.engine().generation().cloned().expect("generation");
        harness.crash();

        // Down: a commit restarts the WAL (the buried cycle), an external
        // TRUNCATE checkpoint backfills it and zeroes the WAL, and a new
        // commit starts a brand-new WAL with RANDOM salts.
        harness
            .fixture()
            .txn(&[
                "CREATE TABLE buried (id INTEGER PRIMARY KEY, data TEXT)",
                "INSERT INTO buried (data) VALUES ('never shipped')",
            ])
            .expect("buried txn");
        harness
            .fixture()
            .checkpoint(CheckpointMode::Truncate)
            .expect("external TRUNCATE checkpoint");
        harness
            .fixture()
            .txn(&["INSERT INTO t (data) VALUES ('fresh cycle')"])
            .expect("txn on the fresh WAL");

        // Simulate the salt collision: rewrite the fresh WAL's salt1 to
        // exactly meta.salt1 + 1, the one value the continuity check accepts.
        let meta = ReplicaMeta::load(&harness.fixture().db_path())
            .expect("load meta")
            .expect("meta present");
        assert_eq!(meta.salt1, sealed.salt.0, "meta records the sealed cycle");
        assert!(
            meta.epoch_shipped_through_checkpoint,
            "the seal left the checkpoint proof intact"
        );
        doctor_wal_salt1(&harness.fixture().wal_path(), meta.salt1.wrapping_add(1));

        // Resume SLIPS: the meta proof plus the (collided) plus-one salt
        // read as a legitimate single restart, so the engine continues the
        // lineage as epoch+1 with the buried cycle silently missing from the
        // replica. This assertion pins the documented limitation; if resume
        // ever learns to detect this state (last-frame re-verification), it
        // should start failing and the test should be updated to assert the
        // detection instead.
        harness.rebuild_engine().await;
        assert_eq!(harness.engine().state(), EngineState::Streaming);
        assert_eq!(
            harness.engine().generation(),
            Some(&old_generation),
            "the slipped resume continues the same generation"
        );
        let resumed = harness.engine().position().cloned().expect("position");
        assert_eq!(resumed.epoch, sealed.epoch + 1, "meta-verified epoch+1");
        assert_eq!(resumed.offset, 0, "the new epoch starts at offset 0");
        assert_eq!(
            resumed.salt.0,
            sealed.salt.0.wrapping_add(1),
            "the collided salt was accepted"
        );
        let progress = harness.engine_mut().sync_once().await.expect("sync");
        assert!(
            progress.shipped_segments >= 1,
            "the fresh cycle ships into the continued lineage: {progress:?}"
        );

        // The SAFETY property: the verification backstop detects the buried
        // cycle (the replica cannot reproduce the source) and the failure
        // forces a fresh generation that heals the replica.
        harness.advance(7 * 60 * 60);
        let report = harness.engine_mut().verify_once().await.expect("verify");
        assert!(
            matches!(report, Some(VerifyReport::Fail { .. })),
            "the restore-and-compare backstop must report the burial, got \
             {report:?}"
        );
        harness.engine_mut().sync_once().await.expect("retrigger");
        harness.until_streaming().await;
        harness.engine_mut().sync_once().await.expect("backlog");
        let restored = harness.assert_restore_equivalent().await;
        assert_ne!(
            restored, old_generation,
            "the verify failure minted a fresh generation that recaptured \
             the buried commits"
        );
    }

    /// Offline WAL wrap, the other provably undetectable subcase: the buried
    /// cycle's WAL evidence is REMOVED entirely while the replicator is down
    /// (commit, external TRUNCATE checkpoint, then the WAL and shm files
    /// deleted). A brand-new WAL gets random salts, so salt continuity
    /// cannot apply; with the checkpoint-proven meta intact, resume trusts
    /// the WAL-less state and waits in `AwaitingEpoch` (the limitation
    /// documented on `resume_without_wal`), with the buried commit silently
    /// absent from the replica.
    ///
    /// As above, the asserted property is the SAFETY backstop: verification
    /// reports the divergence and the forced fresh generation heals it.
    #[tokio::test]
    async fn offline_buried_cycle_without_wal_slips_resume_until_verify_backstop() {
        let mut harness = Harness::new().await;
        let sealed = seal_epoch(&mut harness).await;
        let old_generation = harness.engine().generation().cloned().expect("generation");
        harness.crash();

        // Down: bury a cycle (commit, then TRUNCATE backfills it and zeroes
        // the WAL), then close every source connection without a checkpoint
        // and remove the WAL and shm files outright: the salt evidence is
        // gone entirely.
        harness
            .fixture()
            .txn(&[
                "CREATE TABLE buried (id INTEGER PRIMARY KEY, data TEXT)",
                "INSERT INTO buried (data) VALUES ('never shipped')",
            ])
            .expect("buried txn");
        harness
            .fixture()
            .checkpoint(CheckpointMode::Truncate)
            .expect("external TRUNCATE checkpoint");
        close_source_without_checkpoint(&mut harness);
        std::fs::remove_file(harness.wal_path()).expect("delete wal");
        let shm_path = format!("{}-shm", harness.db_path());
        std::fs::remove_file(&shm_path).expect("delete shm");

        // Resume SLIPS: the meta proof vouches for a WAL-less state, so the
        // engine awaits the next epoch on the same lineage. This pins the
        // documented limitation (see the salt-collision test above for the
        // update policy if resume ever learns to detect it).
        harness.rebuild_engine().await;
        assert_eq!(
            harness.engine().state(),
            EngineState::AwaitingEpoch,
            "a checkpoint-proven meta trusts the WAL-less state"
        );
        assert_eq!(
            harness.engine().generation(),
            Some(&old_generation),
            "the slipped resume continues the same generation"
        );

        // First frames appear via a fresh connection (every pre-deletion
        // connection is gone); the awaited epoch binds the brand-new random
        // salts and ships.
        let stray = fresh_conn(harness.db_path());
        stray
            .execute("INSERT INTO t (data) VALUES ('after burial')", [])
            .expect("stray insert");
        let progress = harness.engine_mut().sync_once().await.expect("sync");
        assert!(
            progress.shipped_segments >= 1,
            "the awaited epoch binds and ships: {progress:?}"
        );
        let position = harness.engine().position().cloned().expect("position");
        assert_eq!(position.epoch, sealed.epoch + 1, "bound as the next epoch");
        assert_eq!(
            harness.engine().generation(),
            Some(&old_generation),
            "still the same generation: the burial is silent"
        );

        // The SAFETY property: the verification backstop detects the buried
        // cycle and the forced fresh generation heals the replica.
        harness.advance(7 * 60 * 60);
        let report = harness.engine_mut().verify_once().await.expect("verify");
        assert!(
            matches!(report, Some(VerifyReport::Fail { .. })),
            "the restore-and-compare backstop must report the burial, got \
             {report:?}"
        );
        harness.engine_mut().sync_once().await.expect("retrigger");
        harness.until_streaming().await;
        harness.engine_mut().sync_once().await.expect("backlog");
        let restored = harness.assert_restore_equivalent().await;
        assert_ne!(
            restored, old_generation,
            "the verify failure minted a fresh generation that recaptured \
             the buried commits"
        );
        drop(stray);
    }

    // 3. WAL file deleted while the process is down (Litestream TestDB_Sync/NoWAL)

    /// The WAL (and shm) file deleted outright while the process is down,
    /// with unshipped progress recorded: a distinct on-disk state from a
    /// checkpoint reset (the salt evidence is gone entirely rather than
    /// replaced). The meta cannot prove a full ship through a checkpoint
    /// (the last segment ship cleared the proof), so resume must force a new
    /// generation, never a silent `AwaitingEpoch` continuation.
    ///
    /// Lives here rather than `tests/crash_recovery.rs` because it is a port
    /// of Litestream's `TestDB_Sync/NoWAL` and belongs with its parity
    /// siblings.
    #[tokio::test]
    async fn wal_deleted_while_down_with_unshipped_progress_forces_new_generation() {
        let mut harness = Harness::new().await;
        // Epoch 0 sealed: 'checkpointed' is backfilled into the db FILE, so
        // the source keeps it when the WAL vanishes.
        seal_epoch(&mut harness).await;
        // Epoch 1: 'shipped-only' restarts the WAL and ships (the ship
        // clears the meta's checkpoint proof); 'unshipped' stays local.
        harness
            .fixture()
            .txn(&["INSERT INTO t (data) VALUES ('shipped-only')"])
            .expect("txn shipped-only");
        let progress = harness.engine_mut().sync_once().await.expect("sync");
        assert!(
            progress.shipped_segments >= 1,
            "the restarted cycle ships: {progress:?}"
        );
        assert_eq!(
            harness.engine().position().expect("position").epoch,
            1,
            "shipping continued as the next epoch"
        );
        harness
            .fixture()
            .txn(&["INSERT INTO t (data) VALUES ('unshipped')"])
            .expect("txn unshipped");
        let old_generation = harness.engine().generation().cloned().expect("generation");
        harness.crash();

        // Down: close every source connection without a checkpoint, then
        // delete the WAL and shm outright. Both epoch-1 commits lived only
        // in the WAL, so the SOURCE rolls back to the checkpointed state;
        // the replica's old generation still holds 'shipped-only'.
        close_source_without_checkpoint(&mut harness);
        std::fs::remove_file(harness.wal_path()).expect("delete wal");
        let shm_path = format!("{}-shm", harness.db_path());
        std::fs::remove_file(&shm_path).expect("delete shm");

        harness.rebuild_engine().await;
        assert_eq!(
            harness.engine().state(),
            EngineState::PendingSnapshot,
            "a WAL-less state without the checkpoint proof forces a new \
             generation, never a silent continuation"
        );

        // Guard against a vacuous pass: a fresh connection proves the
        // deletion really rolled the source back.
        let conn = fresh_conn(harness.db_path());
        let count = |data: &str| -> i64 {
            conn.query_row("SELECT COUNT(*) FROM t WHERE data = ?1", [data], |row| {
                row.get(0)
            })
            .expect("count")
        };
        assert_eq!(count("sealed"), 1, "the checkpointed row survived");
        assert_eq!(count("shipped-only"), 0, "the shipped-only row is gone");
        assert_eq!(count("unshipped"), 0, "the unshipped row is gone");
        drop(conn);

        // The new generation snapshots the rolled-back source; the replica
        // converges to exactly the source state.
        harness.until_streaming().await;
        harness.engine_mut().sync_once().await.expect("backlog");
        let restored = harness.assert_restore_equivalent().await;
        assert_ne!(
            restored, old_generation,
            "the fresh generation is the restore source"
        );
    }
}
