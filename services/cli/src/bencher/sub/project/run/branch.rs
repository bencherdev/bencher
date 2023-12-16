use std::{convert::TryFrom, str::FromStr};

use bencher_client::types::{JsonNewBranch, JsonStartPoint};
use bencher_json::{
    project::branch::BRANCH_MAIN_STR, BranchName, BranchUuid, JsonUuid, JsonUuids, NameId,
    NameIdKind, ResourceId,
};

use crate::{bencher::backend::Backend, cli_println, parser::project::run::CliRunBranch};

use super::BENCHER_BRANCH;

#[derive(Debug, Clone)]
pub enum Branch {
    NameId(NameId),
    Name {
        name: BranchName,
        start_points: Vec<String>,
        create: bool,
    },
    None,
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
    #[error("Failed to get branch: {0}\nDoes it exist? Branches need to already exist when using `--branch` or `BENCHER_BRANCH`.\nSee: https://bencher.dev/docs/explanation/branch-selection/")]
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
            if_branch,
            else_if_branch,
            else_branch,
            endif_branch: _,
        } = run_branch;
        Ok(if let Some(branch) = branch {
            Self::NameId(branch)
        } else if let Ok(env_branch) = std::env::var(BENCHER_BRANCH) {
            env_branch
                .as_str()
                .parse()
                .map(Self::NameId)
                .map_err(BranchError::ParseBranch)?
        } else if let Some(branch_name) = if_branch {
            if let Some(name) = branch_name {
                Self::Name {
                    name,
                    start_points: else_if_branch,
                    create: else_branch,
                }
            } else {
                Self::None
            }
        } else {
            BRANCH_MAIN_STR
                .parse()
                .map(Self::NameId)
                .map_err(BranchError::ParseBranch)?
        })
    }
}

impl Branch {
    pub async fn get(
        &self,
        project: &ResourceId,
        dry_run: bool,
        backend: &Backend,
    ) -> Result<Option<NameId>, BranchError> {
        Ok(match self {
            Self::NameId(name_id) => {
                // Check to make sure that the branch exists before running the benchmarks
                match name_id.try_into().map_err(BranchError::ParseBranch)? {
                    NameIdKind::Uuid(uuid) => {
                        if !dry_run {
                            get_branch(project, &uuid.into(), backend).await?;
                        }
                    },
                    NameIdKind::Slug(slug) => {
                        if !dry_run {
                            get_branch(project, &slug.into(), backend).await?;
                        }
                    },
                    NameIdKind::Name(name) => {
                        let branch_name =
                            name.as_ref().parse().map_err(BranchError::ParseBranch)?;
                        if !dry_run {
                            get_branch_query(project, &branch_name, backend)
                                .await?
                                .ok_or_else(|| BranchError::NoBranches {
                                    project: project.to_string(),
                                    branch_name: branch_name.as_ref().into(),
                                })?;
                        }
                    },
                }
                Some(name_id.clone())
            },
            Self::Name {
                name,
                start_points,
                create,
            } => {
                if let Some(uuid) =
                    if_branch(project, name, start_points, *create, dry_run, backend).await?
                {
                    Some(uuid.into())
                } else {
                    cli_println!(
                        "Failed to find or create branch \"{name}\". Skipping benchmark run."
                    );
                    None
                }
            },
            Self::None => {
                cli_println!("Failed to get branch name. Skipping benchmark run.");
                None
            },
        })
    }
}

async fn if_branch(
    project: &ResourceId,
    branch_name: &BranchName,
    start_points: &[String],
    create: bool,
    dry_run: bool,
    backend: &Backend,
) -> Result<Option<BranchUuid>, BranchError> {
    let branch = get_branch_query(project, branch_name, backend).await?;

    if branch.is_some() {
        return Ok(branch);
    }

    cli_println!("Failed to find branch with name \"{branch_name}\" in project \"{project}\".");

    for (index, start_point) in start_points.iter().enumerate() {
        let count = index.checked_add(1).unwrap_or_default();
        let Ok(start_point) = BranchName::from_str(start_point) else {
            cli_println!(
                "Failed to parse start point branch #{count} \"{start_point}\" for \"{branch_name}\" in project \"{project}\"."
            );
            continue;
        };

        let new_branch =
            if let Some(start_point) = get_branch_query(project, &start_point, backend).await? {
                Some(create_branch(project, branch_name, Some(start_point.into()), backend).await?)
            } else {
                None
            };

        if new_branch.is_some() {
            return Ok(new_branch);
        }

        cli_println!(
            "Failed to find start point branch #{count} \"{start_point}\" for \"{branch_name}\" in project \"{project}\"."
        );
    }

    #[allow(clippy::if_then_some_else_none)]
    Ok(if create {
        // If we're just doing a dry run, we don't need to actually create the branch
        Some(if dry_run {
            BranchUuid::new()
        } else {
            create_branch(project, branch_name, None, backend).await?
        })
    } else {
        None
    })
}

async fn get_branch(
    project: &ResourceId,
    branch: &ResourceId,
    backend: &Backend,
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

async fn get_branch_query(
    project: &ResourceId,
    branch_name: &BranchName,
    backend: &Backend,
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
    start_point: Option<NameId>,
    backend: &Backend,
) -> Result<BranchUuid, BranchError> {
    // Default to cloning the thresholds from the start point branch
    let start_point = start_point.map(|branch| JsonStartPoint {
        branch: branch.into(),
        thresholds: Some(true),
    });
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
