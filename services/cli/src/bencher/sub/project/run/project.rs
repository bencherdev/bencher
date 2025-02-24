use gix::Repository;

use crate::bencher::sub::project::run::branch::find_repo;

const ROOT: &str = "root";

pub fn local_project() {
    let repo = find_repo();
    let repo_name = repo_name(repo.as_ref());
    if let Some(repo_name) = repo_name {
        println!("{repo_name}");
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
