use std::sync::Arc;

use aws_lc_rs::{
    aead::{AES_256_GCM, Aad, NONCE_LEN, Nonce, RandomizedNonceKey},
    error::Unspecified,
    hkdf::{HKDF_SHA256, Salt},
};
use bencher_json::{JobUuid, Secret};
use uuid::Uuid;

use crate::SealedRequest;

/// A different info string derives a different key, which no pending callback opens.
const INFO: &[u8] = b"bencher.callback.v1";
const KEY_LEN: usize = 32;

/// Seals a callback request to its job, with a key derived from the server's secret key.
#[derive(Debug, Clone)]
pub struct CallbackKey(Arc<RandomizedNonceKey>);

#[derive(Debug, thiserror::Error)]
pub enum CallbackKeyError {
    #[error("Failed to derive the callback key: {0}")]
    Derive(Unspecified),
    #[error("Failed to seal the callback request: {0}")]
    Seal(Unspecified),
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
        Ok(SealedRequest::new(
            [nonce.as_ref().as_slice(), &ciphertext].concat(),
        ))
    }

    pub fn open(&self, job: JobUuid, sealed: &SealedRequest) -> Result<Vec<u8>, Unspecified> {
        let (nonce, ciphertext) = sealed
            .as_ref()
            .split_first_chunk::<NONCE_LEN>()
            .ok_or(Unspecified)?;
        let mut plaintext = ciphertext.to_vec();
        let plaintext_len = self
            .0
            .open_in_place(Nonce::from(nonce), aad(job), &mut plaintext)?
            .len();
        plaintext.truncate(plaintext_len);
        Ok(plaintext)
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

#[cfg(test)]
mod tests {
    use std::{collections::HashSet, iter};

    use bencher_json::{JobUuid, Secret};
    use pretty_assertions::assert_eq;

    use super::{CallbackKey, SealedRequest};

    const NONCE_LEN: usize = 12;
    const TAG_LEN: usize = 16;
    const MIN_LEN: usize = NONCE_LEN + TAG_LEN;
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
        SealedRequest::new(bytes)
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
            .map(|sealed| sealed.as_ref()[..NONCE_LEN].to_vec())
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
        key.open(job_b(), &sealed).unwrap_err();
    }

    #[test]
    fn open_refuses_a_key_from_another_secret() {
        let sealed = key("secret-a").seal(job_a(), PLAINTEXT).unwrap();
        key("secret-b").open(job_a(), &sealed).unwrap_err();
    }

    #[test]
    fn open_refuses_any_flipped_byte() {
        let key = key("secret-a");
        let sealed = key.seal(job_a(), PLAINTEXT).unwrap();
        let tag_start = sealed.as_ref().len() - TAG_LEN;
        for index in 0..sealed.as_ref().len() {
            let region = match index {
                i if i < NONCE_LEN => "nonce",
                i if i < tag_start => "ciphertext",
                _ => "tag",
            };
            assert!(
                key.open(job_a(), &flipped(&sealed, index)).is_err(),
                "byte {index} ({region}) was flipped and still opened"
            );
        }
    }

    #[test]
    fn open_refuses_a_truncated_blob() {
        let key = key("secret-a");
        let sealed = key.seal(job_a(), PLAINTEXT).unwrap();
        for len in 0..sealed.as_ref().len() {
            let truncated = SealedRequest::new(sealed.as_ref()[..len].to_vec());
            assert!(
                key.open(job_a(), &truncated).is_err(),
                "length {len} opened"
            );
        }
    }

    /// Sealed under "secret-golden" for `job_a`. A change to the derivation or the layout fails
    /// this test, and would leave every pending callback unopenable after a deploy.
    #[test]
    fn a_blob_sealed_by_an_earlier_build_still_opens() {
        const SEALED: [u8; 43] = [
            0xb4, 0x8d, 0xb1, 0xf2, 0xd6, 0xcb, 0xf6, 0x52, 0x3f, 0x4c, 0x72, 0xb2, 0xb8, 0xfb,
            0xa8, 0x3a, 0x7e, 0x6e, 0x03, 0x1b, 0x30, 0xcf, 0xf0, 0x1b, 0x1d, 0xda, 0xa5, 0x7e,
            0xf6, 0xf2, 0xd7, 0x15, 0x13, 0x6c, 0x3a, 0x25, 0x5a, 0xdc, 0xbb, 0xcd, 0xa6, 0xd6,
            0x26,
        ];
        let sealed = SealedRequest::new(SEALED.to_vec());
        assert_eq!(
            key("secret-golden").open(job_a(), &sealed).unwrap(),
            br#"{"golden":true}"#
        );
    }
}
