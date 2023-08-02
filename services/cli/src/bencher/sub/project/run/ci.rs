use bencher_json::JsonReport;

use crate::parser::project::run::CliRunCi;

use super::{
    urls::{AlertUrls, BenchmarkUrls},
    RunError,
};
use crate::{cli_eprintln, cli_println};

#[derive(Debug)]
pub enum Ci {
    GitHubActions(GitHubActions),
}

impl TryFrom<CliRunCi> for Option<Ci> {
    type Error = RunError;

    fn try_from(ci: CliRunCi) -> Result<Self, Self::Error> {
        let CliRunCi { github_actions } = ci;
        Ok(github_actions.map(|github_actions| Ci::GitHubActions(github_actions.into())))
    }
}

impl Ci {
    pub async fn run(
        &self,
        json_report: &JsonReport,
        benchmark_urls: &BenchmarkUrls,
        alert_urls: &AlertUrls,
    ) -> Result<(), RunError> {
        match self {
            Self::GitHubActions(github_actions) => {
                github_actions
                    .run(json_report, benchmark_urls, alert_urls)
                    .await
            },
        }
    }
}

#[derive(Debug)]
pub struct GitHubActions {
    token: String,
}

impl From<String> for GitHubActions {
    fn from(token: String) -> Self {
        Self { token }
    }
}

impl GitHubActions {
    pub async fn run(
        &self,
        _json_report: &JsonReport,
        _benchmark_urls: &BenchmarkUrls,
        _alert_urls: &AlertUrls,
    ) -> Result<(), RunError> {
        // https://docs.github.com/en/actions/learn-github-actions/variables#default-environment-variables

        // Always set to `true` when GitHub Actions is running the workflow. You can use this variable to differentiate when tests are being run locally or by GitHub Actions.
        match std::env::var("GITHUB_ACTIONS").ok() {
            Some(github_actions) if github_actions == "true" => {},
            _ => {
                cli_println!("Not running as a GitHub Action. Skipping CI integration.");
                return Ok(());
            },
        }

        // The name of the event that triggered the workflow. For example, `workflow_dispatch`.
        match std::env::var("GITHUB_EVENT_NAME").ok() {
            Some(event_name) if event_name == "pull_request" => {},
            _ => {
                cli_println!(
                    "Not running as a GitHub Action pull request. Skipping CI integration."
                );
                return Ok(());
            },
        }

        // The owner and repository name. For example, octocat/Hello-World.
        let (owner, repo) = match std::env::var("GITHUB_REPOSITORY").ok() {
            Some(repository) => {
                if let Some((owner, repo)) = repository.split_once('/') {
                    (owner.to_owned(), repo.to_owned())
                } else {
                    cli_eprintln!("GitHub Action running on a pull request but repository is not in the form `owner/repo` ({repository}). Skipping CI integration.");
                    return Err(RunError::GitHubActionRepository(repository));
                }
            },
            _ => {
                cli_eprintln!("GitHub Action running on a pull request but failed to get repository. Skipping CI integration.");
                return Err(RunError::NoGithubRepository);
            },
        };

        // For workflows triggered by `pull_request`, this is the pull request merge branch.
        // for pull requests it is `refs/pull/<pr_number>/merge`
        let number = match std::env::var("GITHUB_REF").ok() {
            Some(github_ref) => {
                if let Some(number) = github_ref
                    .strip_prefix("refs/pull/")
                    .and_then(|r| r.strip_suffix("/merge"))
                    .and_then(|r| r.parse::<u64>().ok())
                {
                    number
                } else {
                    cli_eprintln!("GitHub Action running on a pull request but ref is not a pull request ref ({github_ref}). Skipping CI integration.");
                    return Err(RunError::GitHubActionRef(github_ref));
                }
            },
            None => {
                cli_eprintln!("GitHub Action running on a pull request but failed to get ref. Skipping CI integration.");
                return Err(RunError::NoGitHubActionRef);
            },
        };

        let _comment = octocrab::Octocrab::builder()
            .user_access_token(self.token.clone())
            .build()
            .map_err(RunError::GitHubActionAuth)?
            .issues(owner, repo)
            .create_comment(number, "Beep Boop")
            .await
            .map_err(RunError::GitHubActionComment)?;

        Ok(())
    }
}
