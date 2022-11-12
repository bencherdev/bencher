use std::{convert::TryFrom, str::FromStr};

use async_trait::async_trait;
use bencher_json::{JsonBranch, JsonNewReport, JsonReport, ResourceId};
use chrono::Utc;
use clap::ValueEnum;
use git2::Oid;
use uuid::Uuid;

use crate::{
    bencher::locality::Locality,
    cli::project::run::{CliRun, CliRunAdapter},
    CliError,
};

mod adapter;
mod fold;
mod runner;

use adapter::RunAdapter;
use fold::Fold;
pub use runner::Output;
use runner::Runner;

use crate::bencher::SubCmd;

const BENCHER_PROJECT: &str = "BENCHER_PROJECT";
const BENCHER_BRANCH: &str = "BENCHER_BRANCH";
const BENCHER_BRANCH_NAME: &str = "BENCHER_BRANCH_NAME";
const BENCHER_TESTBED: &str = "BENCHER_TESTBED";
const BENCHER_ADAPTER: &str = "BENCHER_ADAPTER";
const BENCHER_CMD: &str = "BENCHER_CMD";

#[derive(Debug)]
pub struct Run {
    project: ResourceId,
    locality: Locality,
    runner: Runner,
    branch: Branch,
    hash: Option<Oid>,
    testbed: Uuid,
    adapter: Option<RunAdapter>,
    iter: usize,
    fold: Option<Fold>,
    err: bool,
}

#[derive(Debug, Clone)]
enum Branch {
    Uuid(Uuid),
    Name(String),
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
            hash,
            testbed,
            adapter,
            iter,
            fold,
            err,
        } = run;
        Ok(Self {
            project: unwrap_project(project)?,
            locality: locality.try_into()?,
            runner: command.try_into()?,
            branch: map_branch(branch, if_branch)?,
            hash: map_hash(hash)?,
            testbed: unwrap_testbed(testbed)?,
            adapter: map_adapter(adapter),
            iter: iter.unwrap_or(1),
            fold: fold.map(Into::into),
            err,
        })
    }
}

fn unwrap_project(project: Option<ResourceId>) -> Result<ResourceId, CliError> {
    Ok(if let Some(project) = project {
        project
    } else if let Ok(project) = std::env::var(BENCHER_PROJECT) {
        ResourceId::from_str(&project).map_err(CliError::ResourceId)?
    } else {
        return Err(CliError::ProjectNotFound);
    })
}

fn map_branch(branch: Option<Uuid>, if_branch: Option<String>) -> Result<Branch, CliError> {
    if let Some(branch) = branch {
        Ok(Branch::Uuid(branch))
    } else if let Ok(branch) = std::env::var(BENCHER_BRANCH) {
        Ok(Branch::Uuid(Uuid::from_str(&branch)?))
    } else if let Some(name) = if_branch {
        Ok(Branch::Name(name))
    } else if let Ok(name) = std::env::var(BENCHER_BRANCH_NAME) {
        Ok(Branch::Name(name))
    } else {
        Err(CliError::BranchNotFound)
    }
}

fn map_hash(hash: Option<String>) -> Result<Option<Oid>, CliError> {
    Ok(if let Some(hash) = hash {
        Some(Oid::from_str(&hash)?)
    } else {
        None
    })
}

fn unwrap_testbed(testbed: Option<Uuid>) -> Result<Uuid, CliError> {
    Ok(if let Some(testbed) = testbed {
        testbed
    } else if let Ok(testbed) = std::env::var(BENCHER_TESTBED) {
        Uuid::from_str(&testbed)?
    } else {
        return Err(CliError::TestbedNotFound);
    })
}

fn map_adapter(adapter: Option<CliRunAdapter>) -> Option<RunAdapter> {
    if let Some(adapter) = adapter {
        Some(adapter.into())
    } else if let Ok(adapter_str) = std::env::var(BENCHER_ADAPTER) {
        if let Ok(adapter) = CliRunAdapter::from_str(&adapter_str, false) {
            Some(adapter.into())
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
        let branch = match &self.branch {
            Branch::Uuid(uuid) => *uuid,
            Branch::Name(name) => {
                if let Some(uuid) = if_branch(&self.project, name, &self.locality).await? {
                    uuid
                } else {
                    return Ok(());
                }
            },
        };

        let start_time = Utc::now();

        let mut results = Vec::with_capacity(self.iter);
        for _ in 0..self.iter {
            let output = self.runner.run()?;
            results.push(output.result);
        }

        // TODO disable when quiet
        for result in &results {
            println!("{result}");
        }

        let report = JsonNewReport {
            branch,
            hash: self.hash.map(|hash| hash.to_string()),
            testbed: self.testbed,
            start_time,
            end_time: Utc::now(),
            adapter: self.adapter.map(Into::into),
            fold: self.fold.map(Into::into),
            results,
        };

        // TODO disable when quiet
        println!("{}", serde_json::to_string_pretty(&report)?);

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

async fn if_branch(
    project: &ResourceId,
    name: &str,
    locality: &Locality,
) -> Result<Option<Uuid>, CliError> {
    if let Locality::Backend(backend) = &locality {
        let value = backend
            .get(&format!("/v0/projects/{project}/branches?name={name}"))
            .await?;
        let mut json_branches: Vec<JsonBranch> = serde_json::from_value(value)?;
        let branch_count = json_branches.len();
        if let Some(branch) = json_branches.pop() {
            return if branch_count == 1 {
                Ok(Some(branch.uuid))
            } else {
                Err(CliError::BranchName(
                    project.to_string(),
                    name.into(),
                    branch_count,
                ))
            };
        }
    }

    println!("Failed to find branch with name \"{name}\" in project \"{project}\". Skipping benchmark run.");
    Ok(None)
}
