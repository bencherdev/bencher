#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{jwt::JsonWebToken, JsonUser};

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonSignup {
    pub name: String,
    pub slug: Option<String>,
    pub email: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonLogin {
    pub email: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonAuthToken {
    pub token: JsonWebToken,
}

impl From<String> for JsonAuthToken {
    fn from(token: String) -> Self {
        Self {
            token: JsonWebToken::from(token),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonConfirm {
    pub user: JsonUser,
    pub token: JsonWebToken,
}

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonInvite {
    pub organization: Uuid,
    pub role: Role,
    pub signup: JsonSignup,
}

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Member,
    Leader
}
