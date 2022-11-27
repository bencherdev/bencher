use bencher_valid::{Email, UserName};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::jwt::JsonWebToken;
use crate::JsonUser;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonSignup {
    pub name: UserName,
    pub slug: Option<String>,
    pub email: Email,
    pub invite: Option<JsonWebToken>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonLogin {
    pub email: Email,
    pub invite: Option<JsonWebToken>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonConfirm {
    pub user: JsonUser,
    pub token: JsonWebToken,
}
