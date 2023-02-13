use std::{convert::TryFrom, str::FromStr};

use bencher_json::{
    project::branch::{JsonStartPoint, BRANCH_MAIN_STR},
    BranchName, JsonBranch, JsonNewBranch, ResourceId,
};

use uuid::Uuid;

use crate::{
    bencher::{backend::Backend, locality::Locality},
    cli::project::run::CliRunBranch,
    cli_println, CliError,
};

use super::BENCHER_BRANCH;

#[derive(Debug, Clone)]
pub enum Branch {
    ResourceId(ResourceId),
    Name {
        name: BranchName,
        start_points: Vec<String>,
        create: bool,
    },
    None,
}

impl TryFrom<CliRunBranch> for Branch {
    type Error = CliError;

    fn try_from(run_branch: CliRunBranch) -> Result<Self, Self::Error> {
        let CliRunBranch {
            branch,
            if_branch,
            else_if_branch,
            else_branch,
            endif_branch: _,
        } = run_branch;
        Ok(if let Some(branch) = branch {
            Self::ResourceId(branch)
        } else if let Ok(env_branch) = std::env::var(BENCHER_BRANCH) {
            env_branch.as_str().parse().map(Self::ResourceId)?
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
            BRANCH_MAIN_STR.parse().map(Self::ResourceId)?
        })
    }
}

impl Branch {
    pub async fn resource_id(
        &self,
        project: &ResourceId,
        locality: &Locality,
    ) -> Result<Option<ResourceId>, CliError> {
        Ok(match self {
            Self::ResourceId(resource_id) => Some(resource_id.clone()),
            Self::Name {
                name,
                start_points,
                create,
            } => {
                if let Some(uuid) =
                    if_branch(project, name, start_points, *create, locality).await?
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
    locality: &Locality,
) -> Result<Option<Uuid>, CliError> {
    let Locality::Backend(backend) = &locality else {
        return  Ok(None);
    };

    let branch = get_branch(project, branch_name, backend).await?;

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
            continue
        };

        let new_branch =
            if let Some(start_point) = get_branch(project, &start_point, backend).await? {
                create_branch(project, branch_name, Some(start_point.into()), backend).await?
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

    if create {
        let new_branch = create_branch(project, branch_name, None, backend).await?;

        if new_branch.is_some() {
            return Ok(new_branch);
        }

        cli_println!(
            "Failed to create new branch with name \"{branch_name}\" in project \"{project}\"."
        );
    }

    Ok(None)
}

async fn get_branch(
    project: &ResourceId,
    branch_name: &BranchName,
    backend: &Backend,
) -> Result<Option<Uuid>, CliError> {
    let value = backend
        .get(&format!(
            "/v0/projects/{project}/branches?name={branch_name}"
        ))
        .await?;
    let mut json_branches: Vec<JsonBranch> = serde_json::from_value(value)?;
    let branch_count = json_branches.len();
    if let Some(branch) = json_branches.pop() {
        if branch_count == 1 {
            Ok(Some(branch.uuid))
        } else {
            Err(CliError::BranchName(
                project.to_string(),
                branch_name.as_ref().into(),
                branch_count,
            ))
        }
    } else {
        Ok(None)
    }
}

async fn create_branch(
    project: &ResourceId,
    branch_name: &BranchName,
    start_point: Option<ResourceId>,
    backend: &Backend,
) -> Result<Option<Uuid>, CliError> {
    // Default to cloning the thresholds from the start point branch
    let start_point = start_point.map(|branch| JsonStartPoint {
        branch,
        thresholds: Some(true),
    });
    let new_branch = JsonNewBranch {
        name: branch_name.clone(),
        start_point,
        slug: None,
    };

    let value = backend
        .post(&format!("/v0/projects/{project}/branches"), &new_branch)
        .await?;
    let json_branch: JsonBranch = serde_json::from_value(value)?;

    Ok(Some(json_branch.uuid))
}
