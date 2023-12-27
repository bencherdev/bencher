use bencher_valid::{Email, Jwt, Slug, UserName};
#[cfg(feature = "plus")]
use bencher_valid::{PlanLevel, Secret};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::JsonUser;

#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonSignup {
    pub name: UserName,
    pub slug: Option<Slug>,
    pub email: Email,
    #[cfg(feature = "plus")]
    pub plan: Option<PlanLevel>,
    pub invite: Option<Jwt>,
    /// I agree to the Bencher Terms of Use (https://bencher.dev/legal/terms-of-use), Privacy Policy (https://bencher.dev/legal/privacy), and License Agreement (https://bencher.dev/legal/license)
    pub i_agree: bool,
}

#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonLogin {
    pub email: Email,
    #[cfg(feature = "plus")]
    pub plan: Option<PlanLevel>,
    pub invite: Option<Jwt>,
}

#[cfg(feature = "plus")]
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonOAuth {
    pub code: Secret,
    pub invite: Option<Jwt>,
}

#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonConfirm {
    pub token: Jwt,
}

#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonAccept {
    pub invite: Jwt,
}

#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonAuthAck {
    pub email: Email,
}

#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonAuthUser {
    pub user: JsonUser,
    pub token: Jwt,
}
