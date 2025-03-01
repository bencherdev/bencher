use bencher_valid::Slug;

use crate::{ContextPath, ReportContext};

mod base_36;

#[allow(clippy::multiple_inherent_impl)]
impl ReportContext {
    pub fn slug(&self) -> Slug {
        let name = self.repo_name();
        let hash = self.repo_hash();

        todo!()
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

    pub fn testbed_fingerprint(&self) -> Option<&str> {
        self.0
            .get(ContextPath::TESTBED_FINGERPRINT)
            .map(String::as_str)
    }
}
