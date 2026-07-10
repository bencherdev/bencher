#![cfg(all(feature = "plus", feature = "testing"))]
//! Restore matrix: startup restore and restore-to-position verification.
//!
//! Every replica in this suite is fabricated BY HAND from the foundation
//! APIs (real WALs from `WalFixture`, chunks from `WalScanner`, zstd via
//! `compress_segment`, objects via `ReplicaStorage`): restore must be
//! correct independently of the sync engine that normally produces
//! replicas.
//!
//! NOTE: `unused_crate_dependencies` cannot be handled with a crate-level
//! `#![expect]` here (see `tests/storage_contract.rs`); unused package
//! dependencies are referenced explicitly instead, as rustc recommends.

use async_compression as _;
use aws_credential_types as _;
use aws_sdk_s3 as _;
use futures as _;
use rand as _;
use serde as _;
use serde_json as _;
use thiserror as _;
use uuid as _;
use zstd as _;
// Optional dependency enabled by the otel feature; unused by tests.
#[cfg(feature = "otel")]
use bencher_otel as _;

/// Shared fixtures: a scripted source database, a local replica, and a
/// restore target, plus hand-rolled snapshot/segment shippers.
#[cfg(test)]
pub(crate) mod harness {
    use std::io::Cursor;

    use bencher_json::DateTime;
    use bencher_json::system::config::{JsonReplication, ReplicationTarget};
    use bencher_replica::testing::{CheckpointMode, WalFixture};
    use bencher_replica::{
        GenerationId, LocalStorage, ReplicaConfig, ReplicaStorage, RestoreError, RestoreOutcome,
        SnapshotMeta, WAL_HEADER_SIZE, WalBoundary, WalScanner, compress_segment,
        decompress_segment, restore_if_missing,
    };
    use bytes::Bytes;
    use camino::{Utf8Path, Utf8PathBuf};
    use sha2::{Digest as _, Sha256};

    /// Page size for every fixture database in this suite.
    pub(crate) const PAGE_SIZE: u32 = 4096;
    /// 2026-07-10T14:59:00Z, the base second for deterministic generations.
    const BASE_SECS: i64 = 1_783_695_540;

    pub(crate) fn dir_path(tmp: &tempfile::TempDir) -> &Utf8Path {
        Utf8Path::from_path(tmp.path()).expect("tempdir path is UTF-8")
    }

    pub(crate) fn logger() -> slog::Logger {
        slog::Logger::root(slog::Discard, slog::o!())
    }

    /// Deterministic generation ids: `generation(n)` sorts before
    /// `generation(n + 1)`.
    pub(crate) fn generation(index: u32) -> GenerationId {
        let created = DateTime::try_from(BASE_SECS + i64::from(index)).expect("valid timestamp");
        GenerationId::new(created, index)
    }

    /// A scripted source database, a local replica root, and a separate
    /// restore target directory.
    pub(crate) struct Harness {
        pub fixture: WalFixture,
        pub storage: ReplicaStorage,
        pub config: ReplicaConfig,
        /// Restore target: `<target dir>/bencher.db` (does not exist yet).
        pub db_path: Utf8PathBuf,
        /// Directory holding the fixture (for boundary-state copies).
        pub fixture_dir: Utf8PathBuf,
        pub _fixture_tmp: tempfile::TempDir,
        pub _replica_tmp: tempfile::TempDir,
        pub _target_tmp: tempfile::TempDir,
    }

    impl Harness {
        pub(crate) fn new() -> Self {
            let fixture_tmp = tempfile::tempdir().expect("fixture tempdir");
            let replica_tmp = tempfile::tempdir().expect("replica tempdir");
            let target_tmp = tempfile::tempdir().expect("target tempdir");
            let fixture_dir = dir_path(&fixture_tmp).to_path_buf();
            let fixture = WalFixture::new(&fixture_dir, PAGE_SIZE).expect("fixture");
            // Start from a clean, fully checkpointed state: the WAL is empty
            // and the database file alone is the snapshot boundary.
            fixture
                .checkpoint(CheckpointMode::Truncate)
                .expect("initial checkpoint");
            let replica_root = dir_path(&replica_tmp).to_path_buf();
            let storage = ReplicaStorage::Local(LocalStorage::new(replica_root.clone()));
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
            let db_path = dir_path(&target_tmp).join("bencher.db");
            Self {
                fixture,
                storage,
                config,
                db_path,
                fixture_dir,
                _fixture_tmp: fixture_tmp,
                _replica_tmp: replica_tmp,
                _target_tmp: target_tmp,
            }
        }

        pub(crate) async fn restore(&self) -> Result<RestoreOutcome, RestoreError> {
            restore_if_missing(&logger(), &self.config, &self.db_path).await
        }

        /// Checkpoint (TRUNCATE) the fixture and copy its database file as a
        /// named boundary state for later equivalence assertions. The
        /// truncation also ends the current WAL salt cycle: the next write
        /// starts a new epoch.
        pub(crate) fn boundary_copy(&self, name: &str) -> Utf8PathBuf {
            self.fixture
                .checkpoint(CheckpointMode::Truncate)
                .expect("boundary checkpoint");
            let path = self.fixture_dir.join(name);
            std::fs::copy(self.fixture.db_path(), &path).expect("boundary copy");
            path
        }

        /// Write raw database bytes as a named boundary state.
        pub(crate) fn write_state(&self, name: &str, db: &[u8]) -> Utf8PathBuf {
            let path = self.fixture_dir.join(name);
            std::fs::write(&path, db).expect("write state");
            path
        }

        /// Run `count` single-insert transactions (one commit frame each).
        pub(crate) fn txns(&self, count: usize, tag: &str) {
            for index in 0..count {
                self.fixture
                    .txn(&[&format!("INSERT INTO t (data) VALUES ('{tag}-{index}')")])
                    .expect("txn");
            }
        }
    }

    pub(crate) fn snapshot_key(generation: &GenerationId) -> String {
        format!("generations/{}/snapshot.db.zst", generation.as_str())
    }

    pub(crate) fn snapshot_meta_key(generation: &GenerationId) -> String {
        format!("generations/{}/snapshot.json", generation.as_str())
    }

    /// Segment keys constructed independently of `bencher_replica`'s own key
    /// builder: a deliberate cross-check of the pinned naming scheme.
    pub(crate) fn segment_key(
        generation: &GenerationId,
        epoch: u64,
        salt: (u32, u32),
        start: u64,
        end: u64,
    ) -> String {
        format!(
            "generations/{}/wal/{epoch:010}-{:08x}{:08x}/{start:020}-{end:020}.wal.zst",
            generation.as_str(),
            salt.0,
            salt.1
        )
    }

    /// Upload a snapshot body plus its `snapshot.json` commit marker, with
    /// `wal_boundary.offset = 0` (no mandatory replay threshold).
    pub(crate) async fn put_snapshot(
        storage: &ReplicaStorage,
        generation: &GenerationId,
        db: &[u8],
        boundary_salt: (u32, u32),
    ) {
        put_snapshot_at(storage, generation, db, boundary_salt, 0).await;
    }

    /// [`put_snapshot`] with an explicit `wal_boundary.offset`: the
    /// mandatory-replay threshold the snapshot body is claimed to capture.
    pub(crate) async fn put_snapshot_at(
        storage: &ReplicaStorage,
        generation: &GenerationId,
        db: &[u8],
        boundary_salt: (u32, u32),
        boundary_offset: u64,
    ) {
        let compressed = compress_segment(db).expect("compress snapshot");
        let sha256 = hex::encode(Sha256::digest(&compressed));
        storage
            .put(&snapshot_key(generation), Bytes::from(compressed))
            .await
            .expect("put snapshot body");
        let meta = SnapshotMeta {
            version: 1,
            created: "2026-07-10T14:59:00Z".to_owned(),
            db_bytes: u64::try_from(db.len()).expect("db size"),
            page_size: PAGE_SIZE,
            sha256,
            wal_boundary: WalBoundary {
                epoch: 0,
                salt1: boundary_salt.0,
                salt2: boundary_salt.1,
                offset: boundary_offset,
            },
        };
        storage
            .put(
                &snapshot_meta_key(generation),
                Bytes::from(meta.to_bytes().expect("snapshot meta bytes")),
            )
            .await
            .expect("put snapshot.json");
    }

    /// The salts and per-commit chunks of a raw WAL byte string (one
    /// [`bencher_replica::CommittedChunk`] per committed transaction), for
    /// tests that fabricate segment objects by hand.
    pub(crate) fn wal_commits(wal: &[u8]) -> ((u32, u32), Vec<bencher_replica::CommittedChunk>) {
        let mut scanner = WalScanner::open(Cursor::new(wal.to_vec()))
            .expect("WAL header")
            .expect("WAL is not empty");
        let salt = scanner.header().salt;
        let mut chunks = Vec::new();
        while let Some(chunk) = scanner.next_committed(1).expect("scan WAL") {
            chunks.push(chunk);
        }
        (salt, chunks)
    }

    /// Materialize the database state at a committed WAL prefix: the
    /// fixture's on-disk database file plus `wal[..end]` recovered by
    /// `SQLite` in a scratch copy. Models what an online backup captured
    /// when the snapshot boundary offset `end` was recorded.
    pub(crate) fn state_at_wal_offset(harness: &Harness, wal: &[u8], end: u64) -> Vec<u8> {
        let db = harness.fixture.db_bytes().expect("db bytes");
        let scratch_tmp = tempfile::tempdir().expect("state scratch tempdir");
        let db_path = dir_path(&scratch_tmp).join("state.db");
        let wal_path = dir_path(&scratch_tmp).join("state.db-wal");
        std::fs::write(&db_path, &db).expect("write scratch db");
        let end = usize::try_from(end).expect("boundary offset fits usize");
        std::fs::write(&wal_path, wal.get(..end).expect("boundary within the WAL"))
            .expect("write scratch wal");
        let conn = rusqlite::Connection::open(&db_path).expect("open scratch db");
        conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_row| Ok(()))
            .expect("checkpoint scratch wal");
        drop(conn);
        std::fs::read(&db_path).expect("read scratch db")
    }

    /// Everything shipped for one epoch: per-segment keys, end offsets, and
    /// running checksums (for building `Position`s).
    pub(crate) struct ShippedEpoch {
        pub salt: (u32, u32),
        pub keys: Vec<String>,
        pub ends: Vec<u64>,
        pub checksums: Vec<(u32, u32)>,
    }

    /// Ship every committed transaction of `wal` as one segment per commit
    /// under `epoch`. The first segment starts at offset 0 and carries the
    /// 32-byte WAL header, so restore can rebuild the `-wal` file by
    /// decompress-and-concatenate.
    pub(crate) async fn ship_epoch(
        storage: &ReplicaStorage,
        generation: &GenerationId,
        epoch: u64,
        wal: &[u8],
    ) -> ShippedEpoch {
        let mut scanner = WalScanner::open(Cursor::new(wal.to_vec()))
            .expect("WAL header")
            .expect("WAL is not empty");
        let salt = scanner.header().salt;
        let mut shipped = ShippedEpoch {
            salt,
            keys: Vec::new(),
            ends: Vec::new(),
            checksums: Vec::new(),
        };
        while let Some(chunk) = scanner.next_committed(1).expect("scan WAL") {
            let (start, raw) = if chunk.start_offset == WAL_HEADER_SIZE {
                let mut with_header = wal[..32].to_vec();
                with_header.extend_from_slice(&chunk.bytes);
                (0u64, with_header)
            } else {
                (chunk.start_offset, chunk.bytes.clone())
            };
            let key = segment_key(generation, epoch, salt, start, chunk.end_offset);
            let compressed = compress_segment(&raw).expect("compress segment");
            storage
                .put(&key, Bytes::from(compressed))
                .await
                .expect("put segment");
            shipped.keys.push(key);
            shipped.ends.push(chunk.end_offset);
            shipped.checksums.push(chunk.checksum_at_end);
        }
        shipped
    }

    /// Flip one byte in the middle of a stored object.
    pub(crate) async fn corrupt_object(storage: &ReplicaStorage, key: &str) {
        let bytes = storage.get(key).await.expect("get object to corrupt");
        let mut bytes = bytes.to_vec();
        // A shift instead of `/ 2` keeps the integer_division lint quiet.
        let middle = bytes.len() >> 1;
        bytes[middle] ^= 0xff;
        storage
            .put(key, Bytes::from(bytes))
            .await
            .expect("put corrupted object");
    }

    /// Tamper a stored segment so the OBJECT stays fully valid (zstd content
    /// checksum recomputed, byte length preserved) while the WAL checksum
    /// chain inside it breaks: decompress, flip a payload byte, recompress.
    /// This models a shipped-then-forked or bit-rotted lineage that
    /// per-object integrity checks cannot see.
    pub(crate) async fn recompress_tampered_segment(storage: &ReplicaStorage, key: &str) {
        let compressed = storage.get(key).await.expect("get segment to tamper");
        let mut raw = decompress_segment(&compressed).expect("decompress segment");
        // Flip one byte in the FIRST frame's page payload (frame header is
        // 24 bytes; non-first segments carry no WAL header).
        raw[24 + 50] ^= 0xff;
        let recompressed = compress_segment(&raw).expect("recompress tampered segment");
        storage
            .put(key, Bytes::from(recompressed))
            .await
            .expect("put tampered segment");
    }
}

#[cfg(test)]
mod cases {
    use bencher_replica::testing::{CheckpointMode, assert_replica_equivalent};
    use bencher_replica::{
        Position, ReplicaMeta, RestoreError, RestoreOutcome, SnapshotMeta, VerifyReport,
        WalBoundary, compress_segment, fingerprint_database, verify_against_replica,
    };
    use bytes::Bytes;
    use camino::Utf8PathBuf;
    use pretty_assertions::assert_eq;
    use sha2::{Digest as _, Sha256};

    use super::harness::{
        Harness, corrupt_object, dir_path, generation, logger, put_snapshot, put_snapshot_at,
        recompress_tampered_segment, segment_key, ship_epoch, snapshot_key, snapshot_meta_key,
        state_at_wal_offset, wal_commits,
    };

    /// Arbitrary boundary salts for snapshot-only replicas (no WAL cycle ever
    /// started after the snapshot).
    const NO_WAL_SALT: (u32, u32) = (0x1111_2222, 0x3333_4444);

    fn assert_restored(outcome: &RestoreOutcome) -> (&bencher_replica::GenerationId, u64, u64) {
        let RestoreOutcome::Restored {
            generation,
            db_bytes,
            segments,
        } = outcome
        else {
            panic!("expected Restored, got {outcome:?}")
        };
        (generation, *db_bytes, *segments)
    }

    #[tokio::test]
    async fn restore_snapshot_only() {
        let harness = Harness::new();
        let boundary = harness.boundary_copy("boundary.db");
        let db = harness.fixture.db_bytes().expect("db bytes");
        put_snapshot(&harness.storage, &generation(1), &db, NO_WAL_SALT).await;

        let outcome = harness.restore().await.expect("restore");
        let (restored_generation, db_bytes, segments) = assert_restored(&outcome);
        assert_eq!(restored_generation, &generation(1), "generation");
        assert_eq!(segments, 0, "snapshot only: no segments replayed");
        assert_eq!(
            db_bytes,
            u64::try_from(db.len()).expect("db size"),
            "restored size equals the uncompressed snapshot"
        );
        assert_replica_equivalent(&boundary, &harness.db_path);
    }

    #[tokio::test]
    async fn restore_snapshot_plus_n_segments() {
        for n in [1usize, 2, 17] {
            let harness = Harness::new();
            let db = harness.fixture.db_bytes().expect("db bytes");
            harness.txns(n, "seg");
            let wal = harness.fixture.wal_bytes().expect("wal bytes");
            let shipped = ship_epoch(&harness.storage, &generation(1), 0, &wal).await;
            assert_eq!(
                shipped.ends.len(),
                n,
                "one segment per committed transaction (n = {n})"
            );
            put_snapshot(&harness.storage, &generation(1), &db, shipped.salt).await;
            let boundary = harness.boundary_copy("boundary.db");

            let outcome = harness.restore().await.expect("restore");
            let (_, _, segments) = assert_restored(&outcome);
            assert_eq!(
                segments,
                u64::try_from(n).expect("segment count"),
                "all {n} segments replayed"
            );
            assert_replica_equivalent(&boundary, &harness.db_path);
        }
    }

    #[tokio::test]
    async fn restore_multi_epoch() {
        let harness = Harness::new();
        let db = harness.fixture.db_bytes().expect("db bytes");
        // Epoch 0: three commits.
        harness.txns(3, "epoch0");
        let wal = harness.fixture.wal_bytes().expect("wal bytes");
        let epoch0 = ship_epoch(&harness.storage, &generation(1), 0, &wal).await;
        put_snapshot(&harness.storage, &generation(1), &db, epoch0.salt).await;
        // The boundary checkpoint truncates the WAL; the next write starts
        // epoch 1 with fresh salts.
        harness.boundary_copy("epoch0.db");
        harness.txns(2, "epoch1");
        let wal = harness.fixture.wal_bytes().expect("wal bytes");
        let epoch1 = ship_epoch(&harness.storage, &generation(1), 1, &wal).await;
        assert_ne!(epoch0.salt, epoch1.salt, "a WAL restart changes the salts");
        let boundary = harness.boundary_copy("epoch1.db");

        let outcome = harness.restore().await.expect("restore");
        let (_, _, segments) = assert_restored(&outcome);
        assert_eq!(segments, 5, "both epochs replayed in full");
        assert_replica_equivalent(&boundary, &harness.db_path);

        let meta = ReplicaMeta::load(&harness.db_path)
            .expect("load meta")
            .expect("meta written");
        assert_eq!(meta.epoch, 1, "meta records the last applied epoch");
        assert_eq!(
            (meta.salt1, meta.salt2),
            epoch1.salt,
            "meta records the last epoch's salts"
        );
        assert_eq!(
            Some(&meta.shipped_offset),
            epoch1.ends.last(),
            "meta records the last applied segment end"
        );
    }

    #[tokio::test]
    async fn restore_picks_latest_generation() {
        let harness = Harness::new();
        harness.boundary_copy("state_a.db");
        let db_a = harness.fixture.db_bytes().expect("db bytes");
        put_snapshot(&harness.storage, &generation(1), &db_a, NO_WAL_SALT).await;

        harness.txns(1, "later");
        let boundary_b = harness.boundary_copy("state_b.db");
        let db_b = harness.fixture.db_bytes().expect("db bytes");
        put_snapshot(&harness.storage, &generation(2), &db_b, NO_WAL_SALT).await;

        let outcome = harness.restore().await.expect("restore");
        let (restored_generation, ..) = assert_restored(&outcome);
        assert_eq!(
            restored_generation,
            &generation(2),
            "the newest complete generation wins"
        );
        assert_replica_equivalent(&boundary_b, &harness.db_path);
    }

    #[tokio::test]
    async fn restore_ignores_generation_without_snapshot_json() {
        let harness = Harness::new();
        let boundary_a = harness.boundary_copy("state_a.db");
        let db_a = harness.fixture.db_bytes().expect("db bytes");
        put_snapshot(&harness.storage, &generation(1), &db_a, NO_WAL_SALT).await;

        // A newer generation with a snapshot BODY but no snapshot.json: a
        // crash mid-snapshot. It must be invisible to restore.
        harness.txns(1, "crashed");
        harness.boundary_copy("state_b.db");
        let db_b = harness.fixture.db_bytes().expect("db bytes");
        let compressed = compress_segment(&db_b).expect("compress");
        harness
            .storage
            .put(&snapshot_key(&generation(2)), Bytes::from(compressed))
            .await
            .expect("put orphan snapshot body");

        let outcome = harness.restore().await.expect("restore");
        let (restored_generation, ..) = assert_restored(&outcome);
        assert_eq!(
            restored_generation,
            &generation(1),
            "a generation without snapshot.json is invisible"
        );
        assert_replica_equivalent(&boundary_a, &harness.db_path);
    }

    #[tokio::test]
    async fn restore_empty_replica_no_replica() {
        let harness = Harness::new();
        // Stale advisory meta from a previous life of this volume.
        stale_meta()
            .store(&harness.db_path)
            .expect("store stale meta");

        let outcome = harness.restore().await.expect("restore");
        assert!(
            matches!(outcome, RestoreOutcome::NoReplica),
            "expected NoReplica, got {outcome:?}"
        );
        assert!(
            !harness.db_path.exists(),
            "no database file is fabricated for a fresh server"
        );
        assert_eq!(
            ReplicaMeta::load(&harness.db_path).expect("load meta"),
            None,
            "stale advisory meta is removed when the database is missing"
        );
    }

    #[tokio::test]
    async fn restore_db_exists_skipped() {
        let harness = Harness::new();
        // A perfectly valid replica exists, so a skip can only come from the
        // database file being present.
        let db = harness.fixture.db_bytes().expect("db bytes");
        put_snapshot(&harness.storage, &generation(1), &db, NO_WAL_SALT).await;
        std::fs::write(&harness.db_path, b"pre-existing database").expect("write db");
        let meta = stale_meta();
        meta.store(&harness.db_path).expect("store meta");

        let outcome = harness.restore().await.expect("restore");
        assert!(
            matches!(outcome, RestoreOutcome::Skipped),
            "expected Skipped, got {outcome:?}"
        );
        assert_eq!(
            std::fs::read(&harness.db_path).expect("read db"),
            b"pre-existing database".to_vec(),
            "an existing database file is untouched"
        );
        assert_eq!(
            ReplicaMeta::load(&harness.db_path).expect("load meta"),
            Some(meta),
            "the meta file is untouched too"
        );
    }

    #[tokio::test]
    async fn restore_corrupted_segment_stops_at_last_good_epoch() {
        let harness = Harness::new();
        let db = harness.fixture.db_bytes().expect("db bytes");
        harness.txns(2, "epoch0");
        let wal = harness.fixture.wal_bytes().expect("wal bytes");
        let epoch0 = ship_epoch(&harness.storage, &generation(1), 0, &wal).await;
        put_snapshot(&harness.storage, &generation(1), &db, epoch0.salt).await;
        let boundary0 = harness.boundary_copy("epoch0.db");
        harness.txns(3, "epoch1");
        let wal = harness.fixture.wal_bytes().expect("wal bytes");
        let epoch1 = ship_epoch(&harness.storage, &generation(1), 1, &wal).await;
        assert_eq!(epoch1.keys.len(), 3, "epoch 1 has three segments");
        // Flip a byte inside the middle stored segment of epoch 1.
        corrupt_object(&harness.storage, &epoch1.keys[1]).await;

        let outcome = harness.restore().await.expect("restore boots anyway");
        let (_, _, segments) = assert_restored(&outcome);
        assert_eq!(
            segments, 2,
            "only epoch 0's segments count; the corrupt epoch is discarded"
        );
        assert_replica_equivalent(&boundary0, &harness.db_path);

        let meta = ReplicaMeta::load(&harness.db_path)
            .expect("load meta")
            .expect("meta written");
        assert_eq!(meta.epoch, 0, "meta stops at the last good epoch");
        assert_eq!(
            Some(&meta.shipped_offset),
            epoch0.ends.last(),
            "meta records epoch 0's end"
        );
    }

    /// A stored segment whose object is valid but decompresses to far more
    /// than its key's declared byte range: restore must reject it via the
    /// exact-size cap (never inflating toward the multi-gigabyte ceiling) and
    /// soft-stop, keeping the last good epoch.
    #[tokio::test]
    async fn restore_oversized_segment_stops_at_last_good_epoch() {
        let harness = Harness::new();
        let db = harness.fixture.db_bytes().expect("db bytes");
        harness.txns(2, "epoch0");
        let wal = harness.fixture.wal_bytes().expect("wal bytes");
        let epoch0 = ship_epoch(&harness.storage, &generation(1), 0, &wal).await;
        put_snapshot(&harness.storage, &generation(1), &db, epoch0.salt).await;
        let boundary0 = harness.boundary_copy("epoch0.db");

        harness.txns(2, "epoch1");
        let wal = harness.fixture.wal_bytes().expect("wal bytes");
        let epoch1 = ship_epoch(&harness.storage, &generation(1), 1, &wal).await;
        assert_eq!(epoch1.keys.len(), 2, "epoch 1 has two segments");
        // Replace the first segment body with a valid object that decompresses
        // to 512 KiB, orders of magnitude beyond its key's declared range.
        let bloated = compress_segment(&vec![0u8; 512 * 1024]).expect("compress bloated segment");
        harness
            .storage
            .put(&epoch1.keys[0], Bytes::from(bloated))
            .await
            .expect("put bloated segment");

        let outcome = harness.restore().await.expect("restore boots anyway");
        let (_, _, segments) = assert_restored(&outcome);
        assert_eq!(
            segments, 2,
            "only epoch 0 applies; the oversized segment is discarded"
        );
        assert_replica_equivalent(&boundary0, &harness.db_path);

        let meta = ReplicaMeta::load(&harness.db_path)
            .expect("load meta")
            .expect("meta written");
        assert_eq!(meta.epoch, 0, "meta stops at the last good epoch");
    }

    /// A tampered segment whose OBJECT is fully valid (recompressed, so the
    /// zstd checksum and the byte length both pass) but whose WAL checksum
    /// chain is broken inside the epoch: the chain pre-validation must
    /// soft-stop BEFORE application, discarding that epoch AND every later
    /// one, instead of letting `SQLite` recovery silently truncate at the
    /// break and replay later epochs on top of the wrong base.
    #[tokio::test]
    async fn restore_chain_break_inside_epoch_stops_before_application() {
        let harness = Harness::new();
        let db = harness.fixture.db_bytes().expect("db bytes");
        harness.txns(2, "epoch0");
        let wal = harness.fixture.wal_bytes().expect("wal bytes");
        let epoch0 = ship_epoch(&harness.storage, &generation(1), 0, &wal).await;
        put_snapshot(&harness.storage, &generation(1), &db, epoch0.salt).await;
        let boundary0 = harness.boundary_copy("epoch0.db");

        // Epoch 1: three segments; break the chain inside the middle one
        // with an object-valid recompression tamper.
        harness.txns(3, "epoch1");
        let wal = harness.fixture.wal_bytes().expect("wal bytes");
        let epoch1 = ship_epoch(&harness.storage, &generation(1), 1, &wal).await;
        assert_eq!(epoch1.keys.len(), 3, "epoch 1 has three segments");
        recompress_tampered_segment(&harness.storage, &epoch1.keys[1]).await;

        // Epoch 2 exists and is intact: it must STILL not apply, because
        // its page images build on epoch 1's lost frames.
        harness.txns(1, "epoch2");
        let wal = harness.fixture.wal_bytes().expect("wal bytes");
        let _epoch2 = ship_epoch(&harness.storage, &generation(1), 2, &wal).await;

        let outcome = harness.restore().await.expect("restore boots anyway");
        let (_, _, segments) = assert_restored(&outcome);
        assert_eq!(
            segments, 2,
            "only epoch 0 applies; the broken epoch and everything after are discarded"
        );
        assert_replica_equivalent(&boundary0, &harness.db_path);

        let meta = ReplicaMeta::load(&harness.db_path)
            .expect("load meta")
            .expect("meta written");
        assert_eq!(meta.epoch, 0, "meta stops at the last good epoch");
    }

    /// A single generation that holds the SAME epoch number twice under
    /// different salts: two epoch-1 directories, each a complete, well-formed
    /// WAL lineage. This is the salt-collision state a restore must never be
    /// poisoned by. `plan_epochs` sorts by `(epoch, start, end)` with salt
    /// EXCLUDED, so the two lineages collapse into one epoch-1 group and the
    /// salt collision discards the epoch wholesale. Restore soft-stops at
    /// epoch 0: it boots on the last unambiguous state, never a torn mixture of
    /// the two lineages and never silently picking one. This is safe but
    /// pessimistic (a complete valid lineage is dropped), which is why the
    /// engine's resume path is designed never to CREATE the collision (see
    /// `resume_after_soft_stop_below_corrupt_epoch_forces_new_generation` in
    /// `tests/sync_engine.rs`, and the `meta_matches` note in `src/sync.rs`).
    #[tokio::test]
    async fn restore_duplicate_epoch_salt_collision_soft_stops_at_last_unambiguous_epoch() {
        let harness = Harness::new();
        let db = harness.fixture.db_bytes().expect("db bytes");
        // Epoch 0: two commits, shipped, plus the snapshot boundary.
        harness.txns(2, "epoch0");
        let wal0 = harness.fixture.wal_bytes().expect("wal bytes");
        let epoch0 = ship_epoch(&harness.storage, &generation(1), 0, &wal0).await;
        put_snapshot(&harness.storage, &generation(1), &db, epoch0.salt).await;
        let boundary0 = harness.boundary_copy("epoch0.db");

        // Epoch 1, first lineage: a real WAL cycle shipped under epoch 1.
        harness.txns(2, "epoch1-first");
        let wal_first = harness.fixture.wal_bytes().expect("wal bytes");
        let first_lineage = ship_epoch(&harness.storage, &generation(1), 1, &wal_first).await;
        // Restart the WAL (fresh salts) and ship a SECOND, distinct lineage
        // that also claims epoch 1: the same epoch number, different salts.
        harness
            .fixture
            .checkpoint(CheckpointMode::Truncate)
            .expect("truncate to restart the WAL cycle");
        harness.txns(2, "epoch1-second");
        let wal_second = harness.fixture.wal_bytes().expect("wal bytes");
        let second_lineage = ship_epoch(&harness.storage, &generation(1), 1, &wal_second).await;
        assert_ne!(
            first_lineage.salt, second_lineage.salt,
            "the two epoch-1 lineages carry distinct salts (a real directory collision)"
        );

        let outcome = harness.restore().await.expect("restore boots anyway");
        let (_, _, segments) = assert_restored(&outcome);
        assert_eq!(
            segments, 2,
            "only epoch 0 applies; the duplicate-epoch collision discards both epoch-1 lineages"
        );
        assert_replica_equivalent(&boundary0, &harness.db_path);

        let meta = ReplicaMeta::load(&harness.db_path)
            .expect("load meta")
            .expect("meta written");
        assert_eq!(meta.epoch, 0, "meta stops at the last unambiguous epoch");
        assert_eq!(
            Some(&meta.shipped_offset),
            epoch0.ends.last(),
            "meta records epoch 0's end"
        );
    }

    /// Regression (review B1): a segment PUT that persisted server-side but
    /// surfaced as an error, retried after more commits landed, leaves two
    /// segments sharing a start: `[X, Y)` and `[X, Z)` with `Z > Y`. Both
    /// were scanned from the same verified `(offset, checksum)` seed over the
    /// same epoch bytes, so the shorter is a byte prefix of the longer:
    /// restore keeps only the largest end and replays the full epoch instead
    /// of discarding every byte of it as a broken chain.
    #[tokio::test]
    async fn restore_duplicate_start_segments_keep_largest_end() {
        let harness = Harness::new();
        let db = harness.fixture.db_bytes().expect("db bytes");
        harness.txns(3, "dup");
        let wal = harness.fixture.wal_bytes().expect("wal bytes");
        let (salt, commits) = wal_commits(&wal);
        let [c1, c2, c3] = commits.as_slice() else {
            panic!("expected three commits, got {}", commits.len())
        };
        // The clean leading segment [0, e1), carrying the WAL header.
        let mut leading = wal[..32].to_vec();
        leading.extend_from_slice(&c1.bytes);
        let generation = generation(1);
        harness
            .storage
            .put(
                &segment_key(&generation, 0, salt, 0, c1.end_offset),
                Bytes::from(compress_segment(&leading).expect("compress leading segment")),
            )
            .await
            .expect("put leading segment");
        // The orphaned short ship [e1, e2): persisted server-side, reported
        // as failed.
        harness
            .storage
            .put(
                &segment_key(&generation, 0, salt, c1.end_offset, c2.end_offset),
                Bytes::from(compress_segment(&c2.bytes).expect("compress orphan segment")),
            )
            .await
            .expect("put orphan segment");
        // The retried ship [e1, e3): scanned to the fresh extent, so it
        // covers the third commit too; its prefix is the orphan.
        let mut retried = c2.bytes.clone();
        retried.extend_from_slice(&c3.bytes);
        harness
            .storage
            .put(
                &segment_key(&generation, 0, salt, c1.end_offset, c3.end_offset),
                Bytes::from(compress_segment(&retried).expect("compress retried segment")),
            )
            .await
            .expect("put retried segment");
        put_snapshot(&harness.storage, &generation, &db, salt).await;
        let boundary = harness.boundary_copy("final.db");

        let outcome = harness.restore().await.expect("restore");
        let (_, _, segments) = assert_restored(&outcome);
        assert_eq!(
            segments, 2,
            "the deduped epoch replays as two segments: [0, e1) and [e1, e3)"
        );
        assert_replica_equivalent(&boundary, &harness.db_path);

        let meta = ReplicaMeta::load(&harness.db_path)
            .expect("load meta")
            .expect("meta written");
        assert_eq!(meta.epoch, 0, "the epoch fully applies");
        assert_eq!(
            meta.shipped_offset, c3.end_offset,
            "meta records the largest end, not the orphan's"
        );
    }

    /// The snapshot's `wal_boundary.offset` is the mandatory-replay
    /// threshold: boundary-epoch segments that reach it replay in full, even
    /// when the snapshot body was captured mid-epoch (frames below the
    /// threshold re-apply idempotently).
    #[tokio::test]
    async fn restore_boundary_epoch_reaching_mandatory_offset_replays() {
        let harness = Harness::new();
        harness.txns(4, "seg");
        let wal = harness.fixture.wal_bytes().expect("wal bytes");
        let shipped = ship_epoch(&harness.storage, &generation(1), 0, &wal).await;
        assert_eq!(shipped.ends.len(), 4, "four commits shipped");
        // The snapshot body captured the state at commit 2 (a mid-epoch
        // online backup), so its boundary offset is commit 2's end.
        let mid = state_at_wal_offset(&harness, &wal, shipped.ends[1]);
        put_snapshot_at(
            &harness.storage,
            &generation(1),
            &mid,
            shipped.salt,
            shipped.ends[1],
        )
        .await;
        let boundary = harness.boundary_copy("final.db");

        let outcome = harness.restore().await.expect("restore");
        let (_, _, segments) = assert_restored(&outcome);
        assert_eq!(
            segments, 4,
            "segments reach the mandatory offset, so the whole epoch replays"
        );
        assert_replica_equivalent(&boundary, &harness.db_path);
    }

    /// Boundary-epoch segments ending BELOW the snapshot's mandatory-replay
    /// offset: replaying only that prefix could regress pages the snapshot
    /// body holds at a newer state, so restore falls back to the snapshot
    /// alone (a consistent committed state by itself).
    #[tokio::test]
    async fn restore_boundary_epoch_below_mandatory_offset_restores_snapshot_alone() {
        let harness = Harness::new();
        harness.txns(4, "seg");
        let wal = harness.fixture.wal_bytes().expect("wal bytes");
        let shipped = ship_epoch(&harness.storage, &generation(1), 0, &wal).await;
        assert_eq!(shipped.ends.len(), 4, "four commits shipped");
        // The snapshot body captured the state at commit 3; only the first
        // two segments survive on the replica (the tail was lost), so the
        // available extent falls short of the mandatory offset.
        let mid = state_at_wal_offset(&harness, &wal, shipped.ends[2]);
        let mid_state = harness.write_state("mid.db", &mid);
        put_snapshot_at(
            &harness.storage,
            &generation(1),
            &mid,
            shipped.salt,
            shipped.ends[2],
        )
        .await;
        for key in &shipped.keys[2..] {
            harness
                .storage
                .delete(key)
                .await
                .expect("delete tail segment");
        }

        let outcome = harness.restore().await.expect("restore boots anyway");
        let (_, _, segments) = assert_restored(&outcome);
        assert_eq!(
            segments, 0,
            "a boundary epoch short of the mandatory offset is not replayed at all"
        );
        assert_replica_equivalent(&mid_state, &harness.db_path);

        let meta = ReplicaMeta::load(&harness.db_path)
            .expect("load meta")
            .expect("meta written");
        assert_eq!(meta.epoch, 0, "meta sits at the snapshot boundary");
        assert_eq!(meta.shipped_offset, 0, "nothing was replayed");
    }

    /// A rebound epoch 0 (salts differ from the snapshot boundary's): the
    /// mandatory-replay threshold was measured in the OLD salt cycle, which
    /// was fully backfilled into the backup before the WAL restarted, so no
    /// frame of the new cycle is mandatory and epoch 0 replays even though
    /// its segments end below the recorded offset.
    #[tokio::test]
    async fn restore_rebound_epoch0_ignores_stale_mandatory_offset() {
        let harness = Harness::new();
        harness.txns(3, "old");
        let old_wal = harness.fixture.wal_bytes().expect("old wal bytes");
        let (old_salt, old_commits) = wal_commits(&old_wal);
        let old_extent = old_commits.last().expect("old commits").end_offset;
        // The old cycle is fully backfilled before the snapshot: its state is
        // in the body, and the truncation restarts the WAL with fresh salts.
        harness
            .fixture
            .checkpoint(CheckpointMode::Truncate)
            .expect("backfill the old cycle");
        let db_after_old = harness.fixture.db_bytes().expect("db bytes");
        harness.txns(2, "new");
        let new_wal = harness.fixture.wal_bytes().expect("new wal bytes");
        let shipped = ship_epoch(&harness.storage, &generation(1), 0, &new_wal).await;
        assert_ne!(shipped.salt, old_salt, "the rebound cycle has fresh salts");
        let available = *shipped.ends.last().expect("new segments shipped");
        assert!(
            available < old_extent,
            "the stale threshold ({old_extent}) exceeds the new cycle's extent ({available})"
        );
        put_snapshot_at(
            &harness.storage,
            &generation(1),
            &db_after_old,
            old_salt,
            old_extent,
        )
        .await;
        let boundary = harness.boundary_copy("final.db");

        let outcome = harness.restore().await.expect("restore");
        let (_, _, segments) = assert_restored(&outcome);
        assert_eq!(
            segments, 2,
            "the salt mismatch proves the threshold is stale: epoch 0 replays"
        );
        assert_replica_equivalent(&boundary, &harness.db_path);
    }

    #[tokio::test]
    async fn restore_missing_middle_segment_stops() {
        let harness = Harness::new();
        let db = harness.fixture.db_bytes().expect("db bytes");
        let boundary = harness.write_state("snapshot-boundary.db", &db);
        harness.txns(3, "epoch0");
        let wal = harness.fixture.wal_bytes().expect("wal bytes");
        let epoch0 = ship_epoch(&harness.storage, &generation(1), 0, &wal).await;
        assert_eq!(epoch0.keys.len(), 3, "epoch 0 has three segments");
        put_snapshot(&harness.storage, &generation(1), &db, epoch0.salt).await;
        // Delete segment 2 of 3: the offset chain has a hole.
        harness
            .storage
            .delete(&epoch0.keys[1])
            .await
            .expect("delete middle segment");

        let outcome = harness.restore().await.expect("restore boots anyway");
        let (_, _, segments) = assert_restored(&outcome);
        assert_eq!(
            segments, 0,
            "an epoch with a segment gap is discarded entirely"
        );
        assert_replica_equivalent(&boundary, &harness.db_path);

        let meta = ReplicaMeta::load(&harness.db_path)
            .expect("load meta")
            .expect("meta written");
        assert_eq!(meta.epoch, 0, "meta falls back to the snapshot boundary");
        assert_eq!(meta.shipped_offset, 0, "nothing shipped in the epoch");
        assert_eq!(
            (meta.salt1, meta.salt2),
            epoch0.salt,
            "boundary salts from snapshot.json"
        );
    }

    #[tokio::test]
    async fn restore_corrupted_snapshot_sha256_hard_error() {
        // Tampered recorded hash: the download succeeds but the checksum
        // comparison must hard-fail and leave no files behind.
        let harness = Harness::new();
        let db = harness.fixture.db_bytes().expect("db bytes");
        put_snapshot(&harness.storage, &generation(1), &db, NO_WAL_SALT).await;
        let meta_key = snapshot_meta_key(&generation(1));
        let meta_bytes = harness.storage.get(&meta_key).await.expect("get meta");
        let mut snapshot_meta = SnapshotMeta::from_bytes(&meta_bytes).expect("parse meta");
        snapshot_meta.sha256 = "00".repeat(32);
        harness
            .storage
            .put(
                &meta_key,
                Bytes::from(snapshot_meta.to_bytes().expect("meta bytes")),
            )
            .await
            .expect("put tampered meta");

        let error = harness.restore().await.expect_err("checksum must fail");
        assert!(
            matches!(error, RestoreError::SnapshotChecksum { .. }),
            "expected SnapshotChecksum, got {error}"
        );
        assert_no_db_left_behind(&harness);

        // A corrupted snapshot BODY is also a hard error (zstd decode or
        // checksum), never a half-restored database.
        let harness = Harness::new();
        let db = harness.fixture.db_bytes().expect("db bytes");
        put_snapshot(&harness.storage, &generation(1), &db, NO_WAL_SALT).await;
        corrupt_object(&harness.storage, &snapshot_key(&generation(1))).await;
        harness
            .restore()
            .await
            .expect_err("corrupt snapshot body must fail");
        assert_no_db_left_behind(&harness);
    }

    #[tokio::test]
    async fn restore_snapshot_body_larger_than_db_bytes_fails() {
        // A body that decompresses to far more than the recorded db_bytes:
        // the streamed restore must fail on the size cap, before the checksum
        // verdict, without writing unboundedly.
        let harness = Harness::new();
        let oversized = vec![0u8; 256 * 1024];
        let compressed = compress_segment(&oversized).expect("compress oversized body");
        let sha256 = hex::encode(Sha256::digest(&compressed));
        harness
            .storage
            .put(&snapshot_key(&generation(1)), Bytes::from(compressed))
            .await
            .expect("put oversized snapshot body");
        let meta = SnapshotMeta {
            version: 1,
            created: "2026-07-10T14:59:00Z".to_owned(),
            db_bytes: 4096,
            page_size: 4096,
            sha256,
            wal_boundary: WalBoundary {
                epoch: 0,
                salt1: NO_WAL_SALT.0,
                salt2: NO_WAL_SALT.1,
                offset: 0,
            },
        };
        harness
            .storage
            .put(
                &snapshot_meta_key(&generation(1)),
                Bytes::from(meta.to_bytes().expect("meta bytes")),
            )
            .await
            .expect("put snapshot.json");

        let error = harness
            .restore()
            .await
            .expect_err("oversize snapshot body must fail");
        assert!(
            matches!(error, RestoreError::SnapshotTooLarge { .. }),
            "expected SnapshotTooLarge, got {error}"
        );
        assert_no_db_left_behind(&harness);
    }

    #[tokio::test]
    async fn restore_falls_through_to_older_generation_when_newest_body_missing() {
        // The newest generation has a valid snapshot.json marker but no body
        // (the partial-prune state): restore must fall through to the intact
        // older generation instead of refusing to boot.
        let harness = Harness::new();
        let boundary_old = harness.boundary_copy("old.db");
        let db_old = harness.fixture.db_bytes().expect("db bytes");
        put_snapshot(&harness.storage, &generation(1), &db_old, NO_WAL_SALT).await;

        // Newer generation: upload ONLY snapshot.json (no snapshot.db.zst).
        // The recorded sha256 is never consulted because the body is missing.
        let marker = SnapshotMeta {
            version: 1,
            created: "2026-07-10T14:59:00Z".to_owned(),
            db_bytes: 4096,
            page_size: 4096,
            sha256: "00".repeat(32),
            wal_boundary: WalBoundary {
                epoch: 0,
                salt1: NO_WAL_SALT.0,
                salt2: NO_WAL_SALT.1,
                offset: 0,
            },
        };
        harness
            .storage
            .put(
                &snapshot_meta_key(&generation(2)),
                Bytes::from(marker.to_bytes().expect("marker bytes")),
            )
            .await
            .expect("put newer snapshot.json marker");

        let outcome = harness
            .restore()
            .await
            .expect("restore succeeds from the older generation");
        let (restored_generation, ..) = assert_restored(&outcome);
        assert_eq!(
            restored_generation,
            &generation(1),
            "falls through to the intact older generation"
        );
        assert_replica_equivalent(&boundary_old, &harness.db_path);
    }

    #[tokio::test]
    async fn restore_hard_fails_on_corrupt_newest_body_without_falling_through() {
        // A newest generation whose body is PRESENT but whose recorded sha256
        // is wrong: unlike a missing body, this is corruption and must HARD
        // fail, never silently regress to the older generation.
        let harness = Harness::new();
        harness.boundary_copy("old.db");
        let db_old = harness.fixture.db_bytes().expect("db bytes");
        put_snapshot(&harness.storage, &generation(1), &db_old, NO_WAL_SALT).await;

        put_snapshot(&harness.storage, &generation(2), &db_old, NO_WAL_SALT).await;
        let meta_key = snapshot_meta_key(&generation(2));
        let meta_bytes = harness.storage.get(&meta_key).await.expect("get meta");
        let mut snapshot_meta = SnapshotMeta::from_bytes(&meta_bytes).expect("parse meta");
        snapshot_meta.sha256 = "00".repeat(32);
        harness
            .storage
            .put(
                &meta_key,
                Bytes::from(snapshot_meta.to_bytes().expect("meta bytes")),
            )
            .await
            .expect("put tampered meta");

        let error = harness
            .restore()
            .await
            .expect_err("a corrupt newest body must hard-fail, not fall through");
        assert!(
            matches!(error, RestoreError::SnapshotChecksum { .. }),
            "expected SnapshotChecksum, got {error}"
        );
        assert_no_db_left_behind(&harness);
    }

    #[tokio::test]
    async fn restore_writes_fresh_meta() {
        // Snapshot only: the meta sits at the wal_boundary with offset 0.
        let harness = Harness::new();
        let db = harness.fixture.db_bytes().expect("db bytes");
        put_snapshot(&harness.storage, &generation(1), &db, NO_WAL_SALT).await;
        harness.restore().await.expect("restore");
        let meta = ReplicaMeta::load(&harness.db_path)
            .expect("load meta")
            .expect("meta written");
        assert_eq!(
            meta,
            ReplicaMeta {
                version: 1,
                generation: generation(1).as_str().to_owned(),
                epoch: 0,
                salt1: NO_WAL_SALT.0,
                salt2: NO_WAL_SALT.1,
                shipped_offset: 0,
                epoch_shipped_through_checkpoint: true,
                shadow: false,
            },
            "snapshot-only restore writes a boundary meta"
        );

        // Snapshot plus segments: the meta records the applied end position.
        let harness = Harness::new();
        let db = harness.fixture.db_bytes().expect("db bytes");
        harness.txns(2, "meta");
        let wal = harness.fixture.wal_bytes().expect("wal bytes");
        let shipped = ship_epoch(&harness.storage, &generation(1), 0, &wal).await;
        put_snapshot(&harness.storage, &generation(1), &db, shipped.salt).await;
        harness.restore().await.expect("restore");
        let meta = ReplicaMeta::load(&harness.db_path)
            .expect("load meta")
            .expect("meta written");
        assert_eq!(
            meta,
            ReplicaMeta {
                version: 1,
                generation: generation(1).as_str().to_owned(),
                epoch: 0,
                salt1: shipped.salt.0,
                salt2: shipped.salt.1,
                shipped_offset: *shipped.ends.last().expect("segments shipped"),
                epoch_shipped_through_checkpoint: true,
                shadow: false,
            },
            "restore writes the applied position with checkpoint-verified epoch"
        );
    }

    #[tokio::test]
    async fn restore_to_position_replays_prefix_only() {
        let harness = Harness::new();
        let db = harness.fixture.db_bytes().expect("db bytes");
        harness.txns(2, "at-p");
        // Fingerprint the source at position P (after two commits).
        let source_conn =
            rusqlite::Connection::open(harness.fixture.db_path()).expect("open source");
        let fingerprint_p = fingerprint_database(&source_conn).expect("fingerprint at P");
        // One more commit past P.
        harness.txns(1, "after-p");
        let fingerprint_after = fingerprint_database(&source_conn).expect("fingerprint after P");
        assert_ne!(
            fingerprint_p, fingerprint_after,
            "the fingerprints must differ across the extra commit"
        );
        let wal = harness.fixture.wal_bytes().expect("wal bytes");
        let shipped = ship_epoch(&harness.storage, &generation(1), 0, &wal).await;
        assert_eq!(shipped.ends.len(), 3, "three commits shipped");
        put_snapshot(&harness.storage, &generation(1), &db, shipped.salt).await;

        let position = Position {
            generation: generation(1),
            epoch: 0,
            salt: shipped.salt,
            offset: shipped.ends[1],
            checksum: shipped.checksums[1],
        };
        let scratch_pass = tempfile::tempdir().expect("scratch tempdir");
        let report = verify_against_replica(
            &logger(),
            &harness.storage,
            &position,
            &fingerprint_p,
            dir_path(&scratch_pass),
        )
        .await
        .expect("verify at P");
        assert_eq!(
            report,
            VerifyReport::Pass,
            "the replica restored to P matches the source fingerprint at P"
        );

        let scratch_fail = tempfile::tempdir().expect("scratch tempdir");
        let report = verify_against_replica(
            &logger(),
            &harness.storage,
            &position,
            &fingerprint_after,
            dir_path(&scratch_fail),
        )
        .await
        .expect("verify after P");
        let VerifyReport::Fail { detail } = report else {
            panic!("a fingerprint taken past P must not verify at P, got {report:?}")
        };
        assert!(
            detail.contains("table=t"),
            "the failure names the differing table line: {detail}"
        );
    }

    fn stale_meta() -> ReplicaMeta {
        ReplicaMeta {
            version: 1,
            generation: generation(9).as_str().to_owned(),
            epoch: 7,
            salt1: 0xdead_beef,
            salt2: 0xcafe_f00d,
            shipped_offset: 4128,
            epoch_shipped_through_checkpoint: false,
            shadow: false,
        }
    }

    fn assert_no_db_left_behind(harness: &Harness) {
        assert!(
            !harness.db_path.exists(),
            "no database file after a hard restore error"
        );
        let scratch = Utf8PathBuf::from(format!("{}.restore", harness.db_path));
        assert!(
            !scratch.exists(),
            "the partial .restore scratch file is deleted"
        );
        assert!(
            !Utf8PathBuf::from(format!("{scratch}-wal")).exists(),
            "no scratch -wal left behind"
        );
    }
}
