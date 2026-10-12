use bencher_valid::{DateTime, NonEmpty, OAuthClientId, OAuthScope, ResourceName};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::UserKeyUuid;

crate::typed_uuid::typed_uuid!(OAuthGrantUuid);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonOAuthGrants(pub Vec<JsonOAuthGrant>);

crate::from_vec!(JsonOAuthGrants[JsonOAuthGrant]);

/// An app connected to a user's account through OAuth.
#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonOAuthGrant {
    pub uuid: OAuthGrantUuid,
    pub client: JsonOAuthClient,
    pub scopes: Vec<OAuthScope>,
    /// The user API key the connection was approved with, if any.
    /// Revoking or expiring that key disconnects the app.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_key: Option<JsonOAuthGrantKey>,
    pub created: DateTime,
    /// The last time the app renewed its access.
    pub modified: DateTime,
    pub expiration: DateTime,
}

/// The app an OAuth grant was given to.
#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonOAuthClient {
    /// The URL of the app's metadata document.
    pub id: OAuthClientId,
    /// The host serving the app's metadata document, which is who the app is.
    pub host: String,
    /// The name the app gives itself. It is not verified.
    pub name: NonEmpty,
}

/// The user API key an OAuth grant was approved with.
#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonOAuthGrantKey {
    pub uuid: UserKeyUuid,
    pub name: ResourceName,
}
