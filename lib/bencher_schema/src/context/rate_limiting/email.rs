use bencher_json::Email;
use sha2::{Digest as _, Sha256};

/// An address as the per-address limit keys it: the SHA-256 of the normalized address, so the
/// limiter's snapshot on disk holds no address.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(into = "String", try_from = "String")]
pub(super) struct EmailKey([u8; 32]);

impl From<&Email> for EmailKey {
    fn from(email: &Email) -> Self {
        Self(Sha256::digest(email.as_ref().as_bytes()).into())
    }
}

impl From<EmailKey> for String {
    fn from(key: EmailKey) -> Self {
        hex::encode(key.0)
    }
}

impl TryFrom<String> for EmailKey {
    type Error = hex::FromHexError;

    fn try_from(key: String) -> Result<Self, Self::Error> {
        let mut bytes = [0; 32];
        hex::decode_to_slice(key, &mut bytes)?;
        Ok(Self(bytes))
    }
}
