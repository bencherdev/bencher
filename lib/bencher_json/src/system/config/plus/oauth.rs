use bencher_valid::{OAuthIssuer, OAuthResource, Sanitize};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// OAuth authorization server configuration, for MCP clients to sign in with
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonOAuthServer {
    /// The issuer identifier: the public origin of this API server,
    /// `https`, or `http` only for a loopback host.
    /// Without a URL listed in `resources`, the default resource follows it, so changing it
    /// disconnects the clients on the old default until it is changed back; with one listed,
    /// changing it disconnects nothing while the old origin still reaches this server.
    pub issuer: OAuthIssuer,
    /// The MCP server URLs access tokens are issued for, canonical first.
    /// Defaults to the issuer followed by `/mcp`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resources: Option<Vec<OAuthResource>>,
}

impl Sanitize for JsonOAuthServer {
    fn sanitize(&mut self) {
        // Nothing here is secret yet; destructured so that a new field has to be considered.
        let Self {
            issuer: _,
            resources: _,
        } = self;
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::JsonOAuthServer;

    // A config that does not parse stops the server from starting.
    #[test]
    fn issuer_must_be_https_or_loopback() {
        serde_json::from_value::<JsonOAuthServer>(json!({"issuer": "https://api.example.com"}))
            .unwrap();
        serde_json::from_value::<JsonOAuthServer>(json!({"issuer": "http://localhost:61016"}))
            .unwrap();
        serde_json::from_value::<JsonOAuthServer>(json!({"issuer": "http://api.example.com"}))
            .unwrap_err();
        serde_json::from_value::<JsonOAuthServer>(json!({
            "issuer": "https://api.example.com",
            "resources": ["http://mcp.example.com/mcp"]
        }))
        .unwrap_err();
    }
}
