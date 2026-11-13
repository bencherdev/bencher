use bencher_valid::{Email, EmailChangeSecret};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The secret an email change's link carries, which the console reads from the link's fragment.
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonEmailChangeSecret {
    pub secret: EmailChangeSecret,
}

/// An email change that waits for its confirm, read by the secret its link carries.
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonEmailChange {
    /// The user's email, which the confirm replaces.
    pub email: Email,
    /// The email the confirm makes the user's.
    pub new_email: Email,
}
