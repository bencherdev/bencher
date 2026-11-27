use std::{fmt, str::FromStr};

#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ValidError;

const MAX_LEN: usize = 64;

/// The hostname of the device the Bencher CLI signs in on, as it names itself: 1 to 64 ASCII
/// letters, digits, `.`, `-`, or `_`, starting with a letter or digit.
#[typeshare::typeshare(serialized_as = "String")]
#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(try_from = "String", into = "String")]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Text))]
pub struct OAuthDevice(String);

#[cfg(feature = "db")]
crate::typed_string!(OAuthDevice);

impl TryFrom<String> for OAuthDevice {
    type Error = ValidError;

    fn try_from(device: String) -> Result<Self, Self::Error> {
        let starts = device
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_alphanumeric());
        let valid = starts
            && device.len() <= MAX_LEN
            && device
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'));
        if valid {
            Ok(Self(device))
        } else {
            Err(ValidError::OAuthDevice(device))
        }
    }
}

impl FromStr for OAuthDevice {
    type Err = ValidError;

    fn from_str(device: &str) -> Result<Self, Self::Err> {
        Self::try_from(device.to_owned())
    }
}

impl fmt::Display for OAuthDevice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for OAuthDevice {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<OAuthDevice> for String {
    fn from(device: OAuthDevice) -> Self {
        device.0
    }
}

#[cfg(test)]
mod tests {
    use super::{MAX_LEN, OAuthDevice};

    #[test]
    fn device_is_a_hostname() {
        for device in [
            "ada-macbook",
            "Adas-MacBook-Pro.local",
            "build_01",
            "7",
            &"a".repeat(MAX_LEN),
        ] {
            assert_eq!(device.parse::<OAuthDevice>().unwrap().as_ref(), device);
        }
        for device in [
            "",
            "-ada",
            ".ada",
            "_ada",
            "ada macbook",
            "ada<b>",
            "ada\u{e9}",
            "ada\n",
            &"a".repeat(MAX_LEN + 1),
        ] {
            device.parse::<OAuthDevice>().unwrap_err();
        }
    }
}
