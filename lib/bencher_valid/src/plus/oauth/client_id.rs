use std::{fmt, str::FromStr};

#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::ValidError;

/// An OAuth client identifier: the URL of the client's metadata document.
#[typeshare::typeshare]
#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(try_from = "String", into = "String")]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Text))]
pub struct OAuthClientId(String);

#[cfg(feature = "db")]
crate::typed_string!(OAuthClientId);

impl OAuthClientId {
    /// The host that serves the client's metadata document, which is who the client is.
    pub fn host(&self) -> String {
        client_host(&self.0).unwrap_or_default()
    }
}

fn client_host(client_id: &str) -> Option<String> {
    Url::parse(client_id).ok()?.host_str().map(str::to_owned)
}

impl TryFrom<String> for OAuthClientId {
    type Error = ValidError;

    fn try_from(client_id: String) -> Result<Self, Self::Error> {
        if client_host(&client_id).is_some() {
            Ok(Self(client_id))
        } else {
            Err(ValidError::OAuthClientId(client_id))
        }
    }
}

impl FromStr for OAuthClientId {
    type Err = ValidError;

    fn from_str(client_id: &str) -> Result<Self, Self::Err> {
        Self::try_from(client_id.to_owned())
    }
}

impl fmt::Display for OAuthClientId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for OAuthClientId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<OAuthClientId> for String {
    fn from(client_id: OAuthClientId) -> Self {
        client_id.0
    }
}
