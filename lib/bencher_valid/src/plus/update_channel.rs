use std::{fmt, str::FromStr};

#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::wasm_bindgen;

use crate::ValidError;

const STABLE: &str = "stable";
const CANARY: &str = "canary";

#[typeshare::typeshare]
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
// `rename_all` has no serde effect (serialization goes through the `String`
// conversions) but typeshare reads it to emit lowercase TypeScript enum values.
#[serde(try_from = "String", into = "String", rename_all = "snake_case")]
pub enum UpdateChannel {
    #[default]
    Stable,
    Canary,
}

impl fmt::Display for UpdateChannel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_ref())
    }
}

impl TryFrom<String> for UpdateChannel {
    type Error = ValidError;

    fn try_from(update_channel: String) -> Result<Self, Self::Error> {
        match update_channel.as_str() {
            STABLE => Ok(Self::Stable),
            CANARY => Ok(Self::Canary),
            _ => Err(ValidError::UpdateChannel(update_channel)),
        }
    }
}

impl FromStr for UpdateChannel {
    type Err = ValidError;

    fn from_str(update_channel: &str) -> Result<Self, Self::Err> {
        Self::try_from(update_channel.to_owned())
    }
}

impl AsRef<str> for UpdateChannel {
    fn as_ref(&self) -> &str {
        match self {
            Self::Stable => STABLE,
            Self::Canary => CANARY,
        }
    }
}

impl From<UpdateChannel> for String {
    fn from(update_channel: UpdateChannel) -> Self {
        update_channel.as_ref().to_owned()
    }
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
#[cfg_attr(
    not(any(feature = "wasm", test)),
    expect(dead_code, reason = "exported only for wasm and tests")
)]
pub fn is_valid_update_channel(update_channel: &str) -> bool {
    matches!(update_channel, STABLE | CANARY)
}

#[cfg(test)]
mod tests {
    use super::{UpdateChannel, is_valid_update_channel};
    use pretty_assertions::assert_eq;

    #[test]
    fn update_channel_valid() {
        assert_eq!(true, is_valid_update_channel("stable"));
        assert_eq!(true, is_valid_update_channel("canary"));
    }

    #[test]
    fn update_channel_invalid() {
        assert_eq!(false, is_valid_update_channel(""));
        assert_eq!(false, is_valid_update_channel("nightly"));
        assert_eq!(false, is_valid_update_channel("Stable"));
        assert_eq!(false, is_valid_update_channel("Canary"));
        assert_eq!(false, is_valid_update_channel(" stable"));
        assert_eq!(false, is_valid_update_channel("canary "));
    }

    #[test]
    fn update_channel_default() {
        assert_eq!(UpdateChannel::Stable, UpdateChannel::default());
    }

    #[test]
    fn update_channel_serde_roundtrip() {
        let channel: UpdateChannel = serde_json::from_str("\"stable\"").unwrap();
        assert_eq!(channel, UpdateChannel::Stable);
        let json = serde_json::to_string(&channel).unwrap();
        assert_eq!(json, "\"stable\"");

        let channel: UpdateChannel = serde_json::from_str("\"canary\"").unwrap();
        assert_eq!(channel, UpdateChannel::Canary);
        let json = serde_json::to_string(&channel).unwrap();
        assert_eq!(json, "\"canary\"");

        serde_json::from_str::<UpdateChannel>("\"invalid\"").unwrap_err();
    }

    #[test]
    fn update_channel_from_str_roundtrip() {
        for channel in [UpdateChannel::Stable, UpdateChannel::Canary] {
            let parsed: UpdateChannel = channel.to_string().parse().unwrap();
            assert_eq!(channel, parsed);
        }
        "nightly".parse::<UpdateChannel>().unwrap_err();
    }
}
