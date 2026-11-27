use std::{fmt, str::FromStr};

#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use url::{Host, Url};

use crate::ValidError;

/// The OAuth issuer identifier: an `https` origin, or an `http` one for a loopback host.
/// It is written without a trailing slash, as clients compare it byte for byte.
#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(try_from = "String", into = "String")]
pub struct OAuthIssuer(String);

/// An OAuth protected resource: an `https` URL, or an `http` one for a loopback host,
/// with no query or fragment. It is written with a lowercase scheme and host,
/// no default port, and no trailing slash.
#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(try_from = "String", into = "String")]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Text))]
pub struct OAuthResource(String);

#[cfg(feature = "db")]
impl<DB> diesel::serialize::ToSql<diesel::sql_types::Text, DB> for OAuthResource
where
    DB: diesel::backend::Backend,
    for<'a> String: diesel::serialize::ToSql<diesel::sql_types::Text, DB>
        + Into<<DB::BindCollector<'a> as diesel::query_builder::BindCollector<'a, DB>>::Buffer>,
{
    fn to_sql<'b>(
        &'b self,
        out: &mut diesel::serialize::Output<'b, '_, DB>,
    ) -> diesel::serialize::Result {
        out.set_value(self.0.clone());
        Ok(diesel::serialize::IsNull::No)
    }
}

/// A stored resource may be the Bencher CLI's, on plain `http` at any host.
#[cfg(feature = "db")]
impl<DB> diesel::deserialize::FromSql<diesel::sql_types::Text, DB> for OAuthResource
where
    DB: diesel::backend::Backend,
    String: diesel::deserialize::FromSql<diesel::sql_types::Text, DB>,
{
    fn from_sql(bytes: DB::RawValue<'_>) -> diesel::deserialize::Result<Self> {
        let resource = String::from_sql(bytes)?;
        normalize(&resource, Http::AnyHost)
            .map(|Normalized { origin, path }| Self(format!("{origin}{path}")))
            .ok_or_else(|| ValidError::OAuthResource(resource).into())
    }
}

impl TryFrom<String> for OAuthIssuer {
    type Error = ValidError;

    fn try_from(issuer: String) -> Result<Self, Self::Error> {
        if let Some(normalized) = normalize(&issuer, Http::Loopback)
            && normalized.path.is_empty()
        {
            Ok(Self(normalized.origin))
        } else {
            Err(ValidError::OAuthIssuer(issuer))
        }
    }
}

impl TryFrom<String> for OAuthResource {
    type Error = ValidError;

    fn try_from(resource: String) -> Result<Self, Self::Error> {
        let Some(Normalized { origin, path }) = normalize(&resource, Http::Loopback) else {
            return Err(ValidError::OAuthResource(resource));
        };
        Ok(Self(format!("{origin}{path}")))
    }
}

impl OAuthIssuer {
    /// The issuer at the origin a request reached, for the Bencher CLI's client alone, which is
    /// never configured: `https`, or plain `http` on any host.
    pub fn of_request(origin: &str) -> Option<Self> {
        normalize(origin, Http::AnyHost)
            .filter(|normalized| normalized.path.is_empty())
            .map(|normalized| Self(normalized.origin))
    }
}

impl OAuthResource {
    /// What the Bencher CLI's grant acts for: the REST API at the issuer's origin.
    pub fn rest(issuer: &OAuthIssuer) -> Self {
        Self(issuer.0.clone())
    }

    /// The MCP endpoint served at the issuer's origin.
    pub fn mcp(issuer: &OAuthIssuer) -> Self {
        Self(format!("{issuer}/mcp"))
    }

    /// Where this resource's metadata is published (RFC 9728):
    /// the well-known path inserted between its origin and its path.
    pub fn metadata_url(&self) -> String {
        let (origin, path) = self.origin_and_path();
        format!("{origin}{METADATA_PATH}{path}")
    }

    /// Whether a request whose `Host` header is `host` reached this resource's origin.
    pub fn is_served_at(&self, host: &str) -> bool {
        let (origin, _path) = self.origin_and_path();
        origin
            .split_once("://")
            .is_some_and(|(scheme, _authority)| {
                Url::parse(&format!("{scheme}://{host}"))
                    .is_ok_and(|url| url.origin().ascii_serialization() == origin)
            })
    }

    /// The scheme, host, and port the resource is served at.
    pub fn origin(&self) -> &str {
        self.origin_and_path().0
    }

    /// The origin holds the scheme's two slashes, so the path starts at the third, if any.
    fn origin_and_path(&self) -> (&str, &str) {
        let path = self
            .0
            .match_indices('/')
            .nth(2)
            .map_or(self.0.len(), |(index, _slash)| index);
        self.0.split_at_checked(path).unwrap_or((&self.0, ""))
    }
}

const METADATA_PATH: &str = "/.well-known/oauth-protected-resource";

/// A URL split into its ASCII origin and its path without a trailing slash.
struct Normalized {
    origin: String,
    path: String,
}

/// Where plain `http` is allowed.
#[derive(Clone, Copy)]
enum Http {
    /// Only on a loopback host, as OAuth asks of anything configured.
    Loopback,
    /// On any host, for the Bencher CLI's client where no issuer is configured.
    AnyHost,
}

fn normalize(url: &str, http: Http) -> Option<Normalized> {
    let url = Url::parse(url).ok()?;
    let secure = match url.scheme() {
        "https" => true,
        "http" => false,
        _ => return None,
    };
    let host = url.host()?;
    let plain_http = match http {
        Http::Loopback => is_loopback(&host),
        Http::AnyHost => true,
    };
    if !(secure || plain_http)
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return None;
    }
    Some(Normalized {
        origin: url.origin().ascii_serialization(),
        path: url.path().trim_end_matches('/').to_owned(),
    })
}

fn is_loopback(host: &Host<&str>) -> bool {
    match host {
        Host::Domain(domain) => *domain == "localhost",
        Host::Ipv4(ip) => ip.is_loopback(),
        Host::Ipv6(ip) => ip.is_loopback(),
    }
}

impl FromStr for OAuthIssuer {
    type Err = ValidError;

    fn from_str(issuer: &str) -> Result<Self, Self::Err> {
        Self::try_from(issuer.to_owned())
    }
}

impl FromStr for OAuthResource {
    type Err = ValidError;

    fn from_str(resource: &str) -> Result<Self, Self::Err> {
        Self::try_from(resource.to_owned())
    }
}

impl fmt::Display for OAuthIssuer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Display for OAuthResource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for OAuthIssuer {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for OAuthResource {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<OAuthIssuer> for String {
    fn from(issuer: OAuthIssuer) -> Self {
        issuer.0
    }
}

impl From<OAuthResource> for String {
    fn from(resource: OAuthResource) -> Self {
        resource.0
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::{OAuthIssuer, OAuthResource};

    #[test]
    fn https_or_loopback_http() {
        for issuer in [
            "https://api.example.com",
            "http://localhost:61016",
            "http://127.0.0.1:61016",
            "http://[::1]:61016",
        ] {
            issuer.parse::<OAuthIssuer>().unwrap();
            format!("{issuer}/mcp").parse::<OAuthResource>().unwrap();
        }
        for issuer in [
            "http://api.example.com",
            "http://localhost.example.com",
            "ftp://localhost",
        ] {
            issuer.parse::<OAuthIssuer>().unwrap_err();
            format!("{issuer}/mcp")
                .parse::<OAuthResource>()
                .unwrap_err();
        }
    }

    // Kills plain `http` off loopback for a configured issuer, or refused for the Bencher CLI's
    // issuer at a request's origin, which may serve no TLS.
    #[test]
    fn the_cli_issuer_takes_plain_http_on_any_host() {
        let issuer = OAuthIssuer::of_request("http://Bencher.LAN:6610").unwrap();
        assert_eq!(issuer.as_ref(), "http://bencher.lan:6610");
        assert_eq!(
            OAuthResource::rest(&issuer).as_ref(),
            "http://bencher.lan:6610"
        );
        "http://bencher.lan:6610"
            .parse::<OAuthIssuer>()
            .unwrap_err();
        for refused in [
            "http://bencher.lan:6610/api",
            "http://user@bencher.lan",
            "ftp://bencher.lan",
        ] {
            assert!(OAuthIssuer::of_request(refused).is_none(), "{refused}");
        }
    }

    #[test]
    fn issuer_is_an_origin_without_a_trailing_slash() {
        let issuer: OAuthIssuer = "https://API.example.com:443/".parse().unwrap();
        assert_eq!(issuer.as_ref(), "https://api.example.com");
        assert_eq!(
            OAuthResource::mcp(&issuer).as_ref(),
            "https://api.example.com/mcp"
        );
        "https://api.example.com/bencher"
            .parse::<OAuthIssuer>()
            .unwrap_err();
    }

    #[test]
    fn metadata_url_inserts_the_well_known_path_after_the_origin() {
        for (resource, metadata_url) in [
            (
                "https://mcp.example.com/mcp",
                "https://mcp.example.com/.well-known/oauth-protected-resource/mcp",
            ),
            (
                "http://localhost:61016/api/mcp",
                "http://localhost:61016/.well-known/oauth-protected-resource/api/mcp",
            ),
            (
                "https://mcp.example.com",
                "https://mcp.example.com/.well-known/oauth-protected-resource",
            ),
        ] {
            let resource: OAuthResource = resource.parse().unwrap();
            assert_eq!(resource.metadata_url(), metadata_url);
        }
    }

    #[test]
    fn served_at_the_host_of_its_origin() {
        let resource: OAuthResource = "https://mcp.example.com/mcp".parse().unwrap();
        for host in ["mcp.example.com", "MCP.Example.COM", "mcp.example.com:443"] {
            assert!(resource.is_served_at(host), "{host}");
        }
        for host in [
            "api.example.com",
            "mcp.example.com:8443",
            "mcp.example.com.evil.example",
            "example.com",
            "",
        ] {
            assert!(!resource.is_served_at(host), "{host}");
        }

        let resource: OAuthResource = "http://localhost:61016/mcp".parse().unwrap();
        assert!(resource.is_served_at("localhost:61016"));
        assert!(!resource.is_served_at("localhost"));
    }

    #[test]
    fn resource_is_written_one_way() {
        for resource in [
            "https://mcp.example.com/mcp",
            "https://MCP.example.com/mcp/",
            "https://mcp.example.com:443/mcp",
        ] {
            let resource: OAuthResource = resource.parse().unwrap();
            assert_eq!(resource.as_ref(), "https://mcp.example.com/mcp");
        }
        for resource in [
            "https://mcp.example.com/mcp?tenant=1",
            "https://mcp.example.com/mcp#fragment",
            "https://user@mcp.example.com/mcp",
        ] {
            resource.parse::<OAuthResource>().unwrap_err();
        }
    }
}
