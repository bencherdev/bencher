/// HTTP `Authorization` header name.
pub const AUTHORIZATION: &str = "Authorization";

/// Format a bearer token for the HTTP `Authorization` header.
///
/// Returns a value like `Bearer <token>` suitable for the `Authorization` header.
pub fn bearer_header(token: &str) -> String {
    format!("Bearer {token}")
}

/// Extract the bearer token from an `Authorization` header value.
///
/// Performs case-insensitive matching on the `Bearer` scheme prefix.
/// Returns `Some(token)` with the token trimmed, or `None` if the
/// header value doesn't start with `Bearer `.
pub fn strip_bearer_token(header_value: &str) -> Option<&str> {
    let (scheme, token) = header_value.split_once(' ')?;
    scheme.eq_ignore_ascii_case("Bearer").then(|| token.trim())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bearer_header_formats() {
        assert_eq!(bearer_header("my_token"), "Bearer my_token");
    }

    #[test]
    fn bearer_header_empty() {
        assert_eq!(bearer_header(""), "Bearer ");
    }

    #[test]
    fn strip_bearer_token_valid() {
        assert_eq!(strip_bearer_token("Bearer my_token"), Some("my_token"));
    }

    #[test]
    fn strip_bearer_token_case_insensitive() {
        assert_eq!(strip_bearer_token("bearer my_token"), Some("my_token"));
        assert_eq!(strip_bearer_token("BEARER my_token"), Some("my_token"));
        assert_eq!(strip_bearer_token("bEaReR my_token"), Some("my_token"));
    }

    #[test]
    fn strip_bearer_token_trims() {
        assert_eq!(strip_bearer_token("Bearer  my_token "), Some("my_token"));
    }

    #[test]
    fn strip_bearer_token_not_bearer() {
        assert_eq!(strip_bearer_token("Basic abc123"), None);
    }

    #[test]
    fn strip_bearer_token_no_space() {
        assert_eq!(strip_bearer_token("Bearertoken"), None);
    }

    #[test]
    fn strip_bearer_token_empty() {
        assert_eq!(strip_bearer_token(""), None);
    }
}
