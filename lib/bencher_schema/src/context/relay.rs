use bencher_json::Secret;
use http::HeaderValue;
use sha2::{Digest as _, Sha256};
use subtle::ConstantTimeEq as _;

/// The console's server, which relays the browser's requests to the API with the secret the two
/// share. Only the secret's digest is kept, and a presented secret is compared by its digest in
/// constant time.
pub struct Relay {
    secret_digest: Option<Vec<u8>>,
}

impl Relay {
    pub fn new(secret: Option<&Secret>) -> Self {
        Self {
            secret_digest: secret.map(|secret| digest(secret.as_ref().as_bytes())),
        }
    }

    /// With no secret set, a session cookie is honored from any caller.
    /// With one set, only from a request that carries it.
    pub fn honors_cookie(&self, secret: Option<&HeaderValue>) -> bool {
        self.secret_digest.is_none() || self.is_relayed(secret)
    }

    /// Whether the request carries the secret, so its forwarded IP address can be trusted.
    pub fn is_relayed(&self, secret: Option<&HeaderValue>) -> bool {
        let (Some(secret_digest), Some(secret)) = (&self.secret_digest, secret) else {
            return false;
        };
        digest(secret.as_bytes()).ct_eq(secret_digest).into()
    }
}

fn digest(bytes: &[u8]) -> Vec<u8> {
    Sha256::digest(bytes).to_vec()
}
