use bencher_valid::{Email, Jwt, Plan, Slug, UserName};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::JsonUser;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonSignup {
    pub name: UserName,
    pub slug: Option<Slug>,
    pub email: Email,
    pub plan: Option<Plan>,
    pub invite: Option<Jwt>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonLogin {
    pub email: Email,
    pub plan: Option<Plan>,
    pub invite: Option<Jwt>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonAuthToken {
    pub token: Jwt,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonConfirm {
    pub user: JsonUser,
    pub token: Jwt,
}
