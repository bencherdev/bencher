use std::path::Path;

use git2::Repository;
use tempfile::tempdir;

use crate::adapter::Report;
use crate::error::CliError;
use crate::save::clone::clone;

#[derive(Debug)]
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
        // todo use tempdir
        let into = Path::new("/tmp/bencher_db");
        let repo = clone(&self.url, self.key.as_deref(), &into)?;
        Ok(())
    }
}
