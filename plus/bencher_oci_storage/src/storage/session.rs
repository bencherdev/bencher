//! An upload session's state, committed as numbered generations.
//!
//! Each commit creates the next generation with a create-only put, so of two
//! requests that commit from one generation, exactly one wins. The newest
//! generation is the session's state.

use bencher_json::ProjectUuid;
use object_store::MultipartId;
use object_store::path::Path;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use sha2::digest::common::hazmat::{SerializableState as _, SerializedState};

use crate::types::UploadId;

/// The hash state this binary reads and writes: `sha2` 0.11's serialized SHA-256.
const HASH_STATE_TAG: &str = "sha256/sha2-0.11";

/// The digits of a generation in its key, so that keys sort as numbers.
pub(super) const GENERATION_DIGITS: usize = 20;

/// An upload session as its newest state left it.
pub struct UploadSession {
    id: UploadId,
    generation: u64,
    state: SessionState,
}

impl UploadSession {
    pub(super) fn new(id: UploadId, generation: u64, state: SessionState) -> Self {
        Self {
            id,
            generation,
            state,
        }
    }

    pub fn id(&self) -> &UploadId {
        &self.id
    }

    /// The generation of the state the session loaded with.
    pub(super) fn generation(&self) -> u64 {
        self.generation
    }

    pub(super) fn state(&self) -> &SessionState {
        &self.state
    }

    /// The bytes the session holds.
    pub fn offset(&self) -> u64 {
        match &self.state.phase {
            Phase::Open(progress) => progress.offset,
            Phase::Completing(finished) => finished.size,
            Phase::Promoting(promotion) => promotion.finished.size,
        }
    }
}

/// One generation of a session's state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct SessionState {
    pub repository: ProjectUuid,
    /// Unix timestamp when the session started
    pub created_at: i64,
    /// Every part but the last holds this many bytes.
    pub part_size: u64,
    /// The staging multipart upload, opened at the first full part
    pub upload: Option<MultipartId>,
    pub phase: Phase,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "name", rename_all = "snake_case")]
pub(super) enum Phase {
    /// The session takes bytes.
    Open(Progress),
    /// Every byte is in a part, and the staging upload completes.
    Completing(Finished),
    /// The staging object is whole, and the blob moves to its digest key.
    Promoting(Promotion),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct Progress {
    /// The bytes committed
    pub offset: u64,
    /// The `ETag` of each full part, in order
    pub parts: Vec<String>,
    pub hash: HashState,
    /// The bytes past the last full part, at the tail key for `offset`
    pub tail: Option<Tail>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct Tail {
    /// The hex MD5 of the tail's bytes, which every read of the tail checks
    pub md5: String,
    /// The `ETag` the store gave the tail
    pub e_tag: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct Finished {
    /// The digest the bytes hash to
    pub digest: String,
    pub size: u64,
    /// The `ETag` of every part, the tail's last
    pub parts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct Promotion {
    pub finished: Finished,
    /// The staging object's `ETag`, which the promoted blob's must equal on S3
    pub staging_e_tag: Option<String>,
    /// The multipart upload at the digest key, on S3
    pub destination: Option<MultipartId>,
}

/// A SHA-256 part way through, with a tag that names its format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct HashState {
    tag: String,
    state: String,
}

impl HashState {
    pub fn of(hasher: &Sha256) -> Self {
        Self {
            tag: HASH_STATE_TAG.to_owned(),
            state: hex::encode(hasher.serialize()),
        }
    }

    /// The hasher again, or `None` when this binary cannot read the state.
    pub fn hasher(&self) -> Option<Sha256> {
        if self.tag != HASH_STATE_TAG {
            return None;
        }
        let bytes = hex::decode(&self.state).ok()?;
        let state = SerializedState::<Sha256>::try_from(bytes.as_slice()).ok()?;
        Sha256::deserialize(&state).ok()
    }
}

/// The generation a state key names, or `None` for any other key.
pub(super) fn generation(key: &Path) -> Option<u64> {
    let name = key.filename()?;
    (name.len() == GENERATION_DIGITS)
        .then(|| name.parse().ok())
        .flatten()
}
