use std::str::FromStr;

use derive_more::Display;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[cfg(feature = "client")]
mod target_os;

#[typeshare::typeshare]
#[derive(Debug, Display, Clone, Copy, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct Fingerprint(Uuid);

#[derive(Debug, Clone, thiserror::Error)]
pub enum FingerprintError {
    #[error("Failed to parse fingerprint: {0}")]
    Parse(String),
}

impl FromStr for Fingerprint {
    type Err = FingerprintError;

    fn from_str(fingerprint: &str) -> Result<Self, Self::Err> {
        Ok(Self(fingerprint.parse().map_err(|_e| {
            FingerprintError::Parse(fingerprint.to_owned())
        })?))
    }
}

impl From<Uuid> for Fingerprint {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl From<Fingerprint> for Uuid {
    fn from(fingerprint: Fingerprint) -> Self {
        fingerprint.0
    }
}
