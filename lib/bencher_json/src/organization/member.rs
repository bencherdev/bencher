use std::{fmt, str::FromStr};

use bencher_valid::{Email, Slug, UserName};
use chrono::{DateTime, Utc};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const MEMBER_ROLE: &str = "member";
pub const LEADER_ROLE: &str = "leader";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonNewMember {
    pub name: Option<UserName>,
    pub email: Email,
    pub role: JsonOrganizationRole,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonMembers(pub Vec<JsonMember>);

crate::from_vec!(JsonMembers[JsonMember]);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonMember {
    pub uuid: Uuid,
    pub name: UserName,
    pub slug: Slug,
    pub email: Email,
    pub role: JsonOrganizationRole,
    pub created: DateTime<Utc>,
    pub modified: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonUpdateMember {
    pub role: Option<JsonOrganizationRole>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum JsonOrganizationRole {
    // TODO Team Management
    // Member,
    Leader,
}

impl FromStr for JsonOrganizationRole {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            // TODO Team Management
            // MEMBER_ROLE => Ok(Self::Member),
            LEADER_ROLE => Ok(Self::Leader),
            _ => Err(s.into()),
        }
    }
}

impl fmt::Display for JsonOrganizationRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                // TODO Team Management
                // Self::Member => MEMBER_ROLE,
                Self::Leader => LEADER_ROLE,
            }
        )
    }
}
