use http::{HeaderMap, header::AUTHORIZATION};

use crate::params::Params;

/// How a public client identifies itself to the token and revocation endpoints.
pub(crate) const CLIENT_ID: &str = "client_id";
/// What a confidential client sends to authenticate. This server serves only public clients.
pub(crate) const CLIENT_CREDENTIALS: [&str; 3] =
    ["client_secret", "client_assertion", "client_assertion_type"];

/// Whether a request tries to authenticate its client, in its body or with HTTP Basic
/// (RFC 6749, section 2.3), which a public client never does.
pub(crate) fn authenticates_client(params: &Params<'_>, headers: &HeaderMap) -> bool {
    CLIENT_CREDENTIALS
        .iter()
        .any(|name| params.get(name).is_some())
        || headers
            .get(AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.split_whitespace().next())
            .is_some_and(|scheme| scheme.eq_ignore_ascii_case("Basic"))
}
