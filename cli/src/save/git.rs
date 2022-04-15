use std::fs;
use std::path::Path;

use git2::Repository;
use tempfile::tempdir;

use crate::adapter::Report;
use crate::error::CliError;
use crate::save::clone::clone;

const BENCHER_FILE: &str = "bencher.json";

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
        let report = serde_json::to_string(&report)?;

        let bencher_file = Path::new(BENCHER_FILE);
        let path = into.join(&bencher_file);
        fs::write(path, &report)?;

        let mut index = repo.index()?;
        index.add_path(bencher_file)?;
        index.write()?;

        Ok(())
    }
}
