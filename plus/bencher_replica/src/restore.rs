//! Startup-only restore: rebuild the database from the latest valid
//! generation (snapshot + ordered WAL segment replay).
//!
//! Runs BEFORE any application `SQLite` connection exists, in the same slot as
//! the old `litestream restore -if-replica-exists -if-db-not-exists`.
//!
//! Failure policy, per the design's disaster-recovery stance:
//!
//! - A broken WAL lineage (missing/corrupt segment, epoch gap) is a SOFT
//!   stop: replay ends at the last fully-valid epoch, the offending key is
//!   logged loudly, and the server boots on the older CONSISTENT state (an
//!   older commit boundary beats refusing to boot).
//! - A corrupt snapshot (checksum mismatch) or a restored file that fails
//!   `quick_check` is a HARD error: never boot on a corrupt database, and
//!   never leave a partial file behind.

use std::future::Future;
use std::io::ErrorKind;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Instant;

use async_compression::tokio::bufread::ZstdDecoder;
use camino::{Utf8Path, Utf8PathBuf};
use sha2::Digest as _;
use slog::Logger;
use tokio::io::{AsyncRead, AsyncReadExt as _, AsyncWriteExt as _, BufReader, ReadBuf};
use tokio::task::spawn_blocking;

use crate::backoff::Backoff;
use crate::config::ReplicaConfig;
use crate::meta::{META_VERSION, MetaError, ReplicaMeta};
use crate::position::{
    GENERATIONS_PREFIX, GenerationId, Position, SegmentKey, WAL_DIR, generation_prefix,
    parse_segment_key, segment_key, snapshot_key, snapshot_meta_key,
};
use crate::segment::decompress_segment_with_cap;
use crate::snapshot_meta::SnapshotMeta;
use crate::storage::{ReplicaStorage, StorageError};
use crate::wal::{WalError, WalScanner};

/// Outcome of the startup restore handshake.
#[derive(Debug)]
pub enum RestoreOutcome {
    /// The database file already exists: nothing to do
    /// (`-if-db-not-exists` semantics).
    Skipped,
    /// No valid generation exists on the replica: fresh server
    /// (`-if-replica-exists` semantics).
    NoReplica,
    /// The database was rebuilt from the replica.
    Restored {
        generation: GenerationId,
        /// Restored database size in bytes.
        db_bytes: u64,
        /// Number of WAL segments replayed on top of the snapshot.
        segments: u64,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum RestoreError {
    #[error("Replica storage: {0}")]
    Storage(#[from] StorageError),
    #[error("Replica meta: {0}")]
    Meta(#[from] MetaError),
    #[error("Segment: {0}")]
    Segment(#[from] crate::segment::SegmentError),
    #[error("Failed to write restore file ({path}): {error}")]
    Io {
        path: Utf8PathBuf,
        error: std::io::Error,
    },
    #[error("Failed to download snapshot for generation {}: {}", .generation.as_str(), .error)]
    SnapshotDownload {
        generation: GenerationId,
        error: std::io::Error,
    },
    #[error(
        "Snapshot checksum mismatch for generation {}: expected {expected}, computed {computed}",
        .generation.as_str()
    )]
    SnapshotChecksum {
        generation: GenerationId,
        expected: String,
        computed: String,
    },
    #[error(
        "Snapshot body for generation {} exceeds its recorded size of {db_bytes} bytes",
        .generation.as_str()
    )]
    SnapshotTooLarge {
        generation: GenerationId,
        db_bytes: u64,
    },
    #[error("Failed to apply WAL epoch {epoch}: {error}")]
    Apply { epoch: u64, error: rusqlite::Error },
    #[error(
        "WAL epoch {epoch} application incomplete: expected {expected_frames} frames, checkpoint reported busy {busy}, log {in_log}, checkpointed {checkpointed}"
    )]
    ApplyIncomplete {
        epoch: u64,
        expected_frames: u64,
        busy: i64,
        in_log: i64,
        checkpointed: i64,
    },
    #[error("Failed to run quick_check on the restored database: {0}")]
    QuickCheckRun(rusqlite::Error),
    #[error("Restored database failed quick_check: {0}")]
    QuickCheck(String),
    #[error("Restore task panicked: {0}")]
    Join(tokio::task::JoinError),
}

/// Bounded attempts for each transient storage read in the restore path
/// (initial try plus retries). Restore runs once at startup, so a small
/// bound rides out a blip without stalling boot indefinitely.
const RESTORE_ATTEMPTS: u32 = 3;

/// Adverse-event counter for every restore decision that drops replica data
/// (falling back to an older generation, truncating a broken lineage, or
/// discarding a short boundary epoch): the restore still "succeeds", so
/// without this a silent data regression is indistinguishable from a clean
/// restore in metrics.
fn count_soft_stop() {
    #[cfg(feature = "otel")]
    bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::ReplicaRestoreSoftStop);
}

/// Restore the latest replica state into `db_path` if (and only if) the
/// database file does not exist. Removes any stale `<db>.replica.json` when
/// the database is missing, and writes a fresh one on success so the
/// replicator resumes cleanly.
pub async fn restore_if_missing(
    log: &Logger,
    config: &ReplicaConfig,
    db_path: &Utf8Path,
) -> Result<RestoreOutcome, RestoreError> {
    let exists = db_path
        .as_std_path()
        .try_exists()
        .map_err(|error| RestoreError::Io {
            path: db_path.to_owned(),
            error,
        })?;
    if exists {
        slog::info!(log, "Database file exists; skipping restore";
            "db_path" => db_path.as_str());
        return Ok(RestoreOutcome::Skipped);
    }
    // The database is missing, so any meta file is stale advisory data from
    // a previous life of this volume: remove it before anything else.
    ReplicaMeta::remove(db_path)?;
    let storage = config.build_storage();
    let start = Instant::now();
    let Some(restored) = restore_db(log, &storage, db_path, None).await? else {
        slog::info!(log, "No valid replica generation found; starting fresh";
            "db_path" => db_path.as_str());
        return Ok(RestoreOutcome::NoReplica);
    };
    // Fresh advisory meta: after a restore the local WAL is empty, so the
    // first server write creates a fresh WAL with new salts. Marking the
    // restored epoch as fully shipped through a checkpoint lets the
    // replicator's resume take the meta-verified epoch+1 path instead of
    // forcing a whole-database re-snapshot.
    let meta = ReplicaMeta {
        version: META_VERSION,
        generation: restored.generation.as_str().to_owned(),
        epoch: restored.epoch,
        salt1: restored.salt.0,
        salt2: restored.salt.1,
        shipped_offset: restored.shipped_offset,
        epoch_shipped_through_checkpoint: true,
        shadow: false,
    };
    // Best-effort: the database is already restored and valid, and the meta
    // is advisory (without it the replicator's resume takes the conservative
    // re-snapshot path). Failing boot here over an advisory write would turn
    // a healthy restore into an outage.
    if let Err(error) = meta.store(db_path) {
        slog::warn!(log, "Failed to write advisory replica meta after restore; \
            the next resume will re-snapshot";
            "db_path" => db_path.as_str(), "error" => %error);
    }
    #[cfg(feature = "otel")]
    bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::ReplicaRestore);
    slog::info!(log, "Restored database from replica";
        "db_path" => db_path.as_str(),
        "generation" => restored.generation.as_str(),
        "db_bytes" => restored.db_bytes,
        "segments" => restored.segments,
        "duration_ms" => u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX));
    Ok(RestoreOutcome::Restored {
        generation: restored.generation,
        db_bytes: restored.db_bytes,
        segments: restored.segments,
    })
}

/// Restore into an explicit `scratch_db` path, unconditionally (no
/// database-exists check, no meta handling). Used by verification.
///
/// With `up_to = Some(position)`, only the position's generation is
/// considered and replay covers epochs `< position.epoch` plus, within
/// `position.epoch`, segments with `end <= position.offset` (positions are
/// commit-aligned, so segment boundaries line up exactly).
///
/// `Ok(None)` means no valid generation was found.
pub(crate) async fn restore_to(
    log: &Logger,
    storage: &ReplicaStorage,
    scratch_db: &Utf8Path,
    up_to: Option<&Position>,
) -> Result<Option<RestoredDb>, RestoreError> {
    // Verification restores log through the caller's logger so that the root
    // cause of a verification failure (a vanished segment, a soft-stopped
    // epoch) is observable; divergence itself is reported through the verify
    // result.
    restore_db(log, storage, scratch_db, up_to).await
}

/// A completed restore, before any advisory meta is written.
pub(crate) struct RestoredDb {
    pub generation: GenerationId,
    /// Restored database file size in bytes.
    pub db_bytes: u64,
    /// WAL segments replayed on top of the snapshot.
    pub segments: u64,
    /// Last epoch whose state the restored file reflects (the snapshot's
    /// `wal_boundary.epoch` when no segments were applied).
    pub epoch: u64,
    /// Salts of that epoch: from its directory name, or from the snapshot
    /// boundary when no segments were applied.
    pub salt: (u32, u32),
    /// Raw WAL offset restored through within `epoch` (0 = snapshot only).
    pub shipped_offset: u64,
}

/// Scratch-file cleanup wrapper around [`restore_db_inner`]: stale leftovers
/// from a previously crashed restore are removed up front (a stale
/// `.restore-wal` would otherwise be replayed into the fresh snapshot), and
/// nothing partial survives a hard error.
async fn restore_db(
    log: &Logger,
    storage: &ReplicaStorage,
    db_path: &Utf8Path,
    up_to: Option<&Position>,
) -> Result<Option<RestoredDb>, RestoreError> {
    let paths = RestorePaths::new(db_path);
    paths.clean_scratch(log).await;
    let result = restore_db_inner(log, storage, &paths, up_to).await;
    if result.is_err() {
        paths.clean_scratch(log).await;
    }
    result
}

async fn restore_db_inner(
    log: &Logger,
    storage: &ReplicaStorage,
    paths: &RestorePaths,
    up_to: Option<&Position>,
) -> Result<Option<RestoredDb>, RestoreError> {
    let want = up_to.map(|position| &position.generation);
    for generation in candidate_generations(log, storage, want).await? {
        let Some(snapshot) = load_snapshot_meta(log, storage, &generation).await? else {
            continue;
        };
        // A missing snapshot BODY is the partial-prune state (marker present,
        // body already deleted), not corruption: fall through to the
        // next-older restorable generation instead of refusing to boot.
        // Checksum mismatches and every other download error still HARD-fail,
        // because silently regressing to older data on real corruption would
        // be an operator's decision, not ours.
        match download_snapshot_retrying(log, storage, &generation, &snapshot, paths).await {
            Ok(()) => {},
            Err(RestoreError::Storage(StorageError::NotFound { key })) => {
                slog::warn!(log, "Snapshot body missing (partially pruned generation); trying an older generation";
                    "generation" => generation.as_str(), "key" => key);
                count_soft_stop();
                // A failed download may have left a partial scratch file.
                paths.clean_scratch(log).await;
                continue;
            },
            Err(error) => return Err(error),
        }
        let restored = finish_restore(log, storage, generation, snapshot, paths, up_to).await?;
        return Ok(Some(restored));
    }
    Ok(None)
}

/// After the snapshot body has downloaded and verified: replay the WAL
/// segments on top of it, `quick_check` the result, and finalize it into
/// place.
async fn finish_restore(
    log: &Logger,
    storage: &ReplicaStorage,
    generation: GenerationId,
    snapshot: SnapshotMeta,
    paths: &RestorePaths,
    up_to: Option<&Position>,
) -> Result<RestoredDb, RestoreError> {
    let segments = list_segments(log, storage, &generation).await?;
    let boundary_epoch = snapshot.wal_boundary.epoch;
    let limit = up_to.map(|position| (position.epoch, position.offset));
    let (mut plans, violation) = plan_epochs(segments, boundary_epoch, limit);
    if let Some(offender) = violation {
        slog::error!(log, "WAL segment lineage is broken; replaying only the leading valid epochs (best-effort disaster recovery)";
            "generation" => generation.as_str(),
            "offending_key" => segment_key(&generation, &offender));
        count_soft_stop();
    }
    // `wal_boundary.offset` is the mandatory-replay threshold: at least the
    // committed extent the snapshot body captured (overstated is safe, never
    // understated). Replaying only a PREFIX of the boundary epoch that stops
    // below it could regress pages the snapshot holds at a newer state, so
    // the boundary epoch is all-or-nothing up to the threshold: when the
    // available segments fall short, restore the snapshot alone (a consistent
    // committed state by itself). Overstating merely forces that fallback.
    if let Some(first) = plans.first()
        && first.epoch == boundary_epoch
        // The threshold is meaningful only for the SAME salt cycle it was
        // measured in. A rebound epoch 0 (different salts) means the WAL
        // restarted before anything shipped, which proves the whole old
        // cycle was backfilled into the backup: no frame of the new cycle
        // is mandatory.
        && first.salt == (snapshot.wal_boundary.salt1, snapshot.wal_boundary.salt2)
    {
        let available = first.segments.last().map_or(0, |segment| segment.end);
        if available < snapshot.wal_boundary.offset {
            slog::error!(log, "Boundary epoch segments end below the snapshot's mandatory-replay offset; restoring the snapshot alone";
                "generation" => generation.as_str(),
                "available" => available,
                "mandatory" => snapshot.wal_boundary.offset);
            count_soft_stop();
            plans.clear();
        }
    }
    let applied = apply_epochs(log, storage, &generation, &plans, paths).await?;
    quick_check(paths.restore.clone()).await?;
    let db_bytes = tokio::fs::metadata(&paths.restore)
        .await
        .map_err(|error| RestoreError::Io {
            path: paths.restore.clone(),
            error,
        })?
        .len();
    finalize(log, paths).await?;
    let (epoch, salt, shipped_offset) = applied.last.unwrap_or((
        boundary_epoch,
        (snapshot.wal_boundary.salt1, snapshot.wal_boundary.salt2),
        0,
    ));
    Ok(RestoredDb {
        generation,
        db_bytes,
        segments: applied.segments,
        epoch,
        salt,
        shipped_offset,
    })
}

/// The scratch and target paths of one restore.
struct RestorePaths {
    /// Final destination (`<db>`).
    target: Utf8PathBuf,
    /// Scratch database (`<db>.restore`).
    restore: Utf8PathBuf,
    /// Scratch WAL (`<db>.restore-wal`), rebuilt and replayed per epoch.
    wal: Utf8PathBuf,
    /// Scratch shared-memory sibling (`<db>.restore-shm`).
    shm: Utf8PathBuf,
}

impl RestorePaths {
    fn new(db_path: &Utf8Path) -> Self {
        let restore = Utf8PathBuf::from(format!("{db_path}.restore"));
        Self {
            target: db_path.to_owned(),
            wal: Utf8PathBuf::from(format!("{restore}-wal")),
            shm: Utf8PathBuf::from(format!("{restore}-shm")),
            restore,
        }
    }

    async fn clean_scratch(&self, log: &Logger) {
        remove_quiet(log, &self.restore).await;
        self.clean_scratch_wal(log).await;
    }

    async fn clean_scratch_wal(&self, log: &Logger) {
        remove_quiet(log, &self.wal).await;
        remove_quiet(log, &self.shm).await;
    }
}

/// The restorable generation directories, newest first. Unparsable directory
/// names are skipped. `want` restricts the search to one exact generation
/// (verification restores to a recorded position).
async fn candidate_generations(
    log: &Logger,
    storage: &ReplicaStorage,
    want: Option<&GenerationId>,
) -> Result<Vec<GenerationId>, RestoreError> {
    let components = with_retry(log, Backoff::default(), "list generations", || {
        storage.list_dirs(GENERATIONS_PREFIX)
    })
    .await?;
    let mut generations = Vec::new();
    for component in components.iter().rev() {
        let Some(generation) = GenerationId::parse(component) else {
            slog::warn!(log, "Skipping unparsable generation directory";
                "component" => component.as_str());
            continue;
        };
        if let Some(want) = want
            && &generation != want
        {
            continue;
        }
        generations.push(generation);
    }
    Ok(generations)
}

/// Load and parse a generation's `snapshot.json` commit marker. `Ok(None)`
/// when the marker is absent (crashed mid-snapshot) or unparsable: such a
/// generation is invisible to restore.
async fn load_snapshot_meta(
    log: &Logger,
    storage: &ReplicaStorage,
    generation: &GenerationId,
) -> Result<Option<SnapshotMeta>, RestoreError> {
    let key = snapshot_meta_key(generation);
    let bytes = match with_retry(log, Backoff::default(), "get snapshot.json", || {
        storage.get(&key)
    })
    .await
    {
        Ok(bytes) => bytes,
        Err(StorageError::NotFound { .. }) => {
            slog::warn!(log, "Generation has no snapshot.json (crashed mid-snapshot); skipping";
                "generation" => generation.as_str());
            return Ok(None);
        },
        Err(error) => return Err(error.into()),
    };
    match SnapshotMeta::from_bytes(&bytes) {
        Ok(snapshot) => Ok(Some(snapshot)),
        Err(error) => {
            slog::error!(log, "Skipping generation with unparsable snapshot.json";
                "key" => key, "error" => %error);
            count_soft_stop();
            Ok(None)
        },
    }
}

/// Retry a storage read up to [`RESTORE_ATTEMPTS`] times, waiting `backoff`
/// between attempts, retrying only TRANSIENT backend/network failures.
/// Definitive answers (`NotFound`, `InvalidKey`) return on the first attempt:
/// they are not going to change, and callers depend on `NotFound` to fall
/// through or soft-stop. The `backoff` is passed in (rather than built here)
/// so tests can drive it with zero delay.
async fn with_retry<T, F, Fut>(
    log: &Logger,
    mut backoff: Backoff,
    what: &str,
    mut op: F,
) -> Result<T, StorageError>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, StorageError>>,
{
    let mut attempt = 1u32;
    loop {
        match op().await {
            Ok(value) => return Ok(value),
            Err(error) => {
                if attempt >= RESTORE_ATTEMPTS || !is_retryable(&error) {
                    return Err(error);
                }
                let delay = backoff.next_delay();
                slog::warn!(log, "Transient replica storage error during restore; retrying";
                    "operation" => what, "attempt" => attempt, "error" => %error,
                    "delay_ms" => u64::try_from(delay.as_millis()).unwrap_or(u64::MAX));
                tokio::time::sleep(delay).await;
                attempt = attempt.saturating_add(1);
            },
        }
    }
}

/// Whether a storage error is worth retrying. `NotFound` and `InvalidKey` are
/// definitive; backend/network failures (and injected faults, in tests) are
/// transient.
fn is_retryable(error: &StorageError) -> bool {
    match error {
        StorageError::NotFound { .. } | StorageError::InvalidKey { .. } => false,
        StorageError::Local(_) | StorageError::S3(_) => true,
        #[cfg(any(test, feature = "testing"))]
        StorageError::Injected { .. } => true,
    }
}

/// Retry a whole snapshot-body download on TRANSIENT failures (a mid-stream
/// read reset or a transient backend/network error), up to
/// [`RESTORE_ATTEMPTS`] times. The stream cannot be resumed mid-way, so the
/// entire streamed download is re-run from scratch each attempt. Validation
/// verdicts (checksum mismatch, oversize) and a missing body (`NotFound`,
/// which the caller falls through on) are definitive and returned at once.
async fn download_snapshot_retrying(
    log: &Logger,
    storage: &ReplicaStorage,
    generation: &GenerationId,
    snapshot: &SnapshotMeta,
    paths: &RestorePaths,
) -> Result<(), RestoreError> {
    let mut backoff = Backoff::default();
    let mut attempt = 1u32;
    loop {
        match download_snapshot(storage, generation, snapshot, paths).await {
            Ok(()) => return Ok(()),
            Err(error) => {
                // `SnapshotDownload` wraps the streamed read+decode io::Error,
                // where a mid-stream network reset is indistinguishable from a
                // deterministic zstd corruption error, so BOTH retry (a
                // corrupt object wastes the remaining attempts, then
                // hard-fails). A checksum mismatch is computed from fully
                // downloaded bytes, is definitively corruption, and never
                // retries.
                let retryable = matches!(&error, RestoreError::SnapshotDownload { .. })
                    || matches!(&error, RestoreError::Storage(storage_error) if is_retryable(storage_error));
                if attempt >= RESTORE_ATTEMPTS || !retryable {
                    return Err(error);
                }
                slog::warn!(log, "Transient error downloading the snapshot body; retrying";
                    "generation" => generation.as_str(), "attempt" => attempt, "error" => %error);
                // Discard the partial scratch before the next attempt.
                paths.clean_scratch(log).await;
                let delay = backoff.next_delay();
                tokio::time::sleep(delay).await;
                attempt = attempt.saturating_add(1);
            },
        }
    }
}

/// Stream the snapshot object through a zstd decoder into the scratch
/// database file, hashing the COMPRESSED bytes and verifying them against
/// `snapshot.json`. The file is fsynced before the hash verdict.
async fn download_snapshot(
    storage: &ReplicaStorage,
    generation: &GenerationId,
    snapshot: &SnapshotMeta,
    paths: &RestorePaths,
) -> Result<(), RestoreError> {
    let stream = storage.get_stream(&snapshot_key(generation)).await?;
    let mut decoder = ZstdDecoder::new(BufReader::new(HashingReader::new(stream)));
    let io_err = |error| RestoreError::Io {
        path: paths.restore.clone(),
        error,
    };
    let download_err = |error| RestoreError::SnapshotDownload {
        generation: generation.clone(),
        error,
    };
    let mut file = tokio::fs::File::create(&paths.restore)
        .await
        .map_err(io_err)?;
    // Bound the decompressed write at the snapshot's recorded size (plus one
    // byte, to detect an overrun) so a corrupt or hostile object cannot force
    // an unbounded write to disk before the checksum is even consulted.
    let limit = snapshot.db_bytes.saturating_add(1);
    let written = tokio::io::copy(&mut (&mut decoder).take(limit), &mut file)
        .await
        .map_err(download_err)?;
    if written > snapshot.db_bytes {
        return Err(RestoreError::SnapshotTooLarge {
            generation: generation.clone(),
            db_bytes: snapshot.db_bytes,
        });
    }
    file.sync_all().await.map_err(io_err)?;
    drop(file);
    // Drain anything after the zstd frame so the hash covers the whole
    // object (trailing garbage must fail the checksum, not slip past it).
    let mut reader = decoder.into_inner();
    tokio::io::copy(&mut reader, &mut tokio::io::sink())
        .await
        .map_err(download_err)?;
    let computed = reader.into_inner().finalize_hex();
    if !computed.eq_ignore_ascii_case(&snapshot.sha256) {
        return Err(RestoreError::SnapshotChecksum {
            generation: generation.clone(),
            expected: snapshot.sha256.clone(),
            computed,
        });
    }
    Ok(())
}

/// List and parse every segment key of the generation; foreign keys under
/// the WAL prefix are skipped with a warning.
async fn list_segments(
    log: &Logger,
    storage: &ReplicaStorage,
    generation: &GenerationId,
) -> Result<Vec<SegmentKey>, RestoreError> {
    let prefix = format!("{}{WAL_DIR}/", generation_prefix(generation));
    let keys = with_retry(log, Backoff::default(), "list segments", || {
        storage.list(&prefix)
    })
    .await?;
    let mut segments = Vec::with_capacity(keys.len());
    for key in keys {
        match parse_segment_key(&key) {
            Some((parsed, segment)) if &parsed == generation => segments.push(segment),
            Some(_) | None => {
                slog::warn!(log, "Skipping foreign object under the WAL prefix"; "key" => key);
            },
        }
    }
    Ok(segments)
}

/// One epoch's replay plan: its segments in offset order.
struct EpochPlan {
    epoch: u64,
    salt: (u32, u32),
    segments: Vec<SegmentKey>,
}

/// Group segments into per-epoch replay plans and validate the lineage:
/// epochs must be contiguous starting exactly at `boundary_epoch`; within an
/// epoch the first segment must start at 0 (it carries the WAL header) and
/// each segment must start where the previous one ended.
///
/// Returns the plans for the leading run of fully-valid epochs plus the
/// first offending segment, if any (an epoch with any violation is discarded
/// entirely: replay stops at the last FULLY-valid epoch).
///
/// With `up_to = Some((epoch, offset))`, segments beyond that commit-aligned
/// position are excluded up front; their absence is not a violation.
fn plan_epochs(
    mut segments: Vec<SegmentKey>,
    boundary_epoch: u64,
    up_to: Option<(u64, u64)>,
) -> (Vec<EpochPlan>, Option<SegmentKey>) {
    if let Some((epoch_limit, offset_limit)) = up_to {
        segments.retain(|segment| {
            segment.epoch < epoch_limit
                || (segment.epoch == epoch_limit && segment.end <= offset_limit)
        });
    }
    segments.sort_by_key(|segment| (segment.epoch, segment.start, segment.end));
    let mut groups: Vec<EpochPlan> = Vec::new();
    for segment in segments {
        match groups.last_mut() {
            Some(group) if group.epoch == segment.epoch => group.segments.push(segment),
            Some(_) | None => groups.push(EpochPlan {
                epoch: segment.epoch,
                salt: segment.salt,
                segments: vec![segment],
            }),
        }
    }
    let mut plans = Vec::with_capacity(groups.len());
    let mut expected_epoch = boundary_epoch;
    for group in groups {
        if group.epoch != expected_epoch {
            return (plans, group.segments.first().copied());
        }
        let mut expected_start = 0u64;
        for segment in &group.segments {
            if segment.start != expected_start || segment.salt != group.salt {
                return (plans, Some(*segment));
            }
            expected_start = segment.end;
        }
        plans.push(group);
        expected_epoch = expected_epoch.saturating_add(1);
    }
    (plans, None)
}

/// Segments and end position actually applied.
struct AppliedEpochs {
    segments: u64,
    /// `(epoch, salt, end offset)` of the last fully applied epoch.
    last: Option<(u64, (u32, u32), u64)>,
}

/// Apply the planned epochs in order.
///
/// Failure policy, chosen so a restored database is NEVER a torn mixture:
///
/// - Replica DATA problems (vanished, corrupt, wrongly sized, or
///   chain-broken segments) are detected BEFORE any database mutation
///   (assembly checks plus a full checksum-chain pre-validation of the
///   assembled epoch WAL) and soft-stop the replay: the state produced by
///   the previous epochs is kept, loudly (an older CONSISTENT state beats
///   refusing to boot when the data is simply gone).
/// - LOCAL failures during application (`SQLite` errors, or a checkpoint
///   that consumed fewer frames than the pre-validated WAL contains) are
///   HARD errors: the application may have partially backfilled pages, so
///   the scratch database is discarded and the whole restore fails,
///   retryable once the local condition clears.
/// - Storage failures other than `NotFound` are hard errors too: an
///   unreachable replica is not a broken lineage.
async fn apply_epochs(
    log: &Logger,
    storage: &ReplicaStorage,
    generation: &GenerationId,
    plans: &[EpochPlan],
    paths: &RestorePaths,
) -> Result<AppliedEpochs, RestoreError> {
    let mut applied = AppliedEpochs {
        segments: 0,
        last: None,
    };
    for plan in plans {
        if !build_epoch_wal(log, storage, generation, plan, paths).await? {
            paths.clean_scratch_wal(log).await;
            break;
        }
        // Pre-validate the assembled WAL with our own scanner: the salts,
        // the full cumulative checksum chain, and that every byte belongs
        // to a committed frame. A shipped-then-forked lineage (or any
        // corruption the per-segment checks missed) is caught HERE, before
        // `SQLite` recovery could silently truncate at the break and let
        // later epochs replay on top of the wrong base.
        let Some(expected_frames) = validate_epoch_wal(paths.wal.clone()).await? else {
            slog::error!(log, "Assembled epoch WAL fails checksum-chain validation; stopping at the previous epoch";
                "generation" => generation.as_str(),
                "epoch" => plan.epoch);
            paths.clean_scratch_wal(log).await;
            break;
        };
        let (busy, in_log, checkpointed) =
            match checkpoint_scratch_wal(paths.restore.clone()).await? {
                Ok(row) => row,
                Err(error) => {
                    // The checkpoint may have partially backfilled: torn state.
                    paths.clean_scratch(log).await;
                    return Err(RestoreError::Apply {
                        epoch: plan.epoch,
                        error,
                    });
                },
            };
        let expected = i64::try_from(expected_frames).unwrap_or(i64::MAX);
        if busy != 0 || in_log != checkpointed || checkpointed != expected {
            // Pre-validation proved the WAL is fully committed and intact,
            // so a shortfall here is local trouble, not missing data.
            paths.clean_scratch(log).await;
            return Err(RestoreError::ApplyIncomplete {
                epoch: plan.epoch,
                expected_frames,
                busy,
                in_log,
                checkpointed,
            });
        }
        paths.clean_scratch_wal(log).await;
        applied.segments = applied
            .segments
            .saturating_add(u64::try_from(plan.segments.len()).unwrap_or(u64::MAX));
        if let Some(last) = plan.segments.last() {
            applied.last = Some((plan.epoch, plan.salt, last.end));
        }
    }
    Ok(applied)
}

/// Validate an assembled epoch WAL end to end with the crate's own scanner
/// (async wrapper around [`validate_epoch_wal_blocking`]).
async fn validate_epoch_wal(wal_path: Utf8PathBuf) -> Result<Option<u64>, RestoreError> {
    spawn_blocking(move || validate_epoch_wal_blocking(&wal_path))
        .await
        .map_err(RestoreError::Join)?
}

/// Validate an assembled epoch WAL: the header parses, the cumulative
/// checksum chain is intact, and EVERY byte past the header belongs to a
/// committed frame (assembled epochs are commit-aligned by construction).
/// Returns the frame count, or `None` when the file does not fully validate.
///
/// This file was just written and fsynced by this process, so a read or seek
/// FAILURE is a hard local I/O error (propagated), never a reason to silently
/// boot an older epoch; only invalid WAL CONTENT is the soft stop that yields
/// `None` (see [`epoch_wal_validation_stop`]).
fn validate_epoch_wal_blocking(wal_path: &Utf8Path) -> Result<Option<u64>, RestoreError> {
    let io_err = |error| RestoreError::Io {
        path: wal_path.to_owned(),
        error,
    };
    let file_len = std::fs::metadata(wal_path).map_err(io_err)?.len();
    let file = std::fs::File::open(wal_path).map_err(io_err)?;
    let mut scanner = match WalScanner::open(file) {
        Ok(Some(scanner)) => scanner,
        // A file shorter than a full header is an empty/truncated WAL.
        Ok(None) => return Ok(None),
        Err(error) => return epoch_wal_validation_stop(error, wal_path),
    };
    let frame_size = scanner.header().frame_size();
    loop {
        // A content stop returns Ok(None) from next_committed; only a real
        // read/seek failure surfaces as Err, and that must hard-fail.
        match scanner.next_committed(u64::MAX) {
            Ok(Some(_chunk)) => {},
            Ok(None) => break,
            Err(error) => return epoch_wal_validation_stop(error, wal_path),
        }
    }
    if scanner.offset() != file_len {
        return Ok(None);
    }
    let body = file_len.saturating_sub(crate::wal::WAL_HEADER_SIZE);
    if body % frame_size != 0 {
        return Ok(None);
    }
    // Exact by the modulus check above; checked_div sidesteps the
    // integer_division lint without a bare `/`.
    Ok(body.checked_div(frame_size))
}

/// Classify a [`WalError`] raised while validating an epoch WAL this process
/// just assembled and fsynced. A read or seek failure is a HARD local I/O
/// error (per this module's failure policy); every other variant means the
/// assembled WAL CONTENT does not validate, which is a soft stop (`Ok(None)`:
/// replay ends at the previous epoch).
fn epoch_wal_validation_stop(
    error: WalError,
    wal_path: &Utf8Path,
) -> Result<Option<u64>, RestoreError> {
    match error {
        WalError::Read(error) | WalError::Seek(error) => Err(RestoreError::Io {
            path: wal_path.to_owned(),
            error,
        }),
        WalError::TruncatedHeader(_)
        | WalError::BadMagic(_)
        | WalError::UnsupportedFormat(_)
        | WalError::InvalidPageSize(_)
        | WalError::HeaderChecksum { .. }
        | WalError::MisalignedOffset { .. }
        | WalError::TransactionTooLarge { .. } => Ok(None),
    }
}

/// Decompress-and-concatenate one epoch's segments into the scratch `-wal`
/// file (the first segment carries the 32-byte WAL header, so the
/// concatenation IS a valid WAL file). Returns `Ok(false)` on a soft stop:
/// a vanished, corrupt, or wrongly-sized segment.
async fn build_epoch_wal(
    log: &Logger,
    storage: &ReplicaStorage,
    generation: &GenerationId,
    plan: &EpochPlan,
    paths: &RestorePaths,
) -> Result<bool, RestoreError> {
    let io_err = |error| RestoreError::Io {
        path: paths.wal.clone(),
        error,
    };
    let mut wal_file = tokio::fs::File::create(&paths.wal).await.map_err(io_err)?;
    for segment in &plan.segments {
        let key = segment_key(generation, segment);
        let compressed = match with_retry(log, Backoff::default(), "get segment", || {
            storage.get(&key)
        })
        .await
        {
            Ok(bytes) => bytes,
            Err(StorageError::NotFound { .. }) => {
                slog::error!(log, "WAL segment vanished during restore; stopping at the previous epoch";
                    "key" => key);
                return Ok(false);
            },
            Err(error) => return Err(error.into()),
        };
        let expected = segment.end.saturating_sub(segment.start);
        // The key encodes the exact raw size, so decompress with that as the
        // cap (plus one byte, to catch an overrun): a corrupt or hostile
        // segment fails immediately instead of inflating toward the generic
        // multi-gigabyte ceiling before the length check below rejects it.
        let raw = spawn_blocking(move || {
            decompress_segment_with_cap(&compressed, expected.saturating_add(1))
        })
        .await
        .map_err(RestoreError::Join)?;
        let raw = match raw {
            Ok(raw) => raw,
            Err(error) => {
                let segment_error = RestoreError::Segment(error);
                slog::error!(log, "Corrupt WAL segment; stopping at the previous epoch";
                    "key" => key, "error" => %segment_error);
                return Ok(false);
            },
        };
        if u64::try_from(raw.len()).unwrap_or(u64::MAX) != expected {
            slog::error!(log, "WAL segment length does not match its key; stopping at the previous epoch";
                "key" => key,
                "expected_bytes" => expected,
                "actual_bytes" => raw.len());
            return Ok(false);
        }
        wal_file.write_all(&raw).await.map_err(io_err)?;
    }
    wal_file.sync_all().await.map_err(io_err)?;
    Ok(true)
}

/// Replay the assembled epoch WAL through `SQLite`'s own recovery:
/// `PRAGMA wal_checkpoint(FULL)` validates the salts and the cumulative
/// checksum chain and applies exactly the committed frames (zero
/// hand-written page application). FULL (not TRUNCATE) so the result row
/// `(busy, log, checkpointed)` reports the REAL frame counts (TRUNCATE
/// zeroes them after truncating), letting the caller verify that EVERY
/// frame of the pre-validated WAL was consumed; the scratch WAL file is
/// deleted by the caller afterward. The inner `Result` carries `SQLite`
/// failures, the outer one is task failure.
async fn checkpoint_scratch_wal(
    restore_path: Utf8PathBuf,
) -> Result<Result<(i64, i64, i64), rusqlite::Error>, RestoreError> {
    spawn_blocking(move || {
        let conn = rusqlite::Connection::open(&restore_path)?;
        let row: (i64, i64, i64) = conn.query_row("PRAGMA wal_checkpoint(FULL)", [], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?;
        conn.close().map_err(|(_conn, error)| error)?;
        Ok(row)
    })
    .await
    .map_err(RestoreError::Join)
}

/// `PRAGMA quick_check` on the restored file: anything but "ok" is a HARD
/// error (never boot on a corrupt database; contrast with the soft stop for
/// missing tail data, which yields an older but CONSISTENT state).
async fn quick_check(restore_path: Utf8PathBuf) -> Result<(), RestoreError> {
    let report = spawn_blocking(move || -> Result<Vec<String>, rusqlite::Error> {
        let conn = rusqlite::Connection::open(&restore_path)?;
        let mut statement = conn.prepare("PRAGMA quick_check")?;
        let rows = statement.query_map([], |row| row.get(0))?;
        rows.collect()
    })
    .await
    .map_err(RestoreError::Join)?
    .map_err(RestoreError::QuickCheckRun)?;
    if report == ["ok"] {
        Ok(())
    } else {
        Err(RestoreError::QuickCheck(report.join("; ")))
    }
}

/// Atomically move the verified scratch database into place. Stale `-wal` /
/// `-shm` siblings of the TARGET are removed first: `SQLite` would replay a
/// leftover WAL into the freshly restored file.
async fn finalize(log: &Logger, paths: &RestorePaths) -> Result<(), RestoreError> {
    remove_quiet(log, Utf8Path::new(&format!("{}-wal", paths.target))).await;
    remove_quiet(log, Utf8Path::new(&format!("{}-shm", paths.target))).await;
    tokio::fs::rename(&paths.restore, &paths.target)
        .await
        .map_err(|error| RestoreError::Io {
            path: paths.target.clone(),
            error,
        })?;
    // Best-effort directory fsync so the rename survives an abrupt stop;
    // the file itself is already fsynced.
    if let Some(parent) = paths.target.parent()
        && let Ok(dir) = std::fs::File::open(parent.as_std_path())
    {
        drop(dir.sync_all());
    }
    Ok(())
}

/// Remove a file, treating "already gone" as success; other failures are
/// logged and swallowed (cleanup is best-effort).
async fn remove_quiet(log: &Logger, path: &Utf8Path) {
    match tokio::fs::remove_file(path).await {
        Ok(()) => {},
        Err(error) if error.kind() == ErrorKind::NotFound => {},
        Err(error) => {
            slog::warn!(log, "Failed to remove restore scratch file";
                "path" => path.as_str(), "error" => %error);
        },
    }
}

/// Wraps the raw snapshot byte stream, hashing every COMPRESSED byte read
/// through it so the object hash can be verified against `snapshot.json`
/// without a second pass.
struct HashingReader<R> {
    inner: R,
    hasher: sha2::Sha256,
}

impl<R> HashingReader<R> {
    fn new(inner: R) -> Self {
        Self {
            inner,
            hasher: sha2::Sha256::new(),
        }
    }

    /// Hex digest of everything read so far.
    fn finalize_hex(self) -> String {
        hex::encode(self.hasher.finalize())
    }
}

impl<R: AsyncRead + Unpin> AsyncRead for HashingReader<R> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let this = self.get_mut();
        let already_filled = buf.filled().len();
        let poll = Pin::new(&mut this.inner).poll_read(cx, buf);
        if let Poll::Ready(Ok(())) = &poll {
            this.hasher
                .update(buf.filled().get(already_filled..).unwrap_or(&[]));
        }
        poll
    }
}

#[cfg(test)]
mod tests {
    use camino::Utf8Path;
    use pretty_assertions::assert_eq;

    use super::{
        RESTORE_ATTEMPTS, RestoreError, epoch_wal_validation_stop, is_retryable, plan_epochs,
        validate_epoch_wal_blocking, with_retry,
    };
    use crate::backoff::Backoff;
    use crate::position::SegmentKey;
    use crate::storage::StorageError;
    use crate::wal::WalError;

    const SALT: (u32, u32) = (0x9d2f_1c4a, 0x8b3e_6f70);
    const OTHER_SALT: (u32, u32) = (0x0101_0101, 0x0202_0202);

    fn seg(epoch: u64, start: u64, end: u64) -> SegmentKey {
        seg_with_salt(epoch, SALT, start, end)
    }

    fn seg_with_salt(epoch: u64, salt: (u32, u32), start: u64, end: u64) -> SegmentKey {
        SegmentKey {
            epoch,
            salt,
            start,
            end,
        }
    }

    fn plan_shape(plans: &[super::EpochPlan]) -> Vec<(u64, usize)> {
        plans
            .iter()
            .map(|plan| (plan.epoch, plan.segments.len()))
            .collect()
    }

    #[test]
    fn plan_contiguous_epochs_all_valid() {
        // Unsorted input on purpose: the planner sorts.
        let segments = vec![
            seg(1, 0, 100),
            seg(0, 32, 64),
            seg(0, 0, 32),
            seg(1, 100, 250),
            seg(0, 64, 200),
        ];
        let (plans, violation) = plan_epochs(segments, 0, None);
        assert_eq!(violation, None, "a contiguous lineage has no violation");
        assert_eq!(plan_shape(&plans), vec![(0, 3), (1, 2)]);
    }

    #[test]
    fn plan_stops_at_offset_gap() {
        let segments = vec![seg(0, 0, 32), seg(0, 64, 128), seg(1, 0, 32)];
        let (plans, violation) = plan_epochs(segments, 0, None);
        assert_eq!(
            violation,
            Some(seg(0, 64, 128)),
            "the first non-contiguous segment is the offender"
        );
        assert_eq!(
            plan_shape(&plans),
            Vec::<(u64, usize)>::new(),
            "an epoch with a gap is discarded entirely, along with everything after it"
        );
    }

    #[test]
    fn plan_requires_first_segment_at_zero() {
        let segments = vec![seg(0, 32, 128)];
        let (plans, violation) = plan_epochs(segments, 0, None);
        assert_eq!(violation, Some(seg(0, 32, 128)));
        assert_eq!(plan_shape(&plans), Vec::<(u64, usize)>::new());
    }

    #[test]
    fn plan_requires_epochs_to_start_at_boundary() {
        // The snapshot boundary is epoch 0, but only epoch 1 was shipped.
        let segments = vec![seg(1, 0, 32)];
        let (plans, violation) = plan_epochs(segments, 0, None);
        assert_eq!(violation, Some(seg(1, 0, 32)));
        assert_eq!(plan_shape(&plans), Vec::<(u64, usize)>::new());
    }

    #[test]
    fn plan_stops_at_epoch_number_gap() {
        let segments = vec![seg(0, 0, 32), seg(2, 0, 32)];
        let (plans, violation) = plan_epochs(segments, 0, None);
        assert_eq!(violation, Some(seg(2, 0, 32)), "epoch 1 is missing");
        assert_eq!(plan_shape(&plans), vec![(0, 1)], "epoch 0 still replays");
    }

    #[test]
    fn plan_stops_at_salt_mismatch_within_epoch() {
        // Two epoch dirs claiming the same epoch number with different
        // salts collapse into one group; the second lineage is an offender.
        let segments = vec![seg(0, 0, 32), seg_with_salt(0, OTHER_SALT, 32, 64)];
        let (plans, violation) = plan_epochs(segments, 0, None);
        assert_eq!(violation, Some(seg_with_salt(0, OTHER_SALT, 32, 64)));
        assert_eq!(plan_shape(&plans), Vec::<(u64, usize)>::new());
    }

    #[test]
    fn plan_discards_epoch_with_duplicate_salt_lineages() {
        // The salt-collision case: epoch 1 exists as TWO complete, individually
        // well-formed lineages under different salts (two epoch directories with
        // the same epoch number). Lineage A (SALT) chains 0..40..90 and lineage
        // B (OTHER_SALT) chains 0..50..120; each is valid on its own. The sort
        // key is (epoch, start, end) with salt EXCLUDED, so both collapse into
        // one epoch-1 group whose salt is the first sorted segment's. The second
        // lineage's leading segment then fails the group-salt / offset-chain
        // check, discarding the WHOLE epoch. Restore soft-stops at epoch 0: it
        // never tears the two lineages together and never silently applies
        // either one. This is safe but pessimistic (a complete valid lineage is
        // dropped), and is the reason the resume path must never create the
        // collision in the first place (see `meta_matches` in `sync.rs`).
        let segments = vec![
            seg(0, 0, 32),
            seg(1, 0, 40),
            seg(1, 40, 90),
            seg_with_salt(1, OTHER_SALT, 0, 50),
            seg_with_salt(1, OTHER_SALT, 50, 120),
        ];
        let (plans, violation) = plan_epochs(segments, 0, None);
        assert_eq!(
            plan_shape(&plans),
            vec![(0, 1)],
            "epoch 0 applies; the duplicate-epoch collision discards epoch 1 entirely"
        );
        assert_eq!(
            violation,
            Some(seg_with_salt(1, OTHER_SALT, 0, 50)),
            "the offender is the second lineage's leading segment (start 0 under a foreign salt)"
        );
    }

    #[test]
    fn plan_up_to_filters_epochs_and_offsets() {
        let segments = vec![
            seg(0, 0, 100),
            seg(1, 0, 40),
            seg(1, 40, 90),
            seg(1, 90, 150),
            seg(2, 0, 32),
        ];
        // Stop mid-epoch-1 at the commit-aligned offset 90.
        let (plans, violation) = plan_epochs(segments.clone(), 0, Some((1, 90)));
        assert_eq!(violation, None, "excluded segments are not violations");
        assert_eq!(plan_shape(&plans), vec![(0, 1), (1, 2)]);
        // An offset before any segment of the epoch keeps only prior epochs.
        let (plans, violation) = plan_epochs(segments, 0, Some((1, 0)));
        assert_eq!(violation, None);
        assert_eq!(plan_shape(&plans), vec![(0, 1)]);
    }

    #[test]
    fn epoch_wal_validation_io_error_hard_fails_content_soft_stops() {
        let path = Utf8Path::new("/does/not/matter.wal");
        // A read or seek failure on the file this process just wrote and
        // fsynced is a HARD error: it must never masquerade as a broken chain
        // and silently boot an older epoch.
        for error in [
            WalError::Read(std::io::Error::other("boom")),
            WalError::Seek(std::io::Error::other("boom")),
        ] {
            assert!(
                matches!(
                    epoch_wal_validation_stop(error, path),
                    Err(RestoreError::Io { .. })
                ),
                "an I/O error must hard-fail"
            );
        }
        // Invalid WAL CONTENT is a soft stop: replay ends at the previous
        // epoch (Ok(None)).
        for error in [
            WalError::TruncatedHeader(5),
            WalError::BadMagic(0xdead_beef),
            WalError::UnsupportedFormat(1),
            WalError::InvalidPageSize(3),
            WalError::HeaderChecksum {
                stored: (1, 2),
                computed: (3, 4),
            },
            WalError::MisalignedOffset {
                offset: 33,
                page_size: 4096,
            },
            WalError::TransactionTooLarge {
                bytes: 1 << 30,
                max_bytes: 1 << 20,
            },
        ] {
            assert!(
                matches!(epoch_wal_validation_stop(error, path), Ok(None)),
                "invalid content is a soft stop"
            );
        }
    }

    #[test]
    fn validate_epoch_wal_blocking_hard_fails_on_read_error() {
        // A directory where the WAL file should be: metadata and open succeed
        // on Unix, but the header read fails (EISDIR) -> WalError::Read -> a
        // HARD RestoreError::Io, never a silent Ok(None) soft stop. Even if a
        // platform instead fails at open, the result is still a hard error,
        // which is the property under test.
        let tmp = tempfile::tempdir().unwrap();
        let dir = Utf8Path::from_path(tmp.path()).unwrap().join("epoch.wal");
        std::fs::create_dir(&dir).unwrap();
        assert!(
            matches!(
                validate_epoch_wal_blocking(&dir),
                Err(RestoreError::Io { .. })
            ),
            "a read failure on the assembled WAL must hard-fail"
        );
    }

    fn discard_logger() -> slog::Logger {
        slog::Logger::root(slog::Discard, slog::o!())
    }

    /// A retryable (transient) storage error for the retry tests.
    fn injected() -> StorageError {
        StorageError::Injected {
            op: "get",
            key: "k".to_owned(),
        }
    }

    /// Zero-delay backoff so the retry tests never touch the wall clock.
    fn no_delay() -> Backoff {
        Backoff::new(std::time::Duration::ZERO, std::time::Duration::ZERO)
    }

    #[test]
    fn is_retryable_splits_definitive_from_transient() {
        assert!(
            !is_retryable(&StorageError::NotFound {
                key: "k".to_owned()
            }),
            "NotFound is definitive"
        );
        assert!(
            !is_retryable(&StorageError::InvalidKey {
                key: "k".to_owned(),
                reason: "leading slash",
            }),
            "InvalidKey is a programming error, not transient"
        );
        assert!(
            is_retryable(&injected()),
            "an injected backend fault is transient"
        );
    }

    #[tokio::test]
    async fn with_retry_recovers_from_a_transient_failure() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicU32, Ordering};

        let calls = Arc::new(AtomicU32::new(0));
        let result = with_retry(&discard_logger(), no_delay(), "get", || {
            let calls = Arc::clone(&calls);
            async move {
                let seen = calls.fetch_add(1, Ordering::SeqCst) + 1;
                if seen < 2 {
                    Err(injected())
                } else {
                    Ok::<u32, StorageError>(42)
                }
            }
        })
        .await;
        assert_eq!(result.unwrap(), 42, "the retry succeeds");
        assert_eq!(
            calls.load(Ordering::SeqCst),
            2,
            "exactly one retry after the transient failure"
        );
    }

    #[tokio::test]
    async fn with_retry_gives_up_after_the_attempt_bound() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicU32, Ordering};

        let calls = Arc::new(AtomicU32::new(0));
        let result: Result<u32, StorageError> =
            with_retry(&discard_logger(), no_delay(), "get", || {
                let calls = Arc::clone(&calls);
                async move {
                    calls.fetch_add(1, Ordering::SeqCst);
                    Err(injected())
                }
            })
            .await;
        assert!(
            matches!(result, Err(StorageError::Injected { .. })),
            "a persistent transient error still fails after the bound"
        );
        assert_eq!(
            calls.load(Ordering::SeqCst),
            RESTORE_ATTEMPTS,
            "tried exactly RESTORE_ATTEMPTS times"
        );
    }

    #[tokio::test]
    async fn with_retry_does_not_retry_definitive_errors() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicU32, Ordering};

        let calls = Arc::new(AtomicU32::new(0));
        let result: Result<u32, StorageError> =
            with_retry(&discard_logger(), no_delay(), "get", || {
                let calls = Arc::clone(&calls);
                async move {
                    calls.fetch_add(1, Ordering::SeqCst);
                    Err(StorageError::NotFound {
                        key: "k".to_owned(),
                    })
                }
            })
            .await;
        assert!(
            matches!(result, Err(StorageError::NotFound { .. })),
            "NotFound propagates unchanged"
        );
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "a definitive NotFound is tried exactly once"
        );
    }
}
