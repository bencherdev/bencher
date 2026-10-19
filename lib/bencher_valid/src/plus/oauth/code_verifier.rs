use std::str::FromStr;

#[cfg(feature = "server")]
use crate::OAuthCodeChallenge;
use crate::ValidError;

const MIN_LEN: usize = 43;
const MAX_LEN: usize = 128;

/// A PKCE code verifier: the secret behind an authorization request's code challenge,
/// 43 to 128 unreserved characters.
pub struct OAuthCodeVerifier(
    #[cfg_attr(
        not(feature = "server"),
        expect(dead_code, reason = "read only by the server's PKCE check")
    )]
    String,
);

impl OAuthCodeVerifier {
    /// Whether `challenge` is this verifier's `S256` challenge, compared in constant time.
    #[cfg(feature = "server")]
    pub fn verifies(&self, challenge: &OAuthCodeChallenge) -> bool {
        use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
        use sha2::{Digest as _, Sha256};
        use subtle::ConstantTimeEq as _;

        let computed = URL_SAFE_NO_PAD.encode(Sha256::digest(self.0.as_bytes()));
        computed
            .as_bytes()
            .ct_eq(challenge.as_ref().as_bytes())
            .into()
    }
}

impl FromStr for OAuthCodeVerifier {
    type Err = ValidError;

    fn from_str(verifier: &str) -> Result<Self, Self::Err> {
        if (MIN_LEN..=MAX_LEN).contains(&verifier.len())
            && verifier.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~')
            })
        {
            Ok(Self(verifier.to_owned()))
        } else {
            Err(ValidError::OAuthCodeVerifier)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{MAX_LEN, MIN_LEN, OAuthCodeVerifier};

    #[test]
    fn verifier_is_43_to_128_unreserved_characters() {
        for verifier in [
            "a".repeat(MIN_LEN),
            "a".repeat(MAX_LEN),
            "-._~0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz".to_owned(),
        ] {
            verifier.parse::<OAuthCodeVerifier>().unwrap();
        }
        for verifier in [
            "a".repeat(MIN_LEN - 1),
            "a".repeat(MAX_LEN + 1),
            format!("{}+", "a".repeat(MIN_LEN)),
            format!("{}/", "a".repeat(MIN_LEN)),
            format!("{}=", "a".repeat(MIN_LEN)),
            format!("{} ", "a".repeat(MIN_LEN)),
        ] {
            assert!(verifier.parse::<OAuthCodeVerifier>().is_err(), "{verifier}");
        }
    }

    // RFC 7636, Appendix B
    #[cfg(feature = "server")]
    #[test]
    fn verifies_only_its_own_s256_challenge() {
        use crate::OAuthCodeChallenge;

        let verifier: OAuthCodeVerifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"
            .parse()
            .unwrap();
        let challenge: OAuthCodeChallenge = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
            .parse()
            .unwrap();
        assert!(verifier.verifies(&challenge));

        let other: OAuthCodeVerifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXl"
            .parse()
            .unwrap();
        assert!(!other.verifies(&challenge));
    }
}
