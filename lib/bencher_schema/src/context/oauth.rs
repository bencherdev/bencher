use std::iter;

use bencher_cimd::OAuthClients;
use bencher_egress::EgressClient;
use bencher_json::{OAuthIssuer, OAuthResource, Secret, system::config::JsonOAuthServer};
use bencher_token::{OAuthSigningKey, OAuthSigningKeyError};
use http::{HeaderMap, Uri, header::HOST, uri::Authority};

/// Bencher as an OAuth authorization server for its MCP server,
/// present only when `plus.oauth` is configured.
#[derive(Debug)]
pub struct OAuthServer {
    issuer: OAuthIssuer,
    /// The resource a client gets when it names none.
    canonical_resource: OAuthResource,
    other_resources: Vec<OAuthResource>,
    clients: OAuthClients,
    /// Present only when OIDC is configured.
    signing_key: Option<OAuthSigningKey>,
}

impl TryFrom<JsonOAuthServer> for OAuthServer {
    type Error = OAuthSigningKeyError;

    fn try_from(json: JsonOAuthServer) -> Result<Self, Self::Error> {
        let JsonOAuthServer {
            issuer,
            resources,
            clients,
            signing_key,
        } = json;
        Self::new(
            issuer,
            resources,
            OAuthClients::new(clients),
            signing_key.as_ref(),
        )
    }
}

impl OAuthServer {
    /// Fetch client metadata documents with `egress`.
    pub fn with_egress(
        json: JsonOAuthServer,
        egress: EgressClient,
    ) -> Result<Self, OAuthSigningKeyError> {
        let JsonOAuthServer {
            issuer,
            resources,
            clients,
            signing_key,
        } = json;
        Self::new(
            issuer,
            resources,
            OAuthClients::with_egress(clients, egress),
            signing_key.as_ref(),
        )
    }

    fn new(
        issuer: OAuthIssuer,
        resources: Option<Vec<OAuthResource>>,
        clients: OAuthClients,
        signing_key: Option<&Secret>,
    ) -> Result<Self, OAuthSigningKeyError> {
        let signing_key = signing_key.map(OAuthSigningKey::new).transpose()?;
        let mut resources = resources.unwrap_or_default().into_iter();
        let canonical_resource = resources
            .next()
            .unwrap_or_else(|| OAuthResource::mcp(&issuer));
        Ok(Self {
            issuer,
            canonical_resource,
            other_resources: resources.collect(),
            clients,
            signing_key,
        })
    }

    pub fn issuer(&self) -> &OAuthIssuer {
        &self.issuer
    }

    pub fn canonical_resource(&self) -> &OAuthResource {
        &self.canonical_resource
    }

    /// Whether tokens issued for `resource` are honored.
    pub fn serves(&self, resource: &OAuthResource) -> bool {
        *resource == self.canonical_resource || self.other_resources.contains(resource)
    }

    /// The resource a request reached, chosen among the resources served by its `Host` header,
    /// or without one by the authority of its URI (HTTP/2), else the canonical resource.
    pub fn resource_at(&self, headers: &HeaderMap, uri: &Uri) -> &OAuthResource {
        headers
            .get(HOST)
            .and_then(|host| host.to_str().ok())
            .or_else(|| uri.authority().map(Authority::as_str))
            .and_then(|host| {
                iter::once(&self.canonical_resource)
                    .chain(&self.other_resources)
                    .find(|resource| resource.is_served_at(host))
            })
            .unwrap_or(&self.canonical_resource)
    }

    pub fn clients(&self) -> &OAuthClients {
        &self.clients
    }

    /// The key that signs ID tokens, when OIDC is configured.
    pub fn signing_key(&self) -> Option<&OAuthSigningKey> {
        self.signing_key.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use bencher_json::system::config::JsonOAuthServer;
    use http::{HeaderMap, HeaderValue, Uri, header::HOST};
    use pretty_assertions::assert_eq;

    use super::{OAuthServer, OAuthSigningKeyError};

    fn server(json: serde_json::Value) -> OAuthServer {
        serde_json::from_value::<JsonOAuthServer>(json)
            .unwrap()
            .try_into()
            .unwrap()
    }

    #[test]
    fn resources_default_to_the_issuer_mcp_endpoint() {
        for oauth in [
            server(serde_json::json!({"issuer": "https://api.example.com"})),
            server(serde_json::json!({"issuer": "https://api.example.com", "resources": []})),
        ] {
            assert!(oauth.serves(&"https://api.example.com/mcp".parse().unwrap()));
            assert!(!oauth.serves(&"https://mcp.example.com/mcp".parse().unwrap()));
            assert_eq!(
                oauth.canonical_resource().as_ref(),
                "https://api.example.com/mcp"
            );
        }

        let oauth = server(serde_json::json!({
            "issuer": "https://api.example.com",
            "resources": ["https://mcp.example.com/mcp/", "https://api.example.com/mcp"]
        }));
        assert!(oauth.serves(&"https://mcp.example.com/mcp".parse().unwrap()));
        assert!(oauth.serves(&"https://api.example.com/mcp".parse().unwrap()));
        assert!(!oauth.serves(&"https://api.example.com/other".parse().unwrap()));
        assert_eq!(
            oauth.canonical_resource().as_ref(),
            "https://mcp.example.com/mcp"
        );
    }

    // Kills a key that cannot sign quietly starting the server without OIDC: startup must fail.
    #[test]
    fn a_bad_signing_key_refuses_the_server() {
        let short = include_str!("../../../bencher_token/src/test/oauth_signing_key_1024.pem");
        for (signing_key, short_key) in [(short, true), ("not a key", false)] {
            let json = serde_json::from_value::<JsonOAuthServer>(serde_json::json!({
                "issuer": "https://api.example.com",
                "signing_key": signing_key,
            }))
            .unwrap();
            let error = OAuthServer::try_from(json).unwrap_err();
            assert_eq!(
                matches!(error, OAuthSigningKeyError::Short),
                short_key,
                "{error}"
            );
            assert_eq!(
                matches!(error, OAuthSigningKeyError::Pem(_)),
                !short_key,
                "{error}"
            );
        }
    }

    // Kills holding the key as its PEM, which a debug build prints in full.
    #[test]
    fn debug_output_leaves_out_the_signing_key() {
        let pem = include_str!("../../../bencher_token/src/test/oauth_signing_key.pem");
        let oauth = server(serde_json::json!({
            "issuer": "https://api.example.com",
            "signing_key": pem,
        }));
        assert!(oauth.signing_key().is_some());
        let secret_line = pem.lines().nth(1).unwrap();
        assert!(!format!("{oauth:?}").contains(secret_line));
    }

    #[test]
    fn resource_at_the_host_or_else_the_uri_authority() {
        let oauth = server(serde_json::json!({
            "issuer": "https://api.example.com",
            "resources": ["https://api.example.com/mcp", "https://mcp.example.com/mcp"]
        }));
        let mut headers = HeaderMap::new();
        headers.insert(HOST, HeaderValue::from_static("mcp.example.com"));
        let path: Uri = "/mcp".parse().unwrap();
        let absolute: Uri = "https://mcp.example.com/mcp".parse().unwrap();
        for (headers, uri, resource) in [
            (&headers, &path, "https://mcp.example.com/mcp"),
            (&HeaderMap::new(), &absolute, "https://mcp.example.com/mcp"),
            (&HeaderMap::new(), &path, "https://api.example.com/mcp"),
        ] {
            assert_eq!(oauth.resource_at(headers, uri).as_ref(), resource, "{uri}");
        }
    }
}
