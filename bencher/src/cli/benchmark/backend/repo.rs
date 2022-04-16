use std::fs;
use std::path::Path;

use git2::Direction;
use git2::{Commit, Cred, ObjectType, RemoteCallbacks};
use git2::{Oid, Repository, Signature};
use tempfile::tempdir;

use crate::cli::adapter::Report;
use crate::cli::args::CliRepo;
use crate::error::CliError;

const BENCHER_DIR: &str = "bencherdb";
const BENCHER_FILE: &str = "bencher.json";
const BENCHER_MESSAGE: &str = "bencher";

#[derive(Debug)]
pub struct Repo {
    url: String,
    key: Option<String>,
    name: Option<String>,
    email: Option<String>,
    message: Option<String>,
}

impl From<CliRepo> for Repo {
    fn from(repo: CliRepo) -> Self {
        Repo {
            url: repo.url,
            key: repo.key,
            name: repo.name,
            email: repo.email,
            message: repo.message,
        }
    }
}

impl Repo {
    pub fn save(&self, report: Report) -> Result<(), CliError> {
        // todo use tempdir
        let temp_dir = tempdir()?;
        let bencher_dir = temp_dir.path().join(BENCHER_DIR);

        let repo = self.clone(&bencher_dir)?;

        let report = serde_json::to_string(&report)?;

        let bencher_file = Path::new(BENCHER_FILE);
        fs::write(bencher_dir.join(&bencher_file), &report)?;

        let oid = Self::add(&repo, &bencher_file)?;
        let commit = self.commit(&repo, oid)?;
        println!("Commit added {commit}");

        self.push(&repo)?;

        Ok(())
    }

    fn clone(&self, into: &Path) -> Result<Repository, git2::Error> {
        // Prepare fetch options.
        let mut fo = git2::FetchOptions::new();
        fo.remote_callbacks(self.callbacks(false));

        // Prepare builder.
        let mut builder = git2::build::RepoBuilder::new();
        builder.fetch_options(fo);

        // Clone the project.
        builder.clone(&self.url, into)
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
        let parent_commit = last_commit(repo);
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

    fn push(&self, repo: &Repository) -> Result<(), git2::Error> {
        // Connect remote.
        let mut remote = repo.find_remote("origin")?;
        remote.connect_auth(Direction::Push, Some(self.callbacks(false)), None)?;

        // Prepare push options.
        let mut po = git2::PushOptions::new();

        // Prepare callbacks.
        po.remote_callbacks(self.callbacks(true));

        let url: [&str; 1] = ["refs/heads/master:refs/heads/master"];

        // Push remote.
        remote.push(&url, Some(&mut po))
    }

    fn callbacks(&self, update_reference: bool) -> RemoteCallbacks<'_> {
        let mut callbacks = if let Some(key) = self.key.as_deref() {
            let mut callbacks = RemoteCallbacks::new();
            callbacks.credentials(|_url, username_from_url, _allowed_types| {
                Cred::ssh_key(username_from_url.unwrap(), None, Path::new(key), None)
            });
            callbacks
        } else {
            RemoteCallbacks::new()
        };
        if update_reference {
            callbacks.push_update_reference(move |name, status| {
                if let Some(status) = status {
                    Err(git2::Error::from_str(&format!(
                        "Update reference failed: {status}"
                    )))
                } else {
                    println!("Push commit for refspec {name} succeeded");
                    Ok(())
                }
            });
        }
        callbacks
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

fn last_commit(repo: &Repository) -> Result<Commit, git2::Error> {
    let obj = repo.head()?.resolve()?.peel(ObjectType::Commit)?;
    obj.into_commit()
        .map_err(|_| git2::Error::from_str("Couldn't find last commit"))
}
