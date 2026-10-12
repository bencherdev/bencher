use std::{
    fmt,
    net::{Ipv4Addr, Ipv6Addr},
    str::FromStr,
};

use url::{Host, Url};

use crate::ValidError;

/// Where an OAuth client receives its authorization response.
///
/// It is an absolute `https` URL, or an `http` one on a loopback host, with no fragment or
/// userinfo, kept as written, since a client's redirect URI is matched as a string.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Text))]
pub struct OAuthRedirectUri {
    uri: String,
    url: Url,
}

#[cfg(feature = "db")]
crate::typed_string!(OAuthRedirectUri);

impl OAuthRedirectUri {
    /// Whether a client that registered this URI may be sent to `requested`:
    /// the same string, or for a loopback `http` URI, the same URL on any port.
    pub fn admits(&self, requested: &Self) -> bool {
        if self.uri == requested.uri {
            return true;
        }
        if let Some(registered) = self.any_port()
            && let Some(requested) = requested.any_port()
        {
            registered == requested
        } else {
            false
        }
    }

    /// Whether the response goes to a program on the user's own computer.
    pub fn is_loopback(&self) -> bool {
        self.url.host().as_ref().is_some_and(is_loopback)
    }

    pub fn host(&self) -> String {
        self.url.host_str().unwrap_or_default().to_owned()
    }

    pub fn url(&self) -> &Url {
        &self.url
    }

    /// A loopback `http` URI without its port, which the client picks when it listens.
    fn any_port(&self) -> Option<Url> {
        if self.url.scheme() != "http" {
            return None;
        }
        let mut url = self.url.clone();
        url.set_port(None).ok()?;
        Some(url)
    }
}

impl FromStr for OAuthRedirectUri {
    type Err = ValidError;

    fn from_str(uri: &str) -> Result<Self, Self::Err> {
        let url = Url::parse(uri).map_err(|_err| ValidError::OAuthRedirectUri(uri.to_owned()))?;
        let allowed = match url.scheme() {
            "https" => true,
            "http" => url.host().as_ref().is_some_and(is_loopback),
            _ => false,
        };
        if allowed
            && url.fragment().is_none()
            && url.username().is_empty()
            && url.password().is_none()
        {
            Ok(Self {
                uri: uri.to_owned(),
                url,
            })
        } else {
            Err(ValidError::OAuthRedirectUri(uri.to_owned()))
        }
    }
}

fn is_loopback(host: &Host<&str>) -> bool {
    match host {
        Host::Domain(domain) => *domain == "localhost",
        Host::Ipv4(ip) => *ip == Ipv4Addr::LOCALHOST,
        Host::Ipv6(ip) => *ip == Ipv6Addr::LOCALHOST,
    }
}

impl fmt::Display for OAuthRedirectUri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.uri)
    }
}

impl AsRef<str> for OAuthRedirectUri {
    fn as_ref(&self) -> &str {
        &self.uri
    }
}

#[cfg(test)]
mod tests {
    use super::OAuthRedirectUri;

    fn uri(uri: &str) -> OAuthRedirectUri {
        uri.parse().unwrap()
    }

    #[test]
    fn https_or_loopback_http_without_a_fragment_or_userinfo() {
        for valid in [
            "https://chatgpt.com/connector_platform_oauth_redirect",
            "https://client.example:8443/callback?tenant=1",
            "http://localhost/callback",
            "http://127.0.0.1:61016/callback",
            "http://[::1]:61016/callback",
        ] {
            uri(valid);
        }
        for invalid in [
            "http://client.example/callback",
            "http://localhost.evil.example/callback",
            "http://127.0.0.2/callback",
            "https://client.example/callback#fragment",
            "https://user@client.example/callback",
            "https://:password@client.example/callback",
            "com.example.app:/callback",
            "/callback",
        ] {
            invalid.parse::<OAuthRedirectUri>().unwrap_err();
        }
    }

    #[test]
    fn admits_only_the_registered_string() {
        let registered = uri("https://client.example/callback");
        assert!(registered.admits(&uri("https://client.example/callback")));
        for variant in [
            "https://client.example/callback/",
            "https://client.example/callback/more",
            "https://client.example/callbacks",
            "https://client.example/callback?next=1",
            "https://client.example.evil.example/callback",
            "https://evil.example/https://client.example/callback",
            "https://client.example:443/callback",
            "https://CLIENT.example/callback",
        ] {
            assert!(!registered.admits(&uri(variant)), "{variant}");
        }
    }

    #[test]
    fn admits_a_loopback_http_uri_on_any_port() {
        for (registered, requested) in [
            (
                "http://localhost/callback",
                "http://localhost:53682/callback",
            ),
            ("http://127.0.0.1/callback", "http://127.0.0.1:1/callback"),
            ("http://[::1]:8080/callback", "http://[::1]:9090/callback"),
            (
                "http://localhost/callback?a=1",
                "http://localhost:2/callback?a=1",
            ),
        ] {
            assert!(uri(registered).admits(&uri(requested)), "{requested}");
        }
        for (registered, requested) in [
            ("http://localhost/callback", "http://localhost:53682/other"),
            (
                "http://localhost/callback",
                "http://localhost:53682/callback?a=1",
            ),
            (
                "http://localhost/callback",
                "http://127.0.0.1:53682/callback",
            ),
            (
                "https://localhost/callback",
                "https://localhost:8443/callback",
            ),
            (
                "http://localhost/callback",
                "https://localhost:8443/callback",
            ),
        ] {
            assert!(!uri(registered).admits(&uri(requested)), "{requested}");
        }
    }

    #[test]
    fn loopback_is_where_the_response_goes() {
        assert!(uri("http://localhost:53682/callback").is_loopback());
        assert!(uri("https://127.0.0.1/callback").is_loopback());
        assert!(!uri("https://client.example/callback").is_loopback());
    }
}
