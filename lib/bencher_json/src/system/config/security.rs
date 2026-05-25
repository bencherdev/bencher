use bencher_valid::{DateTime, Sanitize, Secret};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonSecurity {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub issuer: Option<String>,
    pub secret_key: Secret,
    /// Previously-issued secret keys that should still validate tokens minted
    /// within their active window, but never sign new ones. See
    /// `bencher_token::TokenKey::new_with_previous` for the rotation procedure.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_secret_keys: Option<Vec<JsonPreviousSecretKey>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonPreviousSecretKey {
    pub secret_key: Secret,
    /// Earliest moment this key was the active signer. Tokens whose creation
    /// precedes this timestamp must not validate against this key.
    pub creation: DateTime,
    /// Moment this key was retired from active signing service. It still
    /// validates tokens whose creation falls within `[creation, retired]`,
    /// but is never used to sign new tokens and rejects any token whose
    /// creation exceeds this timestamp. For a suspected compromise, set this
    /// to the moment the compromise is believed to have occurred — earlier
    /// than the actual demotion — so any later-created token is rejected
    /// even if validly signed.
    pub retired: DateTime,
}

impl Sanitize for JsonSecurity {
    fn sanitize(&mut self) {
        self.secret_key.sanitize();
        if let Some(previous) = self.previous_secret_keys.as_mut() {
            for entry in previous {
                entry.secret_key.sanitize();
            }
        }
    }
}
