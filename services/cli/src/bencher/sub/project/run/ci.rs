use octocrab::{models::CommentId, Octocrab};

use crate::parser::project::run::CliRunCi;

use super::urls::ReportUrls;
use crate::{cli_eprintln, cli_println};

#[derive(Debug)]
pub enum Ci {
    GitHubActions(GitHubActions),
}

#[derive(thiserror::Error, Debug)]
pub enum CiError {
    #[error("GitHub Action repository is not valid: {0}")]
    GitHubRepository(String),
    #[error("GitHub Action repository not found for pull request")]
    NoGithubRepository,
    #[error("GitHub Action ref is not for a pull request: {0}")]
    GitHubRef(String),
    #[error("GitHub Action ref not found for pull request")]
    NoGitHubRef,
    #[error("Failed to authenticate as GitHub Action: {0}")]
    GitHubAuth(octocrab::Error),
    #[error("Failed to list GitHub PR comments: {0}")]
    GitHubComments(octocrab::Error),
    #[error("Failed to create GitHub PR comment: {0}")]
    GitHubCreateComment(octocrab::Error),
    #[error("Failed to update GitHub PR comment: {0}")]
    GitHubUpdateComment(octocrab::Error),
}

impl TryFrom<CliRunCi> for Option<Ci> {
    type Error = CiError;

    fn try_from(ci: CliRunCi) -> Result<Self, Self::Error> {
        let CliRunCi {
            ci_only_on_alert,
            github_actions,
        } = ci;
        Ok(github_actions
            .map(|github_actions| Ci::GitHubActions((ci_only_on_alert, github_actions).into())))
    }
}

impl Ci {
    pub async fn run(&self, report_urls: &ReportUrls) -> Result<(), CiError> {
        match self {
            Self::GitHubActions(github_actions) => github_actions.run(report_urls).await,
        }
    }
}

#[derive(Debug)]
pub struct GitHubActions {
    ci_only_on_alert: bool,
    token: String,
}

impl From<(bool, String)> for GitHubActions {
    fn from((ci_only_on_alert, token): (bool, String)) -> Self {
        Self {
            ci_only_on_alert,
            token,
        }
    }
}

impl GitHubActions {
    pub async fn run(&self, report_urls: &ReportUrls) -> Result<(), CiError> {
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
                    return Err(CiError::GitHubRepository(repository));
                }
            },
            _ => {
                cli_eprintln!("GitHub Action running on a pull request but failed to get repository. Skipping CI integration.");
                return Err(CiError::NoGithubRepository);
            },
        };

        // For workflows triggered by `pull_request`, this is the pull request merge branch.
        // for pull requests it is `refs/pull/<pr_number>/merge`
        let issue_number = match std::env::var("GITHUB_REF").ok() {
            Some(github_ref) => {
                if let Some(issue_number) = github_ref
                    .strip_prefix("refs/pull/")
                    .and_then(|r| r.strip_suffix("/merge"))
                    .and_then(|r| r.parse::<u64>().ok())
                {
                    issue_number
                } else {
                    cli_eprintln!("GitHub Action running on a pull request but ref is not a pull request ref ({github_ref}). Skipping CI integration.");
                    return Err(CiError::GitHubRef(github_ref));
                }
            },
            None => {
                cli_eprintln!("GitHub Action running on a pull request but failed to get ref. Skipping CI integration.");
                return Err(CiError::NoGitHubRef);
            },
        };

        let github_client = Octocrab::builder()
            .user_access_token(self.token.clone())
            .build()
            .map_err(CiError::GitHubAuth)?;

        // Get the comment ID if it exists
        let comment_id = get_comment(
            &github_client,
            &owner,
            &repo,
            issue_number,
            &report_urls.bencher_div(),
        )
        .await?;

        // Update or create the comment
        let issue_handler = github_client.issues(owner, repo);
        let body = report_urls.html();
        let _comment = if let Some(comment_id) = comment_id {
            issue_handler
                .update_comment(comment_id, body)
                .await
                .map_err(CiError::GitHubUpdateComment)?
        } else {
            if self.ci_only_on_alert && !report_urls.has_alerts() {
                cli_println!("No alerts found. Skipping CI integration.");
                return Ok(());
            }
            issue_handler
                .create_comment(issue_number, body)
                .await
                .map_err(CiError::GitHubCreateComment)?
        };

        Ok(())
    }
}

pub async fn get_comment(
    github_client: &Octocrab,
    owner: &str,
    repo: &str,
    issue_number: u64,
    bencher_div: &str,
) -> Result<Option<CommentId>, CiError> {
    const PER_PAGE: u8 = 100;

    let mut page: u32 = 1;
    loop {
        let comments = github_client
            .issues(owner, repo)
            .list_comments(issue_number)
            .per_page(PER_PAGE)
            .page(page)
            .send()
            .await
            .map_err(CiError::GitHubComments)?;

        let comments_len = comments.items.len();
        if comments_len == 0 {
            return Ok(None);
        }

        for comment in comments.items {
            cli_println!("Found comment: {:#?}", comment);
            if let Some(body_html) = comment.body_html {
                if body_html.starts_with(bencher_div) {
                    return Ok(Some(comment.id));
                }
            }
        }

        if comments_len < usize::from(PER_PAGE) {
            return Ok(None);
        } else {
            page += 1;
        }
    }
}
