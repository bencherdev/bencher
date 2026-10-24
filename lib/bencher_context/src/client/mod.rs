use std::{collections::HashMap, path::Path};

use gix::Repository;

use crate::{ContextPath, RunContext};

mod fingerprint;
mod operating_system;

use fingerprint::Fingerprint;
use operating_system::OperatingSystem;

const ROOT: &str = "root";

impl RunContext {
    pub fn current() -> Self {
        let mut context = Self::default();
        if let Ok(current_dir) = std::env::current_dir() {
            git_context(&mut context, &current_dir);
        }
        testbed_context(&mut context);
        context
    }

    fn insert(&mut self, path: &str, value: String) -> Option<String> {
        let key = Self::key(path);
        self.0.insert(key, value)
    }
}

#[expect(
    clippy::implicit_hasher,
    reason = "concrete HashMap type matches internal storage"
)]
impl From<RunContext> for HashMap<String, String> {
    fn from(context: RunContext) -> Self {
        context.0
    }
}

fn git_context(context: &mut RunContext, directory: &Path) {
    let Some(repo) = find_repo(directory) else {
        return;
    };

    if let Some(repo_name) = repo_name(&repo) {
        context.insert(ContextPath::REPO_NAME, repo_name);
    }

    if let Some(root_commit) = repo_hash(&repo) {
        context.insert(ContextPath::REPO_HASH, root_commit);
    }

    if let Some((branch_ref, branch_ref_name)) = branch_ref(&repo) {
        context.insert(ContextPath::BRANCH_REF, branch_ref);
        context.insert(ContextPath::BRANCH_REF_NAME, branch_ref_name);
    }

    if let Some(hash) = branch_hash(&repo) {
        context.insert(ContextPath::BRANCH_HASH, hash);
    }
}

fn testbed_context(context: &mut RunContext) {
    if let Some(os) = OperatingSystem::current() {
        context.insert(ContextPath::TESTBED_OS, os.to_string());
    }

    if let Some(fingerprint) = Fingerprint::current() {
        context.insert(ContextPath::TESTBED_FINGERPRINT, fingerprint.to_string());
    }
}

fn find_repo(directory: &Path) -> Option<Repository> {
    for directory in directory.ancestors() {
        if let Ok(repo) = gix::open(directory) {
            return Some(repo);
        }
    }
    None
}

fn repo_name(repo: &Repository) -> Option<String> {
    let Some(parent) = repo.path().parent() else {
        return Some(ROOT.to_owned());
    };
    let file_name = parent.file_name()?;
    file_name.to_str().map(ToOwned::to_owned)
}

fn repo_hash(repo: &Repository) -> Option<String> {
    let head_id = repo.head_id().ok()?;
    let rev_walk = repo.rev_walk([head_id]).all().ok()?;
    if let Some(Ok(commit)) = rev_walk.last() {
        Some(commit.id().object().ok()?.id.to_string())
    } else {
        None
    }
}

fn branch_ref(repo: &Repository) -> Option<(String, String)> {
    if let Ok(Some(name)) = repo.head_name() {
        Some((
            String::from_utf8_lossy(name.as_bstr()).to_string(),
            String::from_utf8_lossy(name.shorten()).to_string(),
        ))
    } else {
        None
    }
}

fn branch_hash(repo: &Repository) -> Option<String> {
    let head_id = repo.head_id().ok()?;
    let head_object = head_id.object().ok()?;
    Some(head_object.id.to_string())
}

#[cfg(test)]
mod tests {
    use std::{path::Path, process::Command};

    use super::git_context;
    use crate::{ContextPath, RunContext};

    fn git(directory: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .args([
                "-c",
                "user.name=Bencher",
                "-c",
                "user.email=bencher@example.com",
                "-c",
                "commit.gpgsign=false",
                "-c",
                "core.hooksPath=/dev/null",
            ])
            .args(args)
            .current_dir(directory)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    }

    #[test]
    fn git_context_from_a_repository() {
        let temp_dir = tempfile::tempdir().unwrap();
        let repo = temp_dir.path().join("my-repo");
        let nested = repo.join("nested");
        std::fs::create_dir_all(&nested).unwrap();
        git(&repo, &["init", "--quiet", "--initial-branch", "trunk"]);
        git(
            &repo,
            &["commit", "--quiet", "--allow-empty", "--message", "root"],
        );
        let root = git(&repo, &["rev-parse", "HEAD"]);
        git(
            &repo,
            &["commit", "--quiet", "--allow-empty", "--message", "head"],
        );
        let head = git(&repo, &["rev-parse", "HEAD"]);

        let mut context = RunContext::default();
        git_context(&mut context, &nested);

        let value = |path| context.0.get(&RunContext::key(path)).map(String::as_str);
        assert_eq!(value(ContextPath::REPO_NAME), Some("my-repo"));
        assert_eq!(value(ContextPath::REPO_HASH), Some(root.as_str()));
        assert_eq!(value(ContextPath::BRANCH_REF), Some("refs/heads/trunk"));
        assert_eq!(value(ContextPath::BRANCH_REF_NAME), Some("trunk"));
        assert_eq!(value(ContextPath::BRANCH_HASH), Some(head.as_str()));
    }
}
