use std::net::{Ipv4Addr, Ipv6Addr};

use bencher_json::SessionToken;
use dropshot::RequestInfo;
use http::{HeaderMap, HeaderName, HeaderValue, Method, header, header::InvalidHeaderValue};
use url::{Host, Url};

use crate::context::Relay;

use super::SESSION_LIFETIME;

const HOST_NAME: &str = "__Host-bencher_session";
const PLAIN_NAME: &str = "bencher_session";
const HOST_ATTRIBUTES: &str = "HttpOnly; Secure; SameSite=Strict; Path=/";
const PLAIN_ATTRIBUTES: &str = "HttpOnly; SameSite=Strict; Path=/";
const SEC_FETCH_SITE: HeaderName = HeaderName::from_static("sec-fetch-site");
const SAME_ORIGIN: &[u8] = b"same-origin";

/// The session cookie, in the form the console's URL lets browsers keep it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionCookie {
    /// `__Host-bencher_session`, `Secure`.
    Host,
    /// `bencher_session`, without `Secure`, for a console served over plain HTTP from a host other
    /// than localhost, where browsers refuse both `Secure` and the `__Host-` prefix.
    Plain,
}

impl SessionCookie {
    pub fn new(console_url: &Url) -> Self {
        let is_localhost = match console_url.host() {
            Some(Host::Domain(domain)) => domain == "localhost",
            Some(Host::Ipv4(ip)) => ip == Ipv4Addr::LOCALHOST,
            Some(Host::Ipv6(ip)) => ip == Ipv6Addr::LOCALHOST,
            None => false,
        };
        if console_url.scheme() == "http" && !is_localhost {
            Self::Plain
        } else {
            Self::Host
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Host => HOST_NAME,
            Self::Plain => PLAIN_NAME,
        }
    }

    /// The `Set-Cookie` value that keeps the token for the session's whole life.
    pub fn set(self, token: &SessionToken) -> Result<HeaderValue, InvalidHeaderValue> {
        HeaderValue::try_from(format!(
            "{name}={token}; {attributes}; Max-Age={max_age}",
            name = self.name(),
            token = token.as_ref(),
            attributes = self.attributes(),
            max_age = SESSION_LIFETIME.num_seconds(),
        ))
    }

    /// The `Set-Cookie` value that clears the cookie.
    pub fn clear(self) -> HeaderValue {
        match self {
            Self::Host => HeaderValue::from_static(
                "__Host-bencher_session=; HttpOnly; Secure; SameSite=Strict; Path=/; Max-Age=0",
            ),
            Self::Plain => HeaderValue::from_static(
                "bencher_session=; HttpOnly; SameSite=Strict; Path=/; Max-Age=0",
            ),
        }
    }

    fn attributes(self) -> &'static str {
        match self {
            Self::Host => HOST_ATTRIBUTES,
            Self::Plain => PLAIN_ATTRIBUTES,
        }
    }
}

/// A session cookie that a request presents in place of an `Authorization` header,
/// with what the API needs to judge it.
pub struct CookieCredential {
    host: Option<String>,
    plain: Option<String>,
    method: Method,
    /// The request's `Sec-Fetch-Site` and `Origin`.
    origin_headers: HeaderMap,
    relay_secret: Option<HeaderValue>,
}

impl CookieCredential {
    /// The request's session cookie, in either form, if it has one.
    pub fn from_request(request: &RequestInfo) -> Option<Self> {
        Self::from_headers(request.method(), request.headers())
    }

    pub fn from_headers(method: &Method, headers: &HeaderMap) -> Option<Self> {
        let mut host = None;
        let mut plain = None;
        for (name, value) in headers
            .get_all(header::COOKIE)
            .iter()
            .filter_map(|value| value.to_str().ok())
            .flat_map(|cookies| cookies.split(';'))
            .filter_map(|cookie| cookie.trim().split_once('='))
        {
            if name == HOST_NAME && host.is_none() {
                host = Some(value.to_owned());
            } else if name == PLAIN_NAME && plain.is_none() {
                plain = Some(value.to_owned());
            }
        }
        (host.is_some() || plain.is_some()).then(|| {
            let mut origin_headers = HeaderMap::new();
            for name in [SEC_FETCH_SITE, header::ORIGIN] {
                for value in headers.get_all(&name) {
                    origin_headers.append(name.clone(), value.clone());
                }
            }
            Self {
                host,
                plain,
                method: method.clone(),
                origin_headers,
                relay_secret: headers.get(bencher_json::RELAY_SECRET).cloned(),
            }
        })
    }

    /// The cookie's value, if the API honors it: only the form the console's URL calls for,
    /// and, once a relay secret is set, only from the relay.
    pub fn honored(&self, console_url: &Url, relay: &Relay) -> Option<&str> {
        if !relay.honors_cookie(self.relay_secret.as_ref()) {
            return None;
        }
        match SessionCookie::new(console_url) {
            SessionCookie::Host => self.host.as_deref(),
            SessionCookie::Plain => self.plain.as_deref(),
        }
    }

    pub fn is_allowed_origin(&self, console_url: &Url) -> bool {
        passes_same_origin_check(&self.method, &self.origin_headers, console_url)
    }
}

/// Whether a request may act for the console's reader.
///
/// One that changes state (POST, PUT, PATCH, or DELETE) must come from the console's own origin:
/// by the browser's `Sec-Fetch-Site`, or by its `Origin` where it sends no `Sec-Fetch-Site`.
/// Every other request passes.
pub fn passes_same_origin_check(method: &Method, headers: &HeaderMap, console_url: &Url) -> bool {
    if !matches!(
        *method,
        Method::POST | Method::PUT | Method::PATCH | Method::DELETE
    ) {
        return true;
    }
    if let Some(fetch_site) = headers.get(SEC_FETCH_SITE) {
        fetch_site.as_bytes() == SAME_ORIGIN
    } else {
        headers.get(header::ORIGIN).is_some_and(|origin| {
            origin.as_bytes() == console_url.origin().ascii_serialization().as_bytes()
        })
    }
}

#[cfg(test)]
mod tests {
    use bencher_json::SessionToken;
    use http::{HeaderMap, HeaderValue, Method, header};
    use pretty_assertions::assert_eq;
    use url::Url;

    use super::{CookieCredential, SessionCookie, passes_same_origin_check};
    use crate::context::Relay;

    const TOKEN: &str = "-_v7-_v7-_v7-_v7-_v7-_v7-_v7-_v7-_v7-_v7-_s";

    #[test]
    fn cookie_drops_secure_and_host_prefix_only_on_plain_http_off_localhost() {
        let token: SessionToken = TOKEN.parse().unwrap();
        let host_set = format!(
            "__Host-bencher_session={TOKEN}; HttpOnly; Secure; SameSite=Strict; Path=/; Max-Age=7776000"
        );
        let host_clear =
            "__Host-bencher_session=; HttpOnly; Secure; SameSite=Strict; Path=/; Max-Age=0";
        let plain_set =
            format!("bencher_session={TOKEN}; HttpOnly; SameSite=Strict; Path=/; Max-Age=7776000");
        let plain_clear = "bencher_session=; HttpOnly; SameSite=Strict; Path=/; Max-Age=0";
        for (console_url, set, clear) in [
            ("https://bencher.dev", &host_set, host_clear),
            ("https://bencher.internal", &host_set, host_clear),
            ("http://localhost:3000", &host_set, host_clear),
            ("http://127.0.0.1:3000", &host_set, host_clear),
            ("http://[::1]:3000", &host_set, host_clear),
            ("http://bencher.internal", &plain_set, plain_clear),
            ("http://10.0.0.7:3000", &plain_set, plain_clear),
        ] {
            let cookie = SessionCookie::new(&console_url.parse().unwrap());
            assert_eq!(cookie.set(&token).unwrap(), set.as_str(), "{console_url}");
            assert_eq!(cookie.clear(), clear, "{console_url}");
        }
    }

    // Every write is refused from another site, with or without a session cookie,
    // and no read is.
    #[test]
    fn cross_site_writes_fail_the_same_origin_check() {
        let console_url: Url = "https://bencher.dev".parse().unwrap();
        let mut headers = HeaderMap::new();
        headers.insert("Sec-Fetch-Site", HeaderValue::from_static("cross-site"));
        let mut with_cookie = headers.clone();
        with_cookie.insert(
            header::COOKIE,
            HeaderValue::from_static("__Host-bencher_session=host"),
        );
        for (method, passes) in [
            (Method::GET, true),
            (Method::HEAD, true),
            (Method::POST, false),
            (Method::PUT, false),
            (Method::PATCH, false),
            (Method::DELETE, false),
        ] {
            assert_eq!(
                passes_same_origin_check(&method, &headers, &console_url),
                passes,
                "{method}"
            );
            let cookie = CookieCredential::from_headers(&method, &with_cookie).unwrap();
            assert_eq!(cookie.is_allowed_origin(&console_url), passes, "{method}");
        }
    }

    // A cookie a console of the other form could not have set is never read.
    #[test]
    fn only_the_console_form_of_the_cookie_is_honored() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static(
                "theme=dark; __Host-bencher_session=host; bencher_session=plain",
            ),
        );
        let cookie = CookieCredential::from_headers(&Method::GET, &headers).unwrap();
        let relay = Relay::new(None);
        let secure: Url = "https://bencher.dev".parse().unwrap();
        let plain: Url = "http://bencher.internal".parse().unwrap();
        assert_eq!(cookie.honored(&secure, &relay), Some("host"));
        assert_eq!(cookie.honored(&plain, &relay), Some("plain"));

        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("bencher_session=plain"),
        );
        let cookie = CookieCredential::from_headers(&Method::GET, &headers).unwrap();
        assert_eq!(cookie.honored(&secure, &relay), None);

        headers.insert(header::COOKIE, HeaderValue::from_static("theme=dark"));
        assert!(CookieCredential::from_headers(&Method::GET, &headers).is_none());
    }
}
