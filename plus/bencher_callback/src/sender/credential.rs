use aws_lc_rs::digest::{Context, SHA256, SHA256_OUTPUT_LEN};
use http::header::AUTHORIZATION;
use url::Url;

use super::CallbackRequest;

/// Whose rate limit a callback spends at its host: the `Authorization` value it is sent with, held
/// only as a SHA-256. Callbacks without one share their host's.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct CallbackCredential {
    host: CallbackHost,
    /// The `Authorization` header, or else the URL's userinfo, which the client sends as basic auth
    /// unless the header replaces it.
    authorization: Option<[u8; SHA256_OUTPUT_LEN]>,
}

impl CallbackCredential {
    pub fn of(request: &CallbackRequest) -> Self {
        let CallbackRequest {
            url,
            headers,
            body: _,
        } = request;
        let authorization = if let Some(value) = headers.get(AUTHORIZATION) {
            Some(sha256(&[value.as_bytes()]))
        } else if !url.username().is_empty() || url.password().is_some() {
            Some(sha256(&[
                url.username().as_bytes(),
                b":",
                url.password().unwrap_or_default().as_bytes(),
            ]))
        } else {
            None
        };
        Self {
            host: CallbackHost::of(url),
            authorization,
        }
    }

    pub fn host(&self) -> &CallbackHost {
        &self.host
    }
}

/// Where a callback goes: its host and effective port.
#[derive(Debug, Clone, PartialEq, Eq, Hash, derive_more::Display)]
#[display("{name}:{port}")]
pub struct CallbackHost {
    name: String,
    port: u16,
}

impl CallbackHost {
    fn of(url: &Url) -> Self {
        Self {
            name: url.host_str().unwrap_or_default().to_owned(),
            port: url.port_or_known_default().unwrap_or_default(),
        }
    }
}

fn sha256(parts: &[&[u8]]) -> [u8; SHA256_OUTPUT_LEN] {
    let mut context = Context::new(&SHA256);
    for part in parts {
        context.update(part);
    }
    let mut hash = [0; SHA256_OUTPUT_LEN];
    hash.copy_from_slice(context.finish().as_ref());
    hash
}

#[cfg(test)]
mod tests {
    use http::{HeaderMap, HeaderValue, header::AUTHORIZATION};

    use super::CallbackCredential;
    use crate::CallbackRequest;

    fn credential(url: &str, authorization: Option<&'static str>) -> CallbackCredential {
        let mut headers = HeaderMap::new();
        if let Some(authorization) = authorization {
            headers.insert(AUTHORIZATION, HeaderValue::from_static(authorization));
        }
        CallbackCredential::of(&CallbackRequest {
            url: url.parse().unwrap(),
            headers,
            body: "{}".to_owned(),
        })
    }

    #[test]
    fn a_credential_is_the_host_and_port_and_the_authorization_sent() {
        const DISPATCH: &str = "https://api.github.com/repos/owner/repo/dispatches";
        let token = credential(DISPATCH, Some("Bearer token"));
        for (url, authorization, same) in [
            // Another path on the same host spends the same budget.
            (
                "https://api.github.com/repos/other/repo/dispatches",
                Some("Bearer token"),
                true,
            ),
            (DISPATCH, Some("Bearer other"), false),
            (DISPATCH, None, false),
            (
                "https://github.example.com/repos/owner/repo/dispatches",
                Some("Bearer token"),
                false,
            ),
            (
                "https://api.github.com:8443/repos/owner/repo/dispatches",
                Some("Bearer token"),
                false,
            ),
            // The header replaces the basic auth the client would make of the userinfo.
            (
                "https://user:secret@api.github.com/repos/owner/repo/dispatches",
                Some("Bearer token"),
                true,
            ),
        ] {
            assert_eq!(
                credential(url, authorization) == token,
                same,
                "{url} with {authorization:?}"
            );
        }

        let anonymous = credential(DISPATCH, None);
        for (url, same) in [
            ("https://api.github.com/repos/other/repo/dispatches", true),
            (
                "https://user:secret@api.github.com/repos/owner/repo/dispatches",
                false,
            ),
            (
                "https://user@api.github.com/repos/owner/repo/dispatches",
                false,
            ),
            (
                "https://:secret@api.github.com/repos/owner/repo/dispatches",
                false,
            ),
        ] {
            assert_eq!(credential(url, None) == anonymous, same, "{url}");
        }
        assert!(
            credential("https://user:secret@api.github.com/", None)
                != credential("https://user:other@api.github.com/", None),
            "each userinfo is its own credential"
        );
    }
}
