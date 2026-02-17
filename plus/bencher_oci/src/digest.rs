//! Digest hashing abstraction over SHA-256 and SHA-512.

use sha2::{Digest as _, Sha256, Sha512};

use crate::error::OciError;

/// A digest hasher that supports both SHA-256 and SHA-512.
pub enum DigestHasher {
    Sha256(Sha256),
    Sha512(Sha512),
}

impl DigestHasher {
    /// Create a new hasher for the given algorithm name.
    pub fn from_algorithm(algorithm: &str) -> Result<Self, OciError> {
        match algorithm {
            "sha256" => Ok(Self::Sha256(Sha256::new())),
            "sha512" => Ok(Self::Sha512(Sha512::new())),
            _ => Err(OciError::UnsupportedDigestAlgorithm(algorithm.to_owned())),
        }
    }

    /// Feed data into the hasher.
    pub fn update(&mut self, data: &[u8]) {
        match self {
            Self::Sha256(h) => h.update(data),
            Self::Sha512(h) => h.update(data),
        }
    }

    /// Finalize and return the digest string (e.g. `"sha256:abcd..."`).
    pub fn finalize(self) -> String {
        match self {
            Self::Sha256(h) => format!("sha256:{:x}", h.finalize()),
            Self::Sha512(h) => format!("sha512:{:x}", h.finalize()),
        }
    }

    /// One-shot digest for in-memory data.
    #[cfg(any(feature = "registry", test))]
    pub fn digest(algorithm: &str, data: &[u8]) -> Result<String, OciError> {
        let mut hasher = Self::from_algorithm(algorithm)?;
        hasher.update(data);
        Ok(hasher.finalize())
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::DigestHasher;

    #[test]
    fn digest_hasher_sha256_empty() {
        let result = DigestHasher::digest("sha256", b"").unwrap();
        assert_eq!(
            result,
            "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn digest_hasher_sha512_empty() {
        let result = DigestHasher::digest("sha512", b"").unwrap();
        assert_eq!(
            result,
            "sha512:cf83e1357eefb8bdf1542850d66d8007d620e4050b5715dc83f4a921d36ce9ce47d0d13c5d85f2b0ff8318d2877eec2f63b931bd47417a81a538327af927da3e"
        );
    }

    #[test]
    fn digest_hasher_sha256_streaming() {
        let mut hasher = DigestHasher::from_algorithm("sha256").unwrap();
        hasher.update(b"hello");
        hasher.update(b" world");
        let streaming = hasher.finalize();

        let oneshot = DigestHasher::digest("sha256", b"hello world").unwrap();
        assert_eq!(streaming, oneshot);
    }

    #[test]
    fn digest_hasher_sha512_streaming() {
        let mut hasher = DigestHasher::from_algorithm("sha512").unwrap();
        hasher.update(b"hello");
        hasher.update(b" world");
        let streaming = hasher.finalize();

        let oneshot = DigestHasher::digest("sha512", b"hello world").unwrap();
        assert_eq!(streaming, oneshot);
    }

    #[test]
    fn digest_hasher_unsupported() {
        let result = DigestHasher::from_algorithm("md5");
        assert!(result.is_err());
    }
}
