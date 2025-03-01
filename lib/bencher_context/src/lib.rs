use std::collections::HashMap;

#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[cfg(feature = "client")]
mod client;
#[cfg(feature = "server")]
mod server;

#[cfg(feature = "client")]
pub use client::Fingerprint;

#[typeshare::typeshare]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct ReportContext(pub HashMap<String, String>);

struct ContextPath;

impl ContextPath {
    pub const REPO_NAME: &str = "/repo/name";
    pub const REPO_HASH: &str = "/repo/hash";
    pub const BRANCH_REF: &str = "/branch/ref";
    pub const BRANCH_REF_NAME: &str = "/branch/ref/name";
    pub const BRANCH_HASH: &str = "/branch/hash";
    pub const TESTBED_OS: &str = "/testbed/os";
    pub const TESTBED_FINGERPRINT: &str = "/testbed/fingerprint";
}
