use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_client::types::{JsonNewReport, JsonReportSettings};
use bencher_json::{
    project::testbed::TESTBED_LOCALHOST_STR, GitHash, JsonEndpoint, JsonReport, ResourceId,
};
use chrono::{DateTime, Utc};
use clap::ValueEnum;
use url::Url;

use crate::{
    bencher::{backend::Backend, map_timestamp, sub::project::run::urls::ReportUrls},
    cli_eprintln, cli_println,
    parser::project::run::{CliRun, CliRunAdapter},
    CliError,
};

mod adapter;
mod average;
mod branch;
mod ci;
mod error;
mod fold;
pub mod runner;
mod urls;

use adapter::RunAdapter;
use average::Average;
use branch::Branch;
use ci::Ci;
pub use error::RunError;
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
    html: bool,
    ci: Option<Ci>,
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
            html,
            ci,
            dry_run,
        } = run;
        Ok(Self {
            project: unwrap_project(project)?,
            backend: backend.try_into()?,
            runner: command.try_into()?,
            branch: run_branch.try_into().map_err(RunError::Branch)?,
            hash,
            testbed: unwrap_testbed(testbed)?,
            adapter: map_adapter(adapter),
            average: average.map(Into::into),
            iter: iter.unwrap_or(1),
            fold: fold.map(Into::into),
            backdate: map_timestamp(backdate)?,
            allow_failure,
            err,
            html,
            ci: ci.try_into().map_err(RunError::Ci)?,
            dry_run,
        })
    }
}

fn unwrap_project(project: Option<ResourceId>) -> Result<ResourceId, RunError> {
    Ok(if let Some(project) = project {
        project
    } else if let Ok(env_project) = std::env::var(BENCHER_PROJECT) {
        env_project.parse().map_err(RunError::ParseProject)?
    } else {
        return Err(RunError::ProjectNotFound);
    })
}

fn unwrap_testbed(testbed: Option<ResourceId>) -> Result<ResourceId, RunError> {
    Ok(if let Some(testbed) = testbed {
        testbed
    } else if let Ok(env_testbed) = std::env::var(BENCHER_TESTBED) {
        env_testbed
            .as_str()
            .parse()
            .map_err(RunError::ParseTestbed)?
    } else {
        TESTBED_LOCALHOST_STR
            .parse()
            .map_err(RunError::ParseTestbed)?
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
        self.exec().await.map_err(Into::into)
    }
}

impl Run {
    async fn exec(&self) -> Result<(), RunError> {
        let Some(json_new_report) = &self.generate_report().await? else {
            return Ok(());
        };

        // TODO disable when quiet
        cli_println!(
            "{}",
            serde_json::to_string_pretty(json_new_report).map_err(RunError::SerializeReport)?
        );

        // If performing a dry run, don't actually send the report
        if self.dry_run {
            return Ok(());
        }

        let json_report: JsonReport = self
            .backend
            .send_with(
                |client| async move {
                    client
                        .proj_report_post()
                        .project(self.project.clone())
                        .body(json_new_report.clone())
                        .send()
                        .await
                },
                true,
            )
            .await
            .map_err(RunError::SendReport)?;

        let alerts_count = json_report.alerts.len();
        // TODO disable when quiet
        self.display_results(json_report).await?;

        if self.err && alerts_count > 0 {
            Err(RunError::Alerts(alerts_count))
        } else {
            Ok(())
        }
    }

    async fn generate_report(&self) -> Result<Option<JsonNewReport>, RunError> {
        let Some(branch) = self.branch.resource_id(&self.project, self.dry_run, &self.backend).await? else {
            return Ok(None)
        };

        let start_time = Utc::now();
        let mut results = Vec::with_capacity(self.iter);
        for _ in 0..self.iter {
            let output = self.runner.run()?;
            if output.is_success() {
                results.push(output.stdout)
            } else if self.allow_failure {
                cli_eprintln!("Skipping failure:\n{}", output);
            } else {
                return Err(RunError::ExitStatus(output));
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

        Ok(Some(JsonNewReport {
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
        }))
    }

    async fn display_results(&self, json_report: JsonReport) -> Result<(), RunError> {
        let json_endpoint: JsonEndpoint = self
            .backend
            .send_with(
                |client| async move { client.server_endpoint_get().send().await },
                false,
            )
            .await
            .map_err(RunError::GetEndpoint)?;
        let endpoint_url: Url = json_endpoint.endpoint.into();
        let report_urls = ReportUrls::new(endpoint_url.clone(), json_report);

        // TODO disable when quiet
        if self.html {
            cli_println!("{}", report_urls.html(false));
        } else {
            cli_println!("{report_urls}");
        }

        if let Some(ci) = &self.ci {
            ci.run(&report_urls).await?;
        }

        Ok(())
    }
}
