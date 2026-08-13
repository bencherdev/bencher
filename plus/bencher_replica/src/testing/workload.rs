//! Seeded workload generator and script runner: the property-testing
//! replacement.
//!
//! Workloads are explicit, replayable `Vec<WorkloadOp>` scripts generated
//! from a fixed seed (`rand::rngs::StdRng::seed_from_u64`) with weighted op
//! selection; SQL operations and engine events interleave from the same
//! seed. [`WorkloadRunner`] applies the SQL ops to the source database via
//! rusqlite (creating `t0..t15` tables lazily on first touch) and maps the
//! engine events onto step-driven [`SyncEngine`] calls. Every failure is
//! wrapped in a [`WorkloadError`] carrying the seed, the failing op index,
//! and the op itself, so the calling test can print the full script
//! alongside.
//!
//! Out of scope by design: `ATTACH`, page-size changes, journal-mode flips,
//! `WITHOUT ROWID` tables, multi-process writers.

use std::fmt;
use std::time::Duration;

use bencher_json::Clock;
use camino::{Utf8Path, Utf8PathBuf};
use rand::rngs::StdRng;
use rand::{RngExt as _, SeedableRng as _};
use slog::Logger;

use crate::checkpoint::CheckpointOutcome;
use crate::config::ReplicaConfig;
use crate::local::LocalStorage;
use crate::replicator::ReplicaDb;
use crate::snapshot::SnapshotStatus;
use crate::storage::ReplicaStorage;
use crate::sync::{EngineState, SyncEngine, SyncError};

use super::flaky::{FailurePlan, FlakyStorage};

/// One step of a generated workload: either a SQL-level operation on the
/// source database or an engine event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkloadOp {
    Insert {
        table: u8,
        rows: u16,
    },
    Update {
        table: u8,
        rows: u16,
    },
    Delete {
        table: u8,
        rows: u16,
    },
    CreateTable {
        table: u8,
    },
    CreateIndex {
        table: u8,
    },
    DropTable {
        table: u8,
    },
    /// Forces overflow pages.
    BlobWrite {
        table: u8,
        len: u32,
    },
    /// 1..=50 statements in a single commit.
    BigTxn {
        statements: u8,
    },
    UserVersionBump,
    /// Full-DB rewrite through the WAL; low weight, great frame-burst
    /// stress.
    Vacuum,
    // Engine events, interleaved by the same seed:
    Sync,
    Checkpoint,
    Snapshot,
    RestartReplicator,
    /// A write through a second connection bypassing the app writer mutex.
    StrayWrite,
}

impl WorkloadOp {
    /// Number of [`WorkloadOp`] variants (kept in lockstep with the weight
    /// table by a unit test).
    pub const VARIANT_COUNT: usize = 15;

    /// The variant name, for coverage accounting in tests.
    #[must_use]
    pub const fn kind_name(&self) -> &'static str {
        match self {
            Self::Insert { .. } => "Insert",
            Self::Update { .. } => "Update",
            Self::Delete { .. } => "Delete",
            Self::CreateTable { .. } => "CreateTable",
            Self::CreateIndex { .. } => "CreateIndex",
            Self::DropTable { .. } => "DropTable",
            Self::BlobWrite { .. } => "BlobWrite",
            Self::BigTxn { .. } => "BigTxn",
            Self::UserVersionBump => "UserVersionBump",
            Self::Vacuum => "Vacuum",
            Self::Sync => "Sync",
            Self::Checkpoint => "Checkpoint",
            Self::Snapshot => "Snapshot",
            Self::RestartReplicator => "RestartReplicator",
            Self::StrayWrite => "StrayWrite",
        }
    }
}

impl fmt::Display for WorkloadOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Insert { table, rows } => write!(f, "Insert(t{table} x{rows})"),
            Self::Update { table, rows } => write!(f, "Update(t{table} x{rows})"),
            Self::Delete { table, rows } => write!(f, "Delete(t{table} x{rows})"),
            Self::CreateTable { table } => write!(f, "CreateTable(t{table})"),
            Self::CreateIndex { table } => write!(f, "CreateIndex(t{table})"),
            Self::DropTable { table } => write!(f, "DropTable(t{table})"),
            Self::BlobWrite { table, len } => write!(f, "BlobWrite(t{table}, {len} bytes)"),
            Self::BigTxn { statements } => write!(f, "BigTxn({statements} statements)"),
            Self::UserVersionBump
            | Self::Vacuum
            | Self::Sync
            | Self::Checkpoint
            | Self::Snapshot
            | Self::RestartReplicator
            | Self::StrayWrite => f.write_str(self.kind_name()),
        }
    }
}

/// Generate a deterministic workload of `len` ops from `seed`.
#[must_use]
pub fn generate_workload(seed: u64, len: usize) -> Vec<WorkloadOp> {
    let mut rng = StdRng::seed_from_u64(seed);
    std::iter::repeat_with(|| sample_op(&mut rng))
        .take(len)
        .collect()
}

/// The `t0..t15` table namespace size.
const TABLE_COUNT: u8 = 16;

/// The weight table: mostly `Insert`/`Update`/`Sync`, every variant
/// reachable well within 200 ops across typical seeds (validated by unit
/// tests). Weights sum to [`WEIGHT_TOTAL`].
const WEIGHTED: [(Shape, u32); WorkloadOp::VARIANT_COUNT] = [
    (Shape::Insert, 24),
    (Shape::Sync, 22),
    (Shape::Update, 14),
    (Shape::Delete, 6),
    (Shape::Checkpoint, 6),
    (Shape::BlobWrite, 5),
    (Shape::BigTxn, 4),
    (Shape::CreateTable, 4),
    (Shape::CreateIndex, 3),
    (Shape::DropTable, 3),
    (Shape::UserVersionBump, 2),
    (Shape::Snapshot, 2),
    (Shape::RestartReplicator, 2),
    (Shape::StrayWrite, 2),
    (Shape::Vacuum, 1),
];

/// Sum of the weights in [`WEIGHTED`] (checked by a unit test).
const WEIGHT_TOTAL: u32 = 100;

/// A [`WorkloadOp`] variant without its parameters, for weighted sampling.
#[derive(Debug, Clone, Copy)]
enum Shape {
    Insert,
    Update,
    Delete,
    CreateTable,
    CreateIndex,
    DropTable,
    BlobWrite,
    BigTxn,
    UserVersionBump,
    Vacuum,
    Sync,
    Checkpoint,
    Snapshot,
    RestartReplicator,
    StrayWrite,
}

/// Draw one weighted op from the rng.
fn sample_op(rng: &mut StdRng) -> WorkloadOp {
    let mut roll = rng.random_range(0..WEIGHT_TOTAL);
    for (shape, weight) in WEIGHTED {
        if roll < weight {
            return instantiate(shape, rng);
        }
        roll -= weight;
    }
    // Unreachable while the weights sum to WEIGHT_TOTAL (unit-tested); fall
    // back to the cheapest op rather than panicking.
    WorkloadOp::Sync
}

/// Fill in the parameters for a sampled op shape.
fn instantiate(shape: Shape, rng: &mut StdRng) -> WorkloadOp {
    match shape {
        Shape::Insert => WorkloadOp::Insert {
            table: rng.random_range(0..TABLE_COUNT),
            rows: rng.random_range(1..=20),
        },
        Shape::Update => WorkloadOp::Update {
            table: rng.random_range(0..TABLE_COUNT),
            rows: rng.random_range(1..=10),
        },
        Shape::Delete => WorkloadOp::Delete {
            table: rng.random_range(0..TABLE_COUNT),
            rows: rng.random_range(1..=10),
        },
        Shape::CreateTable => WorkloadOp::CreateTable {
            table: rng.random_range(0..TABLE_COUNT),
        },
        Shape::CreateIndex => WorkloadOp::CreateIndex {
            table: rng.random_range(0..TABLE_COUNT),
        },
        Shape::DropTable => WorkloadOp::DropTable {
            table: rng.random_range(0..TABLE_COUNT),
        },
        Shape::BlobWrite => WorkloadOp::BlobWrite {
            table: rng.random_range(0..TABLE_COUNT),
            // Well past any page size, forcing overflow pages.
            len: rng.random_range(4096..=64 * 1024),
        },
        Shape::BigTxn => WorkloadOp::BigTxn {
            statements: rng.random_range(1..=50),
        },
        Shape::UserVersionBump => WorkloadOp::UserVersionBump,
        Shape::Vacuum => WorkloadOp::Vacuum,
        Shape::Sync => WorkloadOp::Sync,
        Shape::Checkpoint => WorkloadOp::Checkpoint,
        Shape::Snapshot => WorkloadOp::Snapshot,
        Shape::RestartReplicator => WorkloadOp::RestartReplicator,
        Shape::StrayWrite => WorkloadOp::StrayWrite,
    }
}

/// Everything needed to (re)build the engine over the same directories:
/// `RestartReplicator` simulates a process crash by dropping the engine and
/// resuming a fresh one against the same replica root.
pub struct WorkloadEnv<C> {
    pub log: Logger,
    pub config: ReplicaConfig,
    pub db: ReplicaDb<C>,
    pub clock: Clock,
    /// Root of the local replica directory; rebuilt engines wrap it in a
    /// fresh `Flaky(Local)` storage with an empty failure plan.
    pub replica_root: Utf8PathBuf,
}

/// Applies a workload script: SQL ops through dedicated rusqlite writer
/// connections, engine events through the step-driven [`SyncEngine`].
pub struct WorkloadRunner<C> {
    seed: u64,
    script: Vec<WorkloadOp>,
    next_index: usize,
    env: WorkloadEnv<C>,
    engine: SyncEngine<C>,
    /// The scripted "app" writer connection.
    conn: rusqlite::Connection,
    /// A second connection bypassing the app writer mutex (`StrayWrite`).
    stray: rusqlite::Connection,
    /// Bitset of which `t0..t15` tables currently exist.
    tables: u16,
}

/// A workload failure, carrying the seed and (for op failures) the failing
/// op index and op. The calling test prints the full script alongside.
#[derive(Debug, thiserror::Error)]
pub enum WorkloadError {
    #[error("workload seed {seed} setup: {source}")]
    Setup { seed: u64, source: rusqlite::Error },
    #[error("workload seed {seed} op {index} ({op}): {source}")]
    Op {
        seed: u64,
        index: usize,
        op: WorkloadOp,
        source: WorkloadOpError,
    },
    #[error("workload seed {seed} drain: {source}")]
    Drain { seed: u64, source: WorkloadOpError },
}

/// Why a single workload op failed.
#[derive(Debug, thiserror::Error)]
pub enum WorkloadOpError {
    #[error("SQLite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("engine step failed: {0}")]
    Engine(#[from] SyncError),
    #[error("sync tick reported an error: {0}")]
    Tick(SyncError),
    #[error("snapshot still in progress after {steps} steps")]
    SnapshotStalled { steps: u32 },
    #[error("engine failed to quiesce after {ticks} ticks (state: {})", state_name(*state))]
    DrainStalled { ticks: u32, state: EngineState },
}

/// Upper bound on [`SyncEngine::snapshot_step`] calls per `Snapshot` op.
const SNAPSHOT_MAX_STEPS: u32 = 10_000;

/// Upper bound on [`SyncEngine::sync_once`] calls in
/// [`WorkloadRunner::drain`].
const DRAIN_MAX_TICKS: u32 = 10_000;

impl<C> WorkloadRunner<C> {
    /// Open the writer connections over the (already existing, WAL-mode)
    /// source database and take ownership of the engine.
    pub fn new(
        seed: u64,
        script: Vec<WorkloadOp>,
        env: WorkloadEnv<C>,
        engine: SyncEngine<C>,
    ) -> Result<Self, WorkloadError> {
        let setup = |source| WorkloadError::Setup { seed, source };
        let conn = open_writer(&env.db.db_path).map_err(setup)?;
        let stray = open_writer(&env.db.db_path).map_err(setup)?;
        let tables = scan_tables(&conn).map_err(setup)?;
        Ok(Self {
            seed,
            script,
            next_index: 0,
            env,
            engine,
            conn,
            stray,
            tables,
        })
    }

    /// Apply the next script op; `Ok(None)` once the script is exhausted.
    pub async fn step(&mut self) -> Result<Option<AppliedOp>, WorkloadError> {
        let Some(op) = self.script.get(self.next_index).cloned() else {
            return Ok(None);
        };
        let index = self.next_index;
        self.next_index = index.saturating_add(1);
        let checkpoint = self
            .apply(index, &op)
            .await
            .map_err(|source| WorkloadError::Op {
                seed: self.seed,
                index,
                op: op.clone(),
                source,
            })?;
        Ok(Some(AppliedOp {
            index,
            op,
            checkpoint,
        }))
    }

    /// Apply every remaining script op.
    pub async fn run(&mut self) -> Result<(), WorkloadError> {
        while self.step().await?.is_some() {}
        Ok(())
    }

    /// Final drain: drive `sync_once` until any in-flight snapshot has
    /// finished and a tick ships nothing, so the replica holds every
    /// committed transaction.
    pub async fn drain(&mut self) -> Result<(), WorkloadError> {
        self.drain_inner()
            .await
            .map_err(|source| WorkloadError::Drain {
                seed: self.seed,
                source,
            })
    }

    /// The engine under test.
    #[must_use]
    pub fn engine(&self) -> &SyncEngine<C> {
        &self.engine
    }

    /// Mutable access to the engine under test.
    pub fn engine_mut(&mut self) -> &mut SyncEngine<C> {
        &mut self.engine
    }

    async fn drain_inner(&mut self) -> Result<(), WorkloadOpError> {
        for _tick in 0..DRAIN_MAX_TICKS {
            let progress = self.engine.sync_once().await?;
            if let Some(error) = progress.error {
                return Err(WorkloadOpError::Tick(error));
            }
            if progress.backing_off {
                continue;
            }
            // AwaitingEpoch counts as drained: it means everything shipped
            // and checkpointed, with no new frames to bind yet.
            let quiescent = progress.shipped_segments == 0
                && progress.snapshot.is_none()
                && matches!(
                    self.engine.state(),
                    EngineState::Streaming | EngineState::AwaitingEpoch
                );
            if quiescent {
                return Ok(());
            }
        }
        Err(WorkloadOpError::DrainStalled {
            ticks: DRAIN_MAX_TICKS,
            state: self.engine.state(),
        })
    }

    /// Dispatch one op. Returns the checkpoint outcome for `Checkpoint`.
    async fn apply(
        &mut self,
        index: usize,
        op: &WorkloadOp,
    ) -> Result<Option<CheckpointOutcome>, WorkloadOpError> {
        match op {
            WorkloadOp::Insert { table, rows } => self.insert(index, *table, *rows)?,
            WorkloadOp::Update { table, rows } => self.update(index, *table, *rows)?,
            WorkloadOp::Delete { table, rows } => self.delete(*table, *rows)?,
            WorkloadOp::CreateTable { table } => self.ensure_table(*table)?,
            WorkloadOp::CreateIndex { table } => self.create_index(*table)?,
            WorkloadOp::DropTable { table } => self.drop_table(*table)?,
            WorkloadOp::BlobWrite { table, len } => self.blob_write(index, *table, *len)?,
            WorkloadOp::BigTxn { statements } => self.big_txn(index, *statements)?,
            WorkloadOp::UserVersionBump => self.user_version_bump()?,
            WorkloadOp::Vacuum => self.conn.execute_batch("VACUUM")?,
            WorkloadOp::Sync => self.sync().await?,
            WorkloadOp::Checkpoint => return Ok(Some(self.checkpoint().await?)),
            WorkloadOp::Snapshot => self.snapshot().await?,
            WorkloadOp::RestartReplicator => self.restart().await?,
            WorkloadOp::StrayWrite => self.stray_write(index)?,
        }
        Ok(None)
    }

    async fn sync(&mut self) -> Result<(), WorkloadOpError> {
        let progress = self.engine.sync_once().await?;
        if let Some(error) = progress.error {
            return Err(WorkloadOpError::Tick(error));
        }
        Ok(())
    }

    /// Production `sync_once` ships before a due checkpoint; mirror that
    /// order so a scripted checkpoint can actually complete (I1).
    async fn checkpoint(&mut self) -> Result<CheckpointOutcome, WorkloadOpError> {
        match self.engine.state() {
            EngineState::Streaming | EngineState::AwaitingEpoch => {
                self.engine.ship_once().await?;
            },
            EngineState::PendingSnapshot | EngineState::Snapshotting => {},
        }
        Ok(self.engine.checkpoint_once().await?)
    }

    /// Trigger a new-generation snapshot and drive it to completion.
    async fn snapshot(&mut self) -> Result<(), WorkloadOpError> {
        self.engine.trigger_snapshot();
        for _step in 0..SNAPSHOT_MAX_STEPS {
            if self.engine.snapshot_step().await? == SnapshotStatus::Finished {
                return Ok(());
            }
        }
        Err(WorkloadOpError::SnapshotStalled {
            steps: SNAPSHOT_MAX_STEPS,
        })
    }

    /// Process-crash simulation: rebuild the engine over the same replica
    /// root and let the resume logic pick the position back up. The
    /// step-driven engine is inert between steps, so building the successor
    /// before the old engine drops is equivalent to drop-then-rebuild (the
    /// resume reads only on-disk and replica state).
    async fn restart(&mut self) -> Result<(), WorkloadOpError> {
        let storage = ReplicaStorage::Flaky(Box::new(FlakyStorage::new(
            ReplicaStorage::Local(LocalStorage::new(self.env.replica_root.clone())),
            FailurePlan::new(),
        )));
        self.engine = SyncEngine::new_with_storage(
            self.env.log.clone(),
            self.env.config.clone(),
            self.env.db.clone(),
            self.env.clock.clone(),
            false,
            storage,
        )
        .await?;
        Ok(())
    }

    fn insert(&mut self, index: usize, table: u8, rows: u16) -> Result<(), WorkloadOpError> {
        self.ensure_table(table)?;
        let sql = format!("INSERT INTO {} (data) VALUES (?1)", table_name(table));
        let seed = self.seed;
        self.in_txn(|conn| {
            let mut statement = conn.prepare(&sql)?;
            for row in 0..rows {
                statement.execute(rusqlite::params![format!("seed{seed}-op{index}-row{row}")])?;
            }
            Ok(())
        })?;
        Ok(())
    }

    fn update(&mut self, index: usize, table: u8, rows: u16) -> Result<(), WorkloadOpError> {
        if !self.table_exists(table) {
            // Deterministic fallback: updating an absent table becomes an
            // insert (which creates it), keeping scripts replayable.
            return self.insert(index, table, rows);
        }
        let sql = format!(
            "UPDATE {t} SET data = ?1 WHERE id IN (SELECT id FROM {t} ORDER BY id LIMIT ?2)",
            t = table_name(table)
        );
        self.conn.execute(
            &sql,
            rusqlite::params![
                format!("seed{}-op{index}-update", self.seed),
                i64::from(rows)
            ],
        )?;
        Ok(())
    }

    fn delete(&mut self, table: u8, rows: u16) -> Result<(), WorkloadOpError> {
        if !self.table_exists(table) {
            // Deterministic no-op on an absent table.
            return Ok(());
        }
        let sql = format!(
            "DELETE FROM {t} WHERE id IN (SELECT id FROM {t} ORDER BY id LIMIT ?1)",
            t = table_name(table)
        );
        self.conn
            .execute(&sql, rusqlite::params![i64::from(rows)])?;
        Ok(())
    }

    /// Create the table if it does not exist yet (lazy first touch).
    fn ensure_table(&mut self, table: u8) -> Result<(), WorkloadOpError> {
        if self.table_exists(table) {
            return Ok(());
        }
        self.conn.execute_batch(&create_table_sql(table))?;
        self.set_table(table, true);
        Ok(())
    }

    fn create_index(&mut self, table: u8) -> Result<(), WorkloadOpError> {
        self.ensure_table(table)?;
        let slot = table_slot(table);
        self.conn.execute_batch(&format!(
            "CREATE INDEX IF NOT EXISTS idx_t{slot} ON t{slot} (data)"
        ))?;
        Ok(())
    }

    fn drop_table(&mut self, table: u8) -> Result<(), WorkloadOpError> {
        if !self.table_exists(table) {
            // Deterministic no-op on an absent table.
            return Ok(());
        }
        self.conn
            .execute_batch(&format!("DROP TABLE IF EXISTS {}", table_name(table)))?;
        self.set_table(table, false);
        Ok(())
    }

    fn blob_write(&mut self, index: usize, table: u8, len: u32) -> Result<(), WorkloadOpError> {
        self.ensure_table(table)?;
        // u32 always fits usize on supported (64-bit) targets.
        let len = usize::try_from(len).unwrap_or_default();
        // The mask keeps the value in u8 range, so the conversion is total.
        let byte = u8::try_from(index & 0xff).unwrap_or_default();
        let blob = vec![byte; len];
        self.conn.execute(
            &format!("INSERT INTO {} (bin) VALUES (?1)", table_name(table)),
            rusqlite::params![blob],
        )?;
        Ok(())
    }

    /// `statements` inserts in one commit, always into table `t0`.
    fn big_txn(&mut self, index: usize, statements: u8) -> Result<(), WorkloadOpError> {
        const BIG_TABLE: u8 = 0;
        self.ensure_table(BIG_TABLE)?;
        let sql = format!("INSERT INTO {} (data) VALUES (?1)", table_name(BIG_TABLE));
        let seed = self.seed;
        self.in_txn(|conn| {
            let mut statement = conn.prepare(&sql)?;
            for n in 0..statements {
                statement.execute(rusqlite::params![format!("seed{seed}-op{index}-big{n}")])?;
            }
            Ok(())
        })?;
        Ok(())
    }

    fn user_version_bump(&self) -> Result<(), WorkloadOpError> {
        let version: i64 = self
            .conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))?;
        self.conn
            .pragma_update(None, "user_version", version.saturating_add(1))?;
        Ok(())
    }

    /// One insert through the second connection, always into table `t0`.
    fn stray_write(&mut self, index: usize) -> Result<(), WorkloadOpError> {
        const STRAY_TABLE: u8 = 0;
        if !self.table_exists(STRAY_TABLE) {
            self.stray.execute_batch(&create_table_sql(STRAY_TABLE))?;
            self.set_table(STRAY_TABLE, true);
        }
        self.stray.execute(
            &format!("INSERT INTO {} (data) VALUES (?1)", table_name(STRAY_TABLE)),
            rusqlite::params![format!("seed{}-op{index}-stray", self.seed)],
        )?;
        Ok(())
    }

    /// Run `body` inside a single `BEGIN IMMEDIATE` transaction (one commit
    /// frame set), rolling back on error.
    fn in_txn<F>(&self, body: F) -> Result<(), rusqlite::Error>
    where
        F: FnOnce(&rusqlite::Connection) -> Result<(), rusqlite::Error>,
    {
        self.conn.execute_batch("BEGIN IMMEDIATE")?;
        match body(&self.conn) {
            Ok(()) => self.conn.execute_batch("COMMIT"),
            Err(error) => {
                let _rollback = self.conn.execute_batch("ROLLBACK");
                Err(error)
            },
        }
    }

    fn table_exists(&self, table: u8) -> bool {
        self.tables & table_bit(table) != 0
    }

    fn set_table(&mut self, table: u8, exists: bool) {
        if exists {
            self.tables |= table_bit(table);
        } else {
            self.tables &= !table_bit(table);
        }
    }
}

/// One applied script op, with the checkpoint outcome when the op was
/// [`WorkloadOp::Checkpoint`].
#[derive(Debug)]
pub struct AppliedOp {
    pub index: usize,
    pub op: WorkloadOp,
    pub checkpoint: Option<CheckpointOutcome>,
}

/// Build a runner and apply the whole script, returning the runner for the
/// final drain and assertions.
pub async fn run_workload<C>(
    seed: u64,
    script: Vec<WorkloadOp>,
    env: WorkloadEnv<C>,
    engine: SyncEngine<C>,
) -> Result<WorkloadRunner<C>, WorkloadError> {
    let mut runner = WorkloadRunner::new(seed, script, env, engine)?;
    runner.run().await?;
    Ok(runner)
}

/// A writer connection with the same discipline as the app: 5s busy timeout
/// and `wal_autocheckpoint = 0` (invariant I2 applies to every writer).
fn open_writer(db_path: &Utf8Path) -> Result<rusqlite::Connection, rusqlite::Error> {
    let conn = rusqlite::Connection::open(db_path)?;
    conn.busy_timeout(Duration::from_secs(5))?;
    let _pages: i64 = conn.query_row("PRAGMA wal_autocheckpoint = 0", [], |row| row.get(0))?;
    Ok(conn)
}

/// Which `t0..t15` tables already exist (the runner may be built over a
/// database with earlier workload state).
fn scan_tables(conn: &rusqlite::Connection) -> Result<u16, rusqlite::Error> {
    let mut tables = 0u16;
    let mut statement = conn.prepare("SELECT name FROM sqlite_master WHERE type = 'table'")?;
    let names = statement.query_map([], |row| row.get::<_, String>(0))?;
    for name in names {
        let name = name?;
        if let Some(rest) = name.strip_prefix('t')
            && let Ok(slot) = rest.parse::<u8>()
            && slot < TABLE_COUNT
        {
            tables |= table_bit(slot);
        }
    }
    Ok(tables)
}

/// Map any u8 into the `t0..t15` namespace (mask instead of modulo).
const fn table_slot(table: u8) -> u8 {
    table & (TABLE_COUNT - 1)
}

const fn table_bit(table: u8) -> u16 {
    1 << table_slot(table)
}

fn table_name(table: u8) -> String {
    format!("t{}", table_slot(table))
}

fn create_table_sql(table: u8) -> String {
    format!(
        "CREATE TABLE IF NOT EXISTS {} (id INTEGER PRIMARY KEY, data TEXT, bin BLOB)",
        table_name(table)
    )
}

/// [`EngineState`] does not implement `Display`; name it for error messages.
fn state_name(state: EngineState) -> &'static str {
    match state {
        EngineState::Streaming => "Streaming",
        EngineState::AwaitingEpoch => "AwaitingEpoch",
        EngineState::PendingSnapshot => "PendingSnapshot",
        EngineState::Snapshotting => "Snapshotting",
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use pretty_assertions::assert_eq;

    use super::{WEIGHT_TOTAL, WEIGHTED, WorkloadOp, generate_workload};

    const LEN: usize = 200;

    const ALL_VARIANTS: [&str; WorkloadOp::VARIANT_COUNT] = [
        "BigTxn",
        "BlobWrite",
        "Checkpoint",
        "CreateIndex",
        "CreateTable",
        "Delete",
        "DropTable",
        "Insert",
        "RestartReplicator",
        "Snapshot",
        "StrayWrite",
        "Sync",
        "Update",
        "UserVersionBump",
        "Vacuum",
    ];

    #[test]
    fn generate_is_deterministic() {
        assert_eq!(
            generate_workload(42, 500),
            generate_workload(42, 500),
            "the same seed must generate the identical script"
        );
    }

    #[test]
    fn generate_has_requested_length() {
        assert_eq!(generate_workload(7, 123).len(), 123, "script length");
    }

    #[test]
    fn different_seeds_differ() {
        assert_ne!(
            generate_workload(0, LEN),
            generate_workload(1, LEN),
            "different seeds must generate different scripts"
        );
    }

    #[test]
    fn weights_sum_to_total_and_cover_every_variant() {
        let sum: u32 = WEIGHTED.iter().map(|(_, weight)| *weight).sum();
        assert_eq!(sum, WEIGHT_TOTAL, "the weight table must sum to the total");
        let shapes: BTreeSet<&'static str> = WEIGHTED
            .iter()
            .map(|(shape, _)| {
                // Round-trip through instantiate via a fixed rng so the
                // weight table provably reaches every WorkloadOp variant.
                use rand::SeedableRng as _;
                let mut rng = rand::rngs::StdRng::seed_from_u64(0);
                super::instantiate(*shape, &mut rng).kind_name()
            })
            .collect();
        let all: BTreeSet<&'static str> = ALL_VARIANTS.iter().copied().collect();
        assert_eq!(shapes, all, "the weight table must list every variant");
    }

    #[test]
    fn seeds_0_through_7_collectively_cover_every_variant() {
        let mut seen = BTreeSet::new();
        for seed in 0..8u64 {
            for op in generate_workload(seed, LEN) {
                seen.insert(op.kind_name());
            }
        }
        let all: BTreeSet<&'static str> = ALL_VARIANTS.iter().copied().collect();
        assert_eq!(
            seen, all,
            "seeds 0..8 x {LEN} ops must collectively cover every variant"
        );
    }

    #[test]
    fn no_variant_dominates() {
        for seed in 0..8u64 {
            let script = generate_workload(seed, LEN);
            let mut counts = BTreeMap::new();
            for op in &script {
                *counts.entry(op.kind_name()).or_insert(0usize) += 1;
            }
            for (kind, count) in counts {
                assert!(
                    count * 10 <= LEN * 6,
                    "seed {seed}: {kind} appears {count}/{LEN} times, over the 60% cap"
                );
            }
        }
    }
}
