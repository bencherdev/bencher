use bencher_valid::NonEmpty;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// MCP server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonMcp {
    /// Domain verification token for an app built on this MCP server,
    /// served at `/.well-known/openai-apps-challenge`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub openai_apps_challenge: Option<NonEmpty>,
}
