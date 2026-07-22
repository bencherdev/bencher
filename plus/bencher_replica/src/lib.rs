#![cfg(feature = "plus")]

//! # Bencher Replica
//!
//! In-process `SQLite` streaming replication: continuously ships WAL frames
//! to a local-filesystem or S3-compatible replica, takes periodic snapshots,
//! and restores the latest state at API server startup. Replaces Litestream.
//!
//! ## Core invariants
//!
//! - **I1 Ship-before-checkpoint**: a checkpoint is only issued after every
//!   valid committed WAL frame has been durably uploaded to the replica.
//! - **I2 Sole checkpointer**: with replication configured,
//!   `wal_autocheckpoint = 0` is set on every connection that can write.
//! - **I3 Restart safety**: `SQLite` restarts the WAL (new salts) only when a
//!   writer finds it fully backfilled; given I1+I2, fully backfilled implies
//!   fully shipped, so no unshipped frame is ever overwritten.
//! - **I4 DB-file writes = checkpoints**: the sync task is sequential, so the
//!   main DB file is frozen during a snapshot copy; snapshots take no locks.
//! - **I5 Prime directive**: the `SQLite` write lock is only ever held for
//!   O(WAL-tail) work, never O(database).
//! - **I6 Replica is the source of truth**: the local meta file is advisory;
//!   any mismatch resolves to a new generation, never guessing.
//!
//! ## Operational assumptions
//!
//! - **One server per replica target.** There is no fencing: two servers
//!   pointed at the same bucket/prefix will interleave generations and
//!   segments, and the result is undefined. Each replica target must have
//!   exactly one writer.
//! - **Verification needs transient headroom.** A restore-and-compare
//!   verification restores a full copy of the database next to the live one,
//!   so peak disk use is roughly 2x the database size for the duration of the
//!   check.
//! - **Shipping pauses during the snapshot copy.** The sync task is
//!   sequential, so while a snapshot body is copied and uploaded no WAL
//!   segments ship; the recovery point objective widens to the copy duration
//!   (bounded by the throttle) and then catches up.
//! - **S3 targets want an incomplete-multipart lifecycle rule.** Aborted or
//!   crashed snapshot uploads leave orphaned multipart parts that this crate
//!   does not garbage-collect; configure a bucket lifecycle rule to expire
//!   incomplete multipart uploads.

// Dev-dependency used by the integration tests in tests/, not the lib tests.
#[cfg(test)]
use futures as _;

mod backoff;
mod checkpoint;
mod config;
mod local;
mod meta;
mod position;
mod replicator;
mod restore;
mod s3;
mod segment;
mod snapshot;
mod snapshot_meta;
mod storage;
mod sync;
mod verify;
mod wal;

#[cfg(any(test, feature = "testing"))]
pub mod testing;

pub use backoff::Backoff;
pub use checkpoint::{CheckpointError, CheckpointOutcome};
pub use config::{DEFAULT_SHUTDOWN_SYNC_TIMEOUT, ReplicaConfig, ReplicaConfigError};
pub use local::LocalStorage;
pub use meta::{MetaError, ReplicaMeta};
pub use position::{GenerationId, Position, SegmentKey};
pub use replicator::{ReplicaDb, Replicator, ReplicatorHandle};
pub use restore::{RestoreError, RestoreOutcome, restore_if_missing};
pub use s3::S3Storage;
pub use segment::{SEGMENT_MAX_BYTES, SegmentError, compress_segment, decompress_segment};
pub use snapshot::{SnapshotError, SnapshotStatus};
pub use snapshot_meta::{SnapshotMeta, SnapshotMetaError, WalBoundary};
pub use storage::{MultipartUpload, ReplicaStorage, StorageError};
pub use sync::{EngineState, SyncEngine, SyncError, SyncProgress};
pub use verify::{VerifyError, VerifyReport, fingerprint_database, verify_against_replica};
pub use wal::{
    CommittedChunk, FRAME_HEADER_SIZE, FrameHeader, WAL_HEADER_SIZE, WalError, WalHeader,
    WalScanner, wal_checksum,
};
