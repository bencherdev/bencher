//! The checkpoint critical section: a PASSIVE checkpoint under a frozen
//! writer.
//!
//! The exact order, executed while the caller holds the app writer mutex:
//!
//! 1. `BEGIN IMMEDIATE` on the dedicated lock connection (1s busy budget;
//!    a busy stray writer yields [`CheckpointOutcome::SkippedBusy`], retried
//!    next tick).
//! 2. Re-scan the WAL tail from the shipped position (plain `std::fs`): ANY
//!    new committed frame beyond the position defers the checkpoint
//!    ([`CheckpointOutcome::SkippedUnshipped`], invariant I1). Frames past
//!    the last commit (an uncommitted or rolled-back spill) do not block:
//!    they are not durable data.
//! 3. `PRAGMA wal_checkpoint(PASSIVE)` on the second connection. PASSIVE
//!    needs no writer lock, so it runs to completion WHILE the lock
//!    connection holds `BEGIN IMMEDIATE`: no instant exists between the
//!    tail verification and the checkpoint where a stray writer can append
//!    frames that would be backfilled unshipped. This closes the race that
//!    litestream's release-then-checkpoint ordering leaves open, and makes
//!    RESTART/TRUNCATE checkpoints unnecessary everywhere (I3: the next
//!    writer restarts a fully backfilled WAL naturally, at a moment when
//!    everything is shipped).
//! 4. Full backfill (`busy == 0 && log == checkpointed`) is
//!    [`CheckpointOutcome::Completed`]; anything else (e.g. a long reader
//!    pinning a mark) is [`CheckpointOutcome::Partial`]: fine, a WAL
//!    restart is impossible without full backfill, so nothing unshipped can
//!    be overwritten; retry next interval.
//! 5. `ROLLBACK`.
//!
//! [`checkpoint_locked`] is a synchronous `fn` on purpose: it is
//! compile-time proof that no network I/O (no `.await`) is reachable while
//! the `SQLite` write lock is held (invariant I5).

use std::fs::File;
use std::io::{ErrorKind, Read as _};
use std::time::Duration;

use camino::{Utf8Path, Utf8PathBuf};

use crate::position::Position;
use crate::wal::{WAL_HEADER_SIZE, WalError, WalScanner, parse_wal_header};

/// Busy budget for `BEGIN IMMEDIATE` on the lock connection: stray writers
/// (stats/credit sweeps) hold the write lock only briefly, so 1s is ample;
/// on expiry the checkpoint is skipped and retried next tick.
const LOCK_BUSY_TIMEOUT: Duration = Duration::from_secs(1);

/// The two dedicated checkpoint connections, opened lazily on first use and
/// reused across checkpoints.
pub(crate) struct CheckpointConns {
    /// Holds `BEGIN IMMEDIATE` across the critical section.
    lock: rusqlite::Connection,
    /// Runs `PRAGMA wal_checkpoint(PASSIVE)`; `busy_timeout = 0` because
    /// PASSIVE never needs to wait for anything.
    ckpt: rusqlite::Connection,
}

impl CheckpointConns {
    pub(crate) fn open(db_path: &Utf8Path) -> Result<Self, rusqlite::Error> {
        let lock = rusqlite::Connection::open(db_path)?;
        lock.busy_timeout(LOCK_BUSY_TIMEOUT)?;
        disable_autocheckpoint(&lock)?;
        let ckpt = rusqlite::Connection::open(db_path)?;
        ckpt.busy_timeout(Duration::ZERO)?;
        disable_autocheckpoint(&ckpt)?;
        Ok(Self { lock, ckpt })
    }
}

/// The outcome of one checkpoint attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckpointOutcome {
    /// Every WAL frame was backfilled into the database file; the next
    /// writer will restart the WAL legitimately.
    Completed,
    /// Some frames were backfilled but a reader mark pinned the rest (e.g.
    /// the online backup's long read snapshot). Harmless: retried next
    /// interval, and no WAL restart can happen without full backfill.
    Partial,
    /// A stray writer held the `SQLite` write lock past the busy budget;
    /// retried next tick. App writers queue on the tokio mutex instead and
    /// never see this.
    SkippedBusy,
    /// Committed frames beyond the shipped position exist; the checkpoint
    /// is deferred until they ship (invariant I1).
    SkippedUnshipped,
    /// Shadow mode never checkpoints: Litestream keeps checkpoint
    /// ownership until cutover.
    SkippedShadow,
}

#[derive(Debug, thiserror::Error)]
pub enum CheckpointError {
    #[error("Checkpoint connection: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("Checkpoint WAL scan: {0}")]
    Wal(#[from] WalError),
    #[error("Failed to read WAL for checkpoint ({path}): {error}")]
    WalRead {
        path: Utf8PathBuf,
        error: std::io::Error,
    },
}

/// Run the checkpoint critical section. The caller MUST hold the app writer
/// mutex for the whole call, so app writers queue on the tokio mutex (never
/// burning their `busy_timeout`) and only stray connections can contend.
///
/// Synchronous by design: see the module docs. Always releases the
/// `BEGIN IMMEDIATE` transaction, including on error paths.
pub(crate) fn checkpoint_locked(
    conns: &mut CheckpointConns,
    wal_path: &Utf8Path,
    position: &Position,
    max_transaction_bytes: u64,
) -> Result<CheckpointOutcome, CheckpointError> {
    match conns.lock.execute_batch("BEGIN IMMEDIATE") {
        Ok(()) => {},
        Err(error) if is_busy(&error) => return Ok(CheckpointOutcome::SkippedBusy),
        Err(error) => return Err(error.into()),
    }
    let result = locked_section(conns, wal_path, position, max_transaction_bytes);
    // Always release the write lock, even when the section failed.
    let rollback = conns.lock.execute_batch("ROLLBACK");
    match (result, rollback) {
        (Err(error), _) => Err(error),
        (Ok(_), Err(error)) => Err(error.into()),
        (Ok(outcome), Ok(())) => Ok(outcome),
    }
}

/// Steps 2 to 4 of the critical section, with the write lock held.
fn locked_section(
    conns: &CheckpointConns,
    wal_path: &Utf8Path,
    position: &Position,
    max_transaction_bytes: u64,
) -> Result<CheckpointOutcome, CheckpointError> {
    if wal_has_unshipped_commit(wal_path, position, max_transaction_bytes)? {
        return Ok(CheckpointOutcome::SkippedUnshipped);
    }
    let (busy, log, checkpointed): (i64, i64, i64) =
        conns
            .ckpt
            .query_row("PRAGMA wal_checkpoint(PASSIVE)", [], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?;
    if busy == 0 && log == checkpointed {
        Ok(CheckpointOutcome::Completed)
    } else {
        Ok(CheckpointOutcome::Partial)
    }
}

/// Whether any committed frame exists beyond the shipped position (the WAL
/// tail is frozen while the caller holds `BEGIN IMMEDIATE`).
///
/// Conservative on every ambiguity: a missing or restarted WAL while the
/// position claims shipped bytes reports "unshipped", deferring the
/// checkpoint so the ship path can resolve the situation first (rebind,
/// epoch transition, or divergence). Without this, an externally truncated
/// WAL could launder a divergence into a legitimate-looking epoch change.
fn wal_has_unshipped_commit(
    wal_path: &Utf8Path,
    position: &Position,
    max_transaction_bytes: u64,
) -> Result<bool, CheckpointError> {
    let read_err = |error| CheckpointError::WalRead {
        path: wal_path.to_owned(),
        error,
    };
    let mut file = match File::open(wal_path) {
        Ok(file) => file,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(position.offset > 0),
        Err(error) => return Err(read_err(error)),
    };
    let mut raw = [0u8; 32];
    match file.read_exact(&mut raw) {
        Ok(()) => {},
        Err(error) if error.kind() == ErrorKind::UnexpectedEof => {
            return Ok(position.offset > 0);
        },
        Err(error) => return Err(read_err(error)),
    }
    let header = parse_wal_header(&raw)?;
    if header.salt != position.salt {
        // A different salt cycle: anything in it is by definition not
        // shipped at this position.
        return Ok(true);
    }
    let (offset, checksum) = if position.offset == 0 {
        (WAL_HEADER_SIZE, header.checksum)
    } else {
        (position.offset, position.checksum)
    };
    let mut scanner = WalScanner::resume(file, header, offset, checksum)?;
    // Discard-mode scan: only the EXISTENCE of a commit past `offset` matters,
    // so page bytes are checksum-validated and dropped instead of buffered.
    // `max_bytes = 1` stops at the first commit boundary. `max_transaction_bytes`
    // bounds the run since the last commit so a multi-GiB uncommitted tail is
    // not fully re-read inside the checkpoint critical section: an oversized
    // run aborts the scan, which is treated CONSERVATIVELY as "unshipped"
    // (defer the checkpoint) so an ambiguous tail is never backfilled.
    match scanner.scan_committed_extent(1, max_transaction_bytes) {
        Ok(extent) => Ok(extent > offset),
        Err(WalError::TransactionTooLarge { .. }) => Ok(true),
        Err(error) => Err(error.into()),
    }
}

/// Outcome of pinning a read snapshot at the shipped position.
pub(crate) enum PinOutcome {
    /// A read snapshot pinned exactly at the shipped position; dropping the
    /// connection releases it.
    Pinned(rusqlite::Connection),
    /// A stray writer held the `SQLite` write lock past the busy budget;
    /// retry later.
    Busy,
    /// Committed frames beyond the shipped position exist; ship first.
    Unshipped,
}

/// Pin a read snapshot at `position` for verification.
///
/// The lock connection holds `BEGIN IMMEDIATE` across the pin, so no commit
/// can land between the shipped-position check and the snapshot: the pinned
/// state IS the shipped state. This holds even in shadow mode, where
/// Litestream-driven WAL churn would otherwise slip a commit in and produce a
/// spurious verification failure (a stray writer holding the lock just yields
/// [`PinOutcome::Busy`], which the caller retries).
///
/// Synchronous by design, like [`checkpoint_locked`]: no `.await` while the
/// write lock is held.
pub(crate) fn pin_locked(
    conns: &mut CheckpointConns,
    db_path: &Utf8Path,
    wal_path: &Utf8Path,
    position: &Position,
    max_transaction_bytes: u64,
) -> Result<PinOutcome, CheckpointError> {
    match conns.lock.execute_batch("BEGIN IMMEDIATE") {
        Ok(()) => {},
        Err(error) if is_busy(&error) => return Ok(PinOutcome::Busy),
        Err(error) => return Err(error.into()),
    }
    let result = pin_section(db_path, wal_path, position, max_transaction_bytes);
    let rollback = conns.lock.execute_batch("ROLLBACK");
    match (result, rollback) {
        (Err(error), _) => Err(error),
        (Ok(_), Err(error)) => Err(error.into()),
        (Ok(outcome), Ok(())) => Ok(outcome),
    }
}

/// The pin itself, with the write lock held when in exclusive mode.
fn pin_section(
    db_path: &Utf8Path,
    wal_path: &Utf8Path,
    position: &Position,
    max_transaction_bytes: u64,
) -> Result<PinOutcome, CheckpointError> {
    if wal_has_unshipped_commit(wal_path, position, max_transaction_bytes)? {
        return Ok(PinOutcome::Unshipped);
    }
    let pinned = rusqlite::Connection::open(db_path)?;
    pinned.busy_timeout(Duration::ZERO)?;
    // A deferred BEGIN plus any read materializes the snapshot at the
    // current committed state, which (with the write lock held) is exactly
    // the shipped position.
    pinned.execute_batch("BEGIN")?;
    let _tables: i64 =
        pinned.query_row("SELECT count(*) FROM sqlite_master", [], |row| row.get(0))?;
    Ok(PinOutcome::Pinned(pinned))
}

/// `SQLITE_BUSY` (or the shared-cache `SQLITE_LOCKED`) from a statement.
fn is_busy(error: &rusqlite::Error) -> bool {
    matches!(
        error.sqlite_error_code(),
        Some(rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked)
    )
}

/// `wal_autocheckpoint = 0`: the checkpoint connections must never
/// checkpoint on their own (invariant I2).
fn disable_autocheckpoint(conn: &rusqlite::Connection) -> Result<(), rusqlite::Error> {
    let _pages: i64 = conn.query_row("PRAGMA wal_autocheckpoint = 0", [], |row| row.get(0))?;
    Ok(())
}
