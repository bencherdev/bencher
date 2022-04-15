use std::fs;
use std::path::Path;

use git2::{Commit, ObjectType};
use git2::{Oid, Repository, Signature};
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

        // let mut index = repo.index()?;
        // index.add_path(bencher_file)?;
        // index.write()?;
        let oid = Self::add(&repo, &bencher_file)?;

        let commit = self.commit(&repo, oid)?;

        Ok(())
    }

    fn add(repo: &Repository, path: &Path) -> Result<Oid, git2::Error> {
        let mut index = repo.index()?;
        index.add_path(path)?;
        index.write_tree()
    }

    // https://zsiciarz.github.io/24daysofrust/book/vol2/day16.html
    fn commit(&self, repo: &Repository, oid: Oid) -> Result<Oid, git2::Error> {
        let signature = self.signature(repo)?;
        let message = self.message.as_deref().unwrap_or(BENCHER_MESSAGE);
        let tree = repo.find_tree(oid)?;
        let parent_commit = Self::last_commit(repo);
        let parents = if let Ok(parent) = &parent_commit {
            vec![parent]
        } else {
            Vec::new()
        };
        repo.commit(
            Some("HEAD"), //  point HEAD to our new commit
            &signature,   // author
            &signature,   // committer
            message,      // commit message
            &tree,        // tree
            &parents,     // parents
        )
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

    fn last_commit(repo: &Repository) -> Result<Commit, git2::Error> {
        let obj = repo.head()?.resolve()?.peel(ObjectType::Commit)?;
        obj.into_commit()
            .map_err(|_| git2::Error::from_str("Couldn't find last commit"))
    }
}
