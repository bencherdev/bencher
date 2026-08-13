//! The `snapshot.json` object: a generation's atomic commit marker.
//!
//! Uploaded LAST, after the snapshot body (`snapshot.db.zst`) has fully
//! landed: its presence is what makes a generation visible to restore.
//! Generations without a `snapshot.json` are invisible and eventually
//! pruned.

use serde::{Deserialize, Serialize};

/// Current snapshot meta schema version.
pub const SNAPSHOT_META_VERSION: u32 = 1;

#[derive(Debug, thiserror::Error)]
pub enum SnapshotMetaError {
    #[error("Failed to serialize snapshot meta: {0}")]
    Serialize(serde_json::Error),
    #[error("Failed to parse snapshot meta: {0}")]
    Parse(serde_json::Error),
    #[error("Snapshot meta version {found} is not the supported version {SNAPSHOT_META_VERSION}")]
    Version { found: u32 },
}

/// Generation commit marker and restore metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotMeta {
    pub version: u32,
    /// RFC 3339 creation timestamp (informational).
    pub created: String,
    /// Uncompressed database size in bytes at snapshot time.
    pub db_bytes: u64,
    /// Database page size.
    pub page_size: u32,
    /// Hex SHA-256 of the COMPRESSED snapshot object, verified on restore.
    pub sha256: String,
    /// The WAL epoch boundary: restore replays all segments of epochs
    /// `>= wal_boundary.epoch` (always 0 by construction; kept explicit).
    pub wal_boundary: WalBoundary,
}

/// The WAL salt cycle current when the snapshot began.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalBoundary {
    pub epoch: u64,
    pub salt1: u32,
    pub salt2: u32,
    /// The mandatory-replay threshold for the boundary epoch, a raw WAL byte
    /// offset at least as large as the committed extent the snapshot body
    /// captured (it is read AFTER the backup, so a frame committed after the
    /// backup only OVERSTATES it, the safe direction). Restore replays the
    /// boundary epoch only when its available segments reach at least this
    /// far; below it, the snapshot alone (a consistent committed state) is
    /// used. Overstating merely forces that snapshot-only fallback;
    /// understating (a value below the backup extent) could let a PREFIX of
    /// the boundary epoch replay and regress pages the snapshot already
    /// holds at a newer state, so this must never be measured before the
    /// backup.
    /// Deliberately NOT `#[serde(default)]`: defaulting a missing field to 0
    /// would silently disable this guard (0 means "replay any prefix"), so a
    /// marker without it must fail to parse, which makes its generation
    /// unrestorable rather than wrongly restorable.
    pub offset: u64,
}

impl SnapshotMeta {
    /// Serialize for upload.
    pub fn to_bytes(&self) -> Result<Vec<u8>, SnapshotMetaError> {
        serde_json::to_vec_pretty(self).map_err(SnapshotMetaError::Serialize)
    }

    /// Parse a downloaded `snapshot.json`.
    ///
    /// A version other than [`SNAPSHOT_META_VERSION`] is rejected even when
    /// the fields parse: the marker's values feed the decompression cap and
    /// the mandatory-replay boundary, so a structurally-compatible marker
    /// whose field SEMANTICS changed must make its generation unrestorable
    /// (skipped, falling back to an older generation) rather than misread.
    /// Unknown ADDITIVE fields within the same version remain tolerated for
    /// forward compatibility.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, SnapshotMetaError> {
        let meta: Self = serde_json::from_slice(bytes).map_err(SnapshotMetaError::Parse)?;
        if meta.version != SNAPSHOT_META_VERSION {
            return Err(SnapshotMetaError::Version {
                found: meta.version,
            });
        }
        Ok(meta)
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::{SNAPSHOT_META_VERSION, SnapshotMeta, WalBoundary};

    fn test_meta() -> SnapshotMeta {
        SnapshotMeta {
            version: SNAPSHOT_META_VERSION,
            created: "2026-07-10T14:59:00Z".to_owned(),
            db_bytes: 4_350_000_000,
            page_size: 4096,
            sha256: "ab".repeat(32),
            wal_boundary: WalBoundary {
                epoch: 0,
                salt1: 0x9d2f_1c4a,
                salt2: 0x8b3e_6f70,
                offset: 0,
            },
        }
    }

    #[test]
    fn round_trips() {
        let meta = test_meta();
        let bytes = meta.to_bytes().unwrap();
        assert_eq!(SnapshotMeta::from_bytes(&bytes).unwrap(), meta);
    }

    #[test]
    fn rejects_garbage() {
        SnapshotMeta::from_bytes(b"not json").unwrap_err();
    }

    #[test]
    fn version_mismatch_fails_to_parse() {
        let mut meta = test_meta();
        meta.version = SNAPSHOT_META_VERSION + 1;
        let bytes = meta.to_bytes().unwrap();
        let error = SnapshotMeta::from_bytes(&bytes)
            .expect_err("a structurally-compatible future version must be rejected");
        assert!(
            matches!(
                error,
                super::SnapshotMetaError::Version { found } if found == SNAPSHOT_META_VERSION + 1
            ),
            "expected a version error, got: {error:?}"
        );
    }

    #[test]
    fn missing_boundary_offset_fails_to_parse() {
        // The offset is the anti-regression guard; a marker without it must
        // be unparseable (generation skipped), never defaulted to 0, which
        // would mean "replay any prefix".
        let mut value: serde_json::Value =
            serde_json::from_slice(&test_meta().to_bytes().unwrap()).unwrap();
        let boundary = value
            .get_mut("wal_boundary")
            .and_then(serde_json::Value::as_object_mut)
            .expect("wal_boundary object");
        boundary.remove("offset").expect("offset present");
        let bytes = serde_json::to_vec(&value).unwrap();
        SnapshotMeta::from_bytes(&bytes).expect_err("a marker without an offset must not parse");
    }

    #[test]
    fn unknown_fields_are_ignored_for_forward_compat() {
        let mut value: serde_json::Value =
            serde_json::from_slice(&test_meta().to_bytes().unwrap()).unwrap();
        value["future_field"] = serde_json::json!("ignored");
        let bytes = serde_json::to_vec(&value).unwrap();
        assert_eq!(SnapshotMeta::from_bytes(&bytes).unwrap(), test_meta());
    }
}
