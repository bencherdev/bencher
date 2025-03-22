use bencher_valid::{ResourceName, Slug};

use crate::{ContextPath, RunContext};

#[allow(clippy::multiple_inherent_impl)]
impl RunContext {
    fn get(&self, path: &str) -> Option<&String> {
        let key = Self::key(path);
        self.0.get(&key)
    }

    pub fn repo_name(&self) -> Option<&str> {
        self.get(ContextPath::REPO_NAME).map(String::as_str)
    }

    pub fn repo_hash(&self) -> Option<&str> {
        self.get(ContextPath::REPO_HASH).map(String::as_str)
    }

    pub fn branch_ref(&self) -> Option<&str> {
        self.get(ContextPath::BRANCH_REF).map(String::as_str)
    }

    pub fn branch_ref_name(&self) -> Option<&str> {
        self.get(ContextPath::BRANCH_REF_NAME).map(String::as_str)
    }

    pub fn branch_hash(&self) -> Option<&str> {
        self.get(ContextPath::BRANCH_HASH).map(String::as_str)
    }

    pub fn testbed_os(&self) -> Option<&str> {
        self.get(ContextPath::TESTBED_OS).map(String::as_str)
    }

    pub fn testbed_fingerprint(&self) -> Option<&str> {
        self.get(ContextPath::TESTBED_FINGERPRINT)
            .map(String::as_str)
    }

    pub fn name(&self) -> Option<ResourceName> {
        self.repo_name()
            .map_or_else(|| "Project".to_owned(), truncate_name)
            .parse()
            .ok()
    }

    pub fn slug(&self) -> Slug {
        let name = self.repo_name().map(short_name).unwrap_or_default();
        let hash = self.repo_hash().map(short_hash).unwrap_or_default();
        let fingerprint = self
            .testbed_fingerprint()
            .map(short_fingerprint)
            .unwrap_or_default();
        // The spaces here are important,
        // in case any of the values are empty
        // they will essentially be ignored
        let slug = format!("{name} {hash} {fingerprint}");
        debug_assert!(slug.len() <= Slug::MAX_LEN, "Slug is too long: {slug}");
        Slug::new(slug)
    }
}

fn truncate_name(name: &str) -> String {
    name.chars().take(ResourceName::MAX_LEN).collect()
}

const SHORT_NAME_LEN: usize = 42;
const SHORT_HASH_LEN: usize = 7;
const SHORT_FINGERPRINT_LEN: usize = 13;
#[allow(dead_code)]
const DASH_LEN: usize = 1;

// Statically assert that the sum of the lengths of the short names
// is less than or equal to the maximum length of a slug
const _: [(); SHORT_NAME_LEN + DASH_LEN + SHORT_HASH_LEN + DASH_LEN + SHORT_FINGERPRINT_LEN] =
    [(); Slug::MAX_LEN];

fn short_name(name: &str) -> String {
    name.chars().take(SHORT_NAME_LEN).collect()
}

fn short_hash(hash: &str) -> String {
    hash.chars().take(SHORT_HASH_LEN).collect()
}

fn short_fingerprint(fingerprint: &str) -> String {
    fingerprint.chars().take(SHORT_FINGERPRINT_LEN).collect()
}
