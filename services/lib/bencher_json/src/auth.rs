use std::convert::TryFrom;

#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{
    Deserialize,
    Serialize,
};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonSignup {
    pub name:  String,
    pub slug:  Option<String>,
    pub email: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonLogin {
    pub email: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub enum JsonNonce {
    Token(String),
    SecurityCode(JsonSecurityCode),
}

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonSecurityCode(pub [u8; 6]);

impl TryFrom<[u8; 6]> for JsonSecurityCode {
    type Error = &'static str;

    fn try_from(security_code: [u8; 6]) -> Result<Self, Self::Error> {
        for digit in security_code {
            if digit > 9 {
                return Err("Security code digits must be between 0 and 9.");
            }
        }
        Ok(Self(security_code))
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonUser {
    pub uuid:  Uuid,
    pub name:  String,
    pub slug:  String,
    pub email: String,
}
