use bencher_json::{project::report::JsonReportContext, Fingerprint, GitHash};
use gix::Repository;

use super::branch::find_repo;

const ROOT: &str = "root";

pub fn get_context() -> Option<JsonReportContext> {
    let mut context = JsonReportContext::new();

    let repo = find_repo();
    let repo_name = repo_name(repo.as_ref());
    if let Some(repo_name) = repo_name {
        context.insert("bencher.dev/repo/name".to_owned(), Some(repo_name));
    }
    if let Some(repo) = &repo {
        if let Some(root_commit) = find_default_branch_and_root_commit(&repo) {
            context.insert(
                "bencher.dev/repo/hash".to_owned(),
                Some(root_commit.to_string()),
            );
        }
    }

    if let Some(repo) = repo {
        if let Some((branch_ref, branch_ref_name)) = current_branch_name(&repo) {
            context.insert("bencher.dev/branch/ref".to_owned(), Some(branch_ref));
            context.insert(
                "bencher.dev/branch/ref/name".to_owned(),
                Some(branch_ref_name),
            );
        }

        if let Some(hash) = current_branch_hash(&repo) {
            context.insert("bencher.dev/branch/hash".to_owned(), Some(hash));
        }
    }

    let fingerprint = Fingerprint::new();
    if let Some(fingerprint) = fingerprint {
        context.insert(
            "bencher.dev/testbed/fingerprint".to_owned(),
            Some(fingerprint.to_string()),
        );
    }

    Some(context)
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

fn current_branch_hash(repo: &Repository) -> Option<String> {
    let head_id = repo.head_id().ok()?;
    let head_object = head_id.object().ok()?;
    Some(head_object.id.to_string())
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
