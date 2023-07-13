use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_client::types::{JsonNewReport, JsonReportSettings};
use bencher_json::{project::testbed::TESTBED_LOCALHOST_STR, GitHash, ResourceId};
use chrono::{DateTime, Utc};
use clap::ValueEnum;
use url::Url;

use crate::{
    bencher::{backend::Backend, map_timestamp, sub::project::run::urls::BenchmarkUrls},
    cli_eprintln, cli_println,
    parser::project::run::{CliRun, CliRunAdapter},
    CliError,
};

mod adapter;
mod average;
mod branch;
mod fold;
pub mod runner;
mod urls;

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
    backdate: Option<DateTime<Utc>>,
    allow_failure: bool,
    err: bool,
    dry_run: bool,
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
            backdate,
            allow_failure,
            err,
            dry_run,
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
            backdate: map_timestamp(backdate)?,
            allow_failure,
            err,
            dry_run,
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
        let Some(branch) = self.branch.resource_id(&self.project, self.dry_run, &self.backend).await? else {
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

        let end_time = Utc::now();
        // If a backdate is set then use it as the start time and calculate the end time from there
        let (start_time, end_time) = if let Some(backdate) = self.backdate {
            let elapsed = end_time - start_time;
            (backdate, backdate + elapsed)
        } else {
            (start_time, end_time)
        };

        let report = &JsonNewReport {
            branch: branch.into(),
            hash: self.hash.clone().map(Into::into),
            testbed: self.testbed.clone().into(),
            start_time,
            end_time,
            results,
            settings: Some(JsonReportSettings {
                adapter: self.adapter.map(Into::into),
                average: self.average.map(Into::into),
                fold: self.fold.map(Into::into),
            }),
        };

        // TODO disable when quiet
        cli_println!("{}", serde_json::to_string_pretty(report)?);

        // If performing a dry run, don't actually send the report
        if self.dry_run {
            return Ok(());
        }

        let json_report = self
            .backend
            .send_with(
                |client| async move {
                    client
                        .proj_report_post()
                        .project(self.project.clone())
                        .body(report.clone())
                        .send()
                        .await
                },
                true,
            )
            .await?;

        let endpoint_url: Url = self
            .backend
            .send_with(
                |client| async move { client.server_config_endpoint_get().send().await },
                false,
            )
            .await?;
        let benchmark_urls = BenchmarkUrls::new(endpoint_url.clone(), &json_report).await?;

        cli_println!("\nView results:");
        for (name, url) in &benchmark_urls.0 {
            cli_println!("- {name}: {url}");
        }

        if json_report.alerts.is_empty() {
            return Ok(());
        }

        cli_println!("\nView alerts:");
        for alert in &json_report.alerts {
            let mut url = endpoint_url.clone();
            url.set_path(&format!(
                "/console/projects/{}/alerts/{}",
                json_report.project.slug.as_str(),
                alert.uuid
            ));
            cli_println!("- {}: {url}", alert.benchmark.name.as_str());
        }
        cli_println!("\n");

        if self.err {
            return Err(CliError::Alerts(json_report.alerts.len()));
        }

        Ok(())
    }
}
