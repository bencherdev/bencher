use bencher_json::{OAuthIssuer, OAuthResource, system::config::JsonOAuthServer};

/// Bencher as an OAuth authorization server for its MCP server,
/// present only when `plus.oauth` is configured.
#[derive(Debug, Clone)]
pub struct OAuthServer {
    issuer: OAuthIssuer,
    /// Never empty: the first entry is the canonical resource.
    resources: Vec<OAuthResource>,
}

impl From<JsonOAuthServer> for OAuthServer {
    fn from(json: JsonOAuthServer) -> Self {
        let JsonOAuthServer { issuer, resources } = json;
        let resources = resources
            .filter(|resources| !resources.is_empty())
            .unwrap_or_else(|| vec![OAuthResource::mcp(&issuer)]);
        Self { issuer, resources }
    }
}

impl OAuthServer {
    pub fn issuer(&self) -> &OAuthIssuer {
        &self.issuer
    }

    /// Whether tokens issued for `resource` are honored.
    pub fn serves(&self, resource: &OAuthResource) -> bool {
        self.resources.contains(resource)
    }
}

#[cfg(test)]
mod tests {
    use bencher_json::system::config::JsonOAuthServer;

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
        }

        let oauth = server(serde_json::json!({
            "issuer": "https://api.example.com",
            "resources": ["https://mcp.example.com/mcp/"]
        }));
        assert!(oauth.serves(&"https://mcp.example.com/mcp".parse().unwrap()));
        assert!(!oauth.serves(&"https://api.example.com/mcp".parse().unwrap()));
    }
}
