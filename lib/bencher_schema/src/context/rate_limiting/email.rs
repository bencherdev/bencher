use std::net::IpAddr;

use bencher_json::Email;
use sha2::{Digest as _, Sha256};

use super::IpKey;

/// An address as the sign-in email limits key it: the SHA-256 of the normalized address, so the
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

/// An address and who asked to email it, as the tight sign-in email caps key them: the reader's
/// address, keyed as every per-IP limit keys it, or, with no known address, one requester that
/// every such request shares.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(into = "String", try_from = "String")]
pub(super) struct EmailRequesterKey {
    email: EmailKey,
    requester: Option<IpKey>,
}

impl EmailRequesterKey {
    pub(super) fn new(email: EmailKey, requester: Option<IpAddr>) -> Self {
        Self {
            email,
            requester: requester.map(IpKey::from),
        }
    }
}

impl From<EmailRequesterKey> for String {
    fn from(key: EmailRequesterKey) -> Self {
        let EmailRequesterKey { email, requester } = key;
        let email = Self::from(email);
        if let Some(requester) = requester {
            format!("{email} {}", IpAddr::from(requester))
        } else {
            email
        }
    }
}

impl TryFrom<String> for EmailRequesterKey {
    type Error = EmailRequesterKeyError;

    fn try_from(key: String) -> Result<Self, Self::Error> {
        let (email, requester) = match key.split_once(' ') {
            Some((email, requester)) => (
                email.to_owned(),
                Some(
                    requester
                        .parse::<IpAddr>()
                        .map_err(EmailRequesterKeyError::Requester)?,
                ),
            ),
            None => (key, None),
        };
        Ok(Self::new(
            EmailKey::try_from(email).map_err(EmailRequesterKeyError::Email)?,
            requester,
        ))
    }
}

#[derive(Debug, thiserror::Error)]
pub(super) enum EmailRequesterKeyError {
    #[error("Invalid email key: {0}")]
    Email(hex::FromHexError),
    #[error("Invalid requester: {0}")]
    Requester(std::net::AddrParseError),
}
