//! Lock-free, incremental generation snapshots.
//!
//! The snapshot body comes from a SINGLE-STEP `SQLite` online backup into a
//! local scratch file: one `sqlite3_backup_step(-1)` call copies the entire
//! committed state through the pager under one read transaction, so the
//! result is transactionally consistent NO MATTER WHO checkpoints
//! concurrently (a raw file copy would be torn by any external backfill,
//! which matters in shadow mode where Litestream checkpoints freely, and
//! whenever an operator tool touches the database). Writers are never
//! blocked: the backup holds only a read transaction, and a single step
//! never observes writer restarts.
//!
//! The upload is step-driven: each [`copy_step`] call processes at most the
//! throttle budget (`snapshot_throttle_mib * sync_interval`) of the SCRATCH
//! file in 1 MiB reads through a streaming zstd encoder into a multipart
//! upload, so the engine stays responsive between steps. The state machine
//! itself ([`SnapshotJob`]) is advanced by `SyncEngine::snapshot_step`:
//!
//! `ShipTail` (drain committed frames so the OLD generation is complete)
//! -> `CreateGeneration` (new id, online backup into the scratch file,
//! record boundary salts + the mandatory-replay offset, open the multipart
//! upload) -> `Copying` (budgeted steps over the scratch) -> `Finalize`
//! (finish the upload, re-check the boundary salts, PUT `snapshot.json`
//! LAST as the atomic commit marker, reset the position to the new
//! generation's epoch 0, delete the scratch, prune).
//!
//! ## The epoch-0 lazy binding rule
//!
//! The new generation's epoch 0 must bind to the salt cycle from which the
//! first post-snapshot ship actually happens. If the WAL restarts before
//! any epoch-0 segment ships, that is only possible when the old cycle was
//! fully backfilled and hence fully contained in the snapshot just taken,
//! so epoch 0 is REBOUND to the new cycle instead of leaving an empty
//! epoch. This preserves the restore invariant that epochs `0..N` are
//! contiguous and non-empty. Concretely: (a) boundary salts are recorded at
//! `CreateGeneration`; (b) the body is copied and uploaded; (c) BEFORE
//! `snapshot.json` is uploaded the WAL header is re-read and, since nothing
//! has shipped into the new generation yet by construction, a changed salt
//! rebinds the boundary; (d) `snapshot.json` is uploaded LAST. Any restart
//! AFTER that is handled by the ship path's offset-0 rebind, which keeps
//! the position's epoch number and adopts the new salts.

use std::fs::File;
use std::io::{ErrorKind, Read as _, Seek as _, SeekFrom, Write as _};

use camino::{Utf8Path, Utf8PathBuf};
use sha2::Sha256;

use crate::position::GenerationId;
use crate::segment::ZSTD_LEVEL;
use crate::snapshot_meta::SnapshotMetaError;
use crate::storage::MultipartUpload;

/// One mebibyte: the copy read granularity.
pub(crate) const MIB: u64 = 1024 * 1024;

/// Incomplete generations (no `snapshot.json`) older than this are crashed
/// snapshots and get pruned.
pub(crate) const STALE_INCOMPLETE_SECS: i64 = 86_400;

/// The streaming snapshot encoder: compressed output accumulates in the
/// inner buffer and is drained into multipart parts after each step.
pub(crate) type SnapshotEncoder = zstd::stream::Encoder<'static, Vec<u8>>;

/// The status `SyncEngine::snapshot_step` reports to its driver.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotStatus {
    /// More steps remain; call again.
    InProgress,
    /// `snapshot.json` is uploaded and the position now points at the new
    /// generation's epoch 0.
    Finished,
}

#[derive(Debug, thiserror::Error)]
pub enum SnapshotError {
    #[error("Failed to back up the database into the snapshot scratch ({path}): {error}")]
    Backup {
        path: Utf8PathBuf,
        error: rusqlite::Error,
    },
    #[error("Failed to prepare the snapshot scratch file ({path}): {error}")]
    Scratch {
        path: Utf8PathBuf,
        error: std::io::Error,
    },
    #[error("Failed to read the snapshot scratch file ({path}): {error}")]
    DbRead {
        path: Utf8PathBuf,
        error: std::io::Error,
    },
    #[error("Failed to compress the snapshot stream: {0}")]
    Compress(std::io::Error),
    #[error("Snapshot backup did not complete in a single step: {outcome:?}")]
    BackupIncomplete {
        outcome: rusqlite::backup::StepResult,
    },
    #[error("Snapshot meta: {0}")]
    Meta(#[from] SnapshotMetaError),
}

/// The snapshot state machine, held by the engine between steps.
pub(crate) enum SnapshotJob {
    /// Drain committed WAL frames into the old generation first, so it
    /// remains a complete restore point under retention.
    ShipTail,
    /// Fix the new generation id, run the online backup into the scratch
    /// file, record the boundary; open the multipart upload.
    CreateGeneration,
    /// Budgeted upload steps over the scratch file through the zstd encoder.
    Copying(Box<CopyJob>),
    /// Finish the upload, verify the boundary, PUT `snapshot.json` LAST.
    Finalize(Box<FinalizeJob>),
}

/// In-flight copy state.
pub(crate) struct CopyJob {
    pub generation: GenerationId,
    /// WAL salts recorded at `CreateGeneration` (may rebind at finalize).
    pub boundary_salt: (u32, u32),
    /// The mandatory-replay threshold: the committed WAL extent read AFTER
    /// the backup. It is always at least as large as the extent the backup
    /// body captured (a frame committed after the backup only OVERSTATES it,
    /// the safe direction), and restore requires the boundary epoch's
    /// segments to reach at least this far before replaying it. Overstating
    /// merely makes restore fall back to the snapshot alone; understating
    /// (a value below the backup extent) could let a prefix replay that
    /// regresses pages the snapshot already holds, so the read must never
    /// move before the backup.
    pub boundary_offset: u64,
    pub upload: MultipartUpload,
    /// SHA-256 over the COMPRESSED stream, part by part.
    pub hasher: Sha256,
    pub encoder: SnapshotEncoder,
    /// The backup scratch file being uploaded.
    pub scratch: Utf8PathBuf,
    /// Scratch file length in bytes (the uncompressed snapshot size).
    pub db_len: u64,
    /// Next unuploaded byte offset in the scratch file.
    pub offset: u64,
    /// Database page size (informational, stored in `snapshot.json`).
    pub page_size: u32,
    /// LIVE database fingerprint right after the backup; re-checked at
    /// finalize to detect external checkpoints mid-snapshot (sole mode).
    pub live_fingerprint: (u64, [u8; 4]),
}

/// Everything needed to commit the generation after the copy.
pub(crate) struct FinalizeJob {
    pub generation: GenerationId,
    pub boundary_salt: (u32, u32),
    pub boundary_offset: u64,
    pub upload: MultipartUpload,
    /// Hex SHA-256 of the compressed snapshot object.
    pub sha256: String,
    /// Uncompressed database size in bytes.
    pub db_bytes: u64,
    pub page_size: u32,
    /// The scratch file to delete once the generation commits.
    pub scratch: Utf8PathBuf,
    /// LIVE database fingerprint right after the backup.
    pub live_fingerprint: (u64, [u8; 4]),
}

/// The result of one budgeted copy step.
pub(crate) enum CopyStepResult {
    /// More database bytes remain.
    Continue {
        encoder: SnapshotEncoder,
        hasher: Sha256,
        /// Compressed bytes drained this step (may be empty when zstd is
        /// still buffering).
        compressed: Vec<u8>,
        /// Next uncopied byte offset.
        offset: u64,
    },
    /// The whole file was copied and the zstd frame is finished.
    Done {
        hasher: Sha256,
        /// The final compressed tail (frame epilogue included).
        compressed: Vec<u8>,
    },
}

/// A fresh streaming encoder for one snapshot body (content checksum on, so
/// a corrupted object fails loudly at restore).
pub(crate) fn new_snapshot_encoder() -> Result<SnapshotEncoder, SnapshotError> {
    let mut encoder =
        zstd::stream::Encoder::new(Vec::new(), ZSTD_LEVEL).map_err(SnapshotError::Compress)?;
    encoder
        .include_checksum(true)
        .map_err(SnapshotError::Compress)?;
    Ok(encoder)
}

/// Run a SINGLE-STEP online backup of the database into `scratch`: one
/// `sqlite3_backup_step(-1)` copies the whole committed state through the
/// pager under one read transaction. Writers proceed concurrently (a read
/// lock only), no restart can occur within the single step, and the result
/// is transactionally consistent regardless of concurrent checkpoints.
/// Runs inside `spawn_blocking`.
pub(crate) fn backup_to_scratch(
    db_path: &Utf8Path,
    scratch: &Utf8Path,
) -> Result<(), SnapshotError> {
    // A stale scratch from a crashed snapshot must not leak into the copy.
    match std::fs::remove_file(scratch) {
        Ok(()) => {},
        Err(error) if error.kind() == ErrorKind::NotFound => {},
        Err(error) => {
            return Err(SnapshotError::Scratch {
                path: scratch.to_owned(),
                error,
            });
        },
    }
    let backup_err = |error| SnapshotError::Backup {
        path: scratch.to_owned(),
        error,
    };
    let src = rusqlite::Connection::open(db_path).map_err(backup_err)?;
    // The backup source is a reader, but invariant I2 applies to every
    // connection defensively.
    let _pages: i64 = src
        .query_row("PRAGMA wal_autocheckpoint = 0", [], |row| row.get(0))
        .map_err(backup_err)?;
    let mut dst = rusqlite::Connection::open(scratch).map_err(backup_err)?;
    let backup = rusqlite::backup::Backup::new(&src, &mut dst).map_err(backup_err)?;
    let state = backup.step(-1).map_err(backup_err)?;
    // A single `step(-1)` copies the whole database in one pass, so anything
    // but `Done` (legitimately `Busy`/`Locked`) is surfaced honestly rather
    // than disguised as a rusqlite row-count error.
    require_backup_done(state)?;
    drop(backup);
    drop(dst);
    Ok(())
}

/// A single-step online backup either finishes (`Done`) or is reporting a
/// live contention outcome (`Busy`/`Locked`, or a `More` that a one-pass
/// step should never return). The non-`Done` cases become
/// [`SnapshotError::BackupIncomplete`].
fn require_backup_done(outcome: rusqlite::backup::StepResult) -> Result<(), SnapshotError> {
    use rusqlite::backup::StepResult;

    match outcome {
        StepResult::Done => Ok(()),
        // `StepResult` is `#[non_exhaustive]`, so the trailing `_` is required
        // even with every present variant listed.
        StepResult::More | StepResult::Busy | StepResult::Locked | _ => {
            Err(SnapshotError::BackupIncomplete { outcome })
        },
    }
}

/// A cheap fingerprint of the LIVE database file (length plus header change
/// counter, bytes 24..28). In sole mode nothing legitimate mutates the file
/// during a snapshot (the engine is sequential and is the only
/// checkpointer), so any change between backup and finalize proves an
/// external checkpointer that may have buried post-backup frames: the
/// finalize step then schedules a follow-up generation to recapture them.
pub(crate) fn live_db_fingerprint(db_path: &Utf8Path) -> Result<(u64, [u8; 4]), SnapshotError> {
    let read_err = |error| SnapshotError::DbRead {
        path: db_path.to_owned(),
        error,
    };
    let mut file = File::open(db_path).map_err(read_err)?;
    let len = file.metadata().map_err(read_err)?.len();
    let mut header = [0u8; 28];
    let mut change_counter = [0u8; 4];
    match file.read_exact(&mut header) {
        Ok(()) => {
            if let Some(counter) = header.get(24..28) {
                change_counter.copy_from_slice(counter);
            }
        },
        Err(error) if error.kind() == ErrorKind::UnexpectedEof => {},
        Err(error) => return Err(read_err(error)),
    }
    Ok((len, change_counter))
}

/// Read the scratch file length plus the database page size (header bytes
/// 16..18 big-endian; the stored value 1 encodes 65536). A file shorter
/// than the 100-byte header yields a zero page size; the length still
/// bounds the upload.
pub(crate) fn read_scratch_info(scratch: &Utf8Path) -> Result<(u64, u32), SnapshotError> {
    let read_err = |error| SnapshotError::DbRead {
        path: scratch.to_owned(),
        error,
    };
    let mut file = File::open(scratch).map_err(read_err)?;
    let db_len = file.metadata().map_err(read_err)?.len();
    let mut header = [0u8; 28];
    let mut page_size = 0u32;
    match file.read_exact(&mut header) {
        Ok(()) => {
            let high = header.get(16).copied().unwrap_or(0);
            let low = header.get(17).copied().unwrap_or(0);
            let raw = (u32::from(high) << 8) | u32::from(low);
            page_size = if raw == 1 { 0x0001_0000 } else { raw };
        },
        Err(error) if error.kind() == ErrorKind::UnexpectedEof => {},
        Err(error) => return Err(read_err(error)),
    }
    Ok((db_len, page_size))
}

/// Copy up to `budget` bytes of the database file (1 MiB reads) into the
/// encoder, then drain whatever compressed output accumulated. Runs inside
/// `spawn_blocking`.
pub(crate) fn copy_step(
    db_path: &Utf8Path,
    mut encoder: SnapshotEncoder,
    mut hasher: Sha256,
    offset: u64,
    db_len: u64,
    budget: u64,
) -> Result<CopyStepResult, SnapshotError> {
    use sha2::Digest as _;

    let read_err = |error| SnapshotError::DbRead {
        path: db_path.to_owned(),
        error,
    };
    let mut cursor = offset;
    let mut remaining = db_len.saturating_sub(offset).min(budget.max(MIB));
    if remaining > 0 {
        let mut file = File::open(db_path).map_err(read_err)?;
        file.seek(SeekFrom::Start(offset)).map_err(read_err)?;
        let mut buffer = vec![0u8; usize::try_from(remaining.min(MIB)).unwrap_or(1)];
        while remaining > 0 {
            let want = usize::try_from(remaining.min(MIB)).unwrap_or(1);
            let Some(chunk) = buffer.get_mut(..want) else {
                break;
            };
            file.read_exact(chunk).map_err(read_err)?;
            encoder.write_all(chunk).map_err(SnapshotError::Compress)?;
            cursor = cursor.saturating_add(u64::try_from(want).unwrap_or(0));
            remaining = remaining.saturating_sub(u64::try_from(want).unwrap_or(remaining));
        }
    }
    if cursor >= db_len {
        let compressed = encoder.finish().map_err(SnapshotError::Compress)?;
        hasher.update(&compressed);
        Ok(CopyStepResult::Done { hasher, compressed })
    } else {
        let compressed = std::mem::take(encoder.get_mut());
        hasher.update(&compressed);
        Ok(CopyStepResult::Continue {
            encoder,
            hasher,
            compressed,
            offset: cursor,
        })
    }
}

#[cfg(test)]
mod tests {
    use camino::Utf8Path;
    use pretty_assertions::assert_eq;
    use sha2::{Digest as _, Sha256};

    use super::{
        CopyStepResult, MIB, SnapshotError, backup_to_scratch, copy_step, new_snapshot_encoder,
        read_scratch_info, require_backup_done,
    };
    use crate::segment::decompress_segment;

    fn tempdir_path(dir: &tempfile::TempDir) -> &Utf8Path {
        Utf8Path::from_path(dir.path()).unwrap()
    }

    /// Deterministic pseudo-random bytes (xorshift64), incompressible
    /// enough to exercise multi-part output.
    fn noise_bytes(len: usize) -> Vec<u8> {
        let mut state = 0x1234_5678_9abc_def0u64;
        let mut bytes = Vec::with_capacity(len);
        for _ in 0..len {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            bytes.push(u8::try_from(state >> 56).unwrap());
        }
        bytes
    }

    /// Drive `copy_step` to completion with the given budget; returns the
    /// concatenated compressed parts, the digest, and the step count.
    fn copy_all(db_path: &Utf8Path, db_len: u64, budget: u64) -> (Vec<u8>, String, u32) {
        let mut encoder = new_snapshot_encoder().unwrap();
        let mut hasher = Sha256::new();
        let mut offset = 0u64;
        let mut compressed_all = Vec::new();
        let mut steps = 0u32;
        loop {
            steps += 1;
            assert!(steps < 1000, "copy must terminate");
            match copy_step(db_path, encoder, hasher, offset, db_len, budget).unwrap() {
                CopyStepResult::Continue {
                    encoder: next_encoder,
                    hasher: next_hasher,
                    compressed,
                    offset: next_offset,
                } => {
                    assert!(next_offset > offset, "each step advances");
                    compressed_all.extend_from_slice(&compressed);
                    encoder = next_encoder;
                    hasher = next_hasher;
                    offset = next_offset;
                },
                CopyStepResult::Done { hasher, compressed } => {
                    compressed_all.extend_from_slice(&compressed);
                    return (compressed_all, hex::encode(hasher.finalize()), steps);
                },
            }
        }
    }

    #[test]
    fn copy_step_round_trips_and_hashes_compressed_stream() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tempdir_path(&tmp).join("fake.db");
        let body = noise_bytes(usize::try_from(3 * MIB + 500).unwrap());
        std::fs::write(&db_path, &body).unwrap();
        let db_len = u64::try_from(body.len()).unwrap();

        let (compressed, sha256, steps) = copy_all(&db_path, db_len, MIB);
        assert!(steps > 3, "a 3.5 MiB copy at 1 MiB budget takes >3 steps");
        assert_eq!(
            decompress_segment(&compressed).unwrap(),
            body,
            "the concatenated parts form one valid zstd frame"
        );
        assert_eq!(
            sha256,
            hex::encode(Sha256::digest(&compressed)),
            "the incremental hash covers exactly the compressed bytes"
        );

        // A one-shot copy (huge budget) produces the identical content.
        let (_, sha_oneshot, steps) = copy_all(&db_path, db_len, u64::MAX);
        assert_eq!(steps, 1, "one budgeted step suffices");
        drop(sha_oneshot);
    }

    #[test]
    fn copy_step_empty_file_finishes_immediately() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tempdir_path(&tmp).join("empty.db");
        std::fs::write(&db_path, b"").unwrap();
        let (compressed, _, steps) = copy_all(&db_path, 0, MIB);
        assert_eq!(steps, 1, "an empty file is one step");
        assert_eq!(
            decompress_segment(&compressed).unwrap(),
            Vec::<u8>::new(),
            "an empty frame decodes to nothing"
        );
    }

    #[test]
    fn backup_to_scratch_copies_committed_state_including_wal() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tempdir_path(&tmp).join("source.db");
        let scratch = tempdir_path(&tmp).join("source.db.snapshot-scratch");
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA wal_autocheckpoint = 0;
             CREATE TABLE t (id INTEGER PRIMARY KEY, data TEXT);
             INSERT INTO t (data) VALUES ('committed'), ('in the wal');",
        )
        .unwrap();
        // The rows above live only in the WAL (no checkpoint ran): the
        // backup must still include them, unlike a raw file copy.
        backup_to_scratch(&db_path, &scratch).unwrap();
        let copy = rusqlite::Connection::open(&scratch).unwrap();
        let rows: i64 = copy
            .query_row("SELECT count(*) FROM t", [], |row| row.get(0))
            .unwrap();
        assert_eq!(rows, 2, "WAL content is part of the backup");

        // A stale scratch is replaced, not appended to.
        backup_to_scratch(&db_path, &scratch).unwrap();
        let (db_len, page_size) = read_scratch_info(&scratch).unwrap();
        assert!(db_len > 0, "scratch has content");
        assert_eq!(page_size, 4096, "page size parsed from the header");
    }

    #[test]
    fn require_backup_done_rejects_incomplete_outcomes() {
        use rusqlite::backup::StepResult;

        require_backup_done(StepResult::Done).unwrap();
        for outcome in [StepResult::Busy, StepResult::Locked, StepResult::More] {
            let err = require_backup_done(outcome).unwrap_err();
            assert!(
                matches!(err, SnapshotError::BackupIncomplete { outcome: reported } if reported == outcome),
                "a non-Done step is reported honestly, got {err}"
            );
        }
    }

    #[test]
    fn read_scratch_info_short_file_yields_zero_page_size() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tempdir_path(&tmp).join("short.db");
        std::fs::write(&db_path, b"tiny").unwrap();
        let (db_len, page_size) = read_scratch_info(&db_path).unwrap();
        assert_eq!(page_size, 0, "no header, no page size");
        assert_eq!(db_len, 4, "length still bounds the upload");
    }
}
