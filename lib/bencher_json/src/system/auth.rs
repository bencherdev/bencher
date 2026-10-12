use bencher_valid::{DateTime, Email, Jwt, UserName};
#[cfg(feature = "plus")]
use bencher_valid::{NonEmpty, PlanLevel, Secret};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
#[cfg(feature = "plus")]
use url::Url;

#[cfg(feature = "plus")]
use crate::OAuthAuthorizationUuid;
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
    #[expect(clippy::doc_markdown, reason = "OpenAPI spec")]
    /// I agree to the Bencher Terms of Use (https://bencher.dev/legal/terms-of-use), Privacy Policy (https://bencher.dev/legal/privacy), and License Agreement (https://bencher.dev/legal/license)
    pub i_agree: bool,
    #[cfg(feature = "plus")]
    pub recaptcha_token: Option<NonEmpty>,
    /// An app's pending request to connect, which the emailed link returns to.
    #[cfg(feature = "plus")]
    pub authorization: Option<OAuthAuthorizationUuid>,
}

#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonLogin {
    pub email: Email,
    #[cfg(feature = "plus")]
    pub plan: Option<PlanLevel>,
    pub invite: Option<Jwt>,
    #[cfg(feature = "plus")]
    pub recaptcha_token: Option<NonEmpty>,
    /// An app's pending request to connect, which the emailed link returns to.
    #[cfg(feature = "plus")]
    pub authorization: Option<OAuthAuthorizationUuid>,
}

#[cfg(feature = "plus")]
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonOAuthUrl {
    pub url: Url,
}

#[cfg(feature = "plus")]
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonOAuth {
    pub state: Jwt,
    pub code: Secret,
}

#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonOAuthUser {
    pub user: JsonAuthUser,
    #[cfg(feature = "plus")]
    pub plan: Option<PlanLevel>,
    /// An app's pending request to connect, to return to now that the user is signed in.
    #[cfg(feature = "plus")]
    pub authorization: Option<OAuthAuthorizationUuid>,
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
