use std::collections::HashMap;

#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[cfg(feature = "client")]
mod client;

#[cfg(feature = "client")]
pub use client::Fingerprint;

#[typeshare::typeshare]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct ReportContext(pub HashMap<String, String>);

#[allow(clippy::multiple_inherent_impl)]
impl ReportContext {
    pub fn repo_name(&self) -> Option<&str> {
        self.0.get(ContextKey::REPO_NAME).map(String::as_str)
    }

    pub fn repo_hash(&self) -> Option<&str> {
        self.0.get(ContextKey::REPO_HASH).map(String::as_str)
    }

    pub fn branch_ref(&self) -> Option<&str> {
        self.0.get(ContextKey::BRANCH_REF).map(String::as_str)
    }

    pub fn branch_ref_name(&self) -> Option<&str> {
        self.0.get(ContextKey::BRANCH_REF_NAME).map(String::as_str)
    }

    pub fn branch_hash(&self) -> Option<&str> {
        self.0.get(ContextKey::BRANCH_HASH).map(String::as_str)
    }

    pub fn testbed_os(&self) -> Option<&str> {
        self.0.get(ContextKey::TESTBED_OS).map(String::as_str)
    }

    pub fn testbed_fingerprint(&self) -> Option<&str> {
        self.0
            .get(ContextKey::TESTBED_FINGERPRINT)
            .map(String::as_str)
    }
}

struct ContextKey;

impl ContextKey {
    pub const REPO_NAME: &str = "/repo/name";
    pub const REPO_HASH: &str = "/repo/hash";
    pub const BRANCH_REF: &str = "/branch/ref";
    pub const BRANCH_REF_NAME: &str = "/branch/ref/name";
    pub const BRANCH_HASH: &str = "/branch/hash";
    pub const TESTBED_OS: &str = "/testbed/os";
    pub const TESTBED_FINGERPRINT: &str = "/testbed/fingerprint";
}
