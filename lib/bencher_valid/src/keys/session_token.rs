/// A browser session's secret, which only its cookie carries: 32 bytes from the operating
/// system's random number generator, base64url without padding.
#[derive(Clone, Eq, PartialEq)]
pub struct SessionToken(String);

crate::keys::secret_impl!(SessionToken, error = SessionToken);

#[cfg(test)]
#[expect(clippy::string_slice, reason = "test tokens are ASCII")]
mod tests {
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};

    use super::SessionToken;

    #[test]
    fn token_is_32_bytes_of_unpadded_base64url() {
        let valid = URL_SAFE_NO_PAD.encode([0xfb; 32]);
        valid.parse::<SessionToken>().unwrap();
        for token in [
            URL_SAFE_NO_PAD.encode([0xfb; 31]),
            URL_SAFE_NO_PAD.encode([0xfb; 33]),
            format!("{valid}="),
            valid.replace('-', "+").replace('_', "/"),
            // The last character's unused low bits must be zero.
            format!("{}B", &valid[..42]),
            format!(" {}", &valid[1..]),
        ] {
            assert!(token.parse::<SessionToken>().is_err(), "{token}");
        }
    }

    #[cfg(feature = "server")]
    #[test]
    fn generated_tokens_are_valid_and_distinct() {
        let token = SessionToken::generate().unwrap();
        assert_eq!(
            token.as_ref().parse::<SessionToken>().unwrap(),
            token,
            "a generated token parses back"
        );
        assert_ne!(token, SessionToken::generate().unwrap());
    }
}
