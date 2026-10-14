use bencher_valid::{OAuthClientId, OAuthIssuer, OAuthResource, Sanitize, Secret};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};

use crate::JsonOAuthClientMetadata;

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
    /// The only clients that may sign in.
    /// Defaults to any client that publishes a valid metadata document.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clients: Option<Vec<JsonOAuthServerClient>>,
    /// The RSA private key, in PEM format, that signs ID tokens.
    /// It enables the `openid`, `email`, and `profile` scopes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signing_key: Option<Secret>,
}

/// A client that may sign in.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonOAuthServerClient {
    /// The client ID: the URL of the client's metadata document.
    pub id: OAuthClientId,
    /// The client's metadata document, used instead of fetching it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<JsonOAuthClientMetadata>,
}

impl<'de> Deserialize<'de> for JsonOAuthServerClient {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Unchecked {
            id: OAuthClientId,
            metadata: Option<JsonOAuthClientMetadata>,
        }

        let Unchecked { id, metadata } = Unchecked::deserialize(deserializer)?;
        if let Some(metadata) = &metadata {
            metadata
                .clone()
                .validate(id.url())
                .map_err(serde::de::Error::custom)?;
        }
        Ok(Self { id, metadata })
    }
}

impl Sanitize for JsonOAuthServer {
    fn sanitize(&mut self) {
        // Destructured so that a new field has to be considered.
        let Self {
            issuer: _,
            resources: _,
            clients: _,
            signing_key,
        } = self;
        signing_key.sanitize();
    }
}

#[cfg(test)]
mod tests {
    use bencher_valid::Sanitize as _;
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

    // Kills a signing key that survives sanitizing, which the served configuration relies on.
    #[test]
    fn signing_key_is_sanitized() {
        let mut config = serde_json::from_value::<JsonOAuthServer>(json!({
            "issuer": "https://api.example.com",
            "signing_key": "-----BEGIN PRIVATE KEY-----",
        }))
        .unwrap();
        config.sanitize();
        assert!(
            !serde_json::to_string(&config)
                .unwrap()
                .contains("BEGIN PRIVATE KEY")
        );
    }

    #[test]
    fn configured_client_metadata_must_be_valid_for_its_id() {
        let config = |client_id: &str| {
            json!({
                "issuer": "https://api.example.com",
                "clients": [
                    {"id": "https://fetched.example/client.json"},
                    {
                        "id": "https://client.example/client.json",
                        "metadata": {
                            "client_id": client_id,
                            "redirect_uris": ["http://127.0.0.1/callback"],
                        },
                    },
                ],
            })
        };
        serde_json::from_value::<JsonOAuthServer>(config("https://client.example/client.json"))
            .unwrap();
        serde_json::from_value::<JsonOAuthServer>(config("https://other.example/client.json"))
            .unwrap_err();
        serde_json::from_value::<JsonOAuthServer>(json!({
            "issuer": "https://api.example.com",
            "clients": [{"id": "http://client.example/client.json"}],
        }))
        .unwrap_err();
    }
}
