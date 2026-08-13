//! Equivalence oracles for replicated databases.
//!
//! The primary oracle is LOGICAL comparison, not whole-file byte compare:
//! DB header bytes 24..28 (file change counter) and 92..100 (version-valid-
//! for + `SQLite` version) legitimately differ between independently
//! checkpointed copies.
//!
//! Both oracles collect a list of human-readable differences and assert it
//! empty, so a failure prints every context-bearing line at once (and the
//! comparison logic stays a plain, panic-free function).

use camino::Utf8Path;
use rusqlite::{Connection, OpenFlags};

use crate::verify::{
    query_serialized_rows, quote_ident, schema_rows, user_table_names, user_version,
};

/// Assert that `restored` is logically equivalent to `source`:
///
/// 1. `PRAGMA integrity_check` == "ok" on `restored`;
/// 2. `sqlite_master` rows equal (`ORDER BY type, name`);
/// 3. every user table's rows equal (`SELECT * ORDER BY rowid`, values
///    serialized with type tags; `WITHOUT ROWID` tables are out of scope per
///    the workload module docs);
/// 4. `PRAGMA user_version` equal.
///
/// Panics with a readable diff naming the first differing table.
pub fn assert_replica_equivalent(source: &Utf8Path, restored: &Utf8Path) {
    let differences = replica_differences(source, restored);
    assert!(
        differences.is_empty(),
        "restored database {restored} is NOT equivalent to source {source}:\n{}",
        differences.join("\n")
    );
}

/// Strict page-level comparison, masking DB header offsets 24..28 and
/// 92..100. Requires equal file lengths. Used only by the frame-apply
/// unit-test family where both sides are fully controlled.
pub fn assert_pages_equal(a: &Utf8Path, b: &Utf8Path) {
    let differences = page_differences(a, b);
    assert!(
        differences.is_empty(),
        "database pages of {a} and {b} are NOT equal:\n{}",
        differences.join("\n")
    );
}

/// Collect every logical difference between `source` and `restored`; empty
/// means equivalent. Row-level comparison stops at the first differing
/// table.
fn replica_differences(source: &Utf8Path, restored: &Utf8Path) -> Vec<String> {
    let mut differences = Vec::new();
    let Some(source_conn) = open_db(source, &mut differences) else {
        return differences;
    };
    let Some(restored_conn) = open_db(restored, &mut differences) else {
        return differences;
    };

    match integrity_check(&restored_conn) {
        Ok(report) if report == ["ok"] => {},
        Ok(report) => differences.push(format!(
            "integrity_check failed on restored {restored}: {}",
            report.join("; ")
        )),
        Err(error) => differences.push(format!(
            "integrity_check could not run on restored {restored}: {error}"
        )),
    }

    push_list_difference(
        "schema (sqlite_master ORDER BY type, name)",
        schema_rows(&source_conn),
        schema_rows(&restored_conn),
        &mut differences,
    );

    match user_table_names(&source_conn) {
        Ok(tables) => {
            for table in tables {
                let before = differences.len();
                push_list_difference(
                    &format!("table {table} (SELECT * ORDER BY rowid)"),
                    table_rows(&source_conn, &table),
                    table_rows(&restored_conn, &table),
                    &mut differences,
                );
                if differences.len() > before {
                    // Name the FIRST differing table; later tables would
                    // only bury it.
                    break;
                }
            }
        },
        Err(error) => differences.push(format!("failed to list source tables: {error}")),
    }

    let source_version = user_version(&source_conn);
    let restored_version = user_version(&restored_conn);
    match (source_version, restored_version) {
        (Ok(source_version), Ok(restored_version)) => {
            if source_version != restored_version {
                differences.push(format!(
                    "user_version differs: source {source_version} vs restored {restored_version}"
                ));
            }
        },
        (Err(error), Ok(_) | Err(_)) | (Ok(_), Err(error)) => {
            differences.push(format!("user_version query failed: {error}"));
        },
    }
    differences
}

/// Open a database for comparison. `READ_WRITE` (not `READ_ONLY`) because
/// opening a database with a live WAL may need to create the `-shm` file;
/// no CREATE flag, so a missing file is a difference, not an empty database.
fn open_db(path: &Utf8Path, differences: &mut Vec<String>) -> Option<Connection> {
    match Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) {
        Ok(conn) => Some(conn),
        Err(error) => {
            differences.push(format!("failed to open {path}: {error}"));
            None
        },
    }
}

fn integrity_check(conn: &Connection) -> Result<Vec<String>, rusqlite::Error> {
    let mut statement = conn.prepare("PRAGMA integrity_check")?;
    let rows = statement.query_map([], |row| row.get(0))?;
    rows.collect()
}

fn table_rows(conn: &Connection, table: &str) -> Result<Vec<String>, rusqlite::Error> {
    query_serialized_rows(
        conn,
        &format!("SELECT * FROM {} ORDER BY rowid", quote_ident(table)),
    )
}

/// Record the first difference between two serialized row lists (or the
/// query failure that prevented the comparison).
fn push_list_difference(
    kind: &str,
    source: Result<Vec<String>, rusqlite::Error>,
    restored: Result<Vec<String>, rusqlite::Error>,
    differences: &mut Vec<String>,
) {
    let source = match source {
        Ok(rows) => rows,
        Err(error) => {
            differences.push(format!("{kind}: source query failed: {error}"));
            return;
        },
    };
    let restored = match restored {
        Ok(rows) => rows,
        Err(error) => {
            differences.push(format!("{kind}: restored query failed: {error}"));
            return;
        },
    };
    if source == restored {
        return;
    }
    for (index, (source_row, restored_row)) in source.iter().zip(&restored).enumerate() {
        if source_row != restored_row {
            differences.push(format!(
                "{kind}: first difference at row {index}:\n  source:   {source_row}\n  restored: {restored_row}"
            ));
            return;
        }
    }
    // Identical shared prefix: the row counts must differ.
    let shared = source.len().min(restored.len());
    let first_unmatched = source
        .get(shared)
        .or_else(|| restored.get(shared))
        .cloned()
        .unwrap_or_default();
    differences.push(format!(
        "{kind}: row counts differ (source {} vs restored {}); first unmatched row: {first_unmatched}",
        source.len(),
        restored.len()
    ));
}

/// Collect page-level differences after masking the mutable header ranges;
/// empty means byte-equal. Reports only the first differing page.
fn page_differences(a: &Utf8Path, b: &Utf8Path) -> Vec<String> {
    let mut differences = Vec::new();
    let a_bytes = read_masked(a, &mut differences);
    let b_bytes = read_masked(b, &mut differences);
    if !differences.is_empty() {
        return differences;
    }
    if a_bytes.len() != b_bytes.len() {
        differences.push(format!(
            "file lengths differ: {a} is {} bytes, {b} is {} bytes",
            a_bytes.len(),
            b_bytes.len()
        ));
        return differences;
    }
    let page_size = header_page_size(&a_bytes);
    for (page, (page_a, page_b)) in a_bytes
        .chunks(page_size)
        .zip(b_bytes.chunks(page_size))
        .enumerate()
    {
        if page_a != page_b {
            let offset = page_a
                .iter()
                .zip(page_b)
                .position(|(byte_a, byte_b)| byte_a != byte_b)
                .unwrap_or_default();
            differences.push(format!(
                "page {page} differs at byte offset {offset} (page size {page_size})"
            ));
            return differences;
        }
    }
    differences
}

/// Read a whole database file with the legitimately-mutable header ranges
/// zeroed: 24..28 (file change counter) and 92..100 (version-valid-for
/// change counter + `SQLITE_VERSION_NUMBER` of the last writer).
fn read_masked(path: &Utf8Path, differences: &mut Vec<String>) -> Vec<u8> {
    match std::fs::read(path) {
        Ok(mut bytes) => {
            for range in [24..28, 92..100] {
                if let Some(masked) = bytes.get_mut(range) {
                    masked.fill(0);
                }
            }
            bytes
        },
        Err(error) => {
            differences.push(format!("failed to read {path}: {error}"));
            Vec::new()
        },
    }
}

/// Database page size from header bytes 16..18 (big-endian; the value 1
/// encodes 65536). Falls back to whole-file comparison when the header is
/// absent or nonsensical.
fn header_page_size(bytes: &[u8]) -> usize {
    let high = bytes.get(16).copied().unwrap_or(0);
    let low = bytes.get(17).copied().unwrap_or(0);
    let raw = (usize::from(high) << 8) | usize::from(low);
    match raw {
        1 => 0x0001_0000,
        0 => bytes.len().max(1),
        page_size => page_size,
    }
}

#[cfg(test)]
mod tests {
    use camino::{Utf8Path, Utf8PathBuf};

    use super::{assert_pages_equal, assert_replica_equivalent};

    fn tempdir_path(dir: &tempfile::TempDir) -> &Utf8Path {
        Utf8Path::from_path(dir.path()).unwrap()
    }

    fn create_db(path: &Utf8Path, statements: &str) {
        let conn = rusqlite::Connection::open(path).unwrap();
        conn.execute_batch(statements).unwrap();
    }

    const SEED: &str = "CREATE TABLE t (id INTEGER PRIMARY KEY, data TEXT);
         CREATE TABLE mixed (id INTEGER PRIMARY KEY, n INTEGER, r REAL, b BLOB);
         INSERT INTO t (data) VALUES ('alpha'), ('beta');
         INSERT INTO mixed (n, r, b) VALUES (1, 2.5, x'0102'), (NULL, NULL, NULL);";

    fn twin_dbs(tmp: &tempfile::TempDir) -> (Utf8PathBuf, Utf8PathBuf) {
        let source = tempdir_path(tmp).join("source.db");
        let restored = tempdir_path(tmp).join("restored.db");
        create_db(&source, SEED);
        create_db(&restored, SEED);
        (source, restored)
    }

    #[test]
    fn equivalent_databases_pass() {
        let tmp = tempfile::tempdir().unwrap();
        let (source, restored) = twin_dbs(&tmp);
        assert_replica_equivalent(&source, &restored);
    }

    #[test]
    #[should_panic(expected = "table t")]
    fn row_difference_names_the_table() {
        let tmp = tempfile::tempdir().unwrap();
        let (source, restored) = twin_dbs(&tmp);
        let conn = rusqlite::Connection::open(&restored).unwrap();
        conn.execute("UPDATE t SET data = 'gamma' WHERE id = 2", [])
            .unwrap();
        drop(conn);
        assert_replica_equivalent(&source, &restored);
    }

    #[test]
    #[should_panic(expected = "row counts differ")]
    fn row_count_difference_detected() {
        let tmp = tempfile::tempdir().unwrap();
        let (source, restored) = twin_dbs(&tmp);
        let conn = rusqlite::Connection::open(&restored).unwrap();
        conn.execute("INSERT INTO t (data) VALUES ('extra')", [])
            .unwrap();
        drop(conn);
        assert_replica_equivalent(&source, &restored);
    }

    #[test]
    #[should_panic(expected = "schema")]
    fn schema_difference_detected() {
        let tmp = tempfile::tempdir().unwrap();
        let (source, restored) = twin_dbs(&tmp);
        let conn = rusqlite::Connection::open(&restored).unwrap();
        conn.execute_batch("CREATE INDEX t_data ON t (data)")
            .unwrap();
        drop(conn);
        assert_replica_equivalent(&source, &restored);
    }

    #[test]
    #[should_panic(expected = "user_version")]
    fn user_version_difference_detected() {
        let tmp = tempfile::tempdir().unwrap();
        let (source, restored) = twin_dbs(&tmp);
        let conn = rusqlite::Connection::open(&restored).unwrap();
        conn.pragma_update(None, "user_version", 9).unwrap();
        drop(conn);
        assert_replica_equivalent(&source, &restored);
    }

    #[test]
    #[should_panic(expected = "failed to open")]
    fn missing_database_is_not_equivalent() {
        let tmp = tempfile::tempdir().unwrap();
        let (source, _) = twin_dbs(&tmp);
        assert_replica_equivalent(&source, &tempdir_path(&tmp).join("missing.db"));
    }

    #[test]
    fn pages_equal_masks_header_counters() {
        let tmp = tempfile::tempdir().unwrap();
        let (source, _) = twin_dbs(&tmp);
        let copy = tempdir_path(&tmp).join("copy.db");
        let mut bytes = std::fs::read(&source).unwrap();
        // Differing change counters and version-valid-for stamps are
        // legitimate between independently checkpointed copies.
        for offset in (24..28).chain(92..100) {
            bytes[offset] ^= 0xff;
        }
        std::fs::write(&copy, &bytes).unwrap();
        assert_pages_equal(&source, &copy);
    }

    #[test]
    #[should_panic(expected = "page 1 differs")]
    fn page_content_difference_detected() {
        let tmp = tempfile::tempdir().unwrap();
        let (source, _) = twin_dbs(&tmp);
        let copy = tempdir_path(&tmp).join("copy.db");
        let mut bytes = std::fs::read(&source).unwrap();
        let page_size = usize::from(bytes[16]) << 8 | usize::from(bytes[17]);
        bytes[page_size + 100] ^= 0xff;
        std::fs::write(&copy, &bytes).unwrap();
        assert_pages_equal(&source, &copy);
    }

    #[test]
    #[should_panic(expected = "file lengths differ")]
    fn page_length_mismatch_detected() {
        let tmp = tempfile::tempdir().unwrap();
        let (source, _) = twin_dbs(&tmp);
        let copy = tempdir_path(&tmp).join("copy.db");
        let mut bytes = std::fs::read(&source).unwrap();
        bytes.truncate(bytes.len() - 1);
        std::fs::write(&copy, &bytes).unwrap();
        assert_pages_equal(&source, &copy);
    }
}
