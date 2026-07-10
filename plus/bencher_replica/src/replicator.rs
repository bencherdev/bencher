//! Public entry points: start the replicator task, shut it down with a
//! final ship, and expose the fatal-error future for the server's race.
//!
//! The production shell is deliberately trivial: one tokio task ticking
//! [`SyncEngine::sync_once`] every `sync_interval`. Engine construction
//! happens INSIDE the spawned task so [`Replicator::start`] is synchronous
//! and fast; a retryable (storage) construction failure is retried with
//! capped backoff (an unreachable replica at boot must never be mistaken
//! for an empty one), while any other construction failure surfaces through
//! [`ReplicatorHandle::wait_fatal`]. Per-tick storage errors are NOT fatal:
//! the engine backs off internally and the WAL is the buffer.

use std::sync::Arc;
use std::time::Duration;

use bencher_json::Clock;
use bencher_json::system::config::ReplicationTarget;
use camino::Utf8PathBuf;
use slog::Logger;
use tokio::sync::{mpsc, oneshot};
use tokio::time::timeout;

use crate::backoff::Backoff;
use crate::config::ReplicaConfig;
use crate::sync::{SyncEngine, SyncError};

/// The only thing the replicator needs from the application, generic over
/// the writer connection type so this crate never depends on diesel or the
/// schema: the engine only ever HOLDS the mutex guard, never uses `C`.
pub struct ReplicaDb<C> {
    pub db_path: Utf8PathBuf,
    /// The app's single-writer mutex; the checkpoint critical section
    /// acquires it so app writers queue on tokio instead of burning their
    /// `SQLite` `busy_timeout`.
    pub writer: Arc<tokio::sync::Mutex<C>>,
    /// The app's configured busy timeout, for observability parity.
    pub busy_timeout_ms: u32,
}

// Manual impl: `#[derive(Clone)]` would wrongly require `C: Clone`.
impl<C> Clone for ReplicaDb<C> {
    fn clone(&self) -> Self {
        Self {
            db_path: self.db_path.clone(),
            writer: Arc::clone(&self.writer),
            busy_timeout_ms: self.busy_timeout_ms,
        }
    }
}

/// Starts the single replication task.
pub struct Replicator;

impl Replicator {
    /// Spawn the replication task and return its handle. Construction and
    /// resume run inside the task; see the module docs for the failure
    /// contract.
    pub fn start<C: Send + 'static>(
        log: Logger,
        config: ReplicaConfig,
        db: ReplicaDb<C>,
        clock: Clock,
        shadow: bool,
    ) -> ReplicatorHandle {
        let (commands, command_rx) = mpsc::channel(1);
        let (fatal_tx, fatal_rx) = oneshot::channel();
        drop(tokio::spawn(run_replicator(
            log, config, db, clock, shadow, command_rx, fatal_tx,
        )));
        ReplicatorHandle {
            commands,
            fatal: Some(fatal_rx),
        }
    }
}

/// Handle to the running replication task.
pub struct ReplicatorHandle {
    commands: mpsc::Sender<Command>,
    fatal: Option<oneshot::Receiver<SyncError>>,
}

impl ReplicatorHandle {
    /// Ship the remaining WAL tail (`deadline`-bounded) and stop the task.
    /// After a COMPLETE drain in sole mode a final checkpoint runs (after the
    /// deadline, unbounded) to seal the epoch so the next boot resumes in
    /// place rather than re-snapshotting; see [`SyncEngine::final_sync`]. On
    /// the deadline the un-shipped tail simply stays in the local WAL and the
    /// next boot resumes by salt match (lag, never loss).
    ///
    /// PRECONDITION: application writers must already be quiesced (the server
    /// drained) before calling this. The final checkpoint acquires the app
    /// writer mutex with no deadline of its own, and this call awaits the
    /// task's completion unboundedly; a writer still holding the mutex would
    /// stall shutdown. The production caller orders `shutdown(server)` before
    /// `replica_handle.shutdown(deadline)` for exactly this reason.
    pub async fn shutdown(self, deadline: Duration) -> Result<(), SyncError> {
        let Self { commands, fatal: _ } = self;
        let (done, done_rx) = oneshot::channel();
        if commands
            .send(Command::Shutdown { deadline, done })
            .await
            .is_err()
        {
            return Err(SyncError::TaskExited);
        }
        match done_rx.await {
            Ok(result) => result,
            Err(_closed) => Err(SyncError::TaskExited),
        }
    }

    /// Resolves only if the replication task DIED: a non-retryable
    /// construction failure, a fatal tick error, or a panic. In normal
    /// operation (including across storage outages, which back off
    /// internally) this future NEVER resolves; it exists for the server's
    /// shutdown race.
    pub async fn wait_fatal(&mut self) -> Result<(), SyncError> {
        // Take the receiver inside the async body, on first POLL rather than
        // at call time: constructing this future and dropping it unpolled
        // must not disarm fatal detection for a later call.
        let Some(receiver) = self.fatal.take() else {
            // Already consumed by an earlier call: nothing further can
            // ever be reported.
            return std::future::pending().await;
        };
        match receiver.await {
            Ok(error) => Err(error),
            // The sender dropped without a fatal report while this
            // handle still exists: the task panicked.
            Err(_closed) => Err(SyncError::TaskExited),
        }
    }
}

enum Command {
    Shutdown {
        deadline: Duration,
        done: oneshot::Sender<Result<(), SyncError>>,
    },
}

/// The task body: construct (with storage-retry), then tick until shutdown.
async fn run_replicator<C: Send>(
    log: Logger,
    config: ReplicaConfig,
    db: ReplicaDb<C>,
    clock: Clock,
    shadow: bool,
    mut commands: mpsc::Receiver<Command>,
    fatal: oneshot::Sender<SyncError>,
) {
    // Endpoint overrides are for S3-compatible stores on trusted networks;
    // still, plaintext transit deserves a loud line in the boot log. SigV4
    // never transmits the secret, so only the replica DATA is exposed.
    if let ReplicationTarget::S3 {
        endpoint: Some(endpoint),
        ..
    } = &config.target
        && endpoint
            .get(..7)
            .is_some_and(|scheme| scheme.eq_ignore_ascii_case("http://"))
    {
        slog::warn!(log, "Replica S3 endpoint uses plaintext http; replica data transits unencrypted";
            "endpoint" => endpoint.as_str());
    }
    // A zero interval would busy-spin the tick loop.
    let tick = if config.sync_interval.is_zero() {
        Duration::from_secs(1)
    } else {
        config.sync_interval
    };
    let mut construction_backoff = Backoff::default();
    let mut engine = loop {
        match SyncEngine::new(
            log.clone(),
            config.clone(),
            db.clone(),
            clock.clone(),
            shadow,
        )
        .await
        {
            Ok(engine) => break engine,
            Err(error) => {
                // Every failed construction attempt is an adverse event: a
                // retryable failure (e.g. bad S3 credentials) otherwise retries
                // forever with only WARN logs and no metric to alert on.
                #[cfg(feature = "otel")]
                bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::ReplicaInitFailed);
                if error.is_retryable() {
                    let delay = construction_backoff.next_delay();
                    slog::warn!(log, "Replica unreachable at startup; retrying resume";
                        "error" => %error, "retry_in_secs" => delay.as_secs());
                    match timeout(delay, commands.recv()).await {
                        Ok(Some(Command::Shutdown { done, .. })) => {
                            // Nothing was running yet; nothing to ship.
                            drop(done.send(Ok(())));
                            return;
                        },
                        Ok(None) => return,
                        Err(_elapsed) => {},
                    }
                } else {
                    slog::error!(log, "Replicator failed to start"; "error" => %error);
                    drop(fatal.send(error));
                    return;
                }
            },
        }
    };
    // Best-effort reclamation of multipart uploads orphaned by an earlier
    // crash mid-snapshot: each orphan otherwise accrues storage cost until a
    // bucket lifecycle rule (if any) reaps it. Runs once per boot, inside
    // this task so server startup is never delayed, and never fails the
    // replicator.
    engine.storage().abort_incomplete_uploads(&log).await;
    loop {
        match timeout(tick, commands.recv()).await {
            Ok(Some(Command::Shutdown { deadline, done })) => {
                let result = engine.final_sync(deadline).await;
                drop(done.send(result));
                return;
            },
            Ok(None) => {
                slog::warn!(
                    log,
                    "Replicator handle dropped without shutdown; replication stops"
                );
                return;
            },
            Err(_elapsed) => match engine.sync_once().await {
                Ok(progress) => {
                    if let Some(error) = &progress.error {
                        slog::warn!(log, "Sync tick failed; backing off"; "error" => %error);
                    }
                },
                Err(error) => {
                    slog::error!(log, "Replicator died"; "error" => %error);
                    drop(fatal.send(error));
                    return;
                },
            },
        }
    }
}
