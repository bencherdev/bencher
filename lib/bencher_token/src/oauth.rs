use bencher_json::Secret;
use jsonwebtoken::{
    Algorithm, EncodingKey, Header,
    jwk::{AlgorithmParameters, Jwk, JwkSet, PublicKeyUse, ThumbprintHash},
};
use serde::Serialize;

use crate::TokenError;

/// The smallest RSA modulus accepted.
const MIN_MODULUS_BITS: usize = 2048;

/// The RSA key that signs OIDC ID tokens, with its public half as a JSON Web Key.
#[derive(Debug, Clone)]
pub struct OAuthSigningKey {
    encoding_key: EncodingKey,
    jwk: Jwk,
    kid: String,
}

#[derive(Debug, thiserror::Error)]
pub enum OAuthSigningKeyError {
    #[error("The OAuth signing key is not an RSA private key in PEM format: {0}")]
    Pem(jsonwebtoken::errors::Error),
    #[error("The OAuth signing key is not a valid RSA private key: {0}")]
    Rsa(jsonwebtoken::errors::Error),
    #[error("The OAuth signing key must be an RSA key of at least 2048 bits")]
    Short,
}

impl OAuthSigningKey {
    /// An RSA private key in PEM format, PKCS #1 or PKCS #8.
    pub fn new(pem: &Secret) -> Result<Self, OAuthSigningKeyError> {
        let encoding_key = EncodingKey::from_rsa_pem(pem.as_ref().as_bytes())
            .map_err(OAuthSigningKeyError::Pem)?;
        let mut jwk = Jwk::from_encoding_key(&encoding_key, Algorithm::RS256)
            .map_err(OAuthSigningKeyError::Rsa)?;
        // The modulus is unpadded base64url, six bits a character.
        if let AlgorithmParameters::RSA(rsa) = &jwk.algorithm
            && rsa.n.len().saturating_mul(6) < MIN_MODULUS_BITS
        {
            return Err(OAuthSigningKeyError::Short);
        }
        // RFC 7638, so the key ID stays the same for as long as the key does.
        let kid = jwk.thumbprint(ThumbprintHash::SHA256);
        jwk.common.key_id = Some(kid.clone());
        jwk.common.public_key_use = Some(PublicKeyUse::Signature);
        Ok(Self {
            encoding_key,
            jwk,
            kid,
        })
    }

    /// The public key, the only one ID tokens are signed with.
    pub fn jwk_set(&self) -> JwkSet {
        JwkSet {
            keys: vec![self.jwk.clone()],
        }
    }

    /// Sign `claims` as a JSON Web Token (RS256) that names this key.
    pub fn sign<C>(&self, claims: &C) -> Result<String, TokenError>
    where
        C: Serialize,
    {
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some(self.kid.clone());
        jsonwebtoken::encode(&header, claims, &self.encoding_key)
            .map_err(|error| TokenError::Encode { error })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use bencher_json::Secret;
    use jsonwebtoken::{Algorithm, DecodingKey, Validation};

    use super::{OAuthSigningKey, OAuthSigningKeyError};

    const SIGNING_KEY: &str = include_str!("./test/oauth_signing_key.pem");
    const SHORT_SIGNING_KEY: &str = include_str!("./test/oauth_signing_key_1024.pem");

    fn signing_key(pem: &str) -> Result<OAuthSigningKey, OAuthSigningKeyError> {
        OAuthSigningKey::new(&secret(pem))
    }

    fn secret(pem: &str) -> Secret {
        pem.parse().unwrap()
    }

    // Kills a key ID that changes each time the server starts with the same key, which would
    // leave a client's cached key set unable to verify new tokens.
    #[test]
    fn signed_with_the_published_key_under_a_stable_id() {
        let claims = BTreeMap::from([("sub".to_owned(), "user".to_owned())]);
        let token = signing_key(SIGNING_KEY).unwrap().sign(&claims).unwrap();

        let jwk_set = signing_key(SIGNING_KEY).unwrap().jwk_set();
        let header = jsonwebtoken::decode_header(&token).unwrap();
        let jwk = jwk_set.find(header.kid.as_deref().unwrap()).unwrap();
        let mut validation = Validation::new(Algorithm::RS256);
        validation.required_spec_claims.clear();
        validation.validate_exp = false;
        let decoded = jsonwebtoken::decode::<BTreeMap<String, String>>(
            &token,
            &DecodingKey::from_jwk(jwk).unwrap(),
            &validation,
        )
        .unwrap();
        assert_eq!(decoded.claims, claims);
    }

    // Kills accepting a key too short to keep ID tokens from being forged.
    #[test]
    fn a_key_under_2048_bits_is_refused() {
        assert!(matches!(
            signing_key(SHORT_SIGNING_KEY),
            Err(OAuthSigningKeyError::Short)
        ));
    }
}
