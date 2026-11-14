use bencher_valid::Email;
#[cfg(feature = "plus")]
use bencher_valid::{Jwt, Secret};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
#[cfg(feature = "plus")]
use url::Url;

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
pub struct JsonAuthAck {
    pub email: Email,
}
