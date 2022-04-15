use std::fs;
use std::path::Path;

use git2::Repository;
use git2::Signature;
use tempfile::tempdir;

use crate::adapter::Report;
use crate::error::CliError;
use crate::save::clone::clone;

const BENCHER_FILE: &str = "bencher.json";
const BENCHER_MESSAGE: &str = "bencher save";

#[derive(Debug)]
pub struct Git {
    url: String,
    key: Option<String>,
    name: Option<String>,
    email: Option<String>,
    message: Option<String>,
    // repo: Repository,
}

impl Git {
    pub fn new(
        url: String,
        key: Option<String>,
        name: Option<String>,
        email: Option<String>,
        message: Option<String>,
    ) -> Result<Self, CliError> {
        Ok(Self {
            url,
            key,
            name,
            email,
            message,
        })
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

        let signature = self.signature(&repo)?;

        let message = if let Some(message) = &self.message {
            message
        } else {
            BENCHER_MESSAGE
        };
        // repo.commit_create_buffer(&signature, &signature, message)?;

        Ok(())
    }

    fn signature(&self, repo: &Repository) -> Result<Signature, git2::Error> {
        if let Some(name) = &self.name {
            if let Some(email) = &self.email {
                if let Ok(signature) = Signature::now(name, email) {
                    return Ok(signature);
                }
            }
        }
        repo.signature()
    }
}
