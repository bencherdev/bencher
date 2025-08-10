use bencher_valid::{DateTime, Email, Jwt, UserName};
#[cfg(feature = "plus")]
use bencher_valid::{PlanLevel, Secret};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{JsonUser, OrganizationUuid, UserSlug};

#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonSignup {
    pub name: UserName,
    pub slug: Option<UserSlug>,
    pub email: Email,
    #[cfg(feature = "plus")]
    pub plan: Option<PlanLevel>,
    pub invite: Option<Jwt>,
    pub claim: Option<OrganizationUuid>,
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
    #[cfg(feature = "plus")]
    pub plan: Option<PlanLevel>,
    pub invite: Option<Jwt>,
    pub claim: Option<OrganizationUuid>,
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
    pub creation: DateTime,
    pub expiration: DateTime,
}
