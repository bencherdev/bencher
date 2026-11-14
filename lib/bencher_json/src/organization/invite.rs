use bencher_valid::{DateTime, Email, InviteSecret, ResourceName};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{JsonPubUser, OrganizationSlug, organization::member::OrganizationRole};

crate::typed_uuid::typed_uuid!(InviteUuid);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonInvites(pub Vec<JsonInvite>);

crate::from_vec!(JsonInvites[JsonInvite]);

/// An invitation to an organization that waits to be accepted, or has expired unaccepted.
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonInvite {
    /// The invitation UUID.
    pub uuid: InviteUuid,
    /// The email the invitation is for.
    pub email: Email,
    /// The organization role the invitation grants.
    pub role: OrganizationRole,
    /// The user who sent the invitation.
    pub inviter: JsonPubUser,
    /// The date time the invitation was sent.
    pub created: DateTime,
    /// The date time the invitation stops working.
    pub expiration: DateTime,
}

/// An invitation, named by the secret its link carries.
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonInviteSecret {
    pub secret: InviteSecret,
}

/// What an invitation offers, read by the secret its link carries.
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonInvitation {
    /// The email the invitation is for.
    pub email: Email,
    pub organization: JsonOrganizationName,
    /// The organization role the invitation grants.
    pub role: OrganizationRole,
    /// The date time the invitation stops working.
    pub expiration: DateTime,
    /// Whether the invitation is for the signed-in reader, when a browser session reads it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub for_you: Option<bool>,
}

/// The organization an invitation is to.
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonOrganizationName {
    pub name: ResourceName,
    pub slug: OrganizationSlug,
}
