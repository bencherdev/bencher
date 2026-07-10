//! Restore-and-compare verification: prove the replica reproduces the
//! source database at a known position.
//!
//! The full choreography (ship tail, record position P, pin a read snapshot,
//! then compare off-lock) is wired into the sync loop separately; this
//! module provides the pieces: a logical fingerprint over a pinned
//! connection, restore-to-position, and the comparison.
//!
//! The row/schema serialization helpers here are `pub(crate)` and reused by
//! the test-only equivalence oracle in [`crate::testing`] (the reverse
//! dependency is impossible: production code cannot depend on the testing
//! module).

use camino::Utf8Path;
use rusqlite::types::ValueRef;
use sha2::{Digest as _, Sha256};

use crate::position::Position;
use crate::storage::ReplicaStorage;

/// The result of one verification run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifyReport {
    /// Source and restored fingerprints match at the verified position.
    Pass,
    /// Fingerprints differ: the replica lineage is divergent.
    Fail {
        /// Human-readable description of the first difference.
        detail: String,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum VerifyError {
    #[error("Failed to fingerprint database: {0}")]
    Fingerprint(rusqlite::Error),
    #[error("Failed to restore replica for verification: {0}")]
    Restore(#[from] crate::restore::RestoreError),
    #[error("Verification scratch directory: {0}")]
    TempDir(std::io::Error),
    #[error("Verification task panicked: {0}")]
    Join(tokio::task::JoinError),
}

/// Compute a logical fingerprint of the database visible through `conn`
/// (which should hold a pinned read snapshot): the `user_version`, every
/// `sqlite_master` row (`ORDER BY type, name`), per user table (internal
/// `sqlite_%` tables skipped) a row count and a SHA-256 over its rows, and
/// the `sqlite_sequence` AUTOINCREMENT counters (`ORDER BY name`) when that
/// internal table exists. Rows are serialized with type-tagged,
/// delimiter-escaped values in a deterministic order (`rowid`, or every
/// column for `WITHOUT ROWID` tables that have none).
///
/// The output is a deterministic line-oriented byte vector, so a failed
/// comparison can name the first differing line.
pub fn fingerprint_database(conn: &rusqlite::Connection) -> Result<Vec<u8>, VerifyError> {
    let mut lines = Vec::new();
    let version = user_version(conn).map_err(VerifyError::Fingerprint)?;
    lines.push(format!("user_version={version}"));
    for row in schema_rows(conn).map_err(VerifyError::Fingerprint)? {
        lines.push(format!("schema={row}"));
    }
    for table in user_table_names(conn).map_err(VerifyError::Fingerprint)? {
        let (rows, sha256) = table_digest(conn, &table).map_err(VerifyError::Fingerprint)?;
        lines.push(format!("table={table}|rows={rows}|sha256={sha256}"));
    }
    // `sqlite_sequence` holds AUTOINCREMENT high-water marks; it is skipped
    // by `user_table_names` as an internal table, so a restore that corrupted
    // it would otherwise be invisible.
    for row in sqlite_sequence_rows(conn).map_err(VerifyError::Fingerprint)? {
        lines.push(format!("sqlite_sequence={row}"));
    }
    let mut fingerprint = lines.join("\n");
    fingerprint.push('\n');
    Ok(fingerprint.into_bytes())
}

/// Restore the replica into `scratch_dir` up to `position` and compare its
/// fingerprint against `source_fingerprint`. The restore itself runs the
/// same `quick_check` hard gate as a startup restore.
pub async fn verify_against_replica(
    log: &slog::Logger,
    storage: &ReplicaStorage,
    position: &Position,
    source_fingerprint: &[u8],
    scratch_dir: &Utf8Path,
) -> Result<VerifyReport, VerifyError> {
    std::fs::create_dir_all(scratch_dir).map_err(VerifyError::TempDir)?;
    let scratch_db = scratch_dir.join("verify.db");
    let restored = crate::restore::restore_to(log, storage, &scratch_db, Some(position)).await?;
    if restored.is_none() {
        // An empty (or vanished) replica cannot reproduce the source; the
        // caller reacts the same way as to a content mismatch.
        return Ok(VerifyReport::Fail {
            detail: "no valid generation found on the replica".to_owned(),
        });
    }
    let fingerprint = tokio::task::spawn_blocking(move || -> Result<Vec<u8>, VerifyError> {
        // No CREATE flag: a missing scratch database is a bug, not an empty
        // database.
        let conn = rusqlite::Connection::open_with_flags(
            &scratch_db,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(VerifyError::Fingerprint)?;
        fingerprint_database(&conn)
    })
    .await
    .map_err(VerifyError::Join)??;
    if fingerprint == source_fingerprint {
        Ok(VerifyReport::Pass)
    } else {
        Ok(VerifyReport::Fail {
            detail: first_differing_line(source_fingerprint, &fingerprint),
        })
    }
}

/// The `sqlite_master` rows serialized deterministically
/// (`ORDER BY type, name`; columns `type, name, tbl_name, sql`).
pub(crate) fn schema_rows(conn: &rusqlite::Connection) -> Result<Vec<String>, rusqlite::Error> {
    query_serialized_rows(
        conn,
        "SELECT type, name, tbl_name, sql FROM sqlite_master ORDER BY type, name",
    )
}

/// User table names in deterministic order; internal `sqlite_%` tables are
/// skipped (they cannot be queried like user tables).
pub(crate) fn user_table_names(
    conn: &rusqlite::Connection,
) -> Result<Vec<String>, rusqlite::Error> {
    let mut statement = conn.prepare(
        "SELECT name FROM sqlite_master \
         WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
    )?;
    let names = statement.query_map([], |row| row.get(0))?;
    names.collect()
}

/// The `sqlite_sequence` AUTOINCREMENT counters (`name, seq` `ORDER BY
/// name`), serialized like any other row. Empty when the table does not
/// exist (no AUTOINCREMENT column anywhere), so a schema without
/// AUTOINCREMENT never errors.
pub(crate) fn sqlite_sequence_rows(
    conn: &rusqlite::Connection,
) -> Result<Vec<String>, rusqlite::Error> {
    if !table_exists(conn, "sqlite_sequence")? {
        return Ok(Vec::new());
    }
    query_serialized_rows(conn, "SELECT name, seq FROM sqlite_sequence ORDER BY name")
}

/// Whether a table of the given name exists (`sqlite_master`).
fn table_exists(conn: &rusqlite::Connection, name: &str) -> Result<bool, rusqlite::Error> {
    let count: i64 = conn.query_row(
        "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
        [name],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

/// `PRAGMA user_version`.
pub(crate) fn user_version(conn: &rusqlite::Connection) -> Result<i64, rusqlite::Error> {
    conn.query_row("PRAGMA user_version", [], |row| row.get(0))
}

/// Run `sql` and serialize every result row with [`serialize_row`].
pub(crate) fn query_serialized_rows(
    conn: &rusqlite::Connection,
    sql: &str,
) -> Result<Vec<String>, rusqlite::Error> {
    let mut statement = conn.prepare(sql)?;
    let mut rows = statement.query([])?;
    let mut serialized = Vec::new();
    while let Some(row) = rows.next()? {
        serialized.push(serialize_row(row)?);
    }
    Ok(serialized)
}

/// Serialize one result row as `|`-joined tagged values (see [`tag_value`]),
/// with each value's `|` characters escaped first so a value containing the
/// delimiter cannot merge with an adjacent column: `("a|b", "c")` and
/// `("a", "b|c")` must never serialize identically.
pub(crate) fn serialize_row(row: &rusqlite::Row) -> Result<String, rusqlite::Error> {
    let column_count = row.as_ref().column_count();
    let mut values = Vec::with_capacity(column_count);
    for index in 0..column_count {
        values.push(escape_delimiter(&tag_value(row.get_ref(index)?)));
    }
    Ok(values.join("|"))
}

/// Escape the `|` column delimiter inside an already-tagged value. `tag_value`
/// backslash-escapes TEXT via `escape_default` (which doubles `\` and never
/// emits `|`) and every other tag is delimiter-free, so an escaped `\|` is
/// unambiguous: after a join, a raw `|` is preceded by an even run of
/// backslashes exactly when it is a genuine column boundary.
fn escape_delimiter(value: &str) -> String {
    value.replace('|', "\\|")
}

/// Double-quote an identifier for interpolation into SQL.
pub(crate) fn quote_ident(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

/// Serialize one `SQLite` value with an explicit type tag so `1`, `'1'`, and
/// `x'31'` never collide. Deterministic across platforms: REALs carry their
/// exact bit pattern, TEXT is escaped to a single line, BLOBs are hex.
fn tag_value(value: ValueRef<'_>) -> String {
    match value {
        ValueRef::Null => "NULL".to_owned(),
        ValueRef::Integer(int) => format!("INTEGER:{int}"),
        ValueRef::Real(real) => format!("REAL:{real}:{:016x}", real.to_bits()),
        ValueRef::Text(text) => match std::str::from_utf8(text) {
            Ok(text) => format!("TEXT:{}", text.escape_default()),
            Err(_) => format!("TEXT(hex):{}", hex::encode(text)),
        },
        ValueRef::Blob(blob) => format!("BLOB:{}", hex::encode(blob)),
    }
}

/// Row count plus SHA-256 over the serialized rows of one table, streamed
/// row by row (a production fingerprint never materializes a whole table).
fn table_digest(
    conn: &rusqlite::Connection,
    table: &str,
) -> Result<(u64, String), rusqlite::Error> {
    let order = row_order_clause(conn, table)?;
    let mut statement = conn.prepare(&format!(
        "SELECT * FROM {} ORDER BY {order}",
        quote_ident(table)
    ))?;
    let mut rows = statement.query([])?;
    let mut hasher = Sha256::new();
    let mut count = 0u64;
    while let Some(row) = rows.next()? {
        hasher.update(serialize_row(row)?.as_bytes());
        hasher.update(b"\n");
        count = count.saturating_add(1);
    }
    Ok((count, hex::encode(hasher.finalize())))
}

/// A deterministic `ORDER BY` clause for a table's digest. Ordinary tables
/// order by `rowid`; a `WITHOUT ROWID` table has none, so it orders by every
/// column (its PRIMARY KEY is a subset, so the order is canonical). Ordering
/// by `rowid` on a `WITHOUT ROWID` table would raise "no such column: rowid"
/// on every verification, forever.
fn row_order_clause(conn: &rusqlite::Connection, table: &str) -> Result<String, rusqlite::Error> {
    if table_has_rowid(conn, table)? {
        return Ok("rowid".to_owned());
    }
    let clause = table_columns(conn, table)?
        .iter()
        .map(|column| quote_ident(column))
        .collect::<Vec<_>>()
        .join(", ");
    // Every SQLite table has at least one column, and a WITHOUT ROWID table
    // has a PRIMARY KEY, so the clause is never empty.
    Ok(clause)
}

/// Whether the table exposes a `rowid` (true for every ordinary table,
/// including `INTEGER PRIMARY KEY`; false for `WITHOUT ROWID`). Probing with
/// a zero-row `SELECT rowid` is the reliable signal: the column resolves
/// (prepare succeeds) for rowid tables and fails only for `WITHOUT ROWID`
/// ones. Any other preparation failure propagates.
fn table_has_rowid(conn: &rusqlite::Connection, table: &str) -> Result<bool, rusqlite::Error> {
    match conn.prepare(&format!("SELECT rowid FROM {} LIMIT 0", quote_ident(table))) {
        Ok(_statement) => Ok(true),
        Err(rusqlite::Error::SqliteFailure(_, Some(message))) if message.contains("rowid") => {
            Ok(false)
        },
        Err(error) => Err(error),
    }
}

/// Column names in declaration order (the order `SELECT *` yields).
fn table_columns(conn: &rusqlite::Connection, table: &str) -> Result<Vec<String>, rusqlite::Error> {
    let mut statement = conn.prepare(&format!("PRAGMA table_info({})", quote_ident(table)))?;
    let names = statement.query_map([], |row| row.get::<_, String>(1))?;
    names.collect()
}

/// Name the first line where two fingerprints diverge.
fn first_differing_line(source: &[u8], restored: &[u8]) -> String {
    let source_text = String::from_utf8_lossy(source);
    let restored_text = String::from_utf8_lossy(restored);
    let mut source_lines = source_text.lines();
    let mut restored_lines = restored_text.lines();
    loop {
        match (source_lines.next(), restored_lines.next()) {
            (Some(source_line), Some(restored_line)) if source_line == restored_line => {},
            (Some(source_line), Some(restored_line)) => {
                return format!("source: {source_line} | restored: {restored_line}");
            },
            (Some(source_line), None) => return format!("source has extra line: {source_line}"),
            (None, Some(restored_line)) => {
                return format!("restored has extra line: {restored_line}");
            },
            (None, None) => return "fingerprints are equal".to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::{fingerprint_database, first_differing_line};

    fn seeded_db() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE t (id INTEGER PRIMARY KEY, data TEXT);
             CREATE TABLE mixed (id INTEGER PRIMARY KEY, n INTEGER, r REAL, s TEXT, b BLOB);
             INSERT INTO t (data) VALUES ('alpha'), ('beta');
             INSERT INTO mixed (n, r, s, b) VALUES
                 (42, 1.5, 'line1\nline2', x'00ff'),
                 (NULL, NULL, NULL, NULL);",
        )
        .unwrap();
        conn
    }

    #[test]
    fn fingerprint_is_deterministic() {
        let first = fingerprint_database(&seeded_db()).unwrap();
        let second = fingerprint_database(&seeded_db()).unwrap();
        assert_eq!(
            first, second,
            "identical databases produce identical fingerprints"
        );
        let text = String::from_utf8(first).unwrap();
        assert!(
            text.starts_with("user_version=0\n"),
            "fingerprint starts with the user_version line: {text}"
        );
        assert!(
            text.contains("schema=TEXT:table|TEXT:t|TEXT:t|"),
            "fingerprint carries the schema rows: {text}"
        );
        assert!(
            text.contains("table=t|rows=2|sha256="),
            "fingerprint carries per-table digests: {text}"
        );
        assert!(
            text.contains("table=mixed|rows=2|sha256="),
            "fingerprint covers every user table: {text}"
        );
    }

    #[test]
    fn fingerprint_reflects_row_changes() {
        let conn = seeded_db();
        let before = fingerprint_database(&conn).unwrap();
        conn.execute("UPDATE t SET data = 'gamma' WHERE id = 1", [])
            .unwrap();
        let after = fingerprint_database(&conn).unwrap();
        assert_ne!(before, after, "a row change must change the fingerprint");
        let detail = first_differing_line(&before, &after);
        assert!(
            detail.contains("table=t|rows=2|sha256="),
            "the differing line names the changed table: {detail}"
        );
    }

    #[test]
    fn fingerprint_reflects_value_type_not_just_display() {
        // The INTEGER 1 and the TEXT '1' must never fingerprint equally.
        // The column is untyped (BLOB affinity) so no coercion happens.
        let int_conn = rusqlite::Connection::open_in_memory().unwrap();
        int_conn
            .execute_batch(
                "CREATE TABLE v (id INTEGER PRIMARY KEY, x); INSERT INTO v (x) VALUES (1);",
            )
            .unwrap();
        let text_conn = rusqlite::Connection::open_in_memory().unwrap();
        text_conn
            .execute_batch(
                "CREATE TABLE v (id INTEGER PRIMARY KEY, x); INSERT INTO v (x) VALUES ('1');",
            )
            .unwrap();
        assert_ne!(
            fingerprint_database(&int_conn).unwrap(),
            fingerprint_database(&text_conn).unwrap(),
            "tagged serialization distinguishes value types"
        );
    }

    #[test]
    fn fingerprint_reflects_user_version() {
        let conn = seeded_db();
        let before = fingerprint_database(&conn).unwrap();
        conn.pragma_update(None, "user_version", 7).unwrap();
        let after = fingerprint_database(&conn).unwrap();
        assert_ne!(before, after, "user_version is part of the fingerprint");
        assert_eq!(
            first_differing_line(&before, &after),
            "source: user_version=0 | restored: user_version=7",
            "the differing line is the user_version line"
        );
    }

    #[test]
    fn fingerprint_skips_sqlite_internal_tables() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        // AUTOINCREMENT creates the internal sqlite_sequence table.
        conn.execute_batch(
            "CREATE TABLE a (id INTEGER PRIMARY KEY AUTOINCREMENT, data TEXT);
             INSERT INTO a (data) VALUES ('x');",
        )
        .unwrap();
        let text = String::from_utf8(fingerprint_database(&conn).unwrap()).unwrap();
        assert!(
            !text.contains("table=sqlite_sequence"),
            "internal tables are not fingerprinted as user tables: {text}"
        );
        assert!(
            text.contains("TEXT:sqlite_sequence"),
            "internal tables still appear in the schema rows: {text}"
        );
    }

    #[test]
    fn fingerprint_escapes_column_delimiter() {
        // Without escaping, ("a|TEXT:b", "c") and ("a", "b|TEXT:c") both
        // serialize to "TEXT:a|TEXT:b|TEXT:c", so a divergent table hashes
        // equal: a false verification PASS.
        let left = two_text_columns("a|TEXT:b", "c");
        let right = two_text_columns("a", "b|TEXT:c");
        assert_ne!(
            fingerprint_database(&left).unwrap(),
            fingerprint_database(&right).unwrap(),
            "the column delimiter must be escaped so the two rows differ"
        );
    }

    #[test]
    fn fingerprint_reflects_sqlite_sequence() {
        // Two DBs with an identical (empty) user table but different
        // AUTOINCREMENT high-water marks must not fingerprint equally: a
        // restore that corrupted sqlite_sequence has to be caught.
        let low = autoincrement_db(1);
        let high = autoincrement_db(1000);
        assert_ne!(
            fingerprint_database(&low).unwrap(),
            fingerprint_database(&high).unwrap(),
            "sqlite_sequence counters are part of the fingerprint"
        );
        let text = String::from_utf8(fingerprint_database(&high).unwrap()).unwrap();
        assert!(
            text.contains("sqlite_sequence=TEXT:a|INTEGER:1000"),
            "the AUTOINCREMENT counter appears in the fingerprint: {text}"
        );
    }

    #[test]
    fn fingerprint_handles_without_rowid_tables() {
        // `ORDER BY rowid` errors forever on a WITHOUT ROWID table; the
        // fingerprint must still compute and still reflect row changes.
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE wr (k TEXT PRIMARY KEY, v INTEGER) WITHOUT ROWID;
             INSERT INTO wr (k, v) VALUES ('a', 1), ('b', 2);",
        )
        .unwrap();
        let before = fingerprint_database(&conn).unwrap();
        let text = String::from_utf8(before.clone()).unwrap();
        assert!(
            text.contains("table=wr|rows=2|sha256="),
            "the WITHOUT ROWID table is fingerprinted: {text}"
        );
        conn.execute("UPDATE wr SET v = 99 WHERE k = 'a'", [])
            .unwrap();
        let after = fingerprint_database(&conn).unwrap();
        assert_ne!(
            before, after,
            "a change in a WITHOUT ROWID table changes the fingerprint"
        );
    }

    fn two_text_columns(a: &str, b: &str) -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE t (a TEXT, b TEXT);")
            .unwrap();
        conn.execute(
            "INSERT INTO t (a, b) VALUES (?1, ?2)",
            rusqlite::params![a, b],
        )
        .unwrap();
        conn
    }

    fn autoincrement_db(seq: i64) -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        // Insert then delete leaves the table empty but seeds sqlite_sequence;
        // the explicit UPDATE then sets the counter deterministically.
        conn.execute_batch(
            "CREATE TABLE a (id INTEGER PRIMARY KEY AUTOINCREMENT, data TEXT);
             INSERT INTO a (data) VALUES ('x');
             DELETE FROM a;",
        )
        .unwrap();
        conn.execute(
            "UPDATE sqlite_sequence SET seq = ?1 WHERE name = 'a'",
            [seq],
        )
        .unwrap();
        conn
    }

    #[test]
    fn first_differing_line_reports_each_shape() {
        assert_eq!(
            first_differing_line(b"a\nb\n", b"a\nc\n"),
            "source: b | restored: c"
        );
        assert_eq!(
            first_differing_line(b"a\nb\n", b"a\n"),
            "source has extra line: b"
        );
        assert_eq!(
            first_differing_line(b"a\n", b"a\nb\n"),
            "restored has extra line: b"
        );
        assert_eq!(
            first_differing_line(b"a\n", b"a\n"),
            "fingerprints are equal"
        );
    }
}
