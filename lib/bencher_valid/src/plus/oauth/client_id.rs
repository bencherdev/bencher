use std::{fmt, str::FromStr};

#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use typeshare::typeshare;
use url::{Host, Url};

use crate::ValidError;

const MAX_LEN: usize = 2 << 10;

/// An OAuth client identifier: the Bencher CLI's fixed ID, or the URL of an outside app's metadata
/// document.
#[typeshare(serialized_as = "String")]
#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(try_from = "String", into = "String")]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Text))]
pub struct OAuthClientId(#[cfg_attr(feature = "schema", schemars(with = "String"))] Client);

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
enum Client {
    BencherCli,
    Document(Url),
}

#[cfg(feature = "db")]
crate::typed_string!(OAuthClientId);

impl OAuthClientId {
    /// The Bencher CLI's client ID, which no URL can spell.
    pub const BENCHER_CLI: &str = "bencher_cli";

    pub fn bencher_cli() -> Self {
        Self(Client::BencherCli)
    }

    /// Whether this is the Bencher CLI, the client built into the server.
    pub fn is_bencher_cli(&self) -> bool {
        matches!(self.0, Client::BencherCli)
    }

    /// The host that serves the client's metadata document, which is who the client is.
    /// The Bencher CLI has none.
    pub fn host(&self) -> String {
        self.url()
            .and_then(Url::host_str)
            .unwrap_or_default()
            .to_owned()
    }

    /// Where the client's metadata document is published, for any client but the Bencher CLI.
    pub fn url(&self) -> Option<&Url> {
        match &self.0 {
            Client::BencherCli => None,
            Client::Document(url) => Some(url),
        }
    }
}

impl TryFrom<String> for OAuthClientId {
    type Error = ValidError;

    fn try_from(client_id: String) -> Result<Self, Self::Error> {
        if client_id == Self::BENCHER_CLI {
            Ok(Self(Client::BencherCli))
        } else if let Some(url) = parse(&client_id) {
            Ok(Self(Client::Document(url)))
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
        f.write_str(self.as_ref())
    }
}

impl AsRef<str> for OAuthClientId {
    fn as_ref(&self) -> &str {
        match &self.0 {
            Client::BencherCli => Self::BENCHER_CLI,
            Client::Document(url) => url.as_str(),
        }
    }
}

impl From<OAuthClientId> for String {
    fn from(client_id: OAuthClientId) -> Self {
        match client_id.0 {
            Client::BencherCli => OAuthClientId::BENCHER_CLI.to_owned(),
            Client::Document(url) => url.into(),
        }
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

    // Kills a client ID that spells the Bencher CLI's some other way, which an outside app could
    // then publish a document for.
    #[test]
    fn bencher_cli_is_one_spelling() {
        let cli: OAuthClientId = "bencher_cli".parse().unwrap();
        assert!(cli.is_bencher_cli());
        assert_eq!(cli.as_ref(), "bencher_cli");
        assert_eq!(cli.url(), None);
        assert_eq!(cli.host(), "");
        for other in [
            "Bencher_CLI",
            "bencher_cli ",
            "bencher-cli",
            "https://bencher.dev/bencher_cli",
        ] {
            assert!(
                !other
                    .parse::<OAuthClientId>()
                    .is_ok_and(|client_id| client_id.is_bencher_cli()),
                "{other}"
            );
        }
        assert!(
            !"https://client.example/oauth/client.json"
                .parse::<OAuthClientId>()
                .unwrap()
                .is_bencher_cli()
        );
    }

    #[test]
    fn client_id_is_at_most_two_kibibytes() {
        let prefix = "https://client.example/";
        let longest = format!("{prefix}{}", "a".repeat(MAX_LEN - prefix.len()));
        longest.parse::<OAuthClientId>().unwrap();
        format!("{longest}a").parse::<OAuthClientId>().unwrap_err();
    }
}
