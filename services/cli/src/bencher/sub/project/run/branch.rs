use bencher_client::types::{JsonNewBranch, JsonNewStartPoint};
use bencher_json::{
    project::branch::{JsonVersion, BRANCH_MAIN_STR},
    BranchName, BranchUuid, GitHash, JsonBranch, JsonBranches, JsonStartPoint, JsonUuid, JsonUuids,
    NameId, NameIdKind, ResourceId,
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
    ) -> Result<NameId, BranchError> {
        if !dry_run {
            self.exists_or_create(project, log, backend).await?;
        }
        Ok(self.branch.clone())
    }

    async fn exists_or_create(
        &self,
        project: &ResourceId,
        log: bool,
        backend: &AuthBackend,
    ) -> Result<(), BranchError> {
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
                let branch = get_branch(project, &uuid.into(), backend).await?;
                self.check_start_point(project, branch, log, backend)
                    .await?;
            },
            NameIdKind::Slug(slug) => {
                match get_branch(project, &slug.clone().into(), backend).await {
                    Ok(branch) => {
                        self.check_start_point(project, branch, log, backend)
                            .await?;
                    },
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
            },
            NameIdKind::Name(name) => match get_branch_by_name(project, &name, backend).await {
                Ok(Some(branch)) => {
                    self.check_start_point(project, branch, log, backend)
                        .await?;
                },
                Ok(None) => {
                    create_branch(project, &name, self.start_point.clone(), log, backend).await?;
                },
                Err(e) => return Err(e),
            },
        }
        Ok(())
    }

    async fn check_start_point(
        &self,
        project: &ResourceId,
        json_branch: JsonBranch,
        log: bool,
        backend: &AuthBackend,
    ) -> Result<(), BranchError> {
        match (&self.start_point, &json_branch.start_point) {
            (
                Some(StartPoint { branch, hash }),
                Some(JsonStartPoint {
                    branch: json_branch,
                    version:
                        JsonVersion {
                            hash: json_hash, ..
                        },
                }),
            ) => {
                todo!()
            },
            // If a start point is specified and the branch does not have a start point, then the branch needs to be recreated from that start point.
            // Because adding a start point is a one way operation with `bencher run`, this operation will only ever be performed once.
            // Therefore, using a set naming convention for the old branch name and slug is okay: `old_branch@detached`
            (Some(start_point), None) => {
                // Rename old branch to old_branch@detached and slug as well
                rename_branch(project, &json_branch, "detached", log, backend).await?;
                // Create new branch with the same name and slug as the old branch

                todo!()
            },
            // If a start point is not specified, then there is nothing to check.
            // Even if the branch has a start point, it does not need to always be specified.
            // That is, adding a start point is a one way operation with `bencher run`.
            (None, _) => {},
        }

        Ok(())
    }
}

async fn get_branch(
    project: &ResourceId,
    branch: &ResourceId,
    backend: &AuthBackend,
) -> Result<JsonBranch, BranchError> {
    backend
        .send_with(|client| async move {
            client
                .proj_branch_get()
                .project(project.clone())
                .branch(branch.clone())
                .send()
                .await
        })
        .await
        .map_err(BranchError::GetBranch)
}

async fn get_branch_by_name(
    project: &ResourceId,
    branch_name: &BranchName,
    backend: &AuthBackend,
) -> Result<Option<JsonBranch>, BranchError> {
    let json_branches: JsonBranches = backend
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
            Ok(Some(branch))
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
) -> Result<JsonBranch, BranchError> {
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
        let start_point = JsonNewStartPoint {
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

    backend
        .send_with(|client| async move {
            client
                .proj_branch_post()
                .project(project.clone())
                .body(new_branch.clone())
                .send()
                .await
        })
        .await
        .map_err(BranchError::CreateBranch)
}

async fn rename_branch(
    project: &ResourceId,
    json_branch: &JsonBranch,
    suffix: &str,
    log: bool,
    backend: &AuthBackend,
) -> Result<JsonBranch, BranchError> {
    let branch_name = format!(
        "{branch_name}@{suffix}",
        branch_name = json_branch.name.as_ref()
    );
    cli_println_quietable!(
        log,
        "New start point for branch with name \"{branch_name}\" in project \"{project}\".",
        branch_name = json_branch.name.as_ref(),
    );
    cli_println_quietable!(
        log,
        "Renaming detached branch to have name \"{branch_name}\" in project \"{project}\"."
    );

    // TODO this needs to be a branch update not create
    // let new_branch = &JsonNewBranch {
    //     name: branch_name.into(),
    //     slug: None,
    //     soft: Some(true),
    //     start_point: json_branch
    //         .start_point
    //         .clone()
    //         .map(|start_point| JsonNewStartPoint {
    //             branch: start_point.branch.clone(),
    //             hash: start_point.hash.clone(),
    //             thresholds: Some(true),
    //         }),
    // };

    // backend
    //     .send_with(|client| async move {
    //         client
    //             .proj_branch_post()
    //             .project(project.clone())
    //             .body(new_branch.clone())
    //             .send()
    //             .await
    //     })
    //     .await
    //     .map_err(BranchError::CreateBranch)
    todo!()
}
