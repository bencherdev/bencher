//! Concurrent-writer probe: measures whether (and for how long) a real
//! second connection is blocked while the replicator holds locks.
//!
//! Each write runs on a fresh rusqlite connection inside `spawn_blocking`,
//! with a configurable `busy_timeout` and `wal_autocheckpoint = 0` (the
//! probe is a writer, so invariant I2 applies to it too). The probe writes
//! into its own `probe_writes` table, created on first use.

use std::future::Future;
use std::task::Poll;
use std::time::{Duration, Instant};

use camino::{Utf8Path, Utf8PathBuf};
use tokio::task::spawn_blocking;

/// Result of one probe write attempt.
#[derive(Debug)]
pub struct ProbeResult {
    /// The write's outcome; `Err` carries the `SQLite` error (e.g. busy).
    pub result: Result<(), rusqlite::Error>,
    /// Wall-clock duration the write took (observed block time).
    pub blocked: Duration,
}

/// Spawns blocking write attempts against a database on dedicated
/// connections with a configurable `busy_timeout`.
pub struct WriteProbe {
    db_path: Utf8PathBuf,
    busy_timeout: Duration,
}

impl WriteProbe {
    #[must_use]
    pub fn new(db_path: &Utf8Path, busy_timeout: Duration) -> Self {
        Self {
            db_path: db_path.to_owned(),
            busy_timeout,
        }
    }

    /// Perform one INSERT on a fresh connection, reporting the outcome and
    /// observed block duration.
    pub async fn write_once(&self) -> ProbeResult {
        let db_path = self.db_path.clone();
        let busy_timeout = self.busy_timeout;
        let outcome = spawn_blocking(move || {
            let started = Instant::now();
            let result = probe_write(&db_path, busy_timeout);
            ProbeResult {
                result,
                blocked: started.elapsed(),
            }
        })
        .await;
        match outcome {
            Ok(probe) => probe,
            // Surface a task panic as a failed write rather than panicking
            // the caller: the hammer keeps collecting.
            Err(_join_error) => ProbeResult {
                result: Err(rusqlite::Error::InvalidQuery),
                blocked: Duration::ZERO,
            },
        }
    }

    /// Hammer writes continuously until `stop` resolves, returning all
    /// probe results. `stop` is polled between writes, so the final write
    /// always runs to completion.
    pub async fn hammer<F>(&self, stop: F) -> Vec<ProbeResult>
    where
        F: Future<Output = ()> + Send,
    {
        let mut results = Vec::new();
        let mut stop = std::pin::pin!(stop);
        loop {
            let stopped =
                std::future::poll_fn(|cx| Poll::Ready(stop.as_mut().poll(cx).is_ready())).await;
            if stopped {
                return results;
            }
            results.push(self.write_once().await);
        }
    }
}

/// One synchronous probe write on a fresh connection.
fn probe_write(db_path: &Utf8Path, busy_timeout: Duration) -> Result<(), rusqlite::Error> {
    let conn = rusqlite::Connection::open(db_path)?;
    conn.busy_timeout(busy_timeout)?;
    // The probe is a writer: it must never checkpoint behind the
    // replicator's back (invariant I2).
    let _pages: i64 = conn.query_row("PRAGMA wal_autocheckpoint = 0", [], |row| row.get(0))?;
    conn.execute_batch("CREATE TABLE IF NOT EXISTS probe_writes(id INTEGER PRIMARY KEY, at TEXT)")?;
    conn.execute("INSERT INTO probe_writes (at) VALUES ('probe')", [])?;
    Ok(())
}
