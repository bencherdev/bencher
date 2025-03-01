use bencher_valid::{Slug, MAX_LEN};
use uuid::Uuid;

use crate::{ContextPath, ReportContext};

mod base36;

#[allow(clippy::multiple_inherent_impl)]
impl ReportContext {
    pub fn slug(&self) -> Slug {
        // + 42 chars
        let name = self.repo_name().map(truncate_name).unwrap_or_default();
        // + 1 char (-)
        // + 7 chars
        let hash = self.repo_hash().map(short_hash).unwrap_or_default();
        // + 1 char (-)
        // + 13 chars
        let fingerprint = self
            .testbed_fingerprint()
            .map(base36::encode_uuid)
            .as_deref()
            .map(short_fingerprint)
            .unwrap_or_default();
        // The spaces here are important,
        // in case any of the values are empty
        // they will essentially be ignored
        let slug = format!("{name} {hash} {fingerprint}");
        debug_assert!(slug.len() <= MAX_LEN, "Slug is too long: {slug}");
        Slug::new(slug)
    }

    pub fn repo_name(&self) -> Option<&str> {
        self.0.get(ContextPath::REPO_NAME).map(String::as_str)
    }

    pub fn repo_hash(&self) -> Option<&str> {
        self.0.get(ContextPath::REPO_HASH).map(String::as_str)
    }

    pub fn branch_ref(&self) -> Option<&str> {
        self.0.get(ContextPath::BRANCH_REF).map(String::as_str)
    }

    pub fn branch_ref_name(&self) -> Option<&str> {
        self.0.get(ContextPath::BRANCH_REF_NAME).map(String::as_str)
    }

    pub fn branch_hash(&self) -> Option<&str> {
        self.0.get(ContextPath::BRANCH_HASH).map(String::as_str)
    }

    pub fn testbed_os(&self) -> Option<&str> {
        self.0.get(ContextPath::TESTBED_OS).map(String::as_str)
    }

    pub fn testbed_fingerprint(&self) -> Option<Uuid> {
        self.0.get(ContextPath::TESTBED_FINGERPRINT)?.parse().ok()
    }
}

fn truncate_name(name: &str) -> String {
    name.chars().take(42).collect()
}

fn short_hash(hash: &str) -> String {
    hash.chars().take(7).collect()
}

fn short_fingerprint(fingerprint: &str) -> String {
    fingerprint.chars().take(13).collect()
}
