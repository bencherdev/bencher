use derive_more::Display;
#[cfg(feature = "schema")]
use schemars::JsonSchema;

use serde::{Deserialize, Serialize};

#[typeshare::typeshare]
#[derive(
    Debug, Display, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize,
)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum RecaptchaAction {
    Signup,
}
