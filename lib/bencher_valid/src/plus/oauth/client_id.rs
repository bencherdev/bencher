use std::{fmt, str::FromStr};

#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use typeshare::typeshare;
use url::{Host, Url};

use crate::ValidError;

const MAX_LEN: usize = 2 << 10;

/// An OAuth client identifier: the URL of the client's metadata document.
#[typeshare(serialized_as = "String")]
#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(try_from = "String", into = "String")]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Text))]
pub struct OAuthClientId(#[cfg_attr(feature = "schema", schemars(with = "String"))] Url);

#[cfg(feature = "db")]
crate::typed_string!(OAuthClientId);

impl OAuthClientId {
    /// The host that serves the client's metadata document, which is who the client is.
    pub fn host(&self) -> String {
        self.0.host_str().unwrap_or_default().to_owned()
    }

    /// Where the client's metadata document is published.
    pub fn url(&self) -> &Url {
        &self.0
    }
}

impl TryFrom<String> for OAuthClientId {
    type Error = ValidError;

    fn try_from(client_id: String) -> Result<Self, Self::Error> {
        if let Some(url) = parse(&client_id) {
            Ok(Self(url))
        } else {
            Err(ValidError::OAuthClientId(client_id))
        }
    }
}

/// An `https` URL on a domain's default port, with a path other than `/` and no query, fragment,
/// or userinfo, written in its normal form, since a client ID is compared byte for byte.
fn parse(client_id: &str) -> Option<Url> {
    if client_id.len() > MAX_LEN {
        return None;
    }
    let url = Url::parse(client_id).ok()?;
    // Parsing normalizes, so a client ID it changes has dot segments, a default port,
    // or is another spelling of some other client ID.
    (url.as_str() == client_id
        && url.scheme() == "https"
        && matches!(url.host(), Some(Host::Domain(_)))
        && url.port().is_none()
        && url.path() != "/"
        && url.query().is_none()
        && url.fragment().is_none()
        && url.username().is_empty()
        && url.password().is_none())
    .then_some(url)
}

impl FromStr for OAuthClientId {
    type Err = ValidError;

    fn from_str(client_id: &str) -> Result<Self, Self::Err> {
        Self::try_from(client_id.to_owned())
    }
}

impl fmt::Display for OAuthClientId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0.as_str())
    }
}

impl AsRef<str> for OAuthClientId {
    fn as_ref(&self) -> &str {
        self.0.as_str()
    }
}

impl From<OAuthClientId> for String {
    fn from(client_id: OAuthClientId) -> Self {
        client_id.0.into()
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::{MAX_LEN, OAuthClientId};

    #[test]
    fn client_id_is_a_document_url_in_normal_form() {
        for client_id in [
            "https://chatgpt.com/oauth/client.json",
            "https://claude.ai/oauth/claude-code-client-metadata",
            "https://client.example/a/b",
        ] {
            let parsed: OAuthClientId = client_id.parse().unwrap();
            assert_eq!(parsed.as_ref(), client_id);
        }
        assert_eq!(
            "https://client.example/oauth/client.json"
                .parse::<OAuthClientId>()
                .unwrap()
                .host(),
            "client.example"
        );

        for client_id in [
            "http://client.example/client.json",
            "https://203.0.113.7/client.json",
            "https://[2001:db8::1]/client.json",
            "https://client.example:8443/client.json",
            "https://client.example:443/client.json",
            "https://client.example",
            "https://client.example/",
            "https://client.example/client.json?tenant=1",
            "https://client.example/client.json?",
            "https://client.example/client.json#fragment",
            "https://client.example/client.json#",
            "https://user@client.example/client.json",
            "https://:password@client.example/client.json",
            "https://client.example/oauth/../client.json",
            "https://client.example/./client.json",
            "https://client.example/%2e%2e/client.json",
            "https://Client.Example/client.json",
            r"https://client.example\client.json",
            "https://client.example/client json",
            " https://client.example/client.json",
            "client.example/client.json",
        ] {
            client_id.parse::<OAuthClientId>().unwrap_err();
        }
    }

    #[test]
    fn client_id_is_at_most_two_kibibytes() {
        let prefix = "https://client.example/";
        let longest = format!("{prefix}{}", "a".repeat(MAX_LEN - prefix.len()));
        longest.parse::<OAuthClientId>().unwrap();
        format!("{longest}a").parse::<OAuthClientId>().unwrap_err();
    }
}
