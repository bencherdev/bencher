use std::{fmt, sync::Arc};

use aws_lc_rs::{
    aead::{AES_256_GCM, Aad, NONCE_LEN, Nonce, RandomizedNonceKey},
    error::Unspecified,
    hkdf::{HKDF_SHA256, Salt},
};
use bencher_json::{JobUuid, Secret};
use uuid::Uuid;

/// A different info string derives a different key, which no pending callback opens.
const INFO: &[u8] = b"bencher.callback.v1";
const KEY_LEN: usize = 32;
const VERSION: u8 = 1;

/// Seals a callback request to its job, with a key derived from the server's secret key.
#[derive(Clone)]
pub struct CallbackKey(Arc<RandomizedNonceKey>);

#[derive(Debug, thiserror::Error)]
pub enum CallbackKeyError {
    #[error("Failed to derive the callback key: {0}")]
    Derive(Unspecified),
    #[error("Failed to seal the callback request: {0}")]
    Seal(Unspecified),
}

#[derive(Debug, thiserror::Error)]
pub enum CallbackOpenError {
    #[error("The sealed callback request is too short")]
    TooShort,
    #[error("The sealed callback request has an unknown version ({0})")]
    UnknownVersion(u8),
    #[error(
        "The sealed callback request failed authentication (a different job, a different secret key, or altered bytes): {0}"
    )]
    Authentication(Unspecified),
}

impl CallbackKey {
    pub fn new(secret_key: &Secret) -> Result<Self, CallbackKeyError> {
        let key = derive_key(secret_key).map_err(CallbackKeyError::Derive)?;
        RandomizedNonceKey::new(&AES_256_GCM, &key)
            .map(|key| Self(Arc::new(key)))
            .map_err(CallbackKeyError::Derive)
    }

    pub fn seal(&self, job: JobUuid, plaintext: &[u8]) -> Result<SealedRequest, CallbackKeyError> {
        let mut ciphertext = plaintext.to_vec();
        let nonce = self
            .0
            .seal_in_place_append_tag(aad(job), &mut ciphertext)
            .map_err(CallbackKeyError::Seal)?;
        Ok(SealedRequest(
            [&[VERSION], nonce.as_ref().as_slice(), &ciphertext].concat(),
        ))
    }

    pub fn open(&self, job: JobUuid, sealed: &SealedRequest) -> Result<Vec<u8>, CallbackOpenError> {
        let (&version, rest) = sealed.0.split_first().ok_or(CallbackOpenError::TooShort)?;
        if version != VERSION {
            return Err(CallbackOpenError::UnknownVersion(version));
        }
        let (nonce, ciphertext) = rest
            .split_first_chunk::<NONCE_LEN>()
            .ok_or(CallbackOpenError::TooShort)?;
        if ciphertext.len() < AES_256_GCM.tag_len() {
            return Err(CallbackOpenError::TooShort);
        }
        let mut plaintext = ciphertext.to_vec();
        let plaintext_len = self
            .0
            .open_in_place(Nonce::from(nonce), aad(job), &mut plaintext)
            .map_err(CallbackOpenError::Authentication)?
            .len();
        plaintext.truncate(plaintext_len);
        Ok(plaintext)
    }
}

impl fmt::Debug for CallbackKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CallbackKey").finish_non_exhaustive()
    }
}

/// A callback request sealed to one job: a version byte, the nonce, then the ciphertext and its tag.
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Binary))]
pub struct SealedRequest(Vec<u8>);

impl fmt::Debug for SealedRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SealedRequest")
            .field("len", &self.0.len())
            .finish()
    }
}

impl From<Vec<u8>> for SealedRequest {
    fn from(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }
}

impl AsRef<[u8]> for SealedRequest {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

fn derive_key(secret_key: &Secret) -> Result<[u8; KEY_LEN], Unspecified> {
    let mut key = [0; KEY_LEN];
    Salt::none(HKDF_SHA256)
        .extract(secret_key.as_ref().as_bytes())
        .expand(&[INFO], &AES_256_GCM)?
        .fill(&mut key)?;
    Ok(key)
}

fn aad(job: JobUuid) -> Aad<[u8; 16]> {
    Aad::from(Uuid::from(job).into_bytes())
}

#[cfg(feature = "db")]
mod db {
    use diesel::{
        backend::Backend,
        deserialize::{self, FromSql},
        serialize::{self, IsNull, Output, ToSql},
        sql_types::Binary,
        sqlite::Sqlite,
    };

    use super::SealedRequest;

    impl ToSql<Binary, Sqlite> for SealedRequest {
        fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Sqlite>) -> serialize::Result {
            out.set_value(self.0.as_slice());
            Ok(IsNull::No)
        }
    }

    impl<DB> FromSql<Binary, DB> for SealedRequest
    where
        DB: Backend,
        Vec<u8>: FromSql<Binary, DB>,
    {
        fn from_sql(bytes: DB::RawValue<'_>) -> deserialize::Result<Self> {
            Vec::<u8>::from_sql(bytes).map(Self)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashSet, iter};

    use bencher_json::{JobUuid, Secret};
    use pretty_assertions::assert_eq;

    use super::{CallbackKey, CallbackOpenError, SealedRequest, VERSION};

    const NONCE_LEN: usize = 12;
    const TAG_LEN: usize = 16;
    const MIN_LEN: usize = 1 + NONCE_LEN + TAG_LEN;
    const PLAINTEXT: &[u8] =
        br#"{"url":"https://example.com/hook","headers":{"authorization":"Bearer MARKER-7c1f0e2a"}}"#;

    fn secret(secret: &str) -> Secret {
        secret.parse().unwrap()
    }

    fn key(secret_key: &str) -> CallbackKey {
        CallbackKey::new(&secret(secret_key)).unwrap()
    }

    fn job(uuid: &str) -> JobUuid {
        uuid.parse().unwrap()
    }

    fn job_a() -> JobUuid {
        job("0192e4a5-7b3c-7d4e-8f60-1a2b3c4d5e6f")
    }

    fn job_b() -> JobUuid {
        job("0192e4a5-7b3c-7d4e-8f60-1a2b3c4d5e70")
    }

    fn flipped(sealed: &SealedRequest, index: usize) -> SealedRequest {
        let mut bytes = sealed.as_ref().to_vec();
        bytes[index] ^= 0x01;
        SealedRequest::from(bytes)
    }

    #[test]
    fn open_returns_what_seal_was_given() {
        let key = key("secret-a");
        let sealed = key.seal(job_a(), PLAINTEXT).unwrap();
        assert_eq!(key.open(job_a(), &sealed).unwrap(), PLAINTEXT);
    }

    #[test]
    fn empty_plaintext_seals_to_the_minimum_length() {
        let key = key("secret-a");
        let sealed = key.seal(job_a(), b"").unwrap();
        assert_eq!(sealed.as_ref().len(), MIN_LEN);
        assert_eq!(key.open(job_a(), &sealed).unwrap(), b"");
    }

    #[test]
    fn every_seal_draws_a_fresh_random_nonce() {
        const SEALS: usize = 32;
        let shared = key("secret-a");
        let nonces: Vec<Vec<u8>> = iter::repeat_with(|| shared.seal(job_a(), PLAINTEXT).unwrap())
            .take(SEALS)
            .chain(
                iter::repeat_with(|| key("secret-a").seal(job_a(), PLAINTEXT).unwrap()).take(SEALS),
            )
            .map(|sealed| sealed.as_ref()[1..=NONCE_LEN].to_vec())
            .collect();
        let distinct: HashSet<&Vec<u8>> = nonces.iter().collect();
        assert_eq!(distinct.len(), nonces.len(), "a nonce repeated");
        for position in 0..NONCE_LEN {
            assert!(
                nonces
                    .iter()
                    .any(|nonce| nonce[position] != nonces[0][position]),
                "nonce byte {position} never changes"
            );
        }
    }

    #[test]
    fn open_refuses_another_job() {
        let key = key("secret-a");
        let sealed = key.seal(job_a(), PLAINTEXT).unwrap();
        let error = key.open(job_b(), &sealed).unwrap_err();
        assert!(
            matches!(error, CallbackOpenError::Authentication(_)),
            "expected an authentication failure, got {error}"
        );
    }

    #[test]
    fn open_refuses_a_key_from_another_secret() {
        let sealed = key("secret-a").seal(job_a(), PLAINTEXT).unwrap();
        let error = key("secret-b").open(job_a(), &sealed).unwrap_err();
        assert!(
            matches!(error, CallbackOpenError::Authentication(_)),
            "expected an authentication failure, got {error}"
        );
    }

    #[test]
    fn open_refuses_any_flipped_byte() {
        let key = key("secret-a");
        let sealed = key.seal(job_a(), PLAINTEXT).unwrap();
        let tag_start = sealed.as_ref().len() - TAG_LEN;
        for index in 0..sealed.as_ref().len() {
            let region = match index {
                0 => "version",
                i if i <= NONCE_LEN => "nonce",
                i if i < tag_start => "ciphertext",
                _ => "tag",
            };
            let error = key.open(job_a(), &flipped(&sealed, index)).unwrap_err();
            if index == 0 {
                assert!(
                    matches!(error, CallbackOpenError::UnknownVersion(version) if version == VERSION ^ 0x01),
                    "byte {index} ({region}): expected an unknown version, got {error}"
                );
            } else {
                assert!(
                    matches!(error, CallbackOpenError::Authentication(_)),
                    "byte {index} ({region}): expected an authentication failure, got {error}"
                );
            }
        }
    }

    #[test]
    fn open_refuses_a_truncated_blob() {
        let key = key("secret-a");
        let sealed = key.seal(job_a(), PLAINTEXT).unwrap();
        for len in 0..sealed.as_ref().len() {
            let truncated = SealedRequest::from(sealed.as_ref()[..len].to_vec());
            let error = key.open(job_a(), &truncated).unwrap_err();
            if len < MIN_LEN {
                assert!(
                    matches!(error, CallbackOpenError::TooShort),
                    "length {len}: expected too short, got {error}"
                );
            } else {
                assert!(
                    matches!(error, CallbackOpenError::Authentication(_)),
                    "length {len}: expected an authentication failure, got {error}"
                );
            }
        }
    }

    #[test]
    fn open_refuses_an_unknown_version() {
        let key = key("secret-a");
        let sealed = key.seal(job_a(), PLAINTEXT).unwrap();
        for version in [0, 2, u8::MAX] {
            let mut bytes = sealed.as_ref().to_vec();
            bytes[0] = version;
            let error = key.open(job_a(), &SealedRequest::from(bytes)).unwrap_err();
            assert!(
                matches!(error, CallbackOpenError::UnknownVersion(unknown) if unknown == version),
                "version {version}: expected an unknown version, got {error}"
            );
        }
        let error = key
            .open(job_a(), &SealedRequest::from(vec![2]))
            .unwrap_err();
        assert!(
            matches!(error, CallbackOpenError::UnknownVersion(2)),
            "a one byte blob: expected an unknown version, got {error}"
        );
        let error = key
            .open(job_a(), &SealedRequest::from(vec![0; MIN_LEN - 1]))
            .unwrap_err();
        assert!(
            matches!(error, CallbackOpenError::UnknownVersion(0)),
            "a short blob: expected an unknown version, got {error}"
        );
    }

    /// Sealed under "secret-golden" for `job_a`. A change to the derivation or the layout fails
    /// this test, and would leave every pending callback unopenable after a deploy.
    #[test]
    fn a_blob_sealed_by_an_earlier_build_still_opens() {
        const SEALED: [u8; 44] = [
            0x01, 0x3c, 0x34, 0x0e, 0x4b, 0xbd, 0x25, 0xbb, 0x6a, 0xec, 0x4a, 0xae, 0x7a, 0xcf,
            0x0c, 0xf2, 0x69, 0x23, 0xf6, 0xe1, 0x34, 0x2a, 0x52, 0xe9, 0x43, 0xe0, 0xc3, 0x6f,
            0x80, 0xee, 0xf0, 0x9a, 0x14, 0x93, 0x32, 0xbc, 0xc8, 0x70, 0x0c, 0xf0, 0x5d, 0xd8,
            0x9e, 0x71,
        ];
        let sealed = SealedRequest::from(SEALED.to_vec());
        assert_eq!(
            key("secret-golden").open(job_a(), &sealed).unwrap(),
            br#"{"golden":true}"#
        );
    }
}
