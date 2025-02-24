use bencher_fingerprint::Fingerprint;
use gix::Repository;

use crate::bencher::sub::project::run::branch::find_repo;

const ROOT: &str = "root";

pub fn local_project() {
    let repo = find_repo();
    let repo_name = repo_name(repo.as_ref());
    if let Some(repo_name) = repo_name {
        println!("{repo_name}");
    }

    if let Some(repo) = repo {
        if let Some(root_commit) = find_default_branch_and_root_commit(&repo) {
            println!("Root commit hash: {root_commit}");
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

fn find_default_branch_and_root_commit(repo: &Repository) -> Option<String> {
    let head_id = repo.head_id().ok()?;

    let mut _rev_walk = repo.rev_walk([head_id]);

    // let root_commit = rev_walk.all().into_iter().last()?.id().to_string();

    None
}
