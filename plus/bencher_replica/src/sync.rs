//! The sync engine: a single sequential task that ships WAL frames,
//! checkpoints, snapshots, and prunes.
//!
//! Step-driven core: [`SyncEngine::sync_once`] is one production tick, and
//! the finer steps ([`SyncEngine::ship_once`], [`SyncEngine::checkpoint_once`],
//! [`SyncEngine::snapshot_step`], [`SyncEngine::prune_once`]) are public so
//! tests drive every transition deterministically; the production loop in
//! `replicator.rs` is a trivial tick shell. All scheduling decisions go
//! through the injected [`bencher_json::Clock`].
//!
//! Blocking work (rusqlite, WAL file scans, zstd, meta writes) runs under
//! `tokio::task::spawn_blocking` with owned data; the engine is safe on
//! current-thread runtimes.
//!
//! ## Position resume (five-step decision table)
//!
//! At construction the engine reconciles three sources: the replica LIST
//! (the source of truth, invariant I6), the local WAL header, and the
//! advisory meta file.
//!
//! 1. LIST the latest valid generation (has `snapshot.json`) and its last
//!    segment, giving the replica tip `(epoch_r, salt_r, end_r)`. No
//!    generation: schedule the first snapshot (state `PendingSnapshot`).
//! 2. Read the local WAL header (missing or short: step 5).
//! 3. Local salts equal `salt_r`: rescan the local chain from offset 0 and
//!    require it valid through exactly `end_r` (recovering the running
//!    checksum there); resume at `(epoch_r, end_r)`. A local WAL shorter
//!    than `end_r` (replica ahead: `synchronous=NORMAL` rewind after a
//!    crash) or a broken chain is DIVERGENCE: new generation.
//! 4. Salt mismatch: if the meta file matches the replica exactly
//!    (generation, `epoch == epoch_r`, `shipped_offset == end_r`, shipped
//!    through a completed checkpoint, same shadow mode), the old epoch is
//!    provably complete: resume as `epoch_r + 1` at offset 0 with the LOCAL
//!    header's salts (or rebind an empty `epoch_r` in place when
//!    `end_r == 0`). Anything else: new generation.
//! 5. Local WAL absent or shorter than a header: with the same meta proof,
//!    wait in `AwaitingEpoch` and bind the salts when the first frames
//!    appear; otherwise new generation.
//!
//! In shadow mode a resume divergence also just starts a new generation
//! (the shadow replica is disposable).

use std::fs::File;
use std::io::{ErrorKind, Read as _};
use std::sync::Arc;
use std::time::Duration;

use bencher_json::Clock;
use bytes::Bytes;
use camino::{Utf8Path, Utf8PathBuf};
use sha2::{Digest as _, Sha256};
use slog::Logger;
use tokio::task::spawn_blocking;

use crate::backoff::Backoff;
use crate::checkpoint::{
    CheckpointConns, CheckpointError, CheckpointOutcome, PinOutcome, checkpoint_locked, pin_locked,
};
use crate::config::ReplicaConfig;
use crate::meta::{META_VERSION, MetaError, ReplicaMeta};
use crate::position::{
    GENERATIONS_PREFIX, GenerationId, Position, SegmentKey, WAL_DIR, generation_prefix,
    parse_segment_key, segment_key, snapshot_key, snapshot_meta_key,
};
use crate::replicator::ReplicaDb;
use crate::segment::{SEGMENT_MAX_BYTES, SegmentError, compress_segment};
use crate::snapshot::{
    CopyJob, CopyStepResult, FinalizeJob, MIB, STALE_INCOMPLETE_SECS, SnapshotError, SnapshotJob,
    SnapshotStatus, backup_to_scratch, copy_step, live_db_fingerprint, new_snapshot_encoder,
    read_scratch_info,
};
use crate::snapshot_meta::{SNAPSHOT_META_VERSION, SnapshotMeta, SnapshotMetaError, WalBoundary};
use crate::storage::{MultipartUpload, ReplicaStorage, StorageError};
use crate::verify::{VerifyError, VerifyReport, fingerprint_database, verify_against_replica};
use crate::wal::{
    CommittedChunk, WAL_HEADER_SIZE, WalError, WalHeader, WalScanner, parse_wal_header,
};

/// The single-task replication engine, generic over the app writer
/// connection type: the engine only ever HOLDS the mutex guard (to freeze
/// app writers during the checkpoint critical section), never uses `C`.
pub struct SyncEngine<C> {
    log: Logger,
    config: ReplicaConfig,
    db: ReplicaDb<C>,
    clock: Clock,
    shadow: bool,
    storage: ReplicaStorage,
    /// Current shipping position; `None` when no lineage is usable.
    position: Option<Position>,
    /// Salts-unbound resume: `(generation, epoch)` awaiting first frames.
    awaiting: Option<(GenerationId, u64)>,
    /// In-flight snapshot state machine.
    snapshot: Option<SnapshotJob>,
    /// A new-generation snapshot has been requested (divergence, tripwire
    /// abort, or explicit trigger).
    pending_new_generation: bool,
    /// The greatest generation id ever observed (replica tip at resume or
    /// our own snapshots); new generation ids must sort after it so restore
    /// always picks the newest lineage, even across a divergence.
    generation_floor: Option<GenerationId>,
    /// Whether the current epoch is fully shipped AND fully backfilled by a
    /// completed checkpoint (mirrors the advisory meta flag). Cleared by
    /// every segment ship; set by [`CheckpointOutcome::Completed`].
    epoch_checkpointed: bool,
    backoff: Backoff,
    /// Clock second before which ticks are no-ops (storage error backoff).
    backoff_until: Option<i64>,
    /// Lazily opened checkpoint connections, reused across checkpoints.
    conns: Option<CheckpointConns>,
    /// Clock second of the last Completed or Partial checkpoint (Partial
    /// counts for retry pacing). Initialized to construction time.
    last_checkpoint_secs: i64,
    /// Clock second the current generation was created or resumed.
    generation_birth_secs: i64,
    /// Clock second of the last completed verification (pass or fail).
    /// Initialized to construction time, so the first verification runs one
    /// interval after startup.
    last_verify_secs: i64,
    /// Clock second before which verification is not retried after it could
    /// not complete (execution error, or a Busy/Unshipped pin). Paces
    /// verification independently of the WAL-ship backoff so a persistently
    /// broken verify never throttles shipping.
    verify_retry_until: Option<i64>,
    /// The oversized-transaction drain state machine (see [`OversizedDrain`]).
    oversized_drain: OversizedDrain,
    /// Clock second before which an oversized-transaction divergence is not
    /// re-triggered (a broken drain, e.g. a permanently pinned reader or a
    /// shadow replica awaiting Litestream, must not churn snapshots tick after
    /// tick).
    oversized_retrigger_until: Option<i64>,
}

/// How the engine is recovering from an oversized COMMITTED transaction that
/// cannot ship (see [`SyncEngine::diverge_oversized`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OversizedDrain {
    /// Not draining.
    Idle,
    /// A re-snapshot is scheduled to capture the transaction; its finalize
    /// pins epoch 0 at the boundary and advances to `AwaitingRestart`.
    Pending,
    /// Epoch 0 is pinned at the boundary awaiting the WAL restart that a
    /// completed checkpoint allows: the ship path ships nothing, and the next
    /// restart rebinds epoch 0 to the fresh cycle (offset 0) rather than
    /// advancing to epoch+1 (which would leave epoch 0 empty and break
    /// restore's epoch contiguity).
    AwaitingRestart,
}

#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error("Replica storage: {0}")]
    Storage(#[from] StorageError),
    #[error("WAL: {0}")]
    Wal(#[from] WalError),
    #[error("Failed to read local WAL ({path}): {error}")]
    WalIo {
        path: Utf8PathBuf,
        error: std::io::Error,
    },
    #[error("Segment: {0}")]
    Segment(#[from] SegmentError),
    #[error("Replica meta: {0}")]
    Meta(#[from] MetaError),
    #[error("Checkpoint: {0}")]
    Checkpoint(#[from] CheckpointError),
    #[error("Snapshot: {0}")]
    Snapshot(#[from] SnapshotError),
    #[error("Snapshot meta: {0}")]
    SnapshotMeta(#[from] SnapshotMetaError),
    #[error("Verification: {0}")]
    Verify(#[from] VerifyError),
    #[error(
        "A single transaction wrote {bytes} WAL bytes, beyond the restorable segment bound of {max_bytes}; refusing to ship it (split the write, or it would poison every restore of this generation)"
    )]
    TransactionTooLarge { bytes: u64, max_bytes: u64 },
    #[error(
        "An external checkpoint restarted the WAL during a snapshot; the generation was aborted (no snapshot.json) and a fresh one scheduled"
    )]
    SnapshotBoundaryDiverged,
    #[error("Sync task panicked: {0}")]
    Join(#[from] tokio::task::JoinError),
    #[error("The replicator task exited unexpectedly")]
    TaskExited,
}

impl SyncError {
    /// Whether the error is a transient storage failure that the caller
    /// should retry with backoff (as opposed to a fatal local failure).
    #[must_use]
    pub fn is_retryable(&self) -> bool {
        matches!(self, Self::Storage(_))
    }

    /// Whether the error is structurally unrecoverable ("poison"): retrying
    /// the identical operation can never succeed, and because a failed ship
    /// blocks every downstream step (checkpoints, snapshots), a poison error
    /// silently retried every tick would let the local WAL grow without bound.
    /// Poison errors are propagated out of [`SyncEngine::sync_once`] so the
    /// replicator's fatal channel fires and the operator is alerted.
    ///
    /// An oversized transaction qualifies as a LAST-RESORT safety net: the
    /// ship path intercepts it first (see [`Self::is_transaction_too_large`])
    /// and converts it into a recoverable re-snapshot, so in normal operation
    /// it never reaches here. Should it ever surface from an unconverted path,
    /// a loud fatal exit still beats an unbounded retry loop.
    #[must_use]
    pub fn is_poison(&self) -> bool {
        self.is_transaction_too_large()
    }

    /// Whether the error is an oversized transaction (raw WAL bytes since the
    /// last commit beyond the configured bound), from either the WAL scan or
    /// the ship-time chunk check. The ship path converts a COMMITTED oversized
    /// transaction into a re-snapshot: it is already durable, so it cannot be
    /// un-committed and "split", but the snapshot captures its state without
    /// shipping the unshippable WAL.
    #[must_use]
    pub fn is_transaction_too_large(&self) -> bool {
        matches!(
            self,
            Self::TransactionTooLarge { .. } | Self::Wal(WalError::TransactionTooLarge { .. })
        )
    }
}

/// What one [`SyncEngine::sync_once`] tick did.
#[derive(Debug, Default)]
pub struct SyncProgress {
    /// Segments shipped this tick.
    pub shipped_segments: u64,
    /// Checkpoint outcome, when one was attempted.
    pub checkpoint: Option<CheckpointOutcome>,
    /// Snapshot step status, when one ran.
    pub snapshot: Option<SnapshotStatus>,
    /// Verification report, when a scheduled verification completed.
    pub verify: Option<VerifyReport>,
    /// The tick was skipped because a backoff delay is still pending.
    pub backing_off: bool,
    /// The error that armed a backoff this tick (storage or snapshot); the
    /// tick itself still returns `Ok` so the task keeps running.
    pub error: Option<SyncError>,
}

/// The engine's observable state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineState {
    /// Shipping from a bound position.
    Streaming,
    /// A known generation and epoch await their first frames to bind salts.
    AwaitingEpoch,
    /// No usable lineage: a new-generation snapshot must run first.
    PendingSnapshot,
    /// A snapshot is in progress.
    Snapshotting,
}

/// Upper bound on segments shipped per ship pass (approximately 256 MiB of
/// raw WAL; a single oversized transaction ships whole, so one pass can
/// exceed that). A continuously committing writer would otherwise keep
/// feeding the scan loop forever and starve the tick (checkpoints, snapshots,
/// shutdown); the next tick simply resumes where this one stopped.
const MAX_SEGMENTS_PER_PASS: u64 = 32;

/// Minimum age of the current generation before a failed verification may
/// force another one (rate limit: repeated verify failures must not churn
/// whole-database snapshots back to back).
const VERIFY_RETRIGGER_MIN_SECS: i64 = 6 * 60 * 60;

/// Retry spacing after a verification that could not complete (an execution
/// error, or a Busy/Unshipped pin under sustained writes). Modest, so a
/// failed pin does not re-drain the WAL tail on every tick, yet far below the
/// verification interval, so a transient failure heals well within one cycle.
const VERIFY_RETRY_SECS: i64 = 60;

/// Minimum spacing between oversized-transaction re-snapshots. The drain
/// normally breaks the loop (the position pins at the boundary, so the ship
/// path stops re-hitting the oversized transaction), but if the drain cannot
/// complete (a permanently pinned reader, or a shadow replica whose WAL only
/// Litestream can restart), this bounds how often a fresh generation is minted
/// while the condition persists.
const OVERSIZED_RETRIGGER_MIN_SECS: i64 = 60 * 60;

/// The replica's tip, derived from a LIST alone (invariant I6).
struct ReplicaTip {
    generation: GenerationId,
    epoch: u64,
    salt: (u32, u32),
    /// Raw WAL byte offset shipped through in `epoch` (0: nothing shipped).
    end: u64,
    /// The tip epoch's last segment, for content verification at resume.
    last_segment: Option<SegmentKey>,
}

/// The local WAL header, read without touching `SQLite`.
enum WalHeaderState {
    /// No WAL file, or shorter than a full header.
    Missing,
    /// A header exists but does not parse (e.g. torn concurrent write).
    Unreadable(WalError),
    Present {
        header: WalHeader,
        /// The exact on-disk header bytes (shipped verbatim in the first
        /// segment of every epoch).
        raw: [u8; 32],
    },
}

impl<C> SyncEngine<C> {
    /// Build the engine and resolve the resume position against the replica
    /// (see the module docs for the decision table).
    ///
    /// Storage errors here are retryable: an unreachable replica at boot
    /// must NEVER be conflated with an empty one (a new generation is only
    /// ever created from a successful LIST).
    pub async fn new(
        log: Logger,
        config: ReplicaConfig,
        db: ReplicaDb<C>,
        clock: Clock,
        shadow: bool,
    ) -> Result<Self, SyncError> {
        let storage = config.build_storage();
        Self::new_with_storage(log, config, db, clock, shadow, storage).await
    }

    /// [`Self::new`] with an explicit storage backend. Production callers
    /// use [`Self::new`]; tests inject `ReplicaStorage::Flaky` here.
    pub async fn new_with_storage(
        log: Logger,
        config: ReplicaConfig,
        db: ReplicaDb<C>,
        clock: Clock,
        shadow: bool,
        storage: ReplicaStorage,
    ) -> Result<Self, SyncError> {
        let now = clock.timestamp();
        let mut engine = Self {
            log,
            config,
            db,
            clock,
            shadow,
            storage,
            position: None,
            awaiting: None,
            snapshot: None,
            pending_new_generation: false,
            generation_floor: None,
            epoch_checkpointed: false,
            backoff: Backoff::default(),
            backoff_until: None,
            conns: None,
            last_checkpoint_secs: now,
            generation_birth_secs: now,
            last_verify_secs: now,
            verify_retry_until: None,
            oversized_drain: OversizedDrain::Idle,
            oversized_retrigger_until: None,
        };
        engine.resume().await?;
        Ok(engine)
    }

    /// One full production tick: honor backoff, advance a snapshot if one
    /// is active (or required), otherwise ship, checkpoint when due, and
    /// start a snapshot when due. Transient errors (storage and the like) arm
    /// a capped exponential backoff and are reported in the progress instead
    /// of failing the task; a POISON error ([`SyncError::is_poison`]) is
    /// returned as `Err` so the replicator's fatal channel fires (retrying it
    /// forever would only let the WAL grow without bound).
    pub async fn sync_once(&mut self) -> Result<SyncProgress, SyncError> {
        let mut progress = SyncProgress::default();
        let now = self.now_secs();
        if let Some(until) = self.backoff_until {
            if now < until {
                progress.backing_off = true;
                return Ok(progress);
            }
            self.backoff_until = None;
        }

        // Snapshotting, or no usable lineage: the snapshot machine runs.
        if self.snapshot.is_some() || (self.position.is_none() && self.awaiting.is_none()) {
            match self.snapshot_step().await {
                Ok(status) => {
                    progress.snapshot = Some(status);
                    self.backoff.reset();
                },
                Err(error) if error.is_poison() => return Err(error),
                Err(error) => {
                    self.arm_backoff(now);
                    progress.error = Some(error);
                },
            }
            return Ok(progress);
        }

        // Streaming (or awaiting the first frames of a known epoch).
        match self.ship_once().await {
            Ok(segments) => progress.shipped_segments = segments,
            Err(error) => {
                #[cfg(feature = "otel")]
                bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::ReplicaShipFailed);
                if error.is_poison() {
                    return Err(error);
                }
                self.arm_backoff(now);
                progress.error = Some(error);
                return Ok(progress);
            },
        }
        if self.checkpoint_due(now).await {
            match self.checkpoint_once().await {
                Ok(outcome) => progress.checkpoint = Some(outcome),
                Err(error) => {
                    #[cfg(feature = "otel")]
                    bencher_otel::ApiMeter::increment(
                        bencher_otel::ApiCounter::ReplicaCheckpointFailed,
                    );
                    if error.is_poison() {
                        return Err(error);
                    }
                    self.arm_backoff(now);
                    progress.error = Some(error);
                    return Ok(progress);
                },
            }
        }
        if self.verify_due(now) {
            match self.verify_once().await {
                // A completed verification (Pass or Fail); `verify_once`
                // already advanced `last_verify_secs`.
                Ok(Some(report)) => progress.verify = Some(report),
                // Could not pin (Busy/Unshipped) or not ready: space out the
                // retry so a sustained-write pin failure does not re-run the
                // full drain on every tick.
                Ok(None) => {
                    self.verify_retry_until = Some(now.saturating_add(VERIFY_RETRY_SECS));
                },
                // A verification EXECUTION error (restore/fingerprint/storage)
                // must NOT arm the global ship backoff: a persistently broken
                // verify would otherwise throttle WAL shipping forever. Pace
                // it on its own retry clock and keep the tick going. Poison
                // errors still escalate: `verify_once` runs its own ship
                // drain, so a structurally-unrecoverable ship error can
                // surface here first and must reach the fatal channel like
                // the ship/checkpoint arms above.
                Err(error) => {
                    #[cfg(feature = "otel")]
                    bencher_otel::ApiMeter::increment(
                        bencher_otel::ApiCounter::ReplicaVerifyFailed,
                    );
                    if error.is_poison() {
                        return Err(error);
                    }
                    slog::error!(self.log, "Replica verification could not complete";
                        "error" => %error);
                    self.verify_retry_until = Some(now.saturating_add(VERIFY_RETRY_SECS));
                    progress.error = Some(error);
                },
            }
        }
        if self.snapshot_due(now) {
            self.snapshot = Some(SnapshotJob::ShipTail);
        }
        self.backoff.reset();
        self.backoff_until = None;
        Ok(progress)
    }

    /// Ship every complete committed transaction currently in the WAL,
    /// handling salt transitions first. Returns the number of segments
    /// shipped. A divergence (illegitimate salt change) schedules a new
    /// generation and returns 0. No-op while a snapshot is in flight (the
    /// snapshot machine runs its own ship pass).
    pub async fn ship_once(&mut self) -> Result<u64, SyncError> {
        if self.snapshot.is_some() {
            return Ok(0);
        }
        self.ship_pass().await
    }

    /// Run the checkpoint critical section (see `checkpoint.rs`). The app
    /// writer mutex is held for the WHOLE call, so app writers queue on the
    /// tokio mutex and never burn their `busy_timeout`.
    pub async fn checkpoint_once(&mut self) -> Result<CheckpointOutcome, SyncError> {
        if self.shadow {
            return Ok(CheckpointOutcome::SkippedShadow);
        }
        let Some(position) = self.position.clone() else {
            // Nothing is provably shipped without a bound position.
            return Ok(CheckpointOutcome::SkippedUnshipped);
        };
        let guard = Arc::clone(&self.db.writer).lock_owned().await;
        let mut conns = if let Some(conns) = self.conns.take() {
            conns
        } else {
            let db_path = self.db.db_path.clone();
            spawn_blocking(move || CheckpointConns::open(&db_path))
                .await
                .map_err(SyncError::Join)?
                .map_err(|error| SyncError::Checkpoint(error.into()))?
        };
        let wal_path = self.wal_path();
        let max_transaction_bytes = self.config.max_transaction_bytes;
        #[cfg(feature = "otel")]
        let critical_started = std::time::Instant::now();
        let (conns, outcome) = spawn_blocking(move || {
            let outcome =
                checkpoint_locked(&mut conns, &wal_path, &position, max_transaction_bytes);
            (conns, outcome)
        })
        .await
        .map_err(SyncError::Join)?;
        #[cfg(feature = "otel")]
        bencher_otel::ApiMeter::record(
            bencher_otel::ApiHistogram::ReplicaCriticalSectionDuration,
            critical_started.elapsed().as_secs_f64(),
        );
        let outcome = match outcome {
            // Only reuse the connections after a clean section: an error
            // may have left the lock connection inside a transaction, and
            // dropping it rolls back and releases everything.
            Ok(outcome) => {
                self.conns = Some(conns);
                outcome
            },
            Err(error) => return Err(SyncError::Checkpoint(error)),
        };
        if matches!(outcome, CheckpointOutcome::Completed) {
            // The salt change at the next WAL restart is now legitimate.
            self.epoch_checkpointed = true;
            if let Some(position) = self.position.clone() {
                // Crash window (churn, never loss): a crash AFTER the
                // checkpoint completes but BEFORE this meta store persists the
                // `epoch_shipped_through_checkpoint` flag leaves the meta
                // saying the epoch was NOT sealed. If the WAL then restarts
                // before the next boot, resume cannot prove the clean epoch
                // transition and forces a spurious full re-snapshot. This is
                // wasteful churn, not data loss (every frame was shipped
                // before the checkpoint, invariant I1), so it is tolerated
                // rather than guarded with a heavier commit protocol.
                self.store_meta_for(&position).await?;
            }
        }
        drop(guard);
        if matches!(
            outcome,
            CheckpointOutcome::Completed | CheckpointOutcome::Partial
        ) {
            // Partial counts for pacing: retry next interval, not sooner.
            self.last_checkpoint_secs = self.now_secs();
        }
        #[cfg(feature = "otel")]
        bencher_otel::ApiMeter::increment(match outcome {
            CheckpointOutcome::Completed => bencher_otel::ApiCounter::ReplicaCheckpoint,
            CheckpointOutcome::Partial => bencher_otel::ApiCounter::ReplicaCheckpointPartial,
            // Unshipped skips are the checkpoint-liveness alert signal: a
            // sustained-write workload can starve checkpoints (see
            // `checkpoint_due`), so they get their own counter.
            CheckpointOutcome::SkippedUnshipped => {
                bencher_otel::ApiCounter::ReplicaCheckpointSkippedUnshipped
            },
            CheckpointOutcome::SkippedBusy | CheckpointOutcome::SkippedShadow => {
                bencher_otel::ApiCounter::ReplicaCheckpointSkipped
            },
        });
        slog::debug!(self.log, "Checkpoint attempted"; "outcome" => format!("{outcome:?}"));
        Ok(outcome)
    }

    /// Run one restore-and-compare verification: prove the replica
    /// reproduces the source database at the shipped position (the shadow
    /// burn-in check and the post-cutover replacement for Litestream's
    /// `validation`).
    ///
    /// Choreography: drain the WAL tail with ship passes (no locks), then
    /// briefly gate writers (app mutex + `BEGIN IMMEDIATE` in sole mode) to
    /// pin a read snapshot at exactly the shipped position, release the
    /// gate, and compare fingerprints off-lock: the multi-second fingerprint
    /// and restore never block writes (invariant I5).
    ///
    /// Returns `Ok(None)` when verification cannot run right now (no bound
    /// position, a snapshot in flight, a busy stray writer, or frames that
    /// slipped in after the drain): `last_verify_secs` is left untouched, and
    /// the caller (`sync_once`) paces the retry so a persistent Busy/Unshipped
    /// pin does not re-drain every tick. On `Fail`, a new generation is
    /// triggered unless the current one is younger than six hours (rate limit).
    pub async fn verify_once(&mut self) -> Result<Option<VerifyReport>, SyncError> {
        if self.snapshot.is_some() {
            return Ok(None);
        }
        // Drain the committed tail so the pin lands at the true tip. Bounded:
        // a continuously committing writer is caught by the gate re-check.
        for _pass in 0u8..8 {
            if self.ship_once().await? == 0 {
                break;
            }
        }
        let Some(position) = self.position.clone() else {
            return Ok(None);
        };
        let guard = Arc::clone(&self.db.writer).lock_owned().await;
        let mut conns = if let Some(conns) = self.conns.take() {
            conns
        } else {
            let db_path = self.db.db_path.clone();
            spawn_blocking(move || CheckpointConns::open(&db_path))
                .await
                .map_err(SyncError::Join)?
                .map_err(|error| SyncError::Checkpoint(error.into()))?
        };
        let db_path = self.db.db_path.clone();
        let wal_path = self.wal_path();
        // The pin holds the write lock even in shadow mode: without it a
        // commit can slip between the unshipped-tail scan and the snapshot
        // pin, producing a spurious verification failure that would churn a
        // whole new generation. Litestream lock contention just yields Busy,
        // which retries next tick.
        let pin_position = position.clone();
        let max_transaction_bytes = self.config.max_transaction_bytes;
        let (conns, pin) = spawn_blocking(move || {
            let pin = pin_locked(
                &mut conns,
                &db_path,
                &wal_path,
                &pin_position,
                max_transaction_bytes,
            );
            (conns, pin)
        })
        .await
        .map_err(SyncError::Join)?;
        let pin = match pin {
            Ok(pin) => {
                self.conns = Some(conns);
                pin
            },
            Err(error) => {
                drop(guard);
                return Err(SyncError::Checkpoint(error));
            },
        };
        drop(guard);
        let pinned = match pin {
            PinOutcome::Pinned(pinned) => pinned,
            PinOutcome::Busy | PinOutcome::Unshipped => return Ok(None),
        };

        // Off-lock from here: writers proceed while we fingerprint, restore,
        // and compare.
        let fingerprint = spawn_blocking(move || {
            let fingerprint = fingerprint_database(&pinned);
            // Dropping the connection releases the pinned read snapshot.
            drop(pinned);
            fingerprint
        })
        .await
        .map_err(SyncError::Join)?
        .map_err(SyncError::Verify)?;

        let scratch = self.verify_scratch_dir();
        let result =
            verify_against_replica(&self.log, &self.storage, &position, &fingerprint, &scratch)
                .await;
        // Reclaim the DB-sized scratch copy off the async executor, on BOTH
        // the success and error paths: a `?` on the result above would leak a
        // full database copy until the next run, and `remove_dir_all` blocks.
        let cleanup = scratch.clone();
        drop(spawn_blocking(move || std::fs::remove_dir_all(&cleanup)).await);
        let report = result.map_err(SyncError::Verify)?;
        self.record_verify_outcome(&position, &report);
        Ok(Some(report))
    }

    /// Log and meter one COMPLETED verification, and retrigger a fresh
    /// generation on divergence (outside the retrigger window).
    fn record_verify_outcome(&mut self, position: &Position, report: &VerifyReport) {
        let now = self.now_secs();
        self.last_verify_secs = now;
        match report {
            VerifyReport::Pass => {
                #[cfg(feature = "otel")]
                bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::ReplicaVerifyPass);
                slog::info!(self.log, "Replica verification passed";
                    "generation" => position.generation.as_str(),
                    "epoch" => position.epoch,
                    "offset" => position.offset,
                );
            },
            VerifyReport::Fail { detail } => {
                #[cfg(feature = "otel")]
                bencher_otel::ApiMeter::increment(
                    bencher_otel::ApiCounter::ReplicaVerifyDivergence,
                );
                slog::error!(self.log, "Replica verification FAILED";
                    "generation" => position.generation.as_str(),
                    "epoch" => position.epoch,
                    "offset" => position.offset,
                    "detail" => detail,
                );
                if now.saturating_sub(self.generation_birth_secs) >= VERIFY_RETRIGGER_MIN_SECS {
                    self.pending_new_generation = true;
                } else {
                    slog::warn!(
                        self.log,
                        "Verification failure within the retrigger window: not forcing another generation yet"
                    );
                }
            },
        }
    }

    /// Whether a scheduled verification is due.
    fn verify_due(&self, now: i64) -> bool {
        let Some(interval) = self.config.verification_interval else {
            return false;
        };
        if self.snapshot.is_some() || self.position.is_none() {
            return false;
        }
        // Respect the independent verify retry pacing after a run that could
        // not complete, so a broken verify does not re-run every tick.
        if let Some(until) = self.verify_retry_until
            && now < until
        {
            return false;
        }
        let interval = i64::try_from(interval.as_secs()).unwrap_or(i64::MAX);
        now.saturating_sub(self.last_verify_secs) >= interval
    }

    /// Scratch directory for verification restores, next to the database.
    fn verify_scratch_dir(&self) -> Utf8PathBuf {
        let parent = self
            .db
            .db_path
            .parent()
            .map_or_else(|| Utf8PathBuf::from("."), Utf8Path::to_path_buf);
        parent.join(".replica-verify")
    }

    /// Advance the snapshot state machine by one bounded unit of work,
    /// starting a new generation snapshot when none is active. On error the
    /// in-flight upload is aborted and a retrigger is scheduled; the caller
    /// arms the backoff.
    pub async fn snapshot_step(&mut self) -> Result<SnapshotStatus, SyncError> {
        let job = self.snapshot.take().unwrap_or(SnapshotJob::ShipTail);
        match self.run_snapshot_step(job).await {
            Ok(Some(next)) => {
                self.snapshot = Some(next);
                Ok(SnapshotStatus::InProgress)
            },
            Ok(None) => {
                #[cfg(feature = "otel")]
                bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::ReplicaSnapshot);
                Ok(SnapshotStatus::Finished)
            },
            Err(error) => {
                #[cfg(feature = "otel")]
                bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::ReplicaSnapshotFailed);
                // Retrigger: the WAL keeps buffering, nothing is lost.
                self.pending_new_generation = true;
                Err(error)
            },
        }
    }

    /// Delete generations beyond retention plus stale incomplete ones.
    /// Whole-generation deletes only; the current (and any in-flight)
    /// generation is never pruned. Failures are safe to retry at the next
    /// snapshot.
    pub async fn prune_once(&mut self) -> Result<(), SyncError> {
        let components = self.storage.list_dirs(GENERATIONS_PREFIX).await?;
        let mut complete = Vec::new();
        let mut incomplete = Vec::new();
        for component in &components {
            let Some(generation) = GenerationId::parse(component) else {
                slog::warn!(self.log, "Skipping foreign directory under generations/";
                    "component" => component.as_str());
                continue;
            };
            match self.storage.get(&snapshot_meta_key(&generation)).await {
                Ok(_bytes) => complete.push(generation),
                Err(StorageError::NotFound { .. }) => incomplete.push(generation),
                Err(error) => return Err(error.into()),
            }
        }
        let protected = self.protected_generations();
        // `list_dirs` is sorted, so `complete` is oldest first.
        let retention = usize::try_from(self.config.retention_generations).unwrap_or(usize::MAX);
        let prune_count = complete.len().saturating_sub(retention.max(1));
        for generation in complete.iter().take(prune_count) {
            if protected.contains(generation) {
                continue;
            }
            slog::info!(self.log, "Pruning generation beyond retention";
                "generation" => generation.as_str());
            // Delete the `snapshot.json` marker FIRST, as a single explicit
            // delete, before the unordered `delete_prefix` batch. Batch
            // deletion order is unspecified, so a crash mid-prune could
            // otherwise remove `snapshot.db.zst` while leaving the marker,
            // producing a marker-without-body generation that resume/restore
            // would trust. Marker-first leaves only an invisible, markerless
            // generation instead (pruned later as stale-incomplete).
            self.storage.delete(&snapshot_meta_key(generation)).await?;
            self.storage
                .delete_prefix(&generation_prefix(generation))
                .await?;
        }
        // Incomplete generations older than the cutoff are crashed
        // snapshots; lexicographic order equals chronological order.
        let cutoff_secs = self.now_secs().saturating_sub(STALE_INCOMPLETE_SECS);
        let Ok(cutoff_time) = bencher_json::DateTime::try_from(cutoff_secs) else {
            return Ok(());
        };
        let cutoff = GenerationId::new(cutoff_time, 0);
        for generation in &incomplete {
            if protected.contains(generation) || *generation >= cutoff {
                continue;
            }
            slog::info!(self.log, "Pruning stale incomplete generation (crashed snapshot)";
                "generation" => generation.as_str());
            self.storage
                .delete_prefix(&generation_prefix(generation))
                .await?;
        }
        Ok(())
    }

    /// Shutdown ship of the remaining committed tail, `deadline`-bounded.
    ///
    /// The `deadline` bounds ONLY the ship loop. After a COMPLETE drain in
    /// sole mode a final checkpoint then runs (unbounded, after the deadline):
    /// it seals the epoch through a completed checkpoint so the next boot can
    /// trust the WAL-less state and resume in place instead of churning a
    /// whole new generation on every graceful restart (see the checkpoint
    /// rationale below). The checkpoint is crash-safe if it is truncated: it
    /// only ever backfills already-shipped frames (invariant I1), so an
    /// interrupted checkpoint costs a re-snapshot, never data.
    ///
    /// On the deadline (an incomplete drain) or a storage failure NO
    /// checkpoint runs and the remaining frames simply stay in the local WAL:
    /// nothing is lost, only lagged, and the next boot resumes by salt match.
    pub async fn final_sync(&mut self, deadline: Duration) -> Result<(), SyncError> {
        // An in-flight snapshot would make ship_once a silent no-op (and its
        // half generation is invisible to restore anyway): abort it so the
        // tail ships into the CURRENT lineage. The next process re-snapshots
        // when due.
        if let Some(job) = self.snapshot.take() {
            slog::info!(
                self.log,
                "Aborting the in-flight snapshot for shutdown; it will be retaken after restart"
            );
            match job {
                SnapshotJob::ShipTail | SnapshotJob::CreateGeneration => {},
                SnapshotJob::Copying(copy) => self.abort_upload(copy.upload).await,
                SnapshotJob::Finalize(finalize) => self.abort_upload(finalize.upload).await,
            }
            self.pending_new_generation = true;
        }
        let drained = tokio::time::timeout(deadline, async {
            loop {
                match self.ship_once().await {
                    Ok(0) => return true,
                    Ok(_segments) => {},
                    Err(error) => {
                        slog::warn!(self.log,
                            "Final sync ship failed; remaining WAL frames stay local (no data loss)";
                            "error" => %error);
                        return false;
                    },
                }
            }
        })
        .await;
        match drained {
            Ok(true) => {
                slog::info!(self.log, "Final sync complete: WAL tail fully shipped");
                // Checkpoint AFTER the full ship (never before: invariant
                // I1), so the meta records a completed checkpoint. This is
                // load-bearing: when the last in-process connection closes,
                // `SQLite` checkpoints and DELETES the WAL file on its own;
                // without the meta proof the next boot could not trust the
                // WAL-less state and would churn a whole new generation on
                // every graceful restart.
                if !self.shadow {
                    match self.checkpoint_once().await {
                        Ok(outcome) => {
                            slog::info!(self.log, "Shutdown checkpoint";
                                "outcome" => format!("{outcome:?}"));
                        },
                        Err(error) => {
                            slog::warn!(self.log, "Shutdown checkpoint failed; the next boot may re-snapshot";
                                "error" => %error);
                        },
                    }
                }
            },
            Ok(false) => {},
            Err(_elapsed) => slog::warn!(
                self.log,
                "Final sync deadline reached; remaining WAL frames stay local (no data loss: the next boot resumes by salt match)"
            ),
        }
        Ok(())
    }

    /// Request a new-generation snapshot at the next opportunity.
    pub fn trigger_snapshot(&mut self) {
        self.pending_new_generation = true;
    }

    /// Current shipping position, when one is bound.
    #[must_use]
    pub fn position(&self) -> Option<&Position> {
        self.position.as_ref()
    }

    /// The generation currently being written to (or awaited).
    #[must_use]
    pub fn generation(&self) -> Option<&GenerationId> {
        self.position
            .as_ref()
            .map(|position| &position.generation)
            .or_else(|| self.awaiting.as_ref().map(|(generation, _)| generation))
    }

    /// The engine's observable state.
    #[must_use]
    pub fn state(&self) -> EngineState {
        if self.snapshot.is_some() {
            EngineState::Snapshotting
        } else if self.position.is_some() {
            EngineState::Streaming
        } else if self.awaiting.is_some() {
            EngineState::AwaitingEpoch
        } else {
            EngineState::PendingSnapshot
        }
    }

    /// The storage backend (tests reach the fault-injection wrapper here).
    #[must_use]
    pub fn storage(&self) -> &ReplicaStorage {
        &self.storage
    }

    fn now_secs(&self) -> i64 {
        self.clock.timestamp()
    }

    fn wal_path(&self) -> Utf8PathBuf {
        Utf8PathBuf::from(format!("{}-wal", self.db.db_path))
    }

    /// Arm the capped exponential backoff after a failed tick.
    fn arm_backoff(&mut self, now: i64) {
        let delay = i64::try_from(self.backoff.next_delay().as_secs()).unwrap_or(i64::MAX);
        self.backoff_until = Some(now.saturating_add(delay));
    }

    /// Resolve the resume position (see the module docs).
    async fn resume(&mut self) -> Result<(), SyncError> {
        let Some(tip) = self.replica_tip().await? else {
            slog::info!(
                self.log,
                "Replica has no valid generation; scheduling the first snapshot"
            );
            self.pending_new_generation = true;
            return Ok(());
        };
        self.generation_floor = Some(tip.generation.clone());
        let wal_path = self.wal_path();
        let header_state = spawn_blocking(move || read_wal_header_state(&wal_path))
            .await
            .map_err(SyncError::Join)??;
        let db_path = self.db.db_path.clone();
        let meta = spawn_blocking(move || ReplicaMeta::load(&db_path))
            .await
            .map_err(SyncError::Join)??;
        match header_state {
            WalHeaderState::Present { header, .. } => {
                self.resume_with_header(tip, header, meta.as_ref()).await
            },
            WalHeaderState::Missing => {
                self.resume_without_wal(tip, meta.as_ref());
                Ok(())
            },
            WalHeaderState::Unreadable(error) => {
                slog::warn!(self.log, "Local WAL header unreadable at resume; treating as absent";
                    "error" => %error);
                self.resume_without_wal(tip, meta.as_ref());
                Ok(())
            },
        }
    }

    /// Resume steps 3 and 4: a local WAL header exists.
    async fn resume_with_header(
        &mut self,
        tip: ReplicaTip,
        header: WalHeader,
        meta: Option<&ReplicaMeta>,
    ) -> Result<(), SyncError> {
        // A shadow<->sole config flip invalidates the lineage regardless of
        // how well the WAL lines up: cutover forces one clean generation.
        if let Some(meta) = meta
            && meta.shadow != self.shadow
        {
            self.divergence(
                &tip,
                "shadow mode changed since the last run; forcing a clean generation",
            );
            return Ok(());
        }
        if header.salt == tip.salt {
            // Step 3: same salt cycle; verify the local chain reaches the
            // replica end and recover the running checksum there.
            let checksum = if tip.end == 0 {
                Some((0, 0))
            } else {
                let wal_path = self.wal_path();
                let end = tip.end;
                spawn_blocking(move || checksum_at_offset(&wal_path, end))
                    .await
                    .map_err(SyncError::Join)??
            };
            let Some(checksum) = checksum else {
                self.divergence(
                    &tip,
                    "local WAL chain does not reach the replica end (replica ahead or broken chain)",
                );
                return Ok(());
            };
            // CONTENT proof, not just length: with `synchronous = NORMAL` a
            // power loss can rewind the local WAL below the shipped offset
            // (committed frames lived only in the page cache); if writers
            // then re-extend it past `tip.end` with DIFFERENT transactions,
            // the chain reaches `tip.end` again but the content forks from
            // what the replica stored. The cumulative WAL checksum at
            // `tip.end` fingerprints the whole prefix, so comparing it
            // against the replica tip segment's stored value detects the
            // fork (invariant I6: never guess).
            if tip.end > 0 && !self.tip_content_matches(&tip, &header, checksum).await? {
                self.divergence(
                    &tip,
                    "local WAL content at the shipped offset differs from the replica (post-crash rewind fork)",
                );
                return Ok(());
            }
            self.epoch_checkpointed = meta_matches(meta, &tip, self.shadow);
            slog::info!(self.log, "Resuming by salt match";
                "generation" => tip.generation.as_str(),
                "epoch" => tip.epoch, "offset" => tip.end);
            self.position = Some(Position {
                generation: tip.generation,
                epoch: tip.epoch,
                salt: tip.salt,
                offset: tip.end,
                checksum,
            });
            return Ok(());
        }
        // Step 4: salt mismatch; only an exact meta proof avoids a
        // re-snapshot. The proof must include salt CONTINUITY: `SQLite`
        // increments salt1 by exactly one per WAL restart, so a larger jump
        // means an external writer buried at least one whole cycle
        // (checkpointed then overwrote commits we never shipped) while the
        // replicator was down.
        let salt_continuous = meta.is_some_and(|meta| header.salt.0 == meta.salt1.wrapping_add(1));
        if meta_matches(meta, &tip, self.shadow) && salt_continuous {
            let (epoch, note) = if tip.end == 0 {
                // Nothing ever shipped into the tip epoch: rebind it in
                // place so epochs stay contiguous and non-empty.
                (tip.epoch, "rebinding empty epoch to the local WAL salts")
            } else {
                (
                    tip.epoch.saturating_add(1),
                    "meta-verified resume as the next epoch",
                )
            };
            slog::info!(self.log, "Resuming after WAL restart"; "note" => note,
                "generation" => tip.generation.as_str(), "epoch" => epoch);
            self.epoch_checkpointed = false;
            self.position = Some(Position {
                generation: tip.generation,
                epoch,
                salt: header.salt,
                offset: 0,
                checksum: (0, 0),
            });
        } else {
            self.divergence(&tip, "local WAL salts do not match the replica and the meta cannot prove a clean, salt-continuous epoch transition");
        }
        Ok(())
    }

    /// Whether the local WAL's running checksum at the replica tip offset
    /// matches the checksum stored in the replica tip segment's final frame
    /// header. Downloads and decompresses one segment (bounded by the
    /// segment size cap); runs only at resume.
    ///
    /// Conservative: any anomaly (segment gone, undecodable, or too short)
    /// reports a mismatch, so resume diverges instead of guessing.
    async fn tip_content_matches(
        &mut self,
        tip: &ReplicaTip,
        header: &WalHeader,
        local_checksum: (u32, u32),
    ) -> Result<bool, SyncError> {
        let Some(segment) = &tip.last_segment else {
            // `tip.end > 0` without a segment cannot happen (the end comes
            // from the segment key); treat it as a mismatch if it does.
            return Ok(false);
        };
        let key = segment_key(&tip.generation, segment);
        let compressed = match self.storage.get(&key).await {
            Ok(bytes) => bytes,
            Err(StorageError::NotFound { .. }) => {
                slog::warn!(self.log, "Replica tip segment vanished during resume"; "key" => &key);
                return Ok(false);
            },
            Err(error) => return Err(error.into()),
        };
        let raw = match spawn_blocking(move || crate::segment::decompress_segment(&compressed))
            .await
            .map_err(SyncError::Join)?
        {
            Ok(raw) => raw,
            Err(error) => {
                slog::warn!(self.log, "Replica tip segment undecodable during resume";
                    "key" => &key, "error" => %error);
                return Ok(false);
            },
        };
        let Some(stored) = segment_tail_checksum(&raw, header.frame_size()) else {
            slog::warn!(self.log, "Replica tip segment too short during resume"; "key" => &key);
            return Ok(false);
        };
        Ok(stored == local_checksum)
    }

    /// Resume step 5: no usable local WAL.
    ///
    /// Documented limitation: a brand-new WAL file gets RANDOM salts (salt1
    /// increments only on the restart of an EXISTING WAL), so salt
    /// continuity cannot be checked here. An external writer that buries a
    /// cycle AND removes the WAL file while the replicator is down is
    /// undetectable at resume; the periodic restore-and-compare
    /// verification is the backstop for that case.
    fn resume_without_wal(&mut self, tip: ReplicaTip, meta: Option<&ReplicaMeta>) {
        if meta_matches(meta, &tip, self.shadow) {
            let epoch = if tip.end == 0 {
                tip.epoch
            } else {
                tip.epoch.saturating_add(1)
            };
            slog::info!(self.log, "Local WAL absent; awaiting first frames to bind the epoch";
                "generation" => tip.generation.as_str(), "epoch" => epoch);
            self.awaiting = Some((tip.generation, epoch));
        } else {
            self.divergence(
                &tip,
                "local WAL absent and the meta cannot prove a full ship through checkpoint",
            );
        }
    }

    /// Record a divergence: the replica is the source of truth (I6), so any
    /// unprovable mismatch resolves to a new generation, never guessing.
    fn divergence(&mut self, tip: &ReplicaTip, reason: &'static str) {
        if self.shadow {
            // The shadow replica is disposable; boundary losses are
            // expected while Litestream owns checkpoints.
            slog::info!(self.log, "Shadow replica divergence; starting a new generation";
                "reason" => reason, "generation" => tip.generation.as_str());
        } else {
            slog::error!(self.log, "Replica divergence; starting a new generation";
                "reason" => reason, "generation" => tip.generation.as_str());
        }
        #[cfg(feature = "otel")]
        bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::ReplicaDivergence);
        self.position = None;
        self.awaiting = None;
        self.pending_new_generation = true;
    }

    /// Locate the replica tip from a LIST alone: the latest generation with
    /// a `snapshot.json`, and its last segment (or the snapshot boundary
    /// when no segment has shipped).
    async fn replica_tip(&mut self) -> Result<Option<ReplicaTip>, SyncError> {
        let components = self.storage.list_dirs(GENERATIONS_PREFIX).await?;
        for component in components.iter().rev() {
            let Some(generation) = GenerationId::parse(component) else {
                slog::warn!(self.log, "Skipping foreign directory under generations/";
                    "component" => component.as_str());
                continue;
            };
            let bytes = match self.storage.get(&snapshot_meta_key(&generation)).await {
                Ok(bytes) => bytes,
                Err(StorageError::NotFound { .. }) => continue,
                Err(error) => return Err(error.into()),
            };
            let Ok(snapshot) = SnapshotMeta::from_bytes(&bytes) else {
                slog::error!(self.log, "Skipping generation with unparsable snapshot.json";
                    "generation" => generation.as_str());
                continue;
            };
            let wal_prefix = format!("{}{WAL_DIR}/", generation_prefix(&generation));
            let keys = self.storage.list(&wal_prefix).await?;
            // Keys sort lexicographically == (epoch, offset) numerically,
            // so the last parseable key is the tip.
            let mut tip_segment = None;
            for key in &keys {
                if let Some((parsed, segment)) = parse_segment_key(key)
                    && parsed == generation
                {
                    tip_segment = Some(segment);
                }
            }
            return Ok(Some(match tip_segment {
                Some(segment) => ReplicaTip {
                    generation,
                    epoch: segment.epoch,
                    salt: segment.salt,
                    end: segment.end,
                    last_segment: Some(segment),
                },
                None => ReplicaTip {
                    generation,
                    epoch: snapshot.wal_boundary.epoch,
                    salt: (snapshot.wal_boundary.salt1, snapshot.wal_boundary.salt2),
                    end: 0,
                    last_segment: None,
                },
            }));
        }
        Ok(None)
    }

    /// One full ship pass over the local WAL (used by both `ship_once` and
    /// the snapshot's `ShipTail` phase).
    async fn ship_pass(&mut self) -> Result<u64, SyncError> {
        let wal_path = self.wal_path();
        let header_state = {
            let wal_path = wal_path.clone();
            spawn_blocking(move || read_wal_header_state(&wal_path))
                .await
                .map_err(SyncError::Join)??
        };
        let (header, raw_header) = match header_state {
            WalHeaderState::Present { header, raw } => (header, raw),
            WalHeaderState::Missing => return Ok(0),
            WalHeaderState::Unreadable(error) => {
                // Possibly a torn concurrent header write; retry next tick.
                slog::warn!(self.log, "Local WAL header unreadable; skipping this ship pass";
                    "error" => %error);
                return Ok(0);
            },
        };
        // Bind an awaited epoch to the first observed salt cycle.
        if let Some((generation, epoch)) = self.awaiting.take() {
            slog::info!(self.log, "Binding awaited epoch to the local WAL salts";
                "generation" => generation.as_str(), "epoch" => epoch);
            self.epoch_checkpointed = false;
            self.position = Some(Position {
                generation,
                epoch,
                salt: header.salt,
                offset: 0,
                checksum: (0, 0),
            });
        }
        let Some(mut position) = self.position.clone() else {
            return Ok(0);
        };
        if header.salt != position.salt && !self.transition_epoch(&mut position, &header) {
            return Ok(0);
        }
        // An oversized-drain generation pins epoch 0 at the boundary until the
        // WAL restarts (the restart is handled by `transition_epoch` above,
        // which rebinds and clears this state). Until then, ship nothing: the
        // below-boundary frames are already in the snapshot, and any new
        // committed frames above the boundary cannot ship into epoch 0 (its
        // first segment must start at 0). They are backfilled by the draining
        // checkpoint and captured by the next snapshot.
        if self.oversized_drain == OversizedDrain::AwaitingRestart {
            return Ok(0);
        }
        // Discard-scan the committed extent from the current position FIRST
        // (page bytes discarded), then retention-read only up to that extent.
        // Without this, every ship poll re-buffered any large open
        // transaction past the last commit just to discover there was nothing
        // new to ship (a multi-GiB re-allocation each tick). Bounded per pass:
        // the scan stops after roughly a full pass worth of committed data,
        // and the next pass resumes where this one left off.
        let (scan_offset, scan_checksum) = if position.offset == 0 {
            (WAL_HEADER_SIZE, header.checksum)
        } else {
            (position.offset, position.checksum)
        };
        let scan_budget = MAX_SEGMENTS_PER_PASS.saturating_mul(SEGMENT_MAX_BYTES);
        let max_txn_bytes = self.config.max_transaction_bytes;
        let extent = {
            let wal_path = wal_path.clone();
            let scanned = spawn_blocking(move || {
                scan_committed_end(
                    &wal_path,
                    header,
                    scan_offset,
                    scan_checksum,
                    scan_budget,
                    max_txn_bytes,
                )
            })
            .await
            .map_err(SyncError::Join)?;
            match scanned {
                Ok(extent) => extent,
                // A committed transaction beyond the bound cannot ship (restore
                // would reject the oversized segment) and, being durable, cannot
                // be split retroactively. Capture it with a re-snapshot and
                // drain the WAL instead of wedging the ship path.
                Err(error) if error.is_transaction_too_large() => {
                    self.diverge_oversized();
                    return Ok(0);
                },
                Err(error) => return Err(error),
            }
        };
        if extent <= scan_offset {
            return Ok(0);
        }
        self.ship_up_to_extent(&wal_path, header, &raw_header, &mut position, extent)
            .await
    }

    /// Retention-read and ship committed segments from the current position up
    /// to `extent` (the boundary the discard scan found), at most
    /// [`MAX_SEGMENTS_PER_PASS`]. Each read is capped at the extent so the
    /// final chunk stops exactly on the last committed frame and never buffers
    /// the uncommitted tail beyond it.
    async fn ship_up_to_extent(
        &mut self,
        wal_path: &Utf8Path,
        header: WalHeader,
        raw_header: &[u8; 32],
        position: &mut Position,
        extent: u64,
    ) -> Result<u64, SyncError> {
        let mut shipped = 0u64;
        while shipped < MAX_SEGMENTS_PER_PASS {
            let (offset, checksum) = if position.offset == 0 {
                (WAL_HEADER_SIZE, header.checksum)
            } else {
                (position.offset, position.checksum)
            };
            if offset >= extent {
                break;
            }
            let max_bytes = SEGMENT_MAX_BYTES.min(extent.saturating_sub(offset));
            let chunk = {
                let wal_path = wal_path.to_owned();
                spawn_blocking(move || {
                    scan_next_chunk(&wal_path, header, offset, checksum, max_bytes)
                })
                .await
                .map_err(SyncError::Join)??
            };
            let Some(chunk) = chunk else { break };
            // Defensive: the discard scan already intercepts an oversized
            // transaction before the retaining read buffers it, but should one
            // reach here, treat it the same (re-snapshot, never a hard error).
            if let Err(error) = self.ship_chunk(chunk, position, raw_header).await {
                if error.is_transaction_too_large() {
                    self.diverge_oversized();
                    return Ok(shipped);
                }
                return Err(error);
            }
            shipped = shipped.saturating_add(1);
        }
        Ok(shipped)
    }

    /// Ship one committed chunk: refuse an oversized transaction, compress the
    /// segment (prepending the 32-byte WAL header for the first segment of an
    /// epoch, so restore can rebuild a `-wal` file by decompress-and-
    /// concatenate), upload it, advance `position`, and persist the meta.
    async fn ship_chunk(
        &mut self,
        chunk: CommittedChunk,
        position: &mut Position,
        raw_header: &[u8; 32],
    ) -> Result<(), SyncError> {
        // A single transaction is always shipped whole (commit atomicity beats
        // the size cap), but a transaction beyond `max_transaction_bytes`
        // (defaulting to the restore decompression bound) cannot ship: restore
        // would reject the oversized segment. Signal it; `ship_pass` converts
        // it into an oversized-transaction re-snapshot rather than shipping
        // anything corrupt. The discard scan normally intercepts it first, so
        // this is the defensive backstop.
        let chunk_bytes = u64::try_from(chunk.bytes.len()).unwrap_or(u64::MAX);
        let max_transaction_bytes = self.config.max_transaction_bytes;
        if chunk_bytes.saturating_add(WAL_HEADER_SIZE) > max_transaction_bytes {
            return Err(SyncError::TransactionTooLarge {
                bytes: chunk_bytes,
                max_bytes: max_transaction_bytes,
            });
        }
        let start = if position.offset == 0 {
            0
        } else {
            chunk.start_offset
        };
        let key = segment_key(
            &position.generation,
            &SegmentKey {
                epoch: position.epoch,
                salt: position.salt,
                start,
                end: chunk.end_offset,
            },
        );
        let CommittedChunk {
            end_offset,
            checksum_at_end,
            bytes,
            ..
        } = chunk;
        let raw = if start == 0 {
            let mut with_header = raw_header.to_vec();
            with_header.extend_from_slice(&bytes);
            with_header
        } else {
            bytes
        };
        let compressed = spawn_blocking(move || compress_segment(&raw))
            .await
            .map_err(SyncError::Join)??;
        self.storage.put(&key, Bytes::from(compressed)).await?;
        #[cfg(feature = "otel")]
        bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::ReplicaSegmentShip);
        position.offset = end_offset;
        position.checksum = checksum_at_end;
        self.epoch_checkpointed = false;
        self.position = Some(position.clone());
        self.store_meta_for(position).await?;
        slog::debug!(self.log, "Shipped WAL segment"; "key" => &key);
        Ok(())
    }

    /// Handle a WAL salt change before shipping. Returns whether shipping
    /// may proceed; `false` means a divergence was recorded.
    fn transition_epoch(&mut self, position: &mut Position, header: &WalHeader) -> bool {
        if self.oversized_drain == OversizedDrain::AwaitingRestart {
            // The oversized-drain generation pinned epoch 0 at the boundary
            // (offset > 0) waiting for exactly this restart: the checkpoint
            // (ours in sole mode, Litestream's in shadow mode) backfilled the
            // WAL below the boundary and a writer has now restarted it. REBIND
            // epoch 0 to the fresh cycle at offset 0 (rather than advancing to
            // epoch+1, which would leave epoch 0 with no segments and break
            // restore's epoch contiguity); the oversized transaction lives on
            // only in the snapshot body now, never a shipped segment.
            slog::info!(self.log, "Oversized-drain complete: rebinding epoch 0 to the restarted WAL cycle";
                "epoch" => position.epoch);
            position.salt = header.salt;
            position.offset = 0;
            position.checksum = (0, 0);
            self.oversized_drain = OversizedDrain::Idle;
            self.epoch_checkpointed = false;
            self.position = Some(position.clone());
            return true;
        }
        if position.offset == 0 {
            // Nothing shipped in this epoch yet: rebind it to the new
            // cycle (the epoch-0 lazy binding rule; also covers any epoch
            // start). The vanished cycle held nothing beyond what a full
            // backfill already captured.
            if self.shadow {
                slog::warn!(self.log, "Shadow: rebinding epoch to a new WAL cycle (boundary unverified)";
                    "epoch" => position.epoch);
            } else {
                slog::info!(self.log, "Rebinding unshipped epoch to the new WAL salts";
                    "epoch" => position.epoch);
            }
            position.salt = header.salt;
            position.checksum = (0, 0);
            self.epoch_checkpointed = false;
            self.position = Some(position.clone());
            true
        } else if self.shadow {
            if !self.epoch_checkpointed {
                // Commits between the last shadow tick and a
                // Litestream-driven restart can be lost from the shadow
                // replica; the daily verify catches any divergence.
                slog::warn!(self.log, "Shadow: WAL restarted without our checkpoint; epoch boundary unverified";
                    "epoch" => position.epoch);
                #[cfg(feature = "otel")]
                bencher_otel::ApiMeter::increment(
                    bencher_otel::ApiCounter::ReplicaShadowUnverifiedBoundary,
                );
            }
            position.epoch = position.epoch.saturating_add(1);
            position.salt = header.salt;
            position.offset = 0;
            position.checksum = (0, 0);
            self.epoch_checkpointed = false;
            self.position = Some(position.clone());
            true
        } else if self.epoch_checkpointed {
            // `SQLite` increments salt1 by exactly one per WAL restart
            // (`walRestartHdr`). A larger jump proves at least one WHOLE
            // cycle was written and buried (checkpointed then overwritten)
            // by an external writer: its commits never shipped, so the
            // lineage is broken (invariant I6: never guess).
            //
            // Residual limitation (same as the resume path, see
            // `resume_without_wal`): this only catches jumps GREATER than one.
            // A SINGLE external checkpoint-plus-restart increments salt1 by
            // exactly one, indistinguishable from a legitimate restart of our
            // own fully-shipped, fully-backfilled WAL, so if it buried frames
            // committed after our last shipped frame those are silently lost
            // from the replica. The periodic restore-and-compare verification
            // is the backstop for that case.
            if header.salt.0 != position.salt.0.wrapping_add(1) {
                self.transition_divergence(
                    position,
                    "WAL restarted more than once since the last shipped frame (buried cycle)",
                );
                return false;
            }
            slog::info!(self.log, "WAL restarted after a completed checkpoint; starting the next epoch";
                "epoch" => position.epoch.saturating_add(1));
            position.epoch = position.epoch.saturating_add(1);
            position.salt = header.salt;
            position.offset = 0;
            position.checksum = (0, 0);
            self.epoch_checkpointed = false;
            self.position = Some(position.clone());
            true
        } else {
            self.transition_divergence(
                position,
                "WAL restarted with unshipped frames (salt change without our checkpoint)",
            );
            false
        }
    }

    /// Record a mid-stream divergence discovered during an epoch transition.
    fn transition_divergence(&mut self, position: &Position, reason: &'static str) {
        slog::error!(self.log, "Replica divergence; starting a new generation";
            "reason" => reason,
            "epoch" => position.epoch, "shipped_offset" => position.offset);
        #[cfg(feature = "otel")]
        bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::ReplicaDivergence);
        self.position = None;
        self.awaiting = None;
        self.pending_new_generation = true;
    }

    /// Handle an oversized COMMITTED transaction found in the ship path.
    ///
    /// The transaction is already durable in the source, so "split the write"
    /// cannot apply retroactively and shipping it is impossible (restore
    /// rejects a segment beyond the decompression bound). Force a whole-
    /// database re-snapshot to capture its state, then drain the WAL below the
    /// boundary via a checkpoint (the snapshot IS the shipped state for those
    /// frames): [`Self::epoch0_bind_offset`] pins the new position at the
    /// boundary ([`OversizedDrain::AwaitingRestart`]) so the ship path stops
    /// re-hitting the transaction and a normal checkpoint can restart the WAL.
    ///
    /// Rate-limited: the drain normally breaks the loop, but a drain that
    /// cannot complete (a pinned reader, or a shadow replica awaiting
    /// Litestream) must not mint a fresh generation every tick.
    fn diverge_oversized(&mut self) {
        let now = self.now_secs();
        if let Some(until) = self.oversized_retrigger_until
            && now < until
        {
            return;
        }
        slog::error!(self.log,
            "A committed transaction exceeds the shippable WAL bound; forcing a re-snapshot to capture it and draining the WAL below the boundary (split the write to avoid recurrence)";
            "max_transaction_bytes" => self.config.max_transaction_bytes);
        #[cfg(feature = "otel")]
        bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::ReplicaDivergence);
        self.position = None;
        self.awaiting = None;
        self.pending_new_generation = true;
        self.oversized_drain = OversizedDrain::Pending;
        self.oversized_retrigger_until = Some(now.saturating_add(OVERSIZED_RETRIGGER_MIN_SECS));
    }

    /// Where a freshly finalized generation binds epoch 0, as
    /// `(offset, checksum)`. Normally offset 0 (re-ship the WAL from the
    /// start). For an oversized-transaction re-snapshot ([`OversizedDrain::Pending`])
    /// it pins epoch 0 at the boundary and advances to
    /// [`OversizedDrain::AwaitingRestart`] so the ship path stops re-hitting the
    /// unshippable transaction and a normal checkpoint can backfill and restart
    /// the WAL. A stale drain state from any other finalize is reset to
    /// [`OversizedDrain::Idle`].
    async fn epoch0_bind_offset(
        &mut self,
        generation: &GenerationId,
        boundary_offset: u64,
    ) -> Result<(u64, (u32, u32)), SyncError> {
        if self.oversized_drain != OversizedDrain::Pending {
            self.oversized_drain = OversizedDrain::Idle;
            return Ok((0, (0, 0)));
        }
        self.oversized_drain = OversizedDrain::Idle;
        let recovered = if boundary_offset == 0 {
            Some((0, 0))
        } else {
            let wal_path = self.wal_path();
            spawn_blocking(move || checksum_at_offset(&wal_path, boundary_offset))
                .await
                .map_err(SyncError::Join)??
        };
        if let Some(checksum) = recovered {
            self.oversized_drain = OversizedDrain::AwaitingRestart;
            slog::info!(self.log, "Oversized-drain: pinning epoch 0 at the boundary to drain the WAL";
                "generation" => generation.as_str(), "boundary" => boundary_offset);
            Ok((boundary_offset, checksum))
        } else {
            // The local WAL no longer reaches the boundary (a concurrent
            // change); fall back to a plain epoch-0 bind. The oversized
            // transaction is re-detected and the drain retried (rate-limited).
            slog::warn!(self.log, "Oversized-drain boundary checksum unavailable; deferring the drain";
                "generation" => generation.as_str());
            Ok((0, (0, 0)))
        }
    }

    /// Whether a checkpoint should run this tick.
    ///
    /// Checkpoint liveness is not guaranteed under a continuously committing
    /// writer: each attempt re-verifies the tail under `BEGIN IMMEDIATE` and
    /// defers (`SkippedUnshipped`) whenever a fresh commit has landed past the
    /// shipped position (invariant I1), so sustained write traffic can starve
    /// checkpoints and let the WAL grow. The `ReplicaCheckpointSkippedUnshipped`
    /// counter is the alert signal for that condition; a persistently
    /// unshipped-skipped checkpoint means shipping is not keeping up.
    async fn checkpoint_due(&mut self, now: i64) -> bool {
        if self.shadow || self.position.is_none() {
            return false;
        }
        let interval = i64::try_from(self.config.checkpoint_interval.as_secs()).unwrap_or(i64::MAX);
        if now.saturating_sub(self.last_checkpoint_secs) < interval {
            return false;
        }
        let wal_path = self.wal_path();
        let pages = spawn_blocking(move || count_wal_pages(&wal_path))
            .await
            .unwrap_or(0);
        pages >= u64::from(self.config.min_checkpoint_pages)
    }

    /// Whether a snapshot should start this tick.
    fn snapshot_due(&self, now: i64) -> bool {
        if self.snapshot.is_some() {
            return false;
        }
        if self.pending_new_generation {
            return true;
        }
        let interval = i64::try_from(self.config.snapshot_interval.as_secs()).unwrap_or(i64::MAX);
        self.position.is_some() && now.saturating_sub(self.generation_birth_secs) >= interval
    }

    /// Advance the snapshot machine one step; `Ok(None)` means finished.
    async fn run_snapshot_step(
        &mut self,
        job: SnapshotJob,
    ) -> Result<Option<SnapshotJob>, SyncError> {
        match job {
            SnapshotJob::ShipTail => {
                // Drain committed frames into the old generation before
                // fixing the boundary, so it stays a complete restore point
                // under retention; skipped when no lineage is usable (fresh
                // replica or divergence). Only a capped pass (a genuine
                // backlog) repeats: commits racing in DURING the drain are
                // covered by the new generation, which re-ships the whole
                // WAL from offset 0, so chasing them would never terminate
                // under a busy writer.
                if self.position.is_some() {
                    let shipped = self.ship_pass().await?;
                    if shipped >= MAX_SEGMENTS_PER_PASS {
                        return Ok(Some(SnapshotJob::ShipTail));
                    }
                }
                Ok(Some(SnapshotJob::CreateGeneration))
            },
            SnapshotJob::CreateGeneration => self.snapshot_create().await.map(Some),
            SnapshotJob::Copying(copy) => self.snapshot_copy(copy).await,
            SnapshotJob::Finalize(finalize) => {
                self.snapshot_finalize(finalize).await?;
                Ok(None)
            },
        }
    }

    /// A fresh generation id strictly greater than the current one, so
    /// lexicographic order keeps matching creation order even when a
    /// retrigger lands within the same clock second (restore always picks
    /// the greatest valid generation). Bumps the timestamp by up to a
    /// minute before giving up (a rewound clock beyond that is logged and
    /// tolerated: restore would prefer the older lineage until the clock
    /// catches up).
    fn next_generation_id(&self) -> GenerationId {
        let floor = self.generation().max(self.generation_floor.as_ref());
        let now = self.now_secs();
        for bump in 0..60 {
            let Ok(created) = bencher_json::DateTime::try_from(now.saturating_add(bump)) else {
                continue;
            };
            let candidate = GenerationId::generate(created);
            if floor.is_none_or(|floor| candidate > *floor) {
                return candidate;
            }
        }
        // The clock has rewound more than a minute behind the observed tip:
        // fall back to the floor's own SUCCESSOR (same timestamp, suffix
        // plus one), which is guaranteed to sort after it. A below-floor id
        // would make restore silently pick the older lineage forever.
        slog::warn!(
            self.log,
            "Clock is behind the newest generation; minting its successor instead"
        );
        if let Some(successor) = floor.and_then(GenerationId::successor) {
            return successor;
        }
        GenerationId::generate(self.clock.now())
    }

    /// Fix the generation id, run the single-step online backup into the
    /// scratch file, record the boundary; open the multipart upload.
    async fn snapshot_create(&mut self) -> Result<SnapshotJob, SyncError> {
        #[cfg(feature = "otel")]
        bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::ReplicaGenerationCreate);
        let generation = self.next_generation_id();
        let db_path = self.db.db_path.clone();
        let scratch = self.snapshot_scratch_path();
        let (db_len, page_size, live_fingerprint) = {
            let scratch = scratch.clone();
            spawn_blocking(move || {
                backup_to_scratch(&db_path, &scratch)?;
                let (db_len, page_size) = read_scratch_info(&scratch)?;
                let live_fingerprint = live_db_fingerprint(&db_path)?;
                Ok::<_, SnapshotError>((db_len, page_size, live_fingerprint))
            })
            .await
            .map_err(SyncError::Join)??
        };
        // The boundary, read AFTER the backup: the current cycle's salts
        // plus its valid committed length. The length is the mandatory-
        // replay threshold: every backup-contained boundary-cycle frame
        // lies below it (frames committed after the backup only OVERSTATE
        // the threshold, which is the safe direction; they are replayed
        // once shipped, like everything else).
        let (boundary_salt, boundary_offset) = {
            let wal_path = self.wal_path();
            spawn_blocking(move || wal_committed_extent(&wal_path))
                .await
                .map_err(SyncError::Join)??
                .map_or(((0, 0), 0), |extent| (extent.salt, extent.committed_end))
        };
        let upload = self
            .storage
            .start_multipart(&snapshot_key(&generation))
            .await?;
        let encoder = new_snapshot_encoder()?;
        slog::info!(self.log, "Snapshot started";
            "generation" => generation.as_str(), "db_bytes" => db_len);
        Ok(SnapshotJob::Copying(Box::new(CopyJob {
            generation,
            boundary_salt,
            boundary_offset,
            upload,
            hasher: Sha256::new(),
            encoder,
            scratch,
            db_len,
            offset: 0,
            page_size,
            live_fingerprint,
        })))
    }

    /// One budgeted upload step over the backup scratch file.
    async fn snapshot_copy(
        &mut self,
        copy: Box<CopyJob>,
    ) -> Result<Option<SnapshotJob>, SyncError> {
        let CopyJob {
            generation,
            boundary_salt,
            boundary_offset,
            mut upload,
            hasher,
            encoder,
            scratch,
            db_len,
            offset,
            page_size,
            live_fingerprint,
        } = *copy;
        let budget = self.copy_budget();
        let step = {
            let scratch = scratch.clone();
            spawn_blocking(move || copy_step(&scratch, encoder, hasher, offset, db_len, budget))
                .await
                .map_err(SyncError::Join)?
        };
        let step = match step {
            Ok(step) => step,
            Err(error) => {
                slog::error!(self.log, "Snapshot aborted";
                    "generation" => generation.as_str(), "error" => %error);
                self.abort_upload(upload).await;
                self.remove_scratch(&scratch).await;
                return Err(error.into());
            },
        };
        match step {
            CopyStepResult::Continue {
                encoder,
                hasher,
                compressed,
                offset,
            } => {
                if !compressed.is_empty()
                    && let Err(error) = upload.write_part(Bytes::from(compressed)).await
                {
                    self.abort_upload(upload).await;
                    self.remove_scratch(&scratch).await;
                    return Err(error.into());
                }
                Ok(Some(SnapshotJob::Copying(Box::new(CopyJob {
                    generation,
                    boundary_salt,
                    boundary_offset,
                    upload,
                    hasher,
                    encoder,
                    scratch,
                    db_len,
                    offset,
                    page_size,
                    live_fingerprint,
                }))))
            },
            CopyStepResult::Done { hasher, compressed } => {
                if !compressed.is_empty()
                    && let Err(error) = upload.write_part(Bytes::from(compressed)).await
                {
                    self.abort_upload(upload).await;
                    self.remove_scratch(&scratch).await;
                    return Err(error.into());
                }
                Ok(Some(SnapshotJob::Finalize(Box::new(FinalizeJob {
                    generation,
                    boundary_salt,
                    boundary_offset,
                    upload,
                    sha256: hex::encode(hasher.finalize()),
                    db_bytes: db_len,
                    page_size,
                    scratch,
                    live_fingerprint,
                }))))
            },
        }
    }

    /// Commit the generation: finish the body upload, re-check the boundary
    /// salts, PUT `snapshot.json` LAST, reset the position, and prune. A salt
    /// change since `CreateGeneration` is the epoch-0 lazy binding rule in
    /// shadow mode (rebind) but an external-checkpointer divergence in sole
    /// mode (abort with [`SyncError::SnapshotBoundaryDiverged`], no marker).
    /// Validate (or rebind) the snapshot boundary against the CURRENT WAL
    /// salt at finalize time. Nothing has shipped into the new generation yet
    /// by construction (the engine is sequential), so a salt change since
    /// `CreateGeneration` means the WAL restarted between the backup read point
    /// and here; the engine's own checkpoints are suppressed while a snapshot
    /// job is active, so an engine-own path cannot cause this.
    ///
    /// Shadow mode: Litestream owns checkpoints and legitimately restarts the
    /// WAL. Keep committing (the shadow replica is disposable) but flag the
    /// UNVERIFIED boundary and rebind to the new cycle at offset 0, mirroring
    /// the ship-path rebind's observability.
    ///
    /// Sole mode: the restart is an EXTERNAL checkpointer that may have
    /// backfilled (then buried) frames committed after the backup read point
    /// which the replica never saw. Rebinding would commit a marker whose
    /// restore replays new-cycle segments onto a snapshot body missing those
    /// frames (a torn restore). ABORT instead: no snapshot.json is written
    /// (the uploaded body stays invisible and is pruned as stale) and a
    /// fresh generation is scheduled; the WAL keeps buffering, so nothing is
    /// lost, only re-snapshotted.
    async fn finalize_boundary(
        &mut self,
        generation: &GenerationId,
        boundary_salt: (u32, u32),
        boundary_offset: u64,
    ) -> Result<((u32, u32), u64), SyncError> {
        let Some(salt) = self.current_wal_salt().await? else {
            return Ok((boundary_salt, boundary_offset));
        };
        if salt == boundary_salt {
            return Ok((boundary_salt, boundary_offset));
        }
        if self.shadow {
            slog::warn!(self.log,
                "Shadow: rebinding snapshot boundary to a restarted WAL cycle (boundary unverified; restore from this generation may be inconsistent if the buried cycle held unshipped frames)";
                "generation" => generation.as_str());
            #[cfg(feature = "otel")]
            bencher_otel::ApiMeter::increment(
                bencher_otel::ApiCounter::ReplicaShadowUnverifiedBoundary,
            );
            Ok((salt, 0))
        } else {
            slog::error!(self.log,
                "External checkpoint restarted the WAL during the snapshot; aborting the generation (no snapshot.json) and scheduling a fresh one";
                "generation" => generation.as_str(),
                "boundary_salt" => format!("{boundary_salt:?}"),
                "current_salt" => format!("{salt:?}"));
            #[cfg(feature = "otel")]
            bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::ReplicaDivergence);
            Err(SyncError::SnapshotBoundaryDiverged)
        }
    }

    async fn snapshot_finalize(&mut self, finalize: Box<FinalizeJob>) -> Result<(), SyncError> {
        let FinalizeJob {
            generation,
            boundary_salt,
            boundary_offset,
            upload,
            sha256,
            db_bytes,
            page_size,
            scratch,
            live_fingerprint,
        } = *finalize;
        if let Err(error) = upload.finish().await {
            self.remove_scratch(&scratch).await;
            return Err(error.into());
        }
        self.remove_scratch(&scratch).await;
        let (boundary_salt, boundary_offset) = self
            .finalize_boundary(&generation, boundary_salt, boundary_offset)
            .await?;
        let snapshot_meta = SnapshotMeta {
            version: SNAPSHOT_META_VERSION,
            created: self.clock.now().into_inner().to_rfc3339(),
            db_bytes,
            page_size,
            sha256,
            wal_boundary: WalBoundary {
                epoch: 0,
                salt1: boundary_salt.0,
                salt2: boundary_salt.1,
                offset: boundary_offset,
            },
        };
        let marker = snapshot_meta.to_bytes().map_err(SyncError::SnapshotMeta)?;
        self.storage
            .put(&snapshot_meta_key(&generation), Bytes::from(marker))
            .await?;
        slog::info!(self.log, "Snapshot complete";
            "generation" => generation.as_str(), "db_bytes" => db_bytes);
        // A fresh generation normally binds epoch 0 at offset 0; an oversized-
        // transaction re-snapshot instead pins it at the boundary to drain the
        // WAL (see `epoch0_bind_offset`).
        let (offset, checksum) = self
            .epoch0_bind_offset(&generation, boundary_offset)
            .await?;
        let position = Position {
            generation,
            epoch: 0,
            salt: boundary_salt,
            offset,
            checksum,
        };
        self.position = Some(position.clone());
        self.generation_floor = Some(position.generation.clone());
        self.awaiting = None;
        self.epoch_checkpointed = false;
        self.pending_new_generation = false;
        self.generation_birth_secs = self.now_secs();
        // The meta file is advisory: log a failure instead of failing the
        // completed snapshot.
        if let Err(error) = self.store_meta_for(&position).await {
            slog::warn!(self.log, "Failed to store replica meta after snapshot";
                "error" => %error);
        }
        if let Err(error) = self.prune_once().await {
            slog::warn!(self.log, "Prune failed; retrying at the next snapshot";
                "error" => %error);
        }
        // Sole mode: nothing legitimate mutates the LIVE database file
        // during a snapshot, so a moved fingerprint proves an external
        // checkpointer that may have buried post-backup frames the replica
        // never saw. The committed snapshot stays (it is consistent); a
        // follow-up generation recaptures whatever was buried. Shadow mode
        // skips this (Litestream checkpoints constantly; the daily
        // verification is the backstop there).
        if !self.shadow {
            let db_path = self.db.db_path.clone();
            let current = spawn_blocking(move || live_db_fingerprint(&db_path))
                .await
                .map_err(SyncError::Join)??;
            if current != live_fingerprint {
                slog::error!(self.log, "External checkpoint detected during the snapshot; scheduling a follow-up generation";
                    "recorded" => format!("{live_fingerprint:?}"),
                    "current" => format!("{current:?}"));
                #[cfg(feature = "otel")]
                bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::ReplicaDivergence);
                self.pending_new_generation = true;
            }
        }
        Ok(())
    }

    /// The snapshot backup scratch file, next to the database (same
    /// volume). Deleted on completion, on abort, and before reuse.
    fn snapshot_scratch_path(&self) -> Utf8PathBuf {
        Utf8PathBuf::from(format!("{}.snapshot-scratch", self.db.db_path))
    }

    /// Best-effort scratch removal (a leftover is replaced by the next
    /// snapshot's backup).
    async fn remove_scratch(&mut self, scratch: &Utf8Path) {
        let scratch = scratch.to_owned();
        let removed = spawn_blocking(move || match std::fs::remove_file(&scratch) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        })
        .await;
        match removed {
            Ok(Ok(())) => {},
            Ok(Err(error)) => {
                slog::warn!(self.log, "Failed to remove the snapshot scratch file";
                    "error" => %error);
            },
            Err(error) => {
                slog::warn!(self.log, "Scratch removal task panicked"; "error" => %error);
            },
        }
    }

    /// Per-step copy budget: `snapshot_throttle_mib * sync_interval`, at
    /// least 1 MiB.
    fn copy_budget(&self) -> u64 {
        u64::from(self.config.snapshot_throttle_mib)
            .saturating_mul(MIB)
            .saturating_mul(self.config.sync_interval.as_secs().max(1))
            .max(MIB)
    }

    /// Best-effort multipart abort (a failed abort only leaves an invisible
    /// partial upload behind).
    async fn abort_upload(&mut self, upload: MultipartUpload) {
        if let Err(error) = upload.abort().await {
            slog::warn!(self.log, "Failed to abort snapshot upload"; "error" => %error);
        }
    }

    /// The current local WAL header salts, if a parseable header exists.
    async fn current_wal_salt(&mut self) -> Result<Option<(u32, u32)>, SyncError> {
        let wal_path = self.wal_path();
        let state = spawn_blocking(move || read_wal_header_state(&wal_path))
            .await
            .map_err(SyncError::Join)??;
        if let WalHeaderState::Present { header, .. } = state {
            Ok(Some(header.salt))
        } else {
            Ok(None)
        }
    }

    /// Generations that must never be pruned: the one being written to and
    /// the one an in-flight snapshot is building.
    fn protected_generations(&self) -> Vec<GenerationId> {
        let mut protected = Vec::new();
        if let Some(generation) = self.generation() {
            protected.push(generation.clone());
        }
        match &self.snapshot {
            Some(SnapshotJob::Copying(copy)) => protected.push(copy.generation.clone()),
            Some(SnapshotJob::Finalize(finalize)) => protected.push(finalize.generation.clone()),
            Some(SnapshotJob::ShipTail | SnapshotJob::CreateGeneration) | None => {},
        }
        protected
    }

    /// Persist the advisory meta file (temp + fsync + rename) after a state
    /// change.
    async fn store_meta_for(&mut self, position: &Position) -> Result<(), SyncError> {
        let meta = ReplicaMeta {
            version: META_VERSION,
            generation: position.generation.as_str().to_owned(),
            epoch: position.epoch,
            salt1: position.salt.0,
            salt2: position.salt.1,
            shipped_offset: position.offset,
            epoch_shipped_through_checkpoint: self.epoch_checkpointed,
            shadow: self.shadow,
        };
        let db_path = self.db.db_path.clone();
        spawn_blocking(move || meta.store(&db_path))
            .await
            .map_err(SyncError::Join)??;
        Ok(())
    }
}

/// The cumulative checksum stored in the final frame header of a raw WAL
/// segment: frame-header bytes 16..24 of the last frame.
fn segment_tail_checksum(raw: &[u8], frame_size: u64) -> Option<(u32, u32)> {
    let frame_size = usize::try_from(frame_size).ok()?;
    let frame_start = raw.len().checked_sub(frame_size)?;
    let frame_header: &[u8; 24] = raw
        .get(frame_start..frame_start.checked_add(24)?)?
        .try_into()
        .ok()?;
    Some(crate::wal::FrameHeader::parse(frame_header).checksum)
}

/// Whether the advisory meta exactly matches the replica tip: the proof
/// required for an epoch+1 (or awaiting) resume instead of a re-snapshot.
///
/// The `meta.epoch == tip.epoch` clause is what keeps a same-epoch salt
/// collision (two epoch directories with the same number but different salts,
/// which `plan_epochs` would soft-stop on forever) from ever being created.
/// The tip epoch is the highest epoch with a shipped segment, so an epoch+1
/// resume only ever extends at `tip.epoch + 1`, a number no directory holds
/// yet. If the restored meta records a LOWER epoch than the tip (e.g. a restore
/// soft-stopped at epoch N below a corrupt-but-present epoch N+1), this clause
/// fails and resume diverges to a fresh generation rather than binding new
/// salts onto the already-occupied epoch N+1. See the pinned regression tests
/// `plan_discards_epoch_with_duplicate_salt_lineages` (restore.rs) and
/// `resume_after_soft_stop_below_corrupt_epoch_forces_new_generation`.
fn meta_matches(meta: Option<&ReplicaMeta>, tip: &ReplicaTip, shadow: bool) -> bool {
    meta.is_some_and(|meta| {
        meta.generation == tip.generation.as_str()
            && meta.epoch == tip.epoch
            && meta.shipped_offset == tip.end
            && meta.epoch_shipped_through_checkpoint
            && meta.shadow == shadow
    })
}

/// Read and parse the local WAL header without opening `SQLite`.
fn read_wal_header_state(wal_path: &Utf8Path) -> Result<WalHeaderState, SyncError> {
    let read_err = |error| SyncError::WalIo {
        path: wal_path.to_owned(),
        error,
    };
    let mut file = match File::open(wal_path) {
        Ok(file) => file,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(WalHeaderState::Missing),
        Err(error) => return Err(read_err(error)),
    };
    let mut raw = [0u8; 32];
    match file.read_exact(&mut raw) {
        Ok(()) => {},
        Err(error) if error.kind() == ErrorKind::UnexpectedEof => {
            return Ok(WalHeaderState::Missing);
        },
        Err(error) => return Err(read_err(error)),
    }
    match parse_wal_header(&raw) {
        Ok(header) => Ok(WalHeaderState::Present { header, raw }),
        Err(error) => Ok(WalHeaderState::Unreadable(error)),
    }
}

/// Scan the next committed chunk from `offset` (up to `max_bytes`),
/// re-verifying that the WAL header still carries the expected salts (a
/// concurrent restart otherwise invalidates the resume seed). `Ok(None)`
/// means nothing shippable now.
fn scan_next_chunk(
    wal_path: &Utf8Path,
    expected: WalHeader,
    offset: u64,
    checksum: (u32, u32),
    max_bytes: u64,
) -> Result<Option<CommittedChunk>, SyncError> {
    match open_resumed_scanner(wal_path, expected, offset, checksum)? {
        Some(mut scanner) => Ok(scanner.next_committed(max_bytes)?),
        None => Ok(None),
    }
}

/// Discard-scan from `offset` to the committed extent (page bytes discarded),
/// re-verifying the header salts. Returns the absolute WAL offset just past
/// the last commit within `max_bytes` (equal to `offset` when nothing new is
/// committed, the WAL is missing, or the salts have changed). `max_txn_bytes`
/// bounds the bytes since the last commit, so an oversized open transaction
/// errors instead of scanning without bound.
fn scan_committed_end(
    wal_path: &Utf8Path,
    expected: WalHeader,
    offset: u64,
    checksum: (u32, u32),
    max_bytes: u64,
    max_txn_bytes: u64,
) -> Result<u64, SyncError> {
    match open_resumed_scanner(wal_path, expected, offset, checksum)? {
        Some(mut scanner) => Ok(scanner.scan_committed_extent(max_bytes, max_txn_bytes)?),
        None => Ok(offset),
    }
}

/// Open the WAL, re-check the header salts against `expected`, and resume a
/// scanner at `(offset, checksum)`. `Ok(None)` when the WAL is missing,
/// header-short, unparsable, or salt-changed (all handled by the caller as
/// "nothing to scan from here").
fn open_resumed_scanner(
    wal_path: &Utf8Path,
    expected: WalHeader,
    offset: u64,
    checksum: (u32, u32),
) -> Result<Option<WalScanner<File>>, SyncError> {
    let read_err = |error| SyncError::WalIo {
        path: wal_path.to_owned(),
        error,
    };
    let mut file = match File::open(wal_path) {
        Ok(file) => file,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(read_err(error)),
    };
    let mut raw = [0u8; 32];
    match file.read_exact(&mut raw) {
        Ok(()) => {},
        Err(error) if error.kind() == ErrorKind::UnexpectedEof => return Ok(None),
        Err(error) => return Err(read_err(error)),
    }
    let Ok(header) = parse_wal_header(&raw) else {
        return Ok(None);
    };
    if header.salt != expected.salt {
        return Ok(None);
    }
    Ok(Some(WalScanner::resume(file, header, offset, checksum)?))
}

/// The live WAL's header salts plus the byte offset just past its last
/// committed frame.
struct WalExtent {
    salt: (u32, u32),
    committed_end: u64,
}

/// Measure the live WAL's [`WalExtent`] (`None` when the WAL is missing or
/// shorter than a header). Used for the snapshot boundary.
fn wal_committed_extent(wal_path: &Utf8Path) -> Result<Option<WalExtent>, SyncError> {
    let file = match File::open(wal_path) {
        Ok(file) => file,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(SyncError::WalIo {
                path: wal_path.to_owned(),
                error,
            });
        },
    };
    let Some(mut scanner) = WalScanner::open(file)? else {
        return Ok(None);
    };
    let salt = scanner.header().salt;
    // Discard-scan the whole committed WAL (offsets only): materializing every
    // frame just to learn where committed data ends was a needless multi-GiB
    // allocation. Errors PROPAGATE via `?`: swallowing a mid-scan read/seek
    // error and returning the extent measured so far would UNDERSTATE the
    // mandatory-replay boundary, and restore would then replay a truncated WAL
    // prefix on top of a newer snapshot body and regress pages. Overstating
    // the boundary is the only safe direction (see the boundary note in
    // `snapshot_create`), so a scan error must fail the snapshot, not shorten
    // the boundary. No transaction cap here: the boundary is purely the
    // committed length, so an oversized uncommitted tail (which the ship path
    // is what refuses) must not spuriously fail the snapshot.
    let committed_end = scanner.scan_committed_extent(u64::MAX, u64::MAX)?;
    Ok(Some(WalExtent {
        salt,
        committed_end,
    }))
}

/// Rescan the local WAL chain from offset 0 and recover the running
/// checksum at exactly `target` (a commit boundary the replica claims).
/// `Ok(None)` when the valid chain never reaches `target` exactly.
///
/// Discard-scan: only offsets and the running checksum are needed, so page
/// bytes are validated and dropped instead of buffering every committed
/// transaction below `target` on the heap.
fn checksum_at_offset(wal_path: &Utf8Path, target: u64) -> Result<Option<(u32, u32)>, SyncError> {
    let file = File::open(wal_path).map_err(|error| SyncError::WalIo {
        path: wal_path.to_owned(),
        error,
    })?;
    let Some(mut scanner) = WalScanner::open(file)? else {
        return Ok(None);
    };
    // Stop at the first commit whose end reaches `target` (no transaction cap:
    // the checksum must be recoverable regardless of transaction sizes below
    // the target).
    let max_bytes = target.saturating_sub(WAL_HEADER_SIZE);
    scanner.scan_committed_extent(max_bytes, u64::MAX)?;
    if scanner.offset() == target {
        Ok(Some(scanner.checksum()))
    } else {
        // The valid chain never lands exactly on `target` (short, or the next
        // commit overshoots it).
        Ok(None)
    }
}

/// Committed-or-not page count of the WAL file (checkpoint due-ness input);
/// best effort, 0 on any error.
fn count_wal_pages(wal_path: &Utf8Path) -> u64 {
    let Ok(metadata) = std::fs::metadata(wal_path) else {
        return 0;
    };
    let Ok(WalHeaderState::Present { header, .. }) = read_wal_header_state(wal_path) else {
        return 0;
    };
    metadata
        .len()
        .saturating_sub(WAL_HEADER_SIZE)
        .checked_div(header.frame_size())
        .unwrap_or(0)
}
