use bencher_cimd::OAuthClients;
use bencher_egress::EgressClient;
use bencher_json::{OAuthIssuer, OAuthResource, system::config::JsonOAuthServer};

/// Bencher as an OAuth authorization server for its MCP server,
/// present only when `plus.oauth` is configured.
#[derive(Debug)]
pub struct OAuthServer {
    issuer: OAuthIssuer,
    /// The resource a client gets when it names none.
    canonical_resource: OAuthResource,
    other_resources: Vec<OAuthResource>,
    clients: OAuthClients,
}

impl From<JsonOAuthServer> for OAuthServer {
    fn from(json: JsonOAuthServer) -> Self {
        let JsonOAuthServer {
            issuer,
            resources,
            clients,
        } = json;
        Self::new(issuer, resources, OAuthClients::new(clients))
    }
}

impl OAuthServer {
    /// Fetch client metadata documents with `egress`.
    pub fn with_egress(json: JsonOAuthServer, egress: EgressClient) -> Self {
        let JsonOAuthServer {
            issuer,
            resources,
            clients,
        } = json;
        Self::new(
            issuer,
            resources,
            OAuthClients::with_egress(clients, egress),
        )
    }

    fn new(
        issuer: OAuthIssuer,
        resources: Option<Vec<OAuthResource>>,
        clients: OAuthClients,
    ) -> Self {
        let mut resources = resources.unwrap_or_default().into_iter();
        let canonical_resource = resources
            .next()
            .unwrap_or_else(|| OAuthResource::mcp(&issuer));
        Self {
            issuer,
            canonical_resource,
            other_resources: resources.collect(),
            clients,
        }
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

    pub fn clients(&self) -> &OAuthClients {
        &self.clients
    }
}

#[cfg(test)]
mod tests {
    use bencher_json::system::config::JsonOAuthServer;
    use pretty_assertions::assert_eq;

    use super::OAuthServer;

    fn server(json: serde_json::Value) -> OAuthServer {
        serde_json::from_value::<JsonOAuthServer>(json)
            .unwrap()
            .into()
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
}
