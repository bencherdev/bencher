use bencher_json::{Fingerprint, GitHash};
use gix::Repository;

use super::branch::find_repo;

const ROOT: &str = "root";

pub fn context() {
    let repo = find_repo();
    let repo_name = repo_name(repo.as_ref());
    if let Some(repo_name) = repo_name {
        println!("{repo_name}");
    }

    if let Some(repo) = repo {
        if let Some(branch) = current_branch_name(&repo) {
            println!("{branch:?}");
        }

        if let Some(root_commit) = find_default_branch_and_root_commit(&repo) {
            println!("Root commit hash: {root_commit:?}");
        }
    }

    let fingerprint = Fingerprint::new();
    if let Some(fingerprint) = fingerprint {
        println!("{fingerprint}");
    }
}

fn repo_name(repo: Option<&Repository>) -> Option<String> {
    let repo = repo?;
    let Some(parent) = repo.path().parent() else {
        return Some(ROOT.to_owned());
    };
    let file_name = parent.file_name()?;
    file_name.to_str().map(ToOwned::to_owned)
}

fn current_branch_name(repo: &Repository) -> Option<(String, String)> {
    repo.head().ok()?.referent_name().map(|name| {
        (
            String::from_utf8_lossy(name.as_bstr()).to_string(),
            String::from_utf8_lossy(name.shorten()).to_string(),
        )
    })
}

fn find_default_branch_and_root_commit(repo: &Repository) -> Option<GitHash> {
    let head_id = repo.head_id().ok()?;
    let rev_walk = repo.rev_walk([head_id]).all().ok()?;
    if let Some(Ok(commit)) = rev_walk.last() {
        Some(commit.id().object().ok()?.id.into())
    } else {
        None
    }
}
