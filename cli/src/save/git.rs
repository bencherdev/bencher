use std::fmt;

use git2::Repository;

use crate::adapter::Report;
use crate::error::CliError;
use crate::save::clone::clone;

pub struct Git {
    url: String,
    key: Option<String>,
    // repo: Repository,
}

impl Git {
    pub fn new(url: String, key: Option<String>) -> Result<Self, CliError> {
        Ok(Self { url, key })
    }

    pub fn save(&self, report: Report) -> Result<(), CliError> {
        let repo = clone(&self.url, self.key.as_deref())?;
        Ok(())
    }
}

impl fmt::Debug for Git {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Git").field("url", &self.url).finish()
    }
}
