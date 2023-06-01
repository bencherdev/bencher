use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::{
    project::{report::JsonReportSettings, testbed::TESTBED_LOCALHOST_STR},
    GitHash, JsonNewReport, JsonReport, ResourceId,
};
use chrono::Utc;
use clap::ValueEnum;

use crate::{
    bencher::backend::Backend,
    cli::project::run::{CliRun, CliRunAdapter},
    cli_eprintln, cli_println, CliError,
};

mod adapter;
mod average;
mod branch;
mod fold;
pub mod runner;

use adapter::RunAdapter;
use average::Average;
use branch::Branch;
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
    backend: Backend,
    runner: Runner,
    branch: Branch,
    hash: Option<GitHash>,
    testbed: ResourceId,
    adapter: Option<RunAdapter>,
    average: Option<Average>,
    iter: usize,
    fold: Option<Fold>,
    allow_failure: bool,
    err: bool,
}

impl TryFrom<CliRun> for Run {
    type Error = CliError;

    fn try_from(run: CliRun) -> Result<Self, Self::Error> {
        let CliRun {
            project,
            backend,
            command,
            run_branch,
            hash,
            testbed,
            adapter,
            average,
            iter,
            fold,
            allow_failure,
            err,
        } = run;
        Ok(Self {
            project: unwrap_project(project)?,
            backend: backend.try_into()?,
            runner: command.try_into()?,
            branch: run_branch.try_into()?,
            hash,
            testbed: unwrap_testbed(testbed)?,
            adapter: map_adapter(adapter),
            average: average.map(Into::into),
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
        let Some(branch) = self.branch.resource_id(&self.project, &self.backend).await? else {
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
                average: self.average.map(Into::into),
                fold: self.fold.map(Into::into),
            }),
        };

        // TODO disable when quiet
        cli_println!("{}", serde_json::to_string_pretty(&report)?);

        let value = self
            .backend
            .post(&format!("/v0/projects/{}/reports", self.project), &report)
            .await?;
        if self.err {
            let json_report: JsonReport = serde_json::from_value(value)?;
            if !json_report.alerts.is_empty() {
                return Err(CliError::Alerts);
            }
        }

        Ok(())
    }
}
