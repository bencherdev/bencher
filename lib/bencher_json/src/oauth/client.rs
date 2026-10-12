use bencher_valid::{NonEmpty, OAuthRedirectUri, ValidError};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use url::Url;

const NONE: &str = "none";
const AUTHORIZATION_CODE: &str = "authorization_code";
const REFRESH_TOKEN: &str = "refresh_token";
const CODE: &str = "code";
const MAX_NAME_CHARS: usize = 64;

/// An OAuth client's metadata document, published at its client ID.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonOAuthClientMetadata {
    /// Must be the URL the document is published at.
    pub client_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_name: Option<String>,
    pub redirect_uris: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_endpoint_auth_method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_endpoint_auth_methods_supported: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grant_types: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_types: Option<Vec<String>>,
    /// A client with a secret is refused: every client is public.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_secret: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_secret_expires_at: Option<serde_json::Value>,
}

/// What a valid metadata document says about its client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OAuthClientMetadata {
    /// The name the client gives itself, cleaned for display, or the host of its client ID.
    pub name: NonEmpty,
    pub redirect_uris: Vec<OAuthRedirectUri>,
    /// Whether the client asks for refresh tokens.
    pub refresh: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum OAuthClientMetadataError {
    #[error("Client metadata document is not a valid JSON object: {0}")]
    Json(serde_json::Error),
    #[error("Client metadata document names another client ID")]
    ClientId,
    #[error("Client metadata document holds a client secret")]
    ClientSecret,
    #[error(
        "Client metadata document does not allow the `none` token endpoint authentication method"
    )]
    AuthMethod,
    #[error("Client metadata document does not allow the `authorization_code` grant type")]
    GrantType,
    #[error("Client metadata document does not allow the `code` response type")]
    ResponseType,
    #[error("Client metadata document has no redirect URIs")]
    NoRedirectUris,
    #[error("Client metadata document has an invalid redirect URI: {0}")]
    RedirectUri(ValidError),
}

impl OAuthClientMetadata {
    /// Validate the metadata document published at `client_id`.
    pub fn from_document(
        document: &[u8],
        client_id: &Url,
    ) -> Result<Self, OAuthClientMetadataError> {
        serde_json::from_slice::<JsonOAuthClientMetadata>(document)
            .map_err(OAuthClientMetadataError::Json)?
            .validate(client_id)
    }

    /// Whether the client registered a redirect URI that admits `requested`.
    pub fn admits(&self, requested: &OAuthRedirectUri) -> bool {
        self.redirect_uris
            .iter()
            .any(|registered| registered.admits(requested))
    }
}

impl JsonOAuthClientMetadata {
    /// Validate this document as the one published at `client_id`.
    pub fn validate(
        self,
        client_id: &Url,
    ) -> Result<OAuthClientMetadata, OAuthClientMetadataError> {
        let Self {
            client_id: document_client_id,
            client_name,
            redirect_uris,
            token_endpoint_auth_method,
            token_endpoint_auth_methods_supported,
            grant_types,
            response_types,
            client_secret,
            client_secret_expires_at,
        } = self;
        if document_client_id != client_id.as_str() {
            return Err(OAuthClientMetadataError::ClientId);
        }
        let Some(host) = client_id
            .host_str()
            .and_then(|host| host.parse::<NonEmpty>().ok())
        else {
            return Err(OAuthClientMetadataError::ClientId);
        };
        if client_secret.is_some() || client_secret_expires_at.is_some() {
            return Err(OAuthClientMetadataError::ClientSecret);
        }
        // The list of methods a client supports takes precedence over the one it prefers.
        let public = if let Some(methods) = token_endpoint_auth_methods_supported {
            methods.iter().any(|method| method == NONE)
        } else {
            token_endpoint_auth_method.as_deref().unwrap_or(NONE) == NONE
        };
        if !public {
            return Err(OAuthClientMetadataError::AuthMethod);
        }
        let grant_types = grant_types.unwrap_or_else(|| vec![AUTHORIZATION_CODE.to_owned()]);
        if !grant_types.iter().any(|grant| grant == AUTHORIZATION_CODE) {
            return Err(OAuthClientMetadataError::GrantType);
        }
        if !response_types.is_none_or(|types| types.iter().any(|response| response == CODE)) {
            return Err(OAuthClientMetadataError::ResponseType);
        }
        if redirect_uris.is_empty() {
            return Err(OAuthClientMetadataError::NoRedirectUris);
        }
        let redirect_uris = redirect_uris
            .iter()
            .map(|uri| uri.parse::<OAuthRedirectUri>())
            .collect::<Result<Vec<_>, _>>()
            .map_err(OAuthClientMetadataError::RedirectUri)?;
        Ok(OAuthClientMetadata {
            name: display_name(client_name.as_deref()).unwrap_or(host),
            redirect_uris,
            refresh: grant_types.iter().any(|grant| grant == REFRESH_TOKEN),
        })
    }
}

/// A client's name as the consent page may show it: one line of visible text,
/// at most 64 characters.
fn display_name(client_name: Option<&str>) -> Option<NonEmpty> {
    let visible = client_name?
        .chars()
        .map(|c| if c.is_whitespace() { ' ' } else { c })
        .filter(|c| !c.is_control() && !is_invisible(*c))
        .collect::<String>();
    let name = visible
        .trim()
        .chars()
        .take(MAX_NAME_CHARS)
        .collect::<String>();
    name.trim_end().parse().ok()
}

/// Invisible formatting characters, which can disguise a name.
fn is_invisible(c: char) -> bool {
    matches!(
        c,
        '\u{00AD}'
            | '\u{061C}'
            | '\u{180E}'
            | '\u{200B}'..='\u{200F}'
            | '\u{202A}'..='\u{202E}'
            | '\u{2060}'..='\u{2069}'
            | '\u{FEFF}'
            | '\u{FFF9}'..='\u{FFFB}'
    )
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use serde_json::{Value, json};
    use url::Url;

    use super::{OAuthClientMetadata, OAuthClientMetadataError};

    const CLIENT_ID: &str = "https://client.example/oauth/client.json";

    fn validate(document: &Value) -> Result<OAuthClientMetadata, OAuthClientMetadataError> {
        OAuthClientMetadata::from_document(document.to_string().as_bytes(), &client_id())
    }

    fn client_id() -> Url {
        Url::parse(CLIENT_ID).unwrap()
    }

    fn document(fields: &Value) -> Value {
        let mut document = json!({
            "client_id": CLIENT_ID,
            "client_name": "Example",
            "redirect_uris": ["https://client.example/callback"],
        });
        for (key, value) in fields.as_object().unwrap() {
            document[key] = value.clone();
        }
        document
    }

    #[test]
    fn chatgpt_shaped_document_is_public() {
        let metadata = validate(&document(&json!({
            "client_name": "ChatGPT",
            "token_endpoint_auth_method": "private_key_jwt",
            "token_endpoint_auth_methods_supported": ["none", "private_key_jwt"],
            "grant_types": ["authorization_code", "refresh_token"],
            "response_types": ["code"],
            "jwks_uri": "https://client.example/oauth/jwks.json",
        })))
        .unwrap();
        assert_eq!(metadata.name.as_ref(), "ChatGPT");
        assert!(metadata.refresh);
        assert_eq!(
            metadata.redirect_uris,
            vec!["https://client.example/callback".parse().unwrap()]
        );
    }

    #[test]
    fn only_public_clients() {
        validate(&document(&json!({}))).unwrap();
        validate(&document(&json!({"token_endpoint_auth_method": "none"}))).unwrap();
        for fields in [
            json!({"token_endpoint_auth_method": "private_key_jwt"}),
            json!({"token_endpoint_auth_methods_supported": ["private_key_jwt"]}),
            json!({
                "token_endpoint_auth_method": "none",
                "token_endpoint_auth_methods_supported": ["client_secret_basic"],
            }),
        ] {
            assert!(
                matches!(
                    validate(&document(&fields)),
                    Err(OAuthClientMetadataError::AuthMethod)
                ),
                "{fields}"
            );
        }
        for fields in [
            json!({"client_secret": "secret"}),
            json!({"client_secret_expires_at": 0}),
        ] {
            assert!(
                matches!(
                    validate(&document(&fields)),
                    Err(OAuthClientMetadataError::ClientSecret)
                ),
                "{fields}"
            );
        }
    }

    #[test]
    fn client_id_is_where_the_document_is_published() {
        for client_id in [
            "https://client.example/oauth/client.json/",
            "https://client.example/oauth/other.json",
            "https://evil.example/oauth/client.json",
        ] {
            assert!(
                matches!(
                    validate(&document(&json!({"client_id": client_id}))),
                    Err(OAuthClientMetadataError::ClientId)
                ),
                "{client_id}"
            );
        }
    }

    #[test]
    fn code_flow_with_refresh_only_when_asked() {
        assert!(!validate(&document(&json!({}))).unwrap().refresh);
        assert!(
            !validate(&document(&json!({"grant_types": ["authorization_code"]})))
                .unwrap()
                .refresh
        );
        assert!(matches!(
            validate(&document(&json!({"grant_types": ["refresh_token"]}))),
            Err(OAuthClientMetadataError::GrantType)
        ));
        validate(&document(&json!({"response_types": ["code", "token"]}))).unwrap();
        assert!(matches!(
            validate(&document(&json!({"response_types": ["token"]}))),
            Err(OAuthClientMetadataError::ResponseType)
        ));
    }

    #[test]
    fn redirect_uris_are_valid_and_present() {
        assert!(matches!(
            validate(&document(&json!({"redirect_uris": []}))),
            Err(OAuthClientMetadataError::NoRedirectUris)
        ));
        assert!(matches!(
            validate(&document(&json!({
                "redirect_uris": ["https://client.example/callback", "http://client.example/callback"]
            }))),
            Err(OAuthClientMetadataError::RedirectUri(_))
        ));
        assert!(matches!(
            validate(&json!(["not", "an", "object"])),
            Err(OAuthClientMetadataError::Json(_))
        ));

        let metadata = validate(&document(&json!({
            "redirect_uris": ["https://client.example/callback", "http://localhost/callback"]
        })))
        .unwrap();
        assert!(metadata.admits(&"http://localhost:53682/callback".parse().unwrap()));
        assert!(metadata.admits(&"https://client.example/callback".parse().unwrap()));
        assert!(!metadata.admits(&"https://client.example/other".parse().unwrap()));
    }

    #[test]
    fn name_is_one_line_of_visible_text() {
        for (client_name, name) in [
            (json!("  Example\u{0007} App\n"), "Example App"),
            (json!("\u{202E}ppA elpmaxE"), "ppA elpmaxE"),
            (json!("Zero\u{200B}Width"), "ZeroWidth"),
            (json!("Two\tWords\u{2028}Lines\u{2029}"), "Two Words Lines"),
            (json!("Soft\u{00AD}ware\u{061C}"), "Software"),
            (
                json!("\u{FFF9}Annotated\u{FFFA}\u{FFFB}\u{180E}"),
                "Annotated",
            ),
            (json!("x".repeat(65)), &*"x".repeat(64)),
            (json!(format!("{} y", "x".repeat(63))), &*"x".repeat(63)),
            (json!(" \t\u{200F} "), "client.example"),
            (Value::Null, "client.example"),
        ] {
            let metadata = validate(&document(&json!({"client_name": client_name}))).unwrap();
            assert_eq!(metadata.name.as_ref(), name, "{client_name}");
        }
    }
}
