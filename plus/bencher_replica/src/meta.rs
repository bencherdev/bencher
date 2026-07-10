//! Local advisory replication state: `<db>.replica.json`.
//!
//! Written atomically (temp file + fsync + rename, matching the
//! `rate_limiting.json` convention) after generation creation, after each
//! segment ship, after each completed checkpoint, and at shutdown.
//!
//! The meta file is ADVISORY ONLY (invariant I6): it is never consulted by
//! restore and never trusted over the replica. It exists for exactly two
//! cases that a replica LIST plus the local WAL cannot resolve:
//!
//! 1. Crash after our checkpoint but before any new-epoch ship: local WAL
//!    salts no longer match the replica's last epoch. Meta proving "epoch
//!    fully shipped through checkpoint" allows resuming as epoch+1 instead
//!    of re-snapshotting the whole database.
//! 2. Restored-old-volume detection: a stale meta disagrees with the replica
//!    LIST, forcing a new generation instead of appending old-state frames
//!    onto a newer lineage.
//!
//! Any mismatch between meta, local WAL, and replica resolves to "new
//! generation".

use std::fs::{self, File};
use std::io::{ErrorKind, Write as _};

use camino::{Utf8Path, Utf8PathBuf};
use serde::{Deserialize, Serialize};

use crate::position::GenerationId;

/// Current meta file schema version.
pub const META_VERSION: u32 = 1;

/// Suffix appended to the full database file name to form the meta path.
const META_SUFFIX: &str = ".replica.json";
/// Suffix appended to the meta path for the atomic-write temp sibling.
const PARTIAL_SUFFIX: &str = ".partial";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplicaMeta {
    pub version: u32,
    pub generation: String,
    pub epoch: u64,
    pub salt1: u32,
    pub salt2: u32,
    /// Raw WAL byte offset shipped through (commit-aligned).
    pub shipped_offset: u64,
    /// True when every frame of `epoch` was shipped AND a checkpoint fully
    /// backfilled it, so a subsequent WAL restart (new salts) is legitimate.
    pub epoch_shipped_through_checkpoint: bool,
    /// True when written while running in shadow mode (alongside
    /// Litestream). Cutover to sole mode forces a new generation.
    pub shadow: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum MetaError {
    #[error("Failed to read replica meta ({path}): {error}")]
    Read {
        path: Utf8PathBuf,
        error: std::io::Error,
    },
    #[error("Failed to write replica meta ({path}): {error}")]
    Write {
        path: Utf8PathBuf,
        error: std::io::Error,
    },
    #[error("Failed to parse replica meta ({path}): {error}")]
    Parse {
        path: Utf8PathBuf,
        error: serde_json::Error,
    },
    #[error("Failed to serialize replica meta: {0}")]
    Serialize(serde_json::Error),
    #[error("Failed to remove replica meta ({path}): {error}")]
    Remove {
        path: Utf8PathBuf,
        error: std::io::Error,
    },
}

impl ReplicaMeta {
    /// `<db>.replica.json` next to the database file: the suffix is appended
    /// to the full file name (`bencher.db` becomes `bencher.db.replica.json`),
    /// never substituted for an existing extension.
    #[must_use]
    pub fn path_for_db(db_path: &Utf8Path) -> Utf8PathBuf {
        Utf8PathBuf::from(format!("{db_path}{META_SUFFIX}"))
    }

    /// Load the meta file. `Ok(None)` when missing or unparsable (the file is
    /// advisory, so corrupt data is treated as absent per invariant I6);
    /// `Err` only on I/O failures other than not-found.
    pub fn load(db_path: &Utf8Path) -> Result<Option<Self>, MetaError> {
        let path = Self::path_for_db(db_path);
        let json = match fs::read_to_string(&path) {
            Ok(json) => json,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(MetaError::Read { path, error }),
        };
        // Unparsable or version-mismatched meta is treated as absent: the
        // resume then takes the conservative path (a fresh snapshot) rather
        // than trusting advisory fields whose semantics may have changed. A
        // structurally-compatible future version must NOT be silently
        // accepted; the version gate is what makes the format evolvable.
        Ok(serde_json::from_str::<Self>(&json)
            .ok()
            .filter(|meta| meta.version == META_VERSION))
    }

    /// Atomically persist (temp sibling + fsync + rename), so a crash mid-way
    /// leaves either the previous meta or the new one, never a torn file.
    pub fn store(&self, db_path: &Utf8Path) -> Result<(), MetaError> {
        let path = Self::path_for_db(db_path);
        let partial_path = Utf8PathBuf::from(format!("{path}{PARTIAL_SUFFIX}"));
        let json = serde_json::to_string(self).map_err(MetaError::Serialize)?;
        // Write to a temp sibling and fsync it before the atomic rename, so
        // the meta survives an abrupt VM stop rather than lingering in the
        // page cache.
        let write_err = |error| MetaError::Write {
            path: partial_path.clone(),
            error,
        };
        let mut file = File::create(&partial_path).map_err(write_err)?;
        file.write_all(json.as_bytes()).map_err(write_err)?;
        file.sync_all().map_err(write_err)?;
        drop(file);
        fs::rename(&partial_path, &path).map_err(|error| MetaError::Write {
            path: path.clone(),
            error,
        })?;
        // Best-effort fsync of the parent directory so the rename itself is
        // durable. The data file is already fsynced and the rename atomic, so
        // a directory-sync failure must not fail the store.
        if let Some(parent) = path.parent()
            && let Ok(dir) = File::open(parent)
        {
            drop(dir.sync_all());
        }
        Ok(())
    }

    /// Remove the meta file if present (used when the DB file is missing at
    /// restore time). Idempotent.
    pub fn remove(db_path: &Utf8Path) -> Result<(), MetaError> {
        let path = Self::path_for_db(db_path);
        match fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
            Err(error) => Err(MetaError::Remove { path, error }),
        }
    }

    #[must_use]
    pub fn generation_id(&self) -> Option<GenerationId> {
        GenerationId::parse(&self.generation)
    }
}

#[cfg(test)]
mod tests {
    use camino::{Utf8Path, Utf8PathBuf};
    use pretty_assertions::assert_eq;

    use super::{META_VERSION, MetaError, ReplicaMeta};
    use crate::position::GenerationId;

    fn test_db_path(tmp: &tempfile::TempDir) -> Utf8PathBuf {
        Utf8Path::from_path(tmp.path()).unwrap().join("bencher.db")
    }

    fn test_meta() -> ReplicaMeta {
        ReplicaMeta {
            version: META_VERSION,
            generation: "20260710T145900Z-3f8a2c1d".to_owned(),
            epoch: 3,
            salt1: 0x9d2f_1c4a,
            salt2: 0x8b3e_6f70,
            shipped_offset: 524_320,
            epoch_shipped_through_checkpoint: true,
            shadow: false,
        }
    }

    #[test]
    fn version_mismatch_loads_as_absent() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = test_db_path(&tmp);
        let mut meta = test_meta();
        meta.version = META_VERSION + 1;
        meta.store(&db_path).unwrap();
        assert_eq!(
            ReplicaMeta::load(&db_path).unwrap(),
            None,
            "a structurally-compatible future version must load as absent, \
             forcing the conservative resume path"
        );
    }

    #[test]
    fn serde_round_trips() {
        let meta = test_meta();
        let json = serde_json::to_string(&meta).unwrap();
        let parsed: ReplicaMeta = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, meta);
    }

    #[test]
    fn path_for_db_appends_to_full_file_name() {
        assert_eq!(
            ReplicaMeta::path_for_db(Utf8Path::new("/var/lib/bencher/bencher.db")),
            Utf8PathBuf::from("/var/lib/bencher/bencher.db.replica.json")
        );
    }

    #[test]
    fn store_then_load_round_trips() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = test_db_path(&tmp);
        let meta = test_meta();
        meta.store(&db_path).unwrap();
        assert_eq!(ReplicaMeta::load(&db_path).unwrap(), Some(meta));
    }

    #[test]
    fn store_overwrites_and_leaves_no_temp_file() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = test_db_path(&tmp);
        let first = test_meta();
        first.store(&db_path).unwrap();
        let second = ReplicaMeta {
            epoch: 4,
            shipped_offset: 0,
            epoch_shipped_through_checkpoint: false,
            ..test_meta()
        };
        second.store(&db_path).unwrap();
        assert_eq!(ReplicaMeta::load(&db_path).unwrap(), Some(second));
        // The atomic write leaves exactly the meta file behind: no temp
        // sibling droppings.
        let entries: Vec<String> = std::fs::read_dir(tmp.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(entries, vec!["bencher.db.replica.json".to_owned()]);
    }

    #[test]
    fn load_missing_file_is_none() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = test_db_path(&tmp);
        assert_eq!(ReplicaMeta::load(&db_path).unwrap(), None);
    }

    #[test]
    fn load_corrupt_json_is_none() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = test_db_path(&tmp);
        for corrupt in ["", "not json", "{\"version\": \"wat\"}", "[1, 2, 3]"] {
            std::fs::write(ReplicaMeta::path_for_db(&db_path), corrupt).unwrap();
            assert_eq!(ReplicaMeta::load(&db_path).unwrap(), None, "{corrupt:?}");
        }
    }

    #[test]
    fn load_io_error_is_error() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = test_db_path(&tmp);
        // A directory in place of the meta file: reading it is a real I/O
        // error, not a not-found, and must surface as Err.
        std::fs::create_dir(ReplicaMeta::path_for_db(&db_path)).unwrap();
        let err = ReplicaMeta::load(&db_path).unwrap_err();
        assert!(matches!(err, MetaError::Read { .. }), "{err}");
    }

    #[test]
    fn remove_is_idempotent() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = test_db_path(&tmp);
        // Removing a missing file is fine.
        ReplicaMeta::remove(&db_path).unwrap();
        test_meta().store(&db_path).unwrap();
        ReplicaMeta::remove(&db_path).unwrap();
        assert_eq!(ReplicaMeta::load(&db_path).unwrap(), None);
        ReplicaMeta::remove(&db_path).unwrap();
    }

    #[test]
    fn version_field_round_trips_meta_version() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = test_db_path(&tmp);
        test_meta().store(&db_path).unwrap();
        let loaded = ReplicaMeta::load(&db_path).unwrap().unwrap();
        assert_eq!(loaded.version, META_VERSION);
    }

    #[test]
    fn generation_id_parses_stored_generation() {
        let meta = test_meta();
        assert_eq!(
            meta.generation_id(),
            GenerationId::parse("20260710T145900Z-3f8a2c1d")
        );
        let bogus = ReplicaMeta {
            generation: "not-a-generation".to_owned(),
            ..test_meta()
        };
        assert_eq!(bogus.generation_id(), None);
    }
}
