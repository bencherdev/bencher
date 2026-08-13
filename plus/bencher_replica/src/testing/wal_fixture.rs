//! Real `SQLite` WAL fixtures generated deterministically via rusqlite.
//!
//! Not byte-deterministic across runs (`SQLite` randomizes salts); assertions
//! must be structural/invariant-based, never golden-byte.

use camino::{Utf8Path, Utf8PathBuf};

/// A live `SQLite` database in WAL mode (`wal_autocheckpoint = 0`,
/// `synchronous = NORMAL`) with helpers to script commits, spills, and
/// checkpoints.
pub struct WalFixture {
    dir: Utf8PathBuf,
    conn: rusqlite::Connection,
}

/// Failures scripting a [`WalFixture`].
#[derive(Debug, thiserror::Error)]
pub enum FixtureError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("Failed to read fixture WAL: {0}")]
    Io(#[from] std::io::Error),
    #[error(
        "Big transaction did not spill: WAL unchanged at {wal_len} bytes; the test cannot prove anything"
    )]
    NoSpill { wal_len: usize },
}

impl WalFixture {
    /// Create a fresh DB at `<dir>/fixture.db` with the given page size and
    /// a default `t(id INTEGER PRIMARY KEY, data TEXT)` table.
    pub fn new(dir: &Utf8Path, page_size: u32) -> Result<Self, rusqlite::Error> {
        let db_path = dir.join("fixture.db");
        let conn = rusqlite::Connection::open(&db_path)?;
        // page_size must be set BEFORE the database is created by the switch
        // to WAL mode; it is immutable afterwards
        conn.pragma_update(None, "page_size", page_size)?;
        let _mode: String = conn.query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))?;
        disable_autocheckpoint(&conn)?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.execute_batch(
            "CREATE TABLE t (id INTEGER PRIMARY KEY, data TEXT);
             INSERT INTO t (data) VALUES ('init');",
        )?;
        Ok(Self {
            dir: dir.to_owned(),
            conn,
        })
    }

    /// Path to the database file.
    #[must_use]
    pub fn db_path(&self) -> Utf8PathBuf {
        self.dir.join("fixture.db")
    }

    /// Path to the WAL file.
    #[must_use]
    pub fn wal_path(&self) -> Utf8PathBuf {
        self.dir.join("fixture.db-wal")
    }

    /// Execute the statements inside a single transaction (one commit frame).
    pub fn txn(&self, statements: &[&str]) -> Result<(), rusqlite::Error> {
        self.conn.execute_batch("BEGIN IMMEDIATE")?;
        for statement in statements {
            if let Err(err) = self.conn.execute_batch(statement) {
                let _rollback = self.conn.execute_batch("ROLLBACK");
                return Err(err);
            }
        }
        self.conn.execute_batch("COMMIT")
    }

    /// A single transaction touching at least `pages` distinct pages
    /// (multi-frame single commit).
    pub fn txn_touching_pages(&self, pages: u32) -> Result<(), rusqlite::Error> {
        let page_size: u32 = self
            .conn
            .query_row("PRAGMA page_size", [], |row| row.get(0))?;
        // One row per page: a payload of page_size bytes forces at least one
        // overflow page per row
        let data = "x".repeat(page_size as usize);
        self.conn.execute_batch("BEGIN IMMEDIATE")?;
        let result = (|| {
            let mut statement = self.conn.prepare("INSERT INTO t (data) VALUES (?1)")?;
            for _ in 0..pages {
                statement.execute([&data])?;
            }
            Ok(())
        })();
        match result {
            Ok(()) => self.conn.execute_batch("COMMIT"),
            Err(err) => {
                let _rollback = self.conn.execute_batch("ROLLBACK");
                Err(err)
            },
        }
    }

    /// A large transaction with a tiny page cache so `SQLite` spills
    /// uncommitted frames into the WAL mid-transaction, then commits.
    /// With `commit` false, the transaction is rolled back after spilling,
    /// leaving flushed-but-uncommitted frames in the WAL.
    ///
    /// Errors with [`FixtureError::NoSpill`] if the WAL did not grow before
    /// the commit/rollback: a test relying on spilled frames must not pass
    /// silently without them.
    pub fn big_txn_spilling(&self, commit: bool) -> Result<(), FixtureError> {
        // A 10-page cache guarantees a multi-hundred-KiB transaction spills.
        // cache_spill must be set explicitly: some builds (notably Apple's
        // system SQLite) raise the default spill threshold to 20000 pages,
        // which would silently prevent any spilling here.
        self.conn.execute_batch("PRAGMA cache_size = 10")?;
        self.conn.execute_batch("PRAGMA cache_spill = 10")?;
        let wal_before = self.wal_snapshot()?;
        self.conn.execute_batch("BEGIN IMMEDIATE")?;
        let spilled = self.insert_spilling_rows(&wal_before);
        let end = if commit && spilled.is_ok() {
            "COMMIT"
        } else {
            "ROLLBACK"
        };
        let end_result = self.conn.execute_batch(end);
        // Restore the default cache size regardless of the outcome
        let _restore = self.conn.execute_batch("PRAGMA cache_size = -2000");
        spilled?;
        end_result?;
        Ok(())
    }

    /// Run a checkpoint in the given mode on a dedicated connection.
    pub fn checkpoint(&self, mode: CheckpointMode) -> Result<(), rusqlite::Error> {
        let conn = rusqlite::Connection::open(self.db_path())?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        // wal_checkpoint returns a (busy, log, checkpointed) row
        let _busy: i64 = conn.query_row(
            &format!("PRAGMA wal_checkpoint({})", mode.as_str()),
            [],
            |row| row.get(0),
        )?;
        Ok(())
    }

    /// Open a second, independent connection to the same DB (a "stray"
    /// writer bypassing any app-level mutex).
    pub fn stray_conn(&self) -> Result<rusqlite::Connection, rusqlite::Error> {
        let conn = rusqlite::Connection::open(self.db_path())?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        disable_autocheckpoint(&conn)?;
        Ok(conn)
    }

    /// Raw bytes of the current WAL file.
    pub fn wal_bytes(&self) -> Result<Vec<u8>, std::io::Error> {
        std::fs::read(self.wal_path())
    }

    /// Raw bytes of the current DB file.
    pub fn db_bytes(&self) -> Result<Vec<u8>, std::io::Error> {
        std::fs::read(self.db_path())
    }

    /// Insert rows until well past the page cache, then verify the WAL
    /// changed BEFORE the enclosing transaction ends (proof that `SQLite`
    /// spilled uncommitted frames).
    ///
    /// The comparison is by content, not length: a second spill can
    /// overwrite rolled-back frames from an earlier spill without growing
    /// the file. A per-call nonce in the row data (from `total_changes()`,
    /// which never resets, not even on rollback) guarantees the new frames
    /// differ from any stale ones they overwrite.
    fn insert_spilling_rows(&self, wal_before: &[u8]) -> Result<(), FixtureError> {
        let nonce: i64 = self
            .conn
            .query_row("SELECT total_changes()", [], |row| row.get(0))?;
        let data = format!("{nonce}-{}", "x".repeat(512));
        let mut statement = self.conn.prepare("INSERT INTO t (data) VALUES (?1)")?;
        for _ in 0..400 {
            statement.execute([&data])?;
        }
        let wal_mid = self.wal_snapshot()?;
        if wal_mid == wal_before {
            return Err(FixtureError::NoSpill {
                wal_len: wal_mid.len(),
            });
        }
        Ok(())
    }

    /// Current WAL bytes (empty if the file does not exist yet).
    fn wal_snapshot(&self) -> Result<Vec<u8>, std::io::Error> {
        match std::fs::read(self.wal_path()) {
            Ok(bytes) => Ok(bytes),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(err) => Err(err),
        }
    }
}

/// Checkpoint mode for [`WalFixture::checkpoint`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckpointMode {
    Passive,
    Full,
    Restart,
    Truncate,
}

impl CheckpointMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Passive => "PASSIVE",
            Self::Full => "FULL",
            Self::Restart => "RESTART",
            Self::Truncate => "TRUNCATE",
        }
    }
}

/// `wal_autocheckpoint = 0`: the fixture (and its stray connections) never
/// checkpoint behind a test's back (invariant I2).
fn disable_autocheckpoint(conn: &rusqlite::Connection) -> Result<(), rusqlite::Error> {
    let _pages: i64 = conn.query_row("PRAGMA wal_autocheckpoint = 0", [], |row| row.get(0))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    fn tempdir_path(dir: &tempfile::TempDir) -> &Utf8Path {
        Utf8Path::from_path(dir.path()).unwrap()
    }

    fn row_count(conn: &rusqlite::Connection) -> i64 {
        conn.query_row("SELECT COUNT(*) FROM t", [], |row| row.get(0))
            .unwrap()
    }

    #[test]
    fn new_creates_wal_mode_db() {
        let tmp = tempfile::tempdir().unwrap();
        let fixture = WalFixture::new(tempdir_path(&tmp), 4096).unwrap();
        assert!(fixture.db_path().exists(), "db file exists");
        assert!(fixture.wal_path().exists(), "wal file exists");
        assert!(
            !fixture.wal_bytes().unwrap().is_empty(),
            "table creation left frames in the WAL"
        );
        let mode: String = fixture
            .conn
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .unwrap();
        assert_eq!(mode, "wal");
        let autocheckpoint: i64 = fixture
            .conn
            .query_row("PRAGMA wal_autocheckpoint", [], |row| row.get(0))
            .unwrap();
        assert_eq!(autocheckpoint, 0, "fixture is the sole checkpointer");
        let page_size: i64 = fixture
            .conn
            .query_row("PRAGMA page_size", [], |row| row.get(0))
            .unwrap();
        assert_eq!(page_size, 4096);
    }

    #[test]
    fn txn_commits_statements_atomically() {
        let tmp = tempfile::tempdir().unwrap();
        let fixture = WalFixture::new(tempdir_path(&tmp), 4096).unwrap();
        let before = row_count(&fixture.conn);
        fixture
            .txn(&[
                "INSERT INTO t (data) VALUES ('a')",
                "INSERT INTO t (data) VALUES ('b')",
            ])
            .unwrap();
        assert_eq!(row_count(&fixture.conn), before + 2);
        // A failing statement rolls the whole transaction back
        fixture
            .txn(&[
                "INSERT INTO t (data) VALUES ('c')",
                "INSERT INTO no_such_table (data) VALUES ('d')",
            ])
            .unwrap_err();
        assert_eq!(row_count(&fixture.conn), before + 2, "nothing committed");
    }

    #[test]
    fn checkpoint_truncate_resets_wal() {
        let tmp = tempfile::tempdir().unwrap();
        let fixture = WalFixture::new(tempdir_path(&tmp), 4096).unwrap();
        assert!(!fixture.wal_bytes().unwrap().is_empty());
        fixture.checkpoint(CheckpointMode::Truncate).unwrap();
        assert!(
            fixture.wal_bytes().unwrap().is_empty(),
            "TRUNCATE resets the WAL to zero bytes"
        );
        // Data survives the checkpoint
        assert!(row_count(&fixture.conn) >= 1, "rows persist");
    }

    #[test]
    fn big_txn_spilling_rollback_leaves_uncommitted_frames() {
        let tmp = tempfile::tempdir().unwrap();
        let fixture = WalFixture::new(tempdir_path(&tmp), 4096).unwrap();
        let before_rows = row_count(&fixture.conn);
        let before_len = fixture.wal_bytes().unwrap().len();
        fixture.big_txn_spilling(false).unwrap();
        assert_eq!(
            row_count(&fixture.conn),
            before_rows,
            "rollback leaves no rows"
        );
        assert!(
            fixture.wal_bytes().unwrap().len() > before_len,
            "spilled frames remain in the WAL after rollback"
        );
    }

    #[test]
    fn big_txn_spilling_commit_persists_rows() {
        let tmp = tempfile::tempdir().unwrap();
        let fixture = WalFixture::new(tempdir_path(&tmp), 4096).unwrap();
        let before_rows = row_count(&fixture.conn);
        fixture.big_txn_spilling(true).unwrap();
        assert!(
            row_count(&fixture.conn) > before_rows,
            "committed spill persists its rows"
        );
    }

    #[test]
    fn stray_conn_writes_independently() {
        let tmp = tempfile::tempdir().unwrap();
        let fixture = WalFixture::new(tempdir_path(&tmp), 4096).unwrap();
        let stray = fixture.stray_conn().unwrap();
        let autocheckpoint: i64 = stray
            .query_row("PRAGMA wal_autocheckpoint", [], |row| row.get(0))
            .unwrap();
        assert_eq!(autocheckpoint, 0, "stray connections never autocheckpoint");
        let before = row_count(&fixture.conn);
        stray
            .execute("INSERT INTO t (data) VALUES ('stray')", [])
            .unwrap();
        assert_eq!(
            row_count(&fixture.conn),
            before + 1,
            "stray write is visible through the fixture connection"
        );
    }
}
