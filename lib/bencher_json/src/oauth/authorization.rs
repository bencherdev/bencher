use bencher_valid::{DateTime, NonEmpty, OAuthClientId, OAuthScope, UserKey};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use url::Url;

crate::typed_uuid::typed_uuid!(OAuthAuthorizationUuid);

/// An app's pending request to connect to a user's account through OAuth.
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonOAuthAuthorization {
    pub uuid: OAuthAuthorizationUuid,
    pub client: JsonOAuthAuthorizationClient,
    pub redirect: JsonOAuthAuthorizationRedirect,
    pub scopes: Vec<OAuthScope>,
    pub expiration: DateTime,
}

/// The app asking to connect.
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonOAuthAuthorizationClient {
    /// The URL of the app's metadata document.
    pub id: OAuthClientId,
    /// The host serving the app's metadata document, which is who the app is.
    pub host: String,
    /// The name the app gives itself. It is not verified.
    pub name: NonEmpty,
    /// Whether the app is one this server knows by its host.
    pub verified: bool,
}

/// Where the app receives the user's answer.
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonOAuthAuthorizationRedirect {
    pub host: String,
    /// Whether the answer goes to a program on the user's own computer.
    pub loopback: bool,
}

/// A user's answer to an app's request to connect.
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonOAuthDecision {
    pub approve: bool,
    /// A user API key to approve with, in place of the session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<UserKey>,
}

/// Where to send the user's browser with the answer.
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonOAuthRedirect {
    pub url: Url,
}
