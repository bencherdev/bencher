use bencher_client::types::{JsonNewBranch, JsonStartPoint};
use bencher_json::{
    project::branch::BRANCH_MAIN_STR, BranchName, BranchUuid, GitHash, JsonUuid, JsonUuids, NameId,
    NameIdKind, ResourceId,
};

use crate::{
    bencher::backend::AuthBackend, cli_println_quietable, parser::project::run::CliRunBranch,
};

use super::BENCHER_BRANCH;

#[derive(Debug, Clone)]
pub struct Branch {
    branch: NameId,
    start_point: Option<StartPoint>,
}

#[derive(Debug, Clone)]
pub struct StartPoint {
    branch: NameId,
    hash: Option<GitHash>,
}

#[derive(thiserror::Error, Debug)]
pub enum BranchError {
    #[error("Failed to parse UUID, slug, or name for the branch: {0}")]
    ParseBranch(bencher_json::ValidError),
    #[error(
        "No branches were found with name \"{branch_name}\" in project \"{project}\". Exactly one was expected.\nDoes it exist? Branches need to already exist when using `--branch` or `BENCHER_BRANCH`.\nSee: https://bencher.dev/docs/explanation/branch-selection/"
    )]
    NoBranches {
        project: String,
        branch_name: String,
    },
    #[error(
        "{count} branches were found with name \"{branch_name}\" in project \"{project}\"! Exactly one was expected.\nThis is likely a bug. Please report it here: https://github.com/bencherdev/bencher/issues"
    )]
    MultipleBranches {
        project: String,
        branch_name: String,
        count: usize,
    },
    #[error("Failed to get branch: {0}\nDoes it exist? Branches must already exist when using `--branch` or `BENCHER_BRANCH` with a UUID.\nSee: https://bencher.dev/docs/explanation/branch-selection/")]
    GetBranch(crate::bencher::BackendError),
    #[error("Failed to query branches: {0}")]
    GetBranches(crate::bencher::BackendError),
    #[error("Failed to create new branch: {0}")]
    CreateBranch(crate::bencher::BackendError),
}

impl TryFrom<CliRunBranch> for Branch {
    type Error = BranchError;

    fn try_from(run_branch: CliRunBranch) -> Result<Self, Self::Error> {
        let CliRunBranch {
            branch,
            branch_start_point,
            branch_start_point_hash,
            endif_branch: _,
        } = run_branch;
        let branch = if let Some(branch) = branch {
            branch
        } else if let Ok(env_branch) = std::env::var(BENCHER_BRANCH) {
            env_branch
                .as_str()
                .parse()
                .map_err(BranchError::ParseBranch)?
        } else {
            BRANCH_MAIN_STR.parse().map_err(BranchError::ParseBranch)?
        };
        let start_point = branch_start_point
            .first()
            .cloned()
            .map(|branch| StartPoint {
                branch,
                hash: branch_start_point_hash,
            });
        Ok(Self {
            branch,
            start_point,
        })
    }
}

impl Branch {
    pub async fn get(
        &self,
        project: &ResourceId,
        dry_run: bool,
        log: bool,
        backend: &AuthBackend,
    ) -> Result<Option<NameId>, BranchError> {
        // Check to make sure that the branch exists before running the benchmarks
        // TODO If a start point is provided, check to see if the response from `get_branch` matches that start point
        // both the branch and the hash (if provided) have to match!
        // Otherwise, the old branch should be archived and a new branch should be created.
        // The archived branch needs to have its name and slug updated in order to make way for the newly recreated branch.
        // If no hash is provided for the existing branch: my_branch@version/42 where the current branch is on version 42.
        // If a hash is provided for the existing branch: my_branch@hash/1234567890abcdef where the current branch start point has hash 1234567890abcdef.
        match (&self.branch)
            .try_into()
            .map_err(BranchError::ParseBranch)?
        {
            NameIdKind::Uuid(uuid) => {
                if !dry_run {
                    get_branch(project, &uuid.into(), backend).await?;
                }
            },
            NameIdKind::Slug(slug) => {
                if !dry_run {
                    match get_branch(project, &slug.clone().into(), backend).await {
                        Ok(_) => {},
                        Err(BranchError::GetBranch(_)) => {
                            create_branch(
                                project,
                                &slug.into(),
                                self.start_point.clone(),
                                log,
                                backend,
                            )
                            .await?;
                        },
                        Err(e) => return Err(e),
                    }
                }
            },
            NameIdKind::Name(name) => {
                let branch_name: BranchName = name;
                if !dry_run {
                    match get_branch_by_name(project, &branch_name, backend).await {
                        Ok(_) => {},
                        Err(BranchError::NoBranches { .. }) => {
                            create_branch(
                                project,
                                &branch_name,
                                self.start_point.clone(),
                                log,
                                backend,
                            )
                            .await?;
                        },
                        Err(e) => return Err(e),
                    }
                }
            },
        }
        Ok(Some(self.branch.clone()))
    }
}

async fn get_branch(
    project: &ResourceId,
    branch: &ResourceId,
    backend: &AuthBackend,
) -> Result<BranchUuid, BranchError> {
    // Use `JsonUuid` to future proof against breaking changes
    let json_branch: JsonUuid = backend
        .send_with(|client| async move {
            client
                .proj_branch_get()
                .project(project.clone())
                .branch(branch.clone())
                .send()
                .await
        })
        .await
        .map_err(BranchError::GetBranch)?;

    Ok(json_branch.uuid.into())
}

async fn get_branch_by_name(
    project: &ResourceId,
    branch_name: &BranchName,
    backend: &AuthBackend,
) -> Result<Option<BranchUuid>, BranchError> {
    // Use `JsonUuids` to future proof against breaking changes
    let json_branches: JsonUuids = backend
        .send_with(|client| async move {
            client
                .proj_branches_get()
                .project(project.clone())
                .name(branch_name.clone())
                .send()
                .await
        })
        .await
        .map_err(BranchError::GetBranches)?;

    let mut json_branches = json_branches.into_inner();
    let branch_count = json_branches.len();
    if let Some(branch) = json_branches.pop() {
        if branch_count == 1 {
            Ok(Some(branch.uuid.into()))
        } else {
            Err(BranchError::MultipleBranches {
                project: project.to_string(),
                branch_name: branch_name.as_ref().into(),
                count: branch_count,
            })
        }
    } else {
        Ok(None)
    }
}

async fn create_branch(
    project: &ResourceId,
    branch_name: &BranchName,
    start_point: Option<StartPoint>,
    log: bool,
    backend: &AuthBackend,
) -> Result<BranchUuid, BranchError> {
    cli_println_quietable!(
        log,
        "Failed to find branch with name \"{branch_name}\" in project \"{project}\"."
    );
    let (start_point, message) = if let Some(start_point) = start_point {
        let StartPoint { branch, hash } = start_point;
        let message = format!(
            " with start point branch {branch}{}",
            hash.as_ref()
                .map(|hash| format!(" and hash {hash}"))
                .unwrap_or_default(),
        );
        // Default to cloning the thresholds from the start point branch
        let start_point = JsonStartPoint {
            branch: branch.clone().into(),
            hash: hash.clone().map(Into::into),
            thresholds: Some(true),
        };
        (Some(start_point), Some(message))
    } else {
        (None, None)
    };
    cli_println_quietable!(
        log,
        "Creating a new branch with name \"{branch_name}\" in project \"{project}\"{message}.",
        message = message.unwrap_or_default()
    );
    let new_branch = &JsonNewBranch {
        name: branch_name.clone().into(),
        slug: None,
        soft: Some(true),
        start_point,
    };

    // Use `JsonUuid` to future proof against breaking changes
    let json_branch: JsonUuid = backend
        .send_with(|client| async move {
            client
                .proj_branch_post()
                .project(project.clone())
                .body(new_branch.clone())
                .send()
                .await
        })
        .await
        .map_err(BranchError::CreateBranch)?;

    Ok(json_branch.uuid.into())
}
