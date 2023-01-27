use std::{convert::TryFrom, str::FromStr};

use async_trait::async_trait;
use bencher_json::{
    project::{
        branch::BRANCH_MAIN_STR, report::JsonReportSettings, testbed::TESTBED_LOCALHOST_STR,
    },
    BranchName, GitHash, JsonBranch, JsonNewBranch, JsonNewReport, JsonReport, ResourceId,
};
use chrono::Utc;
use clap::ValueEnum;
use uuid::Uuid;

use crate::{
    bencher::{backend::Backend, locality::Locality},
    cli::project::run::{CliRun, CliRunAdapter},
    cli_eprintln, cli_println, CliError,
};

mod adapter;
mod fold;
pub mod runner;

use adapter::RunAdapter;
use fold::Fold;
use runner::Runner;

use crate::bencher::SubCmd;

const BENCHER_PROJECT: &str = "BENCHER_PROJECT";
const BENCHER_BRANCH: &str = "BENCHER_BRANCH";
const BENCHER_TESTBED: &str = "BENCHER_TESTBED";
const BENCHER_ADAPTER: &str = "BENCHER_ADAPTER";
const BENCHER_CMD: &str = "BENCHER_CMD";

#[derive(Debug)]
pub struct Run {
    project: ResourceId,
    locality: Locality,
    runner: Runner,
    branch: Branch,
    hash: Option<GitHash>,
    testbed: ResourceId,
    adapter: Option<RunAdapter>,
    iter: usize,
    fold: Option<Fold>,
    allow_failure: bool,
    err: bool,
}

#[derive(Debug, Clone)]
enum Branch {
    ResourceId(ResourceId),
    Name {
        name: BranchName,
        start_points: Vec<String>,
        create: bool,
    },
    None,
}

impl TryFrom<CliRun> for Run {
    type Error = CliError;

    fn try_from(run: CliRun) -> Result<Self, Self::Error> {
        let CliRun {
            project,
            locality,
            command,
            branch,
            if_branch,
            else_if_branch,
            else_branch,
            endif_branch,
            hash,
            testbed,
            adapter,
            iter,
            fold,
            allow_failure,
            err,
        } = run;
        Ok(Self {
            project: unwrap_project(project)?,
            locality: locality.try_into()?,
            runner: command.try_into()?,
            branch: map_branch(branch, if_branch, else_if_branch, else_branch, endif_branch)?,
            hash: map_hash(hash)?,
            testbed: unwrap_testbed(testbed)?,
            adapter: map_adapter(adapter),
            iter: iter.unwrap_or(1),
            fold: fold.map(Into::into),
            allow_failure,
            err,
        })
    }
}

fn unwrap_project(project: Option<ResourceId>) -> Result<ResourceId, CliError> {
    Ok(if let Some(project) = project {
        project
    } else if let Ok(env_project) = std::env::var(BENCHER_PROJECT) {
        env_project.parse()?
    } else {
        return Err(CliError::ProjectNotFound);
    })
}

#[allow(clippy::option_option)]
fn map_branch(
    branch: Option<ResourceId>,
    if_branch: Option<Option<BranchName>>,
    else_if_branch: Vec<String>,
    else_branch: bool,
    _endif_branch: bool,
) -> Result<Branch, CliError> {
    Ok(if let Some(branch) = branch {
        Branch::ResourceId(branch)
    } else if let Ok(env_branch) = std::env::var(BENCHER_BRANCH) {
        env_branch.as_str().parse().map(Branch::ResourceId)?
    } else if let Some(branch_name) = if_branch {
        if let Some(name) = branch_name {
            Branch::Name {
                name,
                start_points: else_if_branch,
                create: else_branch,
            }
        } else {
            Branch::None
        }
    } else {
        BRANCH_MAIN_STR.parse().map(Branch::ResourceId)?
    })
}

fn map_hash(hash: Option<String>) -> Result<Option<GitHash>, CliError> {
    Ok(if let Some(hash) = hash {
        Some(GitHash::from_str(&hash)?)
    } else {
        None
    })
}

fn unwrap_testbed(testbed: Option<ResourceId>) -> Result<ResourceId, CliError> {
    Ok(if let Some(testbed) = testbed {
        testbed
    } else if let Ok(env_testbed) = std::env::var(BENCHER_TESTBED) {
        env_testbed.as_str().parse()?
    } else {
        TESTBED_LOCALHOST_STR.parse()?
    })
}

fn map_adapter(adapter: Option<CliRunAdapter>) -> Option<RunAdapter> {
    if let Some(adapter) = adapter {
        Some(adapter.into())
    } else if let Ok(env_adapter) = std::env::var(BENCHER_ADAPTER) {
        if let Ok(cli_adapter) = CliRunAdapter::from_str(&env_adapter, false) {
            Some(cli_adapter.into())
        } else {
            None
        }
    } else {
        None
    }
}

#[async_trait]
impl SubCmd for Run {
    async fn exec(&self) -> Result<(), CliError> {
        let Some(branch) = branch_resource_id(&self.project, &self.branch, &self.locality).await? else {
            return Ok(())
        };

        let start_time = Utc::now();
        let mut results = Vec::with_capacity(self.iter);
        for _ in 0..self.iter {
            let output = self.runner.run()?;
            if output.success() {
                results.push(output.stdout)
            } else if self.allow_failure {
                cli_eprintln!("Skipping failure:\n{}", output);
            } else {
                return Err(CliError::Output(output));
            }
        }

        // TODO disable when quiet
        for result in &results {
            cli_println!("{result}");
        }

        let report = JsonNewReport {
            branch,
            hash: self.hash.clone(),
            testbed: self.testbed.clone(),
            start_time,
            end_time: Utc::now(),
            results,
            settings: Some(JsonReportSettings {
                adapter: self.adapter.map(Into::into),
                fold: self.fold.map(Into::into),
            }),
        };

        // TODO disable when quiet
        cli_println!("{}", serde_json::to_string_pretty(&report)?);

        match &self.locality {
            Locality::Local => {},
            Locality::Backend(backend) => {
                let value = backend
                    .post(&format!("/v0/projects/{}/reports", self.project), &report)
                    .await?;
                if self.err {
                    let json_report: JsonReport = serde_json::from_value(value)?;
                    if !json_report.alerts.is_empty() {
                        return Err(CliError::Alerts);
                    }
                }
            },
        }

        Ok(())
    }
}

async fn branch_resource_id(
    project: &ResourceId,
    branch: &Branch,
    locality: &Locality,
) -> Result<Option<ResourceId>, CliError> {
    Ok(match branch {
        Branch::ResourceId(resource_id) => Some(resource_id.clone()),
        Branch::Name {
            name,
            start_points,
            create,
        } => {
            if let Some(uuid) = if_branch(project, name, start_points, *create, locality).await? {
                Some(uuid.into())
            } else {
                cli_println!("Failed to find or create branch \"{name}\". Skipping benchmark run.");
                None
            }
        },
        Branch::None => {
            cli_println!("Failed to get branch name. Skipping benchmark run.");
            None
        },
    })
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
